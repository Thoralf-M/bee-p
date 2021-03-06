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

use crate::whiteflag::{bundle::load_bundle_builder, metadata::WhiteFlagMetadata, worker::LedgerWorker};

use bee_crypto::ternary::Hash;
use bee_protocol::tangle::tangle;
use bee_transaction::{
    bundled::{Bundle, IncomingBundleBuilderError},
    Vertex,
};

use std::collections::HashSet;

const IOTA_SUPPLY: u64 = 2_779_530_283_277_761;

#[derive(Debug)]
pub(crate) enum Error {
    MissingBundle,
    NotATail,
    InvalidBundle(IncomingBundleBuilderError),
}

impl LedgerWorker {
    #[inline]
    fn on_bundle(&mut self, hash: &Hash, bundle: &Bundle, metadata: &mut WhiteFlagMetadata) {
        let mut conflicting = false;
        let (mutates, bundle_mutations) = bundle.ledger_mutations();

        if !mutates {
            metadata.num_tails_zero_value += 1;
        } else {
            // First pass to look for conflicts.
            for (address, diff) in bundle_mutations.iter() {
                let balance = *self.state.get_or_zero(&address) as i64 + diff;

                if balance < 0 || balance.abs() as u64 > IOTA_SUPPLY {
                    metadata.num_tails_conflicting += 1;
                    conflicting = true;
                    break;
                }
            }

            if !conflicting {
                // Second pass to mutate the state.
                for (address, diff) in bundle_mutations {
                    self.state.apply(address.clone(), diff);
                    metadata.diff.apply(address, diff);
                }

                metadata.tails_included.push(*hash);
            }
        }

        metadata.num_tails_referenced += 1;

        // TODO this only actually confirm tails
        tangle().update_metadata(&hash, |meta| {
            if conflicting {
                meta.flags_mut().set_conflicting();
            }
            meta.flags_mut().set_confirmed();
            meta.set_milestone_index(metadata.index);
            meta.set_confirmation_timestamp(metadata.timestamp);
            // TODO Set OTRSI, ...
            // TODO increment metrics confirmed, zero, value and conflict.
        });
    }

    pub(crate) fn visit_bundles_dfs(&mut self, root: Hash, metadata: &mut WhiteFlagMetadata) -> Result<(), Error> {
        let mut hashes = vec![root];
        let mut visited = HashSet::new();

        while let Some(hash) = hashes.last() {
            // TODO pass match to avoid repetitions
            match load_bundle_builder(hash) {
                Some(bundle_builder) => {
                    let trunk = bundle_builder.trunk();
                    let branch = bundle_builder.branch();
                    // TODO justify
                    let meta = tangle().get_metadata(hash).unwrap();

                    if !meta.flags().is_tail() {
                        return Err(Error::NotATail);
                    }

                    // TODO get previous meta instead of loading these bundles ?
                    if meta.flags().is_confirmed() {
                        visited.insert(hash.clone());
                        hashes.pop();
                        continue;
                    }

                    if visited.contains(trunk) && visited.contains(branch) {
                        // TODO check valid and strict semantic
                        let bundle = match bundle_builder.validate() {
                            Ok(builder) => builder.build(),
                            Err(e) => return Err(Error::InvalidBundle(e)),
                        };
                        self.on_bundle(hash, &bundle, metadata);
                        visited.insert(hash.clone());
                        hashes.pop();
                    } else if !visited.contains(trunk) {
                        hashes.push(*trunk);
                    } else if !visited.contains(branch) {
                        hashes.push(*branch);
                    }
                }
                None => {
                    if !tangle().is_solid_entry_point(hash) {
                        return Err(Error::MissingBundle);
                    } else {
                        visited.insert(hash.clone());
                        hashes.pop();
                    }
                }
            }
        }

        Ok(())
    }
}
