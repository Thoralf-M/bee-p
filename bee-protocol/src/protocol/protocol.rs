// Copyright 2020 IOTA Stiftung
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except in compliance with
// the License. You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on
// an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and limitations under the License.

use crate::{
    config::ProtocolConfig,
    event::{LastMilestoneChanged, LastSolidMilestoneChanged},
    milestone::MilestoneIndex,
    peer::{Peer, PeerManager},
    protocol::ProtocolMetrics,
    tangle::tangle,
    worker::{
        BroadcasterWorker, BroadcasterWorkerEvent, HasherWorker, HasherWorkerEvent, MilestoneRequesterWorker,
        MilestoneRequesterWorkerEntry, MilestoneResponderWorker, MilestoneResponderWorkerEvent,
        MilestoneSolidifierWorker, MilestoneSolidifierWorkerEvent, MilestoneValidatorWorker, PeerHandshakerWorker,
        ProcessorWorker, StatusWorker, TpsWorker, TransactionRequesterWorker, TransactionRequesterWorkerEntry,
        TransactionResponderWorker, TransactionResponderWorkerEvent, TransactionSolidifierWorker,
        TransactionSolidifierWorkerEvent,
    },
};

use bee_common::{shutdown::Shutdown, shutdown_stream::ShutdownStream};
use bee_common_ext::{event::Bus, wait_priority_queue::WaitPriorityQueue};
use bee_crypto::ternary::{
    sponge::{CurlP27, CurlP81, Kerl, SpongeKind},
    Hash,
};
use bee_network::{Address, EndpointId, Network, Origin};
use bee_signing::ternary::wots::WotsPublicKey;

use async_std::task::spawn;
use dashmap::DashMap;
use futures::channel::{mpsc, oneshot};
use log::{debug, info, warn};

use std::{ptr, sync::Arc, time::Instant};

static mut PROTOCOL: *const Protocol = ptr::null();

pub struct Protocol {
    pub(crate) config: ProtocolConfig,
    pub(crate) network: Network,
    // TODO temporary
    pub(crate) local_snapshot_timestamp: u64,
    pub(crate) bus: Arc<Bus<'static>>,
    pub(crate) metrics: ProtocolMetrics,
    pub(crate) hasher_worker: mpsc::UnboundedSender<HasherWorkerEvent>,
    pub(crate) transaction_responder_worker: mpsc::UnboundedSender<TransactionResponderWorkerEvent>,
    pub(crate) milestone_responder_worker: mpsc::UnboundedSender<MilestoneResponderWorkerEvent>,
    pub(crate) transaction_requester_worker: WaitPriorityQueue<TransactionRequesterWorkerEntry>,
    pub(crate) milestone_requester_worker: WaitPriorityQueue<MilestoneRequesterWorkerEntry>,
    pub(crate) transaction_solidifier_worker: mpsc::UnboundedSender<TransactionSolidifierWorkerEvent>,
    pub(crate) milestone_solidifier_worker: mpsc::UnboundedSender<MilestoneSolidifierWorkerEvent>,
    pub(crate) broadcaster_worker: mpsc::UnboundedSender<BroadcasterWorkerEvent>,
    pub(crate) peer_manager: PeerManager,
    pub(crate) requested_transactions: DashMap<Hash, (MilestoneIndex, Instant)>,
    pub(crate) requested_milestones: DashMap<MilestoneIndex, Instant>,
}

