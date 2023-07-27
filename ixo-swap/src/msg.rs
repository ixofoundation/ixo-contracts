use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal, Uint128};
use cw1155::TokenId;
use cw20::{BalanceResponse, Expiration};
use serde::{Deserialize, Serialize};

#[cw_serde]
pub struct InstantiateMsg {
    pub token1_denom: Denom,
    pub token2_denom: Denom,
    pub lp_token_code_id: u64,
    pub owner: Option<String>,
    pub protocol_fee_recipient: String,
    // NOTE: Fees percents are out of 100 e.g., 1 = 1%
    pub protocol_fee_percent: Decimal,
    pub lp_fee_percent: Decimal,
}

#[cw_serde]
pub enum Denom {
    Native(String),
    Cw20(Addr),
    Cw1155(Addr, String),
}

#[cw_serde]
pub enum TokenSelect {
    Token1155,
    Token2,
}

#[cw_serde]
pub enum ExecuteMsg {
    AddLiquidity {
        token1155_amounts: Vec<(TokenId, Uint128)>,
        min_liquidity: Uint128,
        max_token2: Uint128,
        expiration: Option<Expiration>,
    },
    RemoveLiquidity {
        amount: Uint128,
        min_token1155: Vec<(TokenId, Uint128)>,
        min_token2: Uint128,
        expiration: Option<Expiration>,
    },
    Swap {
        input_token: TokenSelect,
        input_amount: Uint128,
        min_output: Uint128,
        expiration: Option<Expiration>,
    },
    /// Chained swap converting A -> B and B -> C by leveraging two swap contracts
    PassThroughSwap {
        output_amm_address: String,
        input_token: TokenSelect,
        input_token_amount: Uint128,
        output_min_token: Uint128,
        expiration: Option<Expiration>,
    },
    SwapAndSendTo {
        input_token: TokenSelect,
        input_amount: Uint128,
        recipient: String,
        min_token: Uint128,
        expiration: Option<Expiration>,
    },
    UpdateConfig {
        owner: Option<String>,
        lp_fee_percent: Decimal,
        protocol_fee_percent: Decimal,
        protocol_fee_recipient: String,
    },
    // Freeze adding new deposits
    FreezeDeposits {
        freeze: bool,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Implements CW20. Returns the current balance of the given address, 0 if unset.
    #[returns(BalanceResponse)]
    Balance { address: String },
    #[returns(InfoResponse)]
    Info {},
    #[returns(Token1ForToken2PriceResponse)]
    Token1ForToken2Price { token1_amount: Uint128 },
    #[returns(Token2ForToken1PriceResponse)]
    Token2ForToken1Price { token2_amount: Uint128 },
    #[returns(FeeResponse)]
    Fee {},
    #[returns(OwnerLpTokensBalanceResponse)]
    OwnerLpTokensBalance {
        owner: String,
        tokens_id: Vec<TokenId>,
    },
}

#[cw_serde]
pub struct MigrateMsg {
    pub owner: Option<String>,
    pub protocol_fee_recipient: String,
    pub protocol_fee_percent: Decimal,
    pub lp_fee_percent: Decimal,
    pub freeze_pool: bool,
}

#[cw_serde]
pub struct InfoResponse {
    pub token1_reserve: Uint128,
    pub token1_denom: Denom,
    pub token2_reserve: Uint128,
    pub token2_denom: Denom,
    pub lp_token_supply: Uint128,
    pub lp_token_address: String,
}

#[cw_serde]
pub struct FeeResponse {
    pub owner: Option<String>,
    pub lp_fee_percent: Decimal,
    pub protocol_fee_percent: Decimal,
    pub protocol_fee_recipient: String,
}

#[cw_serde]
pub struct Token1ForToken2PriceResponse {
    pub token2_amount: Uint128,
}

#[cw_serde]
pub struct Token2ForToken1PriceResponse {
    pub token1_amount: Uint128,
}

#[cw_serde]
pub struct OwnerLpTokensBalanceResponse {
    pub balances: Vec<Uint128>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, ::prost::Message)]
pub struct QueryTokenMetadataRequest {
    #[prost(string, tag = "1")]
    pub id: ::prost::alloc::string::String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, ::prost::Message)]
pub struct QueryTokenMetadataResponse {
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub description: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub decimals: ::prost::alloc::string::String,
    #[prost(string, tag = "4")]
    pub image: ::prost::alloc::string::String,
    #[prost(string, tag = "5")]
    pub index: ::prost::alloc::string::String,
}
