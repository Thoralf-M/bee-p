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

use crate::model::{Score, WurtsTransactionMetadata};
use bee_crypto::ternary::Hash;
use bee_protocol::{
    tangle::{tangle as ms_tangle, TransactionMetadata},
    MilestoneIndex,
};
use bee_tangle::TransactionRef;
use bee_transaction::{bundled::BundledTransaction, Vertex};
use rand::Rng;
use std::{
    cmp::{max, min},
    collections::HashMap,
};

const YTRSI_DELTA: u32 = 2; // C1
const OTRSI_DELTA: u32 = 7; // C2
const BELOW_MAX_DEPTH: u32 = 15; // M

pub struct WutrsTangle {
    transaction_metadata: HashMap<Hash, WurtsTransactionMetadata>,
    last_solid_milestone_index_known: MilestoneIndex,
}

impl WutrsTangle {
    pub fn insert(&mut self, transaction: BundledTransaction, hash: Hash) -> Option<TransactionRef> {
        if let Some(tx_ref) = ms_tangle().insert(transaction, hash.clone(), TransactionMetadata::new()) {
            if self.last_solid_milestone_index_known != ms_tangle().get_last_solid_milestone_index() {
                self.last_solid_milestone_index_known = ms_tangle().get_last_solid_milestone_index();
                self.update_confirmed_cone(self.last_solid_milestone_index_known);
            }
            self.propagate_otrsi_and_ytrsi(&hash);
            return Some(tx_ref);
        }
        None
    }

    // once a milestone arrives, update otrsi and ytrsi of all transactions that are confirmed by this milestone.
    // set otrsi and ytrsi values of confirmed transactions to:
    // otrsi=milestone_index
    // ytrsi=milestone_index
    // otrsi or ytrsi of transactions that are confirmed by a previous milestone won't get updated.
    fn update_confirmed_cone(&mut self, milestone_index: MilestoneIndex) {
        let ms_hash = ms_tangle().get_milestone_hash(milestone_index).unwrap();

        if ms_tangle().is_solid_transaction(&ms_hash) {
            let mut parents = vec![ms_hash];

            while let Some(tx_hash) = parents.pop() {
                self.transaction_metadata.get_mut(&tx_hash).unwrap().confirmed = Some(milestone_index);

                // unwrap is safe since the transaction is solid
                let tx_ref = ms_tangle().get(&tx_hash).unwrap();

                let mut metadata = WurtsTransactionMetadata::new();
                metadata.otrsi = Some(milestone_index);
                metadata.ytrsi = Some(milestone_index);
                self.transaction_metadata.insert(tx_hash.clone(), metadata);

                if self
                    .transaction_metadata
                    .get(&tx_ref.trunk())
                    .unwrap()
                    .confirmed
                    .is_none()
                {
                    parents.push(tx_ref.trunk().clone());
                }

                if self
                    .transaction_metadata
                    .get(&tx_ref.branch())
                    .unwrap()
                    .confirmed
                    .is_none()
                {
                    parents.push(tx_ref.branch().clone());
                }
            }
        }
    }

