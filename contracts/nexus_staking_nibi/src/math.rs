// Copyright 2021 Anchor Protocol. Modified by nexus
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use cosmwasm_std::{Decimal, Uint128};

const DECIMAL_FRACTIONAL: u128 = 1_000_000_000_000_000_000u128;

/// return a / b
pub fn decimal_division(a: Uint128, b: Decimal) -> Uint128 {
    let decimal = Decimal::from_ratio(a, b * Uint128::from(DECIMAL_FRACTIONAL));
    decimal * Uint128::from(DECIMAL_FRACTIONAL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decimal_division() {
        let a = Uint128::from(100u64);
        let b = Decimal::from_ratio(Uint128::from(10u64), Uint128::from(50u64));
        let res = decimal_division(a, b);
        assert_eq!(res, Uint128::from(500u64));
    }
}
