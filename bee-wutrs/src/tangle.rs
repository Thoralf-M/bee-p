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
use log::{debug, error, info};
use rand::Rng;
use std::{
    cmp::{max, min},
    collections::{HashMap, HashSet},
    ptr,
    sync::atomic::{AtomicBool, AtomicPtr, Ordering},
};

const YTRSI_DELTA: u32 = 2; // C1
const OTRSI_DELTA: u32 = 7; // C2
const BELOW_MAX_DEPTH: u32 = 15; // M

static TANGLE: AtomicPtr<WutrsTangle> = AtomicPtr::new(ptr::null_mut());
static INITIALIZED: AtomicBool = AtomicBool::new(false);

pub fn init() {
    if !INITIALIZED.compare_and_swap(false, true, Ordering::Relaxed) {
        TANGLE.store(Box::into_raw(WutrsTangle::new().into()), Ordering::Relaxed);
    } else {
        panic!("WURTS Tangle already initialized");
    }
}

pub fn tangle() -> &'static WutrsTangle {
    let tangle = TANGLE.load(Ordering::Relaxed);
    if tangle.is_null() {
        panic!("WURTS Tangle cannot be null");
    } else {
        unsafe { &*tangle }
    }
}

pub struct WutrsTangle {
    transaction_metadata: HashMap<Hash, WurtsTransactionMetadata>,
    last_solid_milestone_index_known: Option<MilestoneIndex>,
}

impl WutrsTangle {
    pub fn new() -> Self {
        Self {
            transaction_metadata: HashMap::new(),
            last_solid_milestone_index_known: None,
        }
    }
    pub fn insert(&mut self, transaction: BundledTransaction, hash: Hash) -> Option<TransactionRef> {
        if let Some(tx_ref) = ms_tangle().insert(transaction, hash.clone(), TransactionMetadata::new()) {
            debug!("Transaction {} inserted...", &hash);
            // ensures that each inserted transaction has a wurts-metadata
            self.transaction_metadata
                .insert(hash.clone(), WurtsTransactionMetadata::new());
            // in the case that a new milestone became solid
            if self.last_solid_milestone_index_known.is_none()
                || self.last_solid_milestone_index_known.unwrap() != ms_tangle().get_last_solid_milestone_index()
            {
                self.last_solid_milestone_index_known = Some(ms_tangle().get_last_solid_milestone_index());
                debug!(
                    "Milestone with index {} become solid. Updating last solid milestone cone...",
                    *self.last_solid_milestone_index_known.unwrap()
                );
                self.update_transactions_referenced_by_milestone(self.last_solid_milestone_index_known.unwrap());
            } else {
                debug!("Propagate OTRSI and YTRSI to incoming transaction...");
                self.inherit_otrsi_and_ytrsi_from_parents(&hash);
            }
            return Some(tx_ref);
        }
        None
    }

    // when a milestone arrives, otrsi and ytrsi of all transactions referenced by this milestone must be updated
    // otrsi or ytrsi of transactions that are referenced by a previous milestone won't get updated
    // set otrsi and ytrsi values of relevant transactions to:
    // otrsi=milestone_index
    // ytrsi=milestone_index
    fn update_transactions_referenced_by_milestone(&mut self, milestone_index: MilestoneIndex) {
        let mut visited = HashSet::new();
        let mut to_visit = vec![ms_tangle().get_milestone_hash(milestone_index).unwrap()];

        while let Some(tx_hash) = to_visit.pop() {
            if visited.contains(&tx_hash) {
                continue;
            } else {
                visited.insert(tx_hash.clone());
            }

            let metadata = self.transaction_metadata.get_mut(&tx_hash).unwrap();
            metadata.confirmed = Some(milestone_index);
            metadata.otrsi = Some(milestone_index);
            metadata.ytrsi = Some(milestone_index);

            // propagate the new otrsi and ytrsi values to the children of this transaction
            // children who already have inherited the new otrsi and ytrsi values, won't get updated
            for child in ms_tangle().get_children(&tx_hash) {
                self.inherit_otrsi_and_ytrsi_from_parents(&child);
            }

            let tx_ref = ms_tangle().get(&tx_hash).unwrap();

            if self
                .transaction_metadata
                .get(&tx_ref.trunk())
                .unwrap()
                .confirmed
                .is_none()
            {
                to_visit.push(tx_ref.trunk().clone());
            }

            if self
                .transaction_metadata
                .get(&tx_ref.branch())
                .unwrap()
                .confirmed
                .is_none()
            {
                to_visit.push(tx_ref.branch().clone());
            }
        }
    }

