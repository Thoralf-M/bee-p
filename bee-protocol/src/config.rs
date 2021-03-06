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

use bee_crypto::ternary::sponge::SpongeKind;
use bee_ternary::{T1B1Buf, T5B1Buf, TryteBuf};
use bee_transaction::bundled::{Address, BundledTransactionField};

use bytemuck::cast_slice;
use serde::Deserialize;

const DEFAULT_MWM: u8 = 14;
const DEFAULT_COO_DEPTH: u8 = 25;
const DEFAULT_COO_PUBLIC_KEY: &str =
    "UDYXTZBE9GZGPM9SSQV9LTZNDLJIZMPUVVXYXFYVBLIEUHLSEWFTKZZLXYRHHWVQV9MNNX9KZC9D9UZWZ";
const DEFAULT_COO_SECURITY: u8 = 2;
const DEFAULT_COO_SPONGE_TYPE: &str = "kerl";
const DEFAULT_TRANSACTION_WORKER_CACHE: usize = 10000;
const DEFAULT_RECEIVER_WORKER_BOUND: usize = 10000;
const DEFAULT_STATUS_INTERVAL: u64 = 10;
const DEFAULT_HANDSHAKE_WINDOW: u64 = 10;

#[derive(Default, Deserialize)]
struct ProtocolCoordinatorConfigBuilder {
    depth: Option<u8>,
    public_key: Option<String>,
    security_level: Option<u8>,
    sponge_type: Option<String>,
}

#[derive(Default, Deserialize)]
struct ProtocolWorkersConfigBuilder {
    transaction_worker_cache: Option<usize>,
    receiver_worker_bound: Option<usize>,
    status_interval: Option<u64>,
}

#[derive(Default, Deserialize)]
pub struct ProtocolConfigBuilder {
    mwm: Option<u8>,
    coordinator: ProtocolCoordinatorConfigBuilder,
    workers: ProtocolWorkersConfigBuilder,
    handshake_window: Option<u64>,
}

impl ProtocolConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mwm(mut self, mwm: u8) -> Self {
        self.mwm.replace(mwm);
        self
    }

    pub fn coo_depth(mut self, coo_depth: u8) -> Self {
        self.coordinator.depth.replace(coo_depth);
        self
    }

    pub fn coo_public_key(mut self, coo_public_key: String) -> Self {
        self.coordinator.public_key.replace(coo_public_key);
        self
    }

    pub fn coo_security_level(mut self, coo_security_level: u8) -> Self {
        self.coordinator.security_level.replace(coo_security_level);
        self
    }

    pub fn coo_sponge_type(mut self, coo_sponge_type: &str) -> Self {
        self.coordinator.sponge_type.replace(coo_sponge_type.to_string());
        self
    }

    pub fn transaction_worker_cache(mut self, transaction_worker_cache: usize) -> Self {
        self.workers.transaction_worker_cache.replace(transaction_worker_cache);
        self
    }

    pub fn receiver_worker_bound(mut self, receiver_worker_bound: usize) -> Self {
        self.workers.receiver_worker_bound.replace(receiver_worker_bound);
        self
    }

    pub fn status_interval(mut self, status_interval: u64) -> Self {
        self.workers.status_interval.replace(status_interval);
        self
    }

    pub fn handshake_window(mut self, handshake_window: u64) -> Self {
        self.handshake_window.replace(handshake_window);
        self
    }

    pub fn finish(self) -> ProtocolConfig {
        let coo_sponge_type = match self
            .coordinator
            .sponge_type
            .unwrap_or_else(|| DEFAULT_COO_SPONGE_TYPE.to_owned())
            .as_str()
        {
            "kerl" => SpongeKind::Kerl,
            "curl27" => SpongeKind::CurlP27,
            "curl81" => SpongeKind::CurlP81,
            _ => SpongeKind::Kerl,
        };

        let coo_public_key_default = Address::from_inner_unchecked(
            TryteBuf::try_from_str(DEFAULT_COO_PUBLIC_KEY)
                .unwrap()
                .as_trits()
                .encode::<T1B1Buf>(),
        );

        let coo_public_key = match TryteBuf::try_from_str(
            &self
                .coordinator
                .public_key
                .unwrap_or_else(|| DEFAULT_COO_PUBLIC_KEY.to_owned()),
        ) {
            Ok(trytes) => match Address::try_from_inner(trytes.as_trits().encode::<T1B1Buf>()) {
                Ok(coo_public_key) => coo_public_key,
                Err(_) => coo_public_key_default,
            },
            Err(_) => coo_public_key_default,
        };

        let mut public_key_bytes = [0u8; 49];
        public_key_bytes.copy_from_slice(cast_slice(coo_public_key.to_inner().encode::<T5B1Buf>().as_i8_slice()));

        ProtocolConfig {
            mwm: self.mwm.unwrap_or(DEFAULT_MWM),
            coordinator: ProtocolCoordinatorConfig {
                depth: self.coordinator.depth.unwrap_or(DEFAULT_COO_DEPTH),
                public_key: coo_public_key,
                public_key_bytes,
                security_level: self.coordinator.security_level.unwrap_or(DEFAULT_COO_SECURITY),
                sponge_type: coo_sponge_type,
            },
            workers: ProtocolWorkersConfig {
                transaction_worker_cache: self
                    .workers
                    .transaction_worker_cache
                    .unwrap_or(DEFAULT_TRANSACTION_WORKER_CACHE),
                receiver_worker_bound: self
                    .workers
                    .receiver_worker_bound
                    .unwrap_or(DEFAULT_RECEIVER_WORKER_BOUND),
                status_interval: self.workers.status_interval.unwrap_or(DEFAULT_STATUS_INTERVAL),
            },
            handshake_window: self.handshake_window.unwrap_or(DEFAULT_HANDSHAKE_WINDOW),
        }
    }
}

#[derive(Clone)]
pub struct ProtocolCoordinatorConfig {
    pub(crate) depth: u8,
    pub(crate) public_key: Address,
    pub(crate) public_key_bytes: [u8; 49],
    pub(crate) security_level: u8,
    pub(crate) sponge_type: SpongeKind,
}

impl ProtocolCoordinatorConfig {
    pub fn depth(&self) -> u8 {
        self.depth
    }
}

#[derive(Clone)]
pub struct ProtocolWorkersConfig {
    pub(crate) transaction_worker_cache: usize,
    pub(crate) receiver_worker_bound: usize,
    pub(crate) status_interval: u64,
}

#[derive(Clone)]
pub struct ProtocolConfig {
    pub(crate) mwm: u8,
    pub(crate) coordinator: ProtocolCoordinatorConfig,
    pub(crate) workers: ProtocolWorkersConfig,
    pub(crate) handshake_window: u64,
}

impl ProtocolConfig {
    pub fn build() -> ProtocolConfigBuilder {
        ProtocolConfigBuilder::new()
    }

    pub fn coordinator(&self) -> &ProtocolCoordinatorConfig {
        &self.coordinator
    }
}

// TODO move out of here
pub(crate) fn slice_eq(a: &[u8; 49], b: &[u8; 49]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    for i in 0..a.len() {
        if a[i] != b[i] {
            return false;
        }
    }

    true
}
