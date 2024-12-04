use cosmwasm_std::Addr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub admin: String,
    pub st_nibi_token: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    AddToWhitelist { address: String },
    RemoveFromWhitelist { address: String },
    AddPoints { address: String, points: u64 },
    SubtractPoints { address: String, points: u64 },
    TransferPoints { from: String, to: String, points: u64 },
    UpdateAdmin { new_admin: String },
    UpdateStNibiToken { new_token: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GetPoints { address: String },
    IsWhitelisted { address: String },
    CheckStNibiBalance { address: String },
    GetAdmin {},
    GetStNibiToken {},
    GetTotalPoints {},
    GetWhitelistedAddresses {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PointsResponse {
    pub address: String,
    pub points: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct WhitelistResponse {
    pub is_whitelisted: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StNibiBalanceResponse {
    pub address: String,
    pub has_balance: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AdminResponse {
    pub admin: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StNibiTokenResponse {
    pub token: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TotalPointsResponse {
    pub total: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct WhitelistedAddressesResponse {
    pub addresses: Vec<String>,
}