    // if the parents of this incoming transaction are solid,
    // or in other words, if this incoming transaction is solid,
    // this incoming transaction will inherit the best otrsi and ytrsi of the parents.
    //
    // in case of an attack, missing transactions might not arrive at all.
    // the propagation of otrsi and ytrsi to a non-solid cone might be unnecessary, since the TSA
    // would not attach to a non-solid cone.
    // therefore, if the incoming transaction is not solid, it won't propagate.
    // this helps to avoid unnecessary tangle walks.
    //
    // if the children of the incoming transaction already arrived before, it means that this incoming transaction was
    // missing. in this case, if the incoming transaction is solid, otrsi and ytrsi values need to be propagated to the
    // the future cone since they might not have
    // otrsi and ytrsi values set.
    fn inherit_otrsi_and_ytrsi_from_parents(&mut self, root: &Hash) {
        let mut visited = HashSet::new();
        let mut to_visit = vec![*root];

        while let Some(tx_hash) = to_visit.pop() {
            if visited.contains(&tx_hash) {
                continue;
            } else {
                visited.insert(tx_hash.clone());
            }

            if !ms_tangle().is_solid_transaction(&tx_hash) {
                continue;
            }

            // get the best otrsi and ytrsi of parents
            let tx_ref = ms_tangle().get(&tx_hash).unwrap();
            let otrsi = min(
                self.otrsi(&tx_ref.trunk()).unwrap(),
                self.otrsi(&tx_ref.branch()).unwrap(),
            );
            let ytrsi = max(
                self.ytrsi(&tx_ref.trunk()).unwrap(),
                self.ytrsi(&tx_ref.branch()).unwrap(),
            );

            // in case the transaction has already inherited otrsi and ytrsi from the parents, continue
            let metadata = self.transaction_metadata.get_mut(&tx_hash).unwrap();
            if metadata.otrsi == Some(otrsi) && metadata.ytrsi == Some(ytrsi) {
                continue;
            } else {
                metadata.otrsi = Some(otrsi);
                metadata.ytrsi = Some(ytrsi);
            }

            for child in ms_tangle().get_children(&tx_hash).iter() {
                to_visit.push(child.clone());
            }
        }
    }

    fn ytrsi(&self, hash: &Hash) -> Option<MilestoneIndex> {
        match self.transaction_metadata.get(&hash) {
            Some(metadata) => metadata.ytrsi,
            None => None,
        }
    }

    fn otrsi(&self, hash: &Hash) -> Option<MilestoneIndex> {
        match self.transaction_metadata.get(&hash) {
            Some(metadata) => metadata.otrsi,
            None => None,
        }
    }

    fn tip_score(&self, tx_hash: &Hash) -> Score {
        if *ms_tangle().get_last_solid_milestone_index() - *self.ytrsi(&tx_hash).unwrap() > YTRSI_DELTA {
            return Score::Lazy;
        }

        if *ms_tangle().get_last_solid_milestone_index() - *self.otrsi(&tx_hash).unwrap() > BELOW_MAX_DEPTH {
            return Score::Lazy;
        }

        let tx_ref = ms_tangle().get(tx_hash).unwrap();

        let trunk_check = {
            if *ms_tangle().get_last_solid_milestone_index() - *self.otrsi(tx_ref.trunk()).unwrap() > OTRSI_DELTA {
                false
            } else {
                true
            }
        };

        let branch_check = {
            if *ms_tangle().get_last_solid_milestone_index() - *self.otrsi(tx_ref.branch()).unwrap() > OTRSI_DELTA {
                false
            } else {
                true
            }
        };

        if !trunk_check && !branch_check {
            return Score::Lazy;
        }

        if !trunk_check || !branch_check {
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

            let score = self.tip_score(&tip) as isize;
            if score == 0 {
                continue;
            }

            score_sum += score;
            score_of_tips.insert(tip, score);
        }

        let mut rng = rand::thread_rng();
        let mut random_number = rng.gen_range(1, score_sum);

        for (hash, score) in score_of_tips.iter() {
            random_number -= *score;
            if random_number <= 0 {
                self.transaction_metadata.get_mut(hash).unwrap().selected += 1;
                return Some(*hash);
            }
        }

        None
    }
}
