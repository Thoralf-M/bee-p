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

use bee_protocol::tangle::{TransactionMetadata, MsTangle, tangle as protocol_tangle};
use bee_crypto::ternary::Hash;
use bee_transaction::{bundled::BundledTransaction, Vertex};
use bee_tangle::{Tangle, TransactionRef};
use dashmap::DashMap;
use std::cmp::{min, max};
use bee_protocol::MilestoneIndex;
use crate::model::TsaMetadata;

const YTRSI_DELTA: u64 = 2; // C1
const OTRSI_DELTA: u64 = 7; // C2
const BELOW_MAX_DEPTH: u64 = 15; // M

pub struct TsaTangle {
    pub tsa_metadata: DashMap<Hash, TsaMetadata>,
}

impl TsaTangle {

    fn get_ytrsi(&self, hash: &Hash) -> Option<MilestoneIndex>{
        let x = match self.tsa_metadata.get(&hash) {
            Some(metadata) => metadata.ytrsi,
            None => None
        };
    }

    fn get_otrsi(&self, hash: &Hash) -> Option<MilestoneIndex>{
        let x = match self.tsa_metadata.get(&hash) {
            Some(metadata) => metadata.otrsi,
            None => None
        };
    }

    pub fn insert(&self, transaction: BundledTransaction, hash: Hash) -> Option<TransactionRef> {
        if let Some(tx) = protocol_tangle().insert(transaction, hash.clone(), TransactionMetadata::new()) {
            self.propagate_otrsi_and_ytrsi(&hash, &tx);
            return Some(tx);
        }
        None
    }

    fn propagate_otrsi_and_ytrsi(&self, hash: &Hash, tx_ref: &TransactionRef) {

        // check if transaction is solid
        if protocol_tangle().is_solid_transaction(&hash) {

            let otrsi = min(
                self.get_otrsi(&tx_ref.trunk()).unwrap(),
                self.get_otrsi(&tx_ref.branch()).unwrap(),
            );
            let ytrsi = max(
                self.get_ytrsi(&tx_ref.trunk()).unwrap(),
                self.get_ytrsi(&tx_ref.branch()).unwrap(),
            );

            let mut metadata = TsaMetadata::new();
            metadata.otrsi = Some(otrsi);
            metadata.ytrsi = Some(ytrsi);

            self.tsa_metadata.insert(hash.clone(), metadata);

        }

    }

}