use cosmwasm_std::{Decimal, StdError, Uint128};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Cw20Error(#[from] cw20_base_lp::ContractError),

    #[error("{0}")]
    PaymentError(#[from] cw_utils::PaymentError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Min liquidity error: requested: {min_liquidity}, available: {liquidity_available}")]
    MinLiquidityError {
        min_liquidity: Uint128,
        liquidity_available: Uint128,
    },

    #[error("Max token error: max_token: {max_token}, tokens_required: {tokens_required}")]
    MaxTokenError {
        max_token: Uint128,
        tokens_required: Uint128,
    },

    #[error("Min token amount error: requested: {min_token}, min_required {min_required}")]
    MinTokenAmountError {
        min_token: Uint128,
        min_required: Uint128,
    },

    #[error("Min token error: minimum token amount can not be 0")]
    MinTokenError {},

    #[error("Insufficient liquidity error: requested: {requested}, available: {available}")]
    InsufficientLiquidityError {
        requested: Uint128,
        available: Uint128,
    },

    #[error("Min token1155 error: requested: {requested}, available: {available}")]
    MinToken1155Error {
        requested: Uint128,
        available: Uint128,
    },

    #[error("Min token2 error: requested: {requested}, available: {available}")]
    MinToken2Error {
        requested: Uint128,
        available: Uint128,
    },

    #[error("Swap min error: min: {min}, available: {available}")]
    SwapMinError { min: Uint128, available: Uint128 },

    #[error("MsgExpirationError")]
    MsgExpirationError {},

    #[error("Total fee ({total_fee_percent}) percent is higher than max ({max_fee_percent})")]
    FeesTooHigh {
        max_fee_percent: Decimal,
        total_fee_percent: Decimal,
    },

    #[error("InsufficientFunds")]
    InsufficientFunds {},

    #[error("Uknown reply id: {id}")]
    UnknownReplyId { id: u64 },

    #[error("Failed to instantiate lp token")]
    InstantiateLpTokenError {},

    #[error("Provided token type do not correspond the type of contract token")]
    InvalidTokenType {},

    #[error("Provided token amount {amount} do not correspond the type of token")]
    InvalidTokenAmount { amount: Uint128 },

    #[error("Provided output amm is invalid")]
    InvalidOutputPool {},

    #[error(
        "Provided percent {percent} is invalid, must be greater than 0 and less than or equal 100"
    )]
    InvalidPercent { percent: Decimal },

    #[error("Unauthorized pool freeze - sender is not an owner or owner has not been set")]
    UnauthorizedPoolFreeze {},

    #[error("This pools are frozen - you can not deposit or swap tokens")]
    FrozenPool {},

    #[error("Provided token address: {address} is duplicated")]
    DuplicatedTokenAddress { address: String },

    #[error("Provided new owner is already an owner of the contract")]
    DuplicatedOwner {},

    #[error("Pools are already in {freeze_status} status")]
    DuplicatedFreezeStatus { freeze_status: bool },

    #[error("Token with id: {id} has unsupported denom")]
    UnsupportedTokenDenom { id: String },
}
