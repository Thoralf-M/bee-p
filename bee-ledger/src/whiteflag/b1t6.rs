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

use bee_ternary::Trits;

use std::convert::TryFrom;

const TRITS_PER_TRYTE: usize = 3;
const TRITS_PER_BYTE: usize = 6;

pub(crate) fn decode(src: &Trits) -> Vec<u8> {
    if src.len() % TRITS_PER_BYTE != 0 {
        // TODO do something
        panic!();
    }

    let mut bytes = Vec::with_capacity(src.len() / TRITS_PER_BYTE);

    for j in (0..src.len()).step_by(TRITS_PER_BYTE) {
        let t1 = i8::try_from(&src[j..j + TRITS_PER_TRYTE]).unwrap();
        let t2 = i8::try_from(&src[j + TRITS_PER_TRYTE..j + TRITS_PER_BYTE]).unwrap();
        let b = decode_group(t1, t2).unwrap();
        bytes.push(b as u8);
    }

    bytes
}

fn decode_group(t1: i8, t2: i8) -> Result<i8, ()> {
    let v = t1 + t2 * 27;

    if v < i8::MIN || v > i8::MAX {
        return Err(());
    }

    Ok(v)
}
