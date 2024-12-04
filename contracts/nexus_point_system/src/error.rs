use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Address is not whitelisted")]
    NotWhitelisted {},

    #[error("Insufficient points")]
    InsufficientPoints {},

    #[error("Invalid address")]
    InvalidAddress {},

    #[error("Invalid token address")]
    InvalidTokenAddress {},

    #[error("Points overflow")]
    PointsOverflow {},

}