    // this function tries to propagate the otrsi and ytrsi values so that
    // the TSA can be performed on most recent values.
    //
    // if the parents of this incoming transaction are solid,
    // or in other words, if this incoming transaction is solid,
    // this function will propagate the otrsi and ytrsi of the parents to the incoming transaction.
    //
    // in case of an attack, missing transactions might not arrive at all.
    // the propagation of otrsi and ytrsi to a non-solid cone might be unnecessary, since the TSA
    // will not select transactions from a non-solid cone.
    // therefore, if the incoming transaction is not solid, it won't propagate.
    // this helps to avoid unnecessary tangle walks.
    //
    // if the children of a transaction arrived before their parents, it means that this incoming transaction was
    // missing. in this case, if the incoming transaction is solid, otrsi and ytrsi values need to be propagated to
    // the solid future cone since they don't have otrsi and ytrsi values set.
    fn propagate_otrsi_and_ytrsi(&mut self, root: &Hash) {
        let mut children = vec![*root];

        while let Some(id) = children.pop() {
            if !ms_tangle().is_solid_transaction(&id) {
                continue;
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

            let mut metadata = WurtsTransactionMetadata::new();
            metadata.otrsi = Some(otrsi);
            metadata.ytrsi = Some(ytrsi);
            self.transaction_metadata.insert(id.clone(), metadata);

            for child in ms_tangle().get_children(&id).iter() {
                children.push(*child);
            }
        }
    }

    fn get_ytrsi(&self, hash: &Hash) -> Option<MilestoneIndex> {
        match self.transaction_metadata.get(&hash) {
            Some(metadata) => metadata.ytrsi,
            None => None,
        }
    }

    fn get_otrsi(&self, hash: &Hash) -> Option<MilestoneIndex> {
        match self.transaction_metadata.get(&hash) {
            Some(metadata) => metadata.otrsi,
            None => None,
        }
    }

    fn get_tip_score(&self, tx_hash: &Hash) -> Score {
        let tx_ref = ms_tangle().get(tx_hash).unwrap();
        let ytrsi = self.transaction_metadata.get(tx_hash).unwrap().ytrsi.unwrap();
        let otrsi = self.transaction_metadata.get(tx_hash).unwrap().otrsi.unwrap();

        if *ms_tangle().get_last_solid_milestone_index() - *ytrsi > YTRSI_DELTA {
            return Score::Lazy;
        }

        if *ms_tangle().get_last_solid_milestone_index() - *otrsi > BELOW_MAX_DEPTH {
            return Score::Lazy;
        }

        let mut parent_otrsi_check = 2;

        if let Some(ma) = self.transaction_metadata.get(&tx_ref.trunk()) {
            // NOTE: removed as suggested by muxxer
            // if ma.score.unwrap_or(Score::NonLazy) == Score::Lazy {
            //     return Score::Lazy;
            // }

            if *ms_tangle().get_last_solid_milestone_index() - *ma.otrsi.unwrap() > OTRSI_DELTA {
                parent_otrsi_check -= 1;
            }
        }

        if let Some(pa) = self.transaction_metadata.get(&tx_ref.branch()) {
            // NOTE: removed as suggested by muxxer
            // if pa.score.unwrap_or(Score::NonLazy) == Score::Lazy {
            //     return Score::Lazy;
            // }

            if *ms_tangle().get_last_solid_milestone_index() - *pa.otrsi.unwrap() > OTRSI_DELTA {
                parent_otrsi_check -= 1;
            }
        }

        if parent_otrsi_check == 0 {
            println!("[get_score ] both parents failed 'parent_otrsi_check");

            return Score::Lazy;
        }

        if parent_otrsi_check == 1 {
            println!("[get_score ] one of the parents failed 'parent_otrsi_check (makes tip semi-lazy)");

            return Score::SemiLazy;
        }

        Score::NonLazy
    }

    /// Updates tip score, and performs the tip selection algorithm (TSA).
    pub fn select_tip(&mut self) -> Option<Hash> {
        if ms_tangle().num_tips() == 0 {
            return None;
        }

        let mut score_of_tips = HashMap::new();
        let mut score_sum = 0_isize;
        for tip in ms_tangle().tips() {
            if !ms_tangle().is_solid_transaction(&tip) {
                continue;
            }

            if self.transaction_metadata.get(&tip).unwrap().selected == 2 {
                continue;
            }

            let score = self.get_tip_score(&tip) as isize;
            if score == 0 {
                continue;
            }

            score_sum += score;
            score_of_tips.insert(tip, score);
        }

        // TODO: randomly select tip
        let mut rng = rand::thread_rng();
        let mut random_number = rng.gen_range(1, score_sum);

        for (id, score) in score_of_tips.iter() {
            random_number -= *score;
            if random_number <= 0 {
                self.transaction_metadata.get_mut(id).unwrap().selected += 1;
                return Some(*id);
            }
        }

        None
    }
}
