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

use serde_json::Value as JsonValue;

use crate::{
    format::items::hash::HashItem,
    service::{TransactionByHashParams, TransactionByHashResponse},
};
use std::convert::{From, TryFrom};

impl TryFrom<&str> for TransactionByHashParams {
    type Error = &'static str;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(TransactionByHashParams {
            hash: HashItem::try_from(value)?,
        })
    }
}

impl From<TransactionByHashResponse> for JsonValue {
    fn from(res: TransactionByHashResponse) -> Self {
        match res.tx_ref {
            Some(tx_ref) => JsonValue::from(&tx_ref),
            None => JsonValue::Null,
        }
    }
}
