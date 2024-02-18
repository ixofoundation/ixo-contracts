use std::collections::HashMap;

use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal, Uint128};
use cw1155::TokenId;
use cw20::{BalanceResponse, Expiration};
use serde::{Deserialize, Serialize};

use crate::token_amount::TokenAmount;

#[cw_serde]
pub struct InstantiateMsg {
    pub token1155_denom: Denom,
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
        token1155_amounts: HashMap<TokenId, Uint128>,
        min_liquidity: Uint128,
        max_token2: Uint128,
        expiration: Option<Expiration>,
    },
    RemoveLiquidity {
        amount: Uint128,
        min_token1155: TokenAmount,
        min_token2: Uint128,
        expiration: Option<Expiration>,
    },
    Swap {
        input_token: TokenSelect,
        input_amount: TokenAmount,
        min_output: TokenAmount,
        expiration: Option<Expiration>,
    },
    /// Chained swap converting A -> B and B -> C by leveraging two swap contracts
    PassThroughSwap {
        output_amm_address: String,
        input_token: TokenSelect,
        input_token_amount: TokenAmount,
        output_min_token: TokenAmount,
        expiration: Option<Expiration>,
    },
    SwapAndSendTo {
        input_token: TokenSelect,
        input_amount: TokenAmount,
        recipient: String,
        min_token: TokenAmount,
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
    #[returns(Token1155ForToken2PriceResponse)]
    Token1155ForToken2Price { token1155_amount: TokenAmount },
    #[returns(Token2ForToken1155PriceResponse)]
    Token2ForToken1155Price { token2_amount: TokenAmount },
    #[returns(FeeResponse)]
    Fee {},
    #[returns(TokenSuppliesResponse)]
    TokenSupplies { tokens_id: Vec<TokenId> },
    #[returns(FreezeStatusResponse)]
    FreezeStatus {},
}

#[cw_serde]
pub struct InfoResponse {
    pub token1155_reserve: Uint128,
    pub token1155_denom: Denom,
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
pub struct Token1155ForToken2PriceResponse {
    pub token2_amount: Uint128,
}

#[cw_serde]
pub struct Token2ForToken1155PriceResponse {
    pub token1155_amount: Uint128,
}

#[cw_serde]
pub struct TokenSuppliesResponse {
    pub supplies: Vec<Uint128>,
}

#[cw_serde]
pub struct FreezeStatusResponse {
    pub status: bool,
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
