use cosmwasm_std::{StdError, Uint128};
use thiserror::Error;
#[derive(Error, Debug)]
pub enum BalanceError {
    #[error("Insufficient balance: required {required}, but only have {available}")]
    InsufficientBalance { required: Uint128, available: Uint128 },
    
    #[error("Invalid amount: {reason}")]
    InvalidAmount { reason: String },
    
    #[error("Staker not found")]
    StakerNotFound {},
    
    #[error("Balance update would result in negative amount")]
    NegativeBalance {},
    
    #[error("Exchange rate cannot be zero")]
    ZeroExchangeRate {},
    
    #[error("Update timestamp is before last update")]
    InvalidTimestamp {},
    
    #[error("Std error: {0}")]
    Std(#[from] StdError),
}
