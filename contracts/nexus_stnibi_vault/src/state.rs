use cosmwasm_std::{
    entry_point, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
    Uint128, CosmosMsg, BankMsg, attr, StdError,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub stnibi_denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserInfo {
    pub locked_amount: Uint128,
}

use cw_storage_plus::{Item, Map};

pub const STATE: Item<State> = Item::new("state");
pub const USER_INFO: Map<&str, UserInfo> = Map::new("user_info");
