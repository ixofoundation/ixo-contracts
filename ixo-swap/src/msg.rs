use cw1155_lp::TokenInfo;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Binary, Decimal, Uint128};

use cw20::Expiration;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub token1_denom: Denom,
    pub token2_denom: Denom,
    pub lp_token: Option<TokenSelect>,
    pub lp_token_code_id: u64,
    pub owner: Option<String>,
    pub protocol_fee_recipient: String,
    // NOTE: Fees percents are out of 100 e.g., 1 = 1%
    pub protocol_fee_percent: Decimal,
    pub lp_fee_percent: Decimal,
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
        input_token: TokenSelect,
        input_amounts: Vec<Uint128>,
        min_outputs: Vec<Uint128>,
        expiration: Option<Expiration>,
    },
    /// Chained swap converting A -> B and B -> C by leveraging two swap contracts
    PassThroughSwap {
        output_amm_address: String,
        input_token: TokenSelect,
        input_tokens_amount: Vec<Uint128>,
        output_min_tokens: Vec<Uint128>,
        expiration: Option<Expiration>,
    },
    SwapAndSendTo {
        input_token: TokenSelect,
        input_amounts: Vec<Uint128>,
        recipient: String,
        min_tokens: Vec<Uint128>,
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Implements CW20. Returns the current balance of the given address, 0 if unset.
    Balance {
        address: String,
    },
    Info {},
    Token1ForToken2Price {
        token1_amounts: Vec<Uint128>,
    },
    Token2ForToken1Price {
        token2_amounts: Vec<Uint128>,
    },
    Fee {},
    Token {
        token_id: String,
    },
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct MigrateMsg {
    pub owner: Option<String>,
    pub protocol_fee_recipient: String,
    pub protocol_fee_percent: Decimal,
    pub lp_fee_percent: Decimal,
    pub freeze_pool: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TokenResponse {
    name: String,
    description: String,
    decimals: String,
    image: String,
    index: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct A {
    pub path: String,
    pub data: Binary,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InfoResponse {
    pub token1_reserves: Vec<TokenInfo>,
    pub token1_denom: Denom,
    pub token2_reserves: Vec<TokenInfo>,
    pub token2_denom: Denom,
    pub lp_token_supplies: Vec<TokenInfo>,
    pub lp_token_address: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
pub struct FeeResponse {
    pub owner: Option<String>,
    pub lp_fee_percent: Decimal,
    pub protocol_fee_percent: Decimal,
    pub protocol_fee_recipient: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Token1ForToken2PriceResponse {
    pub token2_amounts: Vec<Uint128>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Token2ForToken1PriceResponse {
    pub token1_amounts: Vec<Uint128>,
}
