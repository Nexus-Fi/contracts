use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const ADMIN: Item<Addr> = Item::new("admin");
pub const WHITELIST: Map<&Addr, bool> = Map::new("whitelist");
pub const POINTS: Map<&Addr, u64> = Map::new("points");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub st_nibi_token: Addr,
}

pub const CONFIG: Item<Config> = Item::new("config");