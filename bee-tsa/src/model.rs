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

use bee_protocol::MilestoneIndex;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Score {
    Lazy = 0,
    SemiLazy = 1,
    NonLazy = 2,
}

pub struct TsaMetadata {
    pub otrsi: Option<MilestoneIndex>, // can only be missing if ma and pa were missing; same for ytrsi
    pub ytrsi: Option<MilestoneIndex>,
    pub selected: u8, //number of times we selected it in the TSA
}

impl TsaMetadata {
    pub fn new() -> Self {
        Self {
            otrsi: None,
            ytrsi: None,
            selected: 0,
        }
    }
}