impl Protocol {
    pub async fn init(
        config: ProtocolConfig,
        network: Network,
        local_snapshot_timestamp: u64,
        bus: Arc<Bus<'static>>,
        shutdown: &mut Shutdown,
    ) {
        if unsafe { !PROTOCOL.is_null() } {
            warn!("Already initialized.");
            return;
        }

        let (hasher_worker_tx, hasher_worker_rx) = mpsc::unbounded();
        let (hasher_worker_shutdown_tx, hasher_worker_shutdown_rx) = oneshot::channel();

        let (processor_worker_tx, processor_worker_rx) = mpsc::unbounded();
        let (processor_worker_shutdown_tx, processor_worker_shutdown_rx) = oneshot::channel();

        let (transaction_responder_worker_tx, transaction_responder_worker_rx) = mpsc::unbounded();
        let (transaction_responder_worker_shutdown_tx, transaction_responder_worker_shutdown_rx) = oneshot::channel();

        let (milestone_responder_worker_tx, milestone_responder_worker_rx) = mpsc::unbounded();
        let (milestone_responder_worker_shutdown_tx, milestone_responder_worker_shutdown_rx) = oneshot::channel();

        let (transaction_requester_worker_shutdown_tx, transaction_requester_worker_shutdown_rx) = oneshot::channel();

        let (milestone_requester_worker_shutdown_tx, milestone_requester_worker_shutdown_rx) = oneshot::channel();

        let (milestone_validator_worker_tx, milestone_validator_worker_rx) = mpsc::unbounded();
        let (milestone_validator_worker_shutdown_tx, milestone_validator_worker_shutdown_rx) = oneshot::channel();

        let (transaction_solidifier_worker_tx, transaction_solidifier_worker_rx) = mpsc::unbounded();
        let (transaction_solidifier_worker_shutdown_tx, transaction_solidifier_worker_shutdown_rx) = oneshot::channel();

        let (milestone_solidifier_worker_tx, milestone_solidifier_worker_rx) = mpsc::unbounded();
        let (milestone_solidifier_worker_shutdown_tx, milestone_solidifier_worker_shutdown_rx) = oneshot::channel();

        let (broadcaster_worker_tx, broadcaster_worker_rx) = mpsc::unbounded();
        let (broadcaster_worker_shutdown_tx, broadcaster_worker_shutdown_rx) = oneshot::channel();

        let (status_worker_shutdown_tx, status_worker_shutdown_rx) = oneshot::channel();

        let (tps_worker_shutdown_tx, tps_worker_shutdown_rx) = oneshot::channel();

        let protocol = Protocol {
            config,
            network: network.clone(),
            local_snapshot_timestamp,
            bus,
            metrics: ProtocolMetrics::new(),
            hasher_worker: hasher_worker_tx,
            transaction_responder_worker: transaction_responder_worker_tx,
            milestone_responder_worker: milestone_responder_worker_tx,
            transaction_requester_worker: Default::default(),
            milestone_requester_worker: Default::default(),
            transaction_solidifier_worker: transaction_solidifier_worker_tx,
            milestone_solidifier_worker: milestone_solidifier_worker_tx,
            broadcaster_worker: broadcaster_worker_tx,
            peer_manager: PeerManager::new(network.clone()),
            requested_transactions: Default::default(),
            requested_milestones: Default::default(),
        };

        unsafe {
            PROTOCOL = Box::leak(protocol.into()) as *const _;
        }

        Protocol::get().bus.add_listener(on_last_solid_milestone_changed);
        // Protocol::get().bus.add_listener(on_snapshot_milestone_changed);
        Protocol::get().bus.add_listener(on_last_milestone_changed);

        shutdown.add_worker_shutdown(
            hasher_worker_shutdown_tx,
            spawn(
                HasherWorker::new(
                    processor_worker_tx,
                    Protocol::get().config.workers.transaction_worker_cache,
                    ShutdownStream::new(hasher_worker_shutdown_rx, hasher_worker_rx),
                )
                .run(),
            ),
        );

        shutdown.add_worker_shutdown(
            processor_worker_shutdown_tx,
            spawn(
                ProcessorWorker::new(
                    milestone_validator_worker_tx,
                    ShutdownStream::new(processor_worker_shutdown_rx, processor_worker_rx),
                )
                .run(),
            ),
        );

        shutdown.add_worker_shutdown(
            transaction_responder_worker_shutdown_tx,
            spawn(
                TransactionResponderWorker::new(ShutdownStream::new(
                    transaction_responder_worker_shutdown_rx,
                    transaction_responder_worker_rx,
                ))
                .run(),
            ),
        );

        shutdown.add_worker_shutdown(
            milestone_responder_worker_shutdown_tx,
            spawn(
                MilestoneResponderWorker::new(ShutdownStream::new(
                    milestone_responder_worker_shutdown_rx,
                    milestone_responder_worker_rx,
                ))
                .run(),
            ),
        );

        shutdown.add_worker_shutdown(
            transaction_requester_worker_shutdown_tx,
            spawn(
                TransactionRequesterWorker::new(ShutdownStream::from_fused(
                    transaction_requester_worker_shutdown_rx,
                    Protocol::get().transaction_requester_worker.incoming(),
                ))
                .run(),
            ),
        );

        shutdown.add_worker_shutdown(
            milestone_requester_worker_shutdown_tx,
            spawn(
                MilestoneRequesterWorker::new(ShutdownStream::from_fused(
                    milestone_requester_worker_shutdown_rx,
                    Protocol::get().milestone_requester_worker.incoming(),
                ))
                .run(),
            ),
        );

        match Protocol::get().config.coordinator.sponge_type {
            SpongeKind::Kerl => shutdown.add_worker_shutdown(
                milestone_validator_worker_shutdown_tx,
                spawn(
                    MilestoneValidatorWorker::<Kerl, WotsPublicKey<Kerl>>::new(ShutdownStream::new(
                        milestone_validator_worker_shutdown_rx,
                        milestone_validator_worker_rx,
                    ))
                    .run(),
                ),
            ),
            SpongeKind::CurlP27 => shutdown.add_worker_shutdown(
                milestone_validator_worker_shutdown_tx,
                spawn(
                    MilestoneValidatorWorker::<CurlP27, WotsPublicKey<CurlP27>>::new(ShutdownStream::new(
                        milestone_validator_worker_shutdown_rx,
                        milestone_validator_worker_rx,
                    ))
                    .run(),
                ),
            ),
            SpongeKind::CurlP81 => shutdown.add_worker_shutdown(
                milestone_validator_worker_shutdown_tx,
                spawn(
                    MilestoneValidatorWorker::<CurlP81, WotsPublicKey<CurlP81>>::new(ShutdownStream::new(
                        milestone_validator_worker_shutdown_rx,
                        milestone_validator_worker_rx,
                    ))
                    .run(),
                ),
            ),
        };

        shutdown.add_worker_shutdown(
            transaction_solidifier_worker_shutdown_tx,
            spawn(
                TransactionSolidifierWorker::new(ShutdownStream::new(
                    transaction_solidifier_worker_shutdown_rx,
                    transaction_solidifier_worker_rx,
                ))
                .run(),
            ),
        );

        shutdown.add_worker_shutdown(
            milestone_solidifier_worker_shutdown_tx,
            spawn(
                MilestoneSolidifierWorker::new(ShutdownStream::new(
                    milestone_solidifier_worker_shutdown_rx,
                    milestone_solidifier_worker_rx,
                ))
                .run(),
            ),
        );

        shutdown.add_worker_shutdown(
            broadcaster_worker_shutdown_tx,
            spawn(
                BroadcasterWorker::new(
                    network,
                    ShutdownStream::new(broadcaster_worker_shutdown_rx, broadcaster_worker_rx),
                )
                .run(),
            ),
        );

        shutdown.add_worker_shutdown(
            status_worker_shutdown_tx,
            spawn(StatusWorker::new(Protocol::get().config.workers.status_interval).run(status_worker_shutdown_rx)),
        );

        shutdown.add_worker_shutdown(
            tps_worker_shutdown_tx,
            spawn(TpsWorker::new().run(tps_worker_shutdown_rx)),
        );
    }

