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

use bee_common::{LoggerConfig, LoggerConfigBuilder};
use bee_network::{NetworkConfig, NetworkConfigBuilder};
use bee_peering::{PeeringConfig, PeeringConfigBuilder};
use bee_protocol::{ProtocolConfig, ProtocolConfigBuilder};
use bee_snapshot::{SnapshotConfig, SnapshotConfigBuilder};

use serde::Deserialize;

pub(crate) const CONFIG_PATH: &str = "./config.toml";

#[derive(Default, Deserialize)]
pub(crate) struct NodeConfigBuilder {
    logger: LoggerConfigBuilder,
    network: NetworkConfigBuilder,
    peering: PeeringConfigBuilder,
    protocol: ProtocolConfigBuilder,
    snapshot: SnapshotConfigBuilder,
}

impl NodeConfigBuilder {
    pub(crate) fn finish(self) -> NodeConfig {
        NodeConfig {
            logger: self.logger.finish(),
            network: self.network.finish(),
            peering: self.peering.finish(),
            protocol: self.protocol.finish(),
            snapshot: self.snapshot.finish(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct NodeConfig {
    pub(crate) logger: LoggerConfig,
    pub(crate) network: NetworkConfig,
    pub(crate) peering: PeeringConfig,
    pub(crate) protocol: ProtocolConfig,
    pub(crate) snapshot: SnapshotConfig,
}
