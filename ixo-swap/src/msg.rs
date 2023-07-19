use cw1155_lp::TokenInfo;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Decimal, Uint128};

use cw20::Expiration;

use crate::state::Config;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub token1_denom: Denom,
    pub token2_denom: Denom,
    pub lp_token: Option<TokenSelect>,
    pub lp_token_code_id: u64,
    pub owner: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Denom {
    Native(String),
    Cw20(Addr),
    Cw1155(Addr),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub enum TokenSelect {
    Token1,
    Token2,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    AddLiquidity {
        input_token1: Vec<TokenInfo>,
        max_token2: Vec<TokenInfo>,
        min_liquidities: Vec<Uint128>,
        expiration: Option<Expiration>,
    },
    RemoveLiquidity {
        input_amounts: Vec<Uint128>,
        min_token1: Vec<TokenInfo>,
        min_token2: Vec<TokenInfo>,
        expiration: Option<Expiration>,
    },
    Swap {
        input_token_select: TokenSelect,
        input_tokens: Vec<TokenInfo>,
        output_min_tokens: Vec<TokenInfo>,
        expiration: Option<Expiration>,
    },
    /// Chained swap converting A -> B and B -> C by leveraging two swap contracts
    PassThroughSwap {
        output_amm_address: String,
        input_token_select: TokenSelect,
        input_tokens: Vec<TokenInfo>,
        output_min_tokens: Vec<TokenInfo>,
        expiration: Option<Expiration>,
    },
    SwapAndSendTo {
        input_token_select: TokenSelect,
        input_tokens: Vec<TokenInfo>,
        output_min_tokens: Vec<TokenInfo>,
        recipient: String,
        expiration: Option<Expiration>,
    },
    UpdateFees {
        protocol_fee_recipient: String,
        protocol_fee_percent: Decimal,
        lp_fee_percent: Decimal,
    },
    UpdateConfig {
        config: Config,
    },
    TransferOwnership {
        owner: String,
    },
    FreezeDeposits {
        freeze: bool,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Implements CW20. Returns the current balance of the given address, 0 if unset.
    Balance {
        address: String,
    },
    Info {},
    Token1ForToken2Price {
        input_tokens: Vec<TokenInfo>,
        output_tokens: Option<Vec<TokenInfo>>,
    },
    Token2ForToken1Price {
        input_tokens: Vec<TokenInfo>,
        output_tokens: Option<Vec<TokenInfo>>,
    },
    Fee {},
    Config {},
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct MigrateMsg {
    pub owner: String,
    pub protocol_fee_recipient: String,
    pub protocol_fee_percent: Decimal,
    pub lp_fee_percent: Decimal,
    pub freeze_pool: bool,
    pub config: Config,
}

#[derive(Clone, PartialEq, ::prost::Message)]
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InfoResponse {
    pub owner: String,
    pub token1_reserves: Vec<TokenInfo>,
    pub token1_denom: Denom,
    pub token2_reserves: Vec<TokenInfo>,
    pub token2_denom: Denom,
    pub lp_token_supplies: Vec<TokenInfo>,
    pub lp_token_address: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
pub struct FeeResponse {
    pub lp_fee_percent: Decimal,
    pub protocol_fee_percent: Decimal,
    pub protocol_fee_recipient: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Token1ForToken2PriceResponse {
    pub token2_amounts: Vec<TokenInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Token2ForToken1PriceResponse {
    pub token1_amounts: Vec<TokenInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub config: Config,
}
