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
    milestone::MilestoneIndex, protocol::Protocol, tangle::tangle, worker::solidifier::TransactionSolidifierWorkerEvent,
};
use bee_common::worker::Error as WorkerError;
use bee_crypto::ternary::Hash;

use futures::{channel::mpsc, SinkExt, StreamExt};
use log::{error, info, warn};

const MILESTONE_REQUEST_RANGE: u8 = 50;

type Receiver = crate::worker::Receiver<mpsc::Receiver<MilestoneSolidifierWorkerEvent>>;
type TransactionReceiver = crate::worker::Receiver<mpsc::Receiver<TransactionSolidifierWorkerEvent>>;

pub(crate) enum MilestoneSolidifierWorkerEvent {
    Trigger,
    NewSolidMilestone(MilestoneIndex),
    NewTransaction(Hash, MilestoneIndex),
}

pub(crate) struct MilestoneSolidifierWorker {
    receiver: Receiver,
    lower_index: MilestoneIndex,
    senders: Vec<mpsc::Sender<TransactionSolidifierWorkerEvent>>,
}

impl MilestoneSolidifierWorker {
    pub(crate) fn new(receiver: Receiver, senders: Vec<mpsc::Sender<TransactionSolidifierWorkerEvent>>) -> Self {
        let solid_index = tangle().get_last_solid_milestone_index();
        Self {
            receiver,
            lower_index: solid_index + MilestoneIndex(1),
            senders,
        }
    }

    // async fn solidify(&self, hash: Hash, target_index: u32) -> bool {
    //     let mut missing_hashes = HashSet::new();
    //
    //     tangle().walk_approvees_depth_first(
    //         hash,
    //         |_| {},
    //         |transaction| true,
    //         |missing_hash| {
    //             missing_hashes.insert(*missing_hash);
    //         },
    //     );
    //
    //     // TODO refactor with async closures when stabilized
    //     match missing_hashes.is_empty() {
    //         true => true,
    //         false => {
    //             for missing_hash in missing_hashes {
    //                 Protocol::request_transaction(missing_hash, target_index).await;
    //             }
    //
    //             false
    //         }
    //     }
    // }
    //
    // async fn process_target(&self, target_index: u32) -> bool {
    //     match tangle().get_milestone_hash(target_index.into()) {
    //         Some(target_hash) => match self.solidify(target_hash, target_index).await {
    //             true => {
    //                 tangle().update_solid_milestone_index(target_index.into());
    //                 Protocol::broadcast_heartbeat(
    //                     *tangle().get_last_solid_milestone_index(),
    //                     *tangle().get_snapshot_milestone_index(),
    //                 )
    //                 .await;
    //                 true
    //             }
    //             false => false,
    //         },
    //         None => {
    //             // There is a gap, request the milestone
    //             Protocol::request_milestone(target_index, None);
    //             false
    //         }
    //     }
    // }

    fn request_milestones(&self) {
        let solid_milestone_index = *tangle().get_last_solid_milestone_index();

        // TODO this may request unpublished milestones
        for index in solid_milestone_index..solid_milestone_index + MILESTONE_REQUEST_RANGE as u32 {
            let index = index.into();
            if !tangle().contains_milestone(index) {
                Protocol::request_milestone(index, None);
            }
        }
    }

    async fn solidify_milestone(&mut self, target_index: MilestoneIndex) {
        // if let Some(target_hash) = tangle().get_milestone_hash(target_index) {
        //     if tangle().is_solid_transaction(&target_hash) {
        //         // TODO set confirmation index + trigger ledger
        //         tangle().update_last_solid_milestone_index(target_index);
        //         Protocol::broadcast_heartbeat(
        //             tangle().get_last_solid_milestone_index(),
        //             tangle().get_snapshot_milestone_index(),
        //         )
        //         .await;
        //     } else {
        //         Protocol::trigger_transaction_solidification(target_hash, target_index).await;
        //     }
        // }
        if let Some(sender) = self.senders.get_mut((target_index.0 - self.lower_index.0) as usize) {
            if let Some(target_hash) = tangle().get_milestone_hash(target_index) {
                if !tangle().is_solid_transaction(&target_hash) {
                    if let Err(e) = sender
                        .send(TransactionSolidifierWorkerEvent(target_hash, target_index))
                        .await
                    {
                        warn!("Triggering transaction solidification failed: {}.", e);
                    }
                }
            }
        } else {
            error!("There is no solidifier running for milestone {}", target_index.0);
        }
    }

    pub(crate) async fn run(mut self) -> Result<(), WorkerError> {
        info!("Running.");

        while let Some(event) = self.receiver.next().await {
            match event {
                MilestoneSolidifierWorkerEvent::Trigger => {
                    self.request_milestones();
                    for i in 0..self.senders.len() as u32 {
                        let index = self.lower_index + MilestoneIndex(i);
                        self.solidify_milestone(index).await;
                    }
                }
                MilestoneSolidifierWorkerEvent::NewSolidMilestone(index) => {
                    if index != self.lower_index {
                        error!(
                            "New solid milestone with index {} does not match index {}",
                            index.0, self.lower_index.0
                        );
                    } else {
                        self.lower_index = self.lower_index + MilestoneIndex(1);
                    }
                }
                MilestoneSolidifierWorkerEvent::NewTransaction(hash, index) => {
                    if let Some(sender) = self.senders.get_mut((index.0 - self.lower_index.0) as usize) {
                        if let Err(e) = sender.send(TransactionSolidifierWorkerEvent(hash, index)).await {
                            warn!("Triggering transaction solidification failed: {}.", e);
                        }
                    } else {
                        error!("There is no solidifier running for milestone {}", index.0);
                    }
                }
            }
            // while tangle().get_last_solid_milestone_index() < tangle().get_last_milestone_index() {
            //     if !self.process_target(*tangle().get_last_solid_milestone_index() + 1).await {
            //         break;
            //     }
            // }
        }

        info!("Stopped.");

        Ok(())
    }
}

#[cfg(test)]
mod tests {}
