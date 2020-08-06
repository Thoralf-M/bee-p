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

use bee_protocol::tangle::{TransactionMetadata, MsTangle, tangle as ms_tangle};
use bee_crypto::ternary::Hash;
use bee_transaction::{bundled::BundledTransaction, Vertex};
use bee_tangle::{Tangle, TransactionRef};
use dashmap::DashMap;
use std::cmp::{min, max};
use bee_protocol::MilestoneIndex;
use crate::model::WutrsMetadata;
use std::ops::Add;

const YTRSI_DELTA: u32 = 2; // C1
const OTRSI_DELTA: u32 = 7; // C2
const BELOW_MAX_DEPTH: u32 = 15; // M

pub struct WutrsTangle {
    pub metadata: DashMap<Hash, WutrsMetadata>,
    pub last_milestone_index: MilestoneIndex,
}

impl WutrsTangle {

    pub fn insert(&self, transaction: BundledTransaction, hash: Hash) -> Option<TransactionRef> {
        if let Some(tx_ref) = ms_tangle().insert(transaction, hash.clone(), TransactionMetadata::new()) {
            // check if a new milestone arrived
            if self.last_milestone_index != ms_tangle().get_last_milestone_index() {
                self.last_milestone_index.add(MilestoneIndex(1));
                self.set_otrsi_and_ytrsi_for_incoming_milestone(&ms_tangle().get_milestone_hash(self.last_milestone_index).unwrap())
            }
            self.propagate_otrsi_and_ytrsi(&hash);
            return Some(tx_ref);
        }
        None
    }

    // this function sets the otrsi and ytrsi for the incoming milestone
    fn set_otrsi_and_ytrsi_for_incoming_milestone(&self, root: &Hash) {
        let mut metadata = WutrsMetadata::new();
        metadata.otrsi = Some(MilestoneIndex(*ms_tangle().get_last_milestone_index() - BELOW_MAX_DEPTH));
        metadata.ytrsi = Some(ms_tangle().get_last_milestone_index());
        self.metadata.insert(root.clone(), metadata);
    }

    /*
    // this function tries to propagate the otrsi and ytrsi values so that
    // the TSA can be performed on most recent values.

    // if the parents of this incoming transaction are solid,
    // or in other words, if this incoming transaction is solid,
    // this function will propagate the otrsi and ytrsi of the parents to the incoming transaction.

    // in case of an attack, missing transactions might not arrive at all.
    // the propagation of otrsi and ytrsi to a non-solid cone might be unnecessary, since the TSA
    // will not select transactions from a non-solid cone.
    // therefore, if the incoming transaction is not solid, it won't propagate.
    // this helps to avoid unnecessary tangle walks.

    // if the children of a transaction arrived before their parents, it means that this incoming transaction was missing.
    // in this case, if the incoming transaction is solid, otrsi and ytrsi values need to be propagated to the solid future cone
    // since they don't have otrsi and ytrsi values set.
    */
    fn propagate_otrsi_and_ytrsi(&self, root: &Hash) {

        let mut children = vec![*root];

        while let Some(id) = children.pop() {

            if !ms_tangle().is_solid_transaction(&id) {
                continue
            }

            // unwrap is safe since the transaction is solid
            let tx_ref = ms_tangle().get(&id).unwrap();

            // therefore also the parents are solid
            let otrsi = min(
                self.get_otrsi(&tx_ref.trunk()).unwrap(),
                self.get_otrsi(&tx_ref.branch()).unwrap(),
            );
            let ytrsi = max(
                self.get_ytrsi(&tx_ref.trunk()).unwrap(),
                self.get_ytrsi(&tx_ref.branch()).unwrap(),
            );

            let mut metadata = WutrsMetadata::new();
            metadata.otrsi = Some(otrsi);
            metadata.ytrsi = Some(ytrsi);
            self.metadata.insert(id.clone(), metadata);

            for child in ms_tangle().get_children(&id).iter() {
                children.push(*child);
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