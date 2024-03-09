use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, Uint128};
use cw1155::TokenId;
use cw_storage_plus::{Item, Map};

use crate::msg::Denom;

pub const LP_ADDRESS: Item<Addr> = Item::new("lp_token");

#[cw_serde]
pub struct Token {
    pub reserve: Uint128,
    pub denom: Denom,
}

pub const TOKEN1155: Item<Token> = Item::new("token1155");
pub const TOKEN2: Item<Token> = Item::new("token2");
pub const TOKEN_SUPPLIES: Map<TokenId, Uint128> = Map::new("lp_supplies");

pub const OWNER: Item<Addr> = Item::new("owner");
pub const PENDING_OWNER: Item<Option<Addr>> = Item::new("pending-owner");

pub const MAX_SLIPPAGE_PERCENT: Item<Decimal> = Item::new("max-slippage-percent");

#[cw_serde]
pub struct Fees {
    pub protocol_fee_recipient: Addr,
    pub protocol_fee_percent: Decimal,
    pub lp_fee_percent: Decimal,
}

pub const FEES: Item<Fees> = Item::new("fees");

pub const FROZEN: Item<bool> = Item::new("frozen");
