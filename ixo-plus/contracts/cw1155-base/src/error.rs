use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("token_id already claimed")]
    Claimed {},

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Expired")]
    Expired {},
}
