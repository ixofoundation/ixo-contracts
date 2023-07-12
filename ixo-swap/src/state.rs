use cw1155_lp::TokenInfo;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Decimal};
use cw_storage_plus::Item;

use crate::msg::{Denom, TokenSelect};

pub const LP_TOKEN_ADDRESS: Item<Addr> = Item::new("lp_token_address");
pub const LP_TOKEN: Item<Option<TokenSelect>> = Item::new("lp_token");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Token {
    pub reserves: Vec<TokenInfo>,
    pub denom: Denom,
}

pub const TOKEN1: Item<Token> = Item::new("token1");
pub const TOKEN2: Item<Token> = Item::new("token2");

pub const OWNER: Item<Option<Addr>> = Item::new("owner");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Fees {
    pub protocol_fee_recipient: Addr,
    pub protocol_fee_percent: Decimal,
    pub lp_fee_percent: Decimal,
}

pub const FEES: Item<Fees> = Item::new("fees");

pub const FROZEN: Item<bool> = Item::new("frozen");