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
    pub metadata: DashMap<Hash, TsaMetadata>,
}

impl TsaTangle {

    pub fn insert(&self, transaction: BundledTransaction, hash: Hash) -> Option<TransactionRef> {
        if let Some(tx_ref) = protocol_tangle().insert(transaction, hash.clone(), TransactionMetadata::new()) {
            self.propagate_otrsi_and_ytrsi(&hash);
            return Some(tx_ref);
        }
        None
    }

    fn propagate_otrsi_and_ytrsi(&self, root: &Hash) {

        // this function tries to propagate the otrsi and ytrsi values so that
        // the TSA can be performed on the most accurate values.

        // if the parents of this incoming transaction are solid,
        // or in other words, if this incoming transaction is solid,
        // this function will propagate the otrsi and ytrsi of the parents to the incoming transaction.

        // in case of an attack, missing transactions might not arrive at all.
        // the propagation of otrsi and ytrsi to a non-solid cone might be unnecessary, since the TSA
        // would not select transactions from that cone.
        // therefore, if the incoming transaction is non-solid, it won't propagate.
        // this helps to avoid unnecessary tangle walks.

        // if the children of a transaction already did arrive before the parent,it implies that the incoming transaction was missing.
        // in this case, if the incoming transaction is solid, otrsi and ytrsi values need to be propagated to the solid future cone
        // since they don't have otrsi and ytrsi values set.

        // if a milestone transaction arrives, otrsi and ytrsi values for transactions do change.
        // because of this, the past and future cones ne

        let mut children = vec![*root];

        while let Some(id) = children.pop() {

            if !protocol_tangle().is_solid_transaction(&id) {
                continue
            }

            match protocol_tangle().get(&id) {
                Some(tx_ref) => {

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

                    self.metadata.insert(id.clone(), metadata);

                    // propagate state to solid children
                    for child in protocol_tangle().get_children(&id).iter() {
                        children.push(*child);
                    }

                }
                None => continue
            }

        }
    }

    fn get_ytrsi(&self, hash: &Hash) -> Option<MilestoneIndex>{
        match self.metadata.get(&hash) {
            Some(metadata) => metadata.ytrsi,
            None => None
        }
    }

    fn get_otrsi(&self, hash: &Hash) -> Option<MilestoneIndex>{
        match self.metadata.get(&hash) {
            Some(metadata) => metadata.otrsi,
            None => None
        }
    }

}