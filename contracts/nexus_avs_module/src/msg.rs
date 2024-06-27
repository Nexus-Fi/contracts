

use std::collections::HashMap;

use cosmwasm_std::{Addr,Binary, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_schema::cw_serde;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct OperatorDetails {
    pub earnings_receiver: Addr,
    pub delegation_approver: Option<Addr>,
    pub metadata_uri: String,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Operator {
    pub details: OperatorDetails,
    pub is_registered: bool,
}
pub const STATE: Item<State> = Item::new("state");
pub const OPERATORS: Map<&Addr, Operator> = Map::new("operators");
pub const DELEGATIONS: Map<&Addr, Addr> = Map::new("delegations");
static OPERATOR_DETAILS: Item<OperatorDetails> = Item::new("operator_details");
