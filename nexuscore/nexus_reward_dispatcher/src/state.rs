
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Decimal};

use cw_storage_plus::Item;

pub static CONFIG: Item<Config> = Item::new("config");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: Addr,
    pub staking_nibi_contract: Addr,
    pub stnibi_reward_denom: String,
    pub nexusfi_fee_address: Addr,
    pub nexusfi_fee_rate: Decimal,
}