    pub(crate) fn get() -> &'static Protocol {
        if unsafe { PROTOCOL.is_null() } {
            panic!("Uninitialized protocol.");
        } else {
            unsafe { &*PROTOCOL }
        }
    }

    pub fn register(
        epid: EndpointId,
        address: Address,
        origin: Origin,
    ) -> (mpsc::Sender<Vec<u8>>, oneshot::Sender<()>) {
        // TODO check if not already added ?

        let peer = Arc::new(Peer::new(epid, address, origin));

        let (receiver_tx, receiver_rx) = mpsc::channel(Protocol::get().config.workers.receiver_worker_bound);
        let (receiver_shutdown_tx, receiver_shutdown_rx) = oneshot::channel();

        Protocol::get().peer_manager.add(peer.clone());

        spawn(PeerHandshakerWorker::new(Protocol::get().network.clone(), peer).run(receiver_rx, receiver_shutdown_rx));

        (receiver_tx, receiver_shutdown_tx)
    }
}

fn on_last_milestone_changed(last_milestone: &LastMilestoneChanged) {
    info!(
        "New milestone #{} {}.",
        *last_milestone.0.index,
        last_milestone
            .0
            .hash()
            .iter_trytes()
            .map(|trit| char::from(trit))
            .collect::<String>()
    );
    tangle().update_last_milestone_index(last_milestone.0.index);

    // TODO block_on ?
    // TODO uncomment on Chrysalis Pt1.
    // block_on(Protocol::broadcast_heartbeat(
    //     tangle().get_last_solid_milestone_index(),
    //     tangle().get_snapshot_milestone_index(),
    // ));
}

// TODO Chrysalis
// fn on_snapshot_milestone_changed(last_solid_milestone: &LastSolidMilestoneChanged) {
//     // TODO block_on ?
//     // TODO uncomment on Chrysalis Pt1.
//     // block_on(Protocol::broadcast_heartbeat(
//     //     tangle().get_last_solid_milestone_index(),
//     //     tangle().get_snapshot_milestone_index(),
//     // ));
// }

fn on_last_solid_milestone_changed(last_solid_milestone: &LastSolidMilestoneChanged) {
    debug!("New solid milestone #{}.", *last_solid_milestone.0.index);
    tangle().update_last_solid_milestone_index(last_solid_milestone.0.index);

    Protocol::broadcast_heartbeat(
        last_solid_milestone.0.index,
        tangle().get_snapshot_milestone_index(),
        tangle().get_last_milestone_index(),
    );
}
