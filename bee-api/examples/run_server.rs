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

use bee_api::{config::ApiConfigBuilder, rest};
use bee_ternary::{T1B1Buf, TritBuf, TryteBuf};

use bee_common_ext::shutdown::Shutdown;
use bee_crypto::ternary::Hash;
use bee_protocol::tangle::{tangle, TransactionMetadata};
use bee_transaction::bundled::{BundledTransaction, BundledTransactionField};

use std::time::Duration;

#[tokio::main]
async fn main() {
    bee_protocol::tangle::init();

    let tx = BundledTransaction::from_trits(&TritBuf::<T1B1Buf>::zeros(BundledTransaction::trit_len())).unwrap();
    let tx_hash = Hash::try_from_inner(
        TryteBuf::try_from_str("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")
            .unwrap()
            .as_trits()
            .encode::<T1B1Buf>(),
    )
    .unwrap();

    tangle().insert(tx, tx_hash, TransactionMetadata::new());
    assert_eq!(tangle().contains(&tx_hash), true);

    let mut shutdown = Shutdown::new();
    rest::server::run(ApiConfigBuilder::new().finish(), &mut shutdown);

    let seconds = 10;
    println!("Shutdown API in {} seconds...", seconds);
    tokio::time::delay_for(Duration::from_secs(seconds)).await;

    match shutdown.execute().await {
        Ok(_) => println!("Shutdown was successful!"),
        Err(_err) => println!("Shutdown was not successful!"),
    }
}
