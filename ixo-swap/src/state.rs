use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, Uint128};
use cw1155::TokenId;
use cw_storage_plus::{Item, Map};

use crate::msg::Denom;

pub const LP_ADDRESS: Item<Addr> = Item::new("lp_token");
pub const LP_TOKENS: Map<(Addr, TokenId), Uint128> = Map::new("lp_tokens");

#[cw_serde]
pub struct Token {
    pub reserve: Uint128,
    pub denom: Denom,
}

pub const TOKEN1: Item<Token> = Item::new("token1");
pub const TOKEN2: Item<Token> = Item::new("token2");

pub const OWNER: Item<Option<Addr>> = Item::new("owner");

#[cw_serde]
pub struct Fees {
    pub protocol_fee_recipient: Addr,
    pub protocol_fee_percent: Decimal,
    pub lp_fee_percent: Decimal,
}

pub const FEES: Item<Fees> = Item::new("fees");

pub const FROZEN: Item<bool> = Item::new("frozen");
