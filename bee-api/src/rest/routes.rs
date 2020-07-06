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

use crate::service::{Service, ServiceImpl, TransactionsByBundleParams};

use serde_json::Value as JsonValue;

use async_trait::async_trait;

use crate::{
    api::Api,
    format::json_utils::{json_error_obj, json_success_obj},
    service::{TransactionByHashParams, TransactionsByHashesParams},
};
use std::convert::TryFrom;

type WarpJsonReply = Result<warp::reply::Json, warp::Rejection>;
pub struct RestApi;
#[async_trait]
impl Api for RestApi {
    type NodeInfoApiResponse = WarpJsonReply;
    type TransactionsByBundleApiParams = JsonValue;
    type TransactionsByBundleApiResponse = WarpJsonReply;
    type TransactionByHashApiParams = String;
    type TransactionByHashApiResponse = WarpJsonReply;
    type TransactionsByHashesApiParams = JsonValue;
    type TransactionsByHashesApiResponse = WarpJsonReply;

    async fn node_info() -> Self::NodeInfoApiResponse {
        Ok(warp::reply::json(&JsonValue::from(ServiceImpl::node_info())))
    }

    async fn transactions_by_bundle(params: Self::TransactionsByBundleApiParams) -> Self::TransactionsByBundleApiResponse {
        match TransactionsByBundleParams::try_from(&params) {
            Ok(params) =>

                match ServiceImpl::transactions_by_bundle(params) {
                    Ok(res) => {
                        Ok(warp::reply::json(&json_success_obj(
                                             res.into()
                        )))
                    }
                    Err(e) => Ok(warp::reply::json(&json_error_obj(
                        &e.msg,
                    )))
                }
                ,
            Err(msg) => Ok(warp::reply::json(&json_error_obj(
                msg,
            ))),
        }
    }

    async fn transaction_by_hash(params: Self::TransactionByHashApiParams) -> Self::TransactionByHashApiResponse {
        match TransactionByHashParams::try_from(params.as_str()) {
            Ok(params) => Ok(warp::reply::json(&json_success_obj(
                ServiceImpl::transaction_by_hash(params).into(),
            ))),
            Err(msg) => Ok(warp::reply::json(&json_error_obj(
                msg,
            ))),
        }
    }

    async fn transactions_by_hashes(params: Self::TransactionsByHashesApiParams) -> Self::TransactionsByHashesApiResponse {
        match TransactionsByHashesParams::try_from(&params) {
            Ok(params) => Ok(warp::reply::json(&json_success_obj(
                ServiceImpl::transactions_by_hashes(params).into(),
            ))),
            Err(msg) => Ok(warp::reply::json(&json_error_obj(
                msg,
            ))),
        }
    }
}
