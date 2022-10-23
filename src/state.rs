use cosmwasm_schema::cw_serde;

use cosmwasm_std::{Addr, Decimal, Uint128};
use cw_storage_plus::Item;

#[cw_serde]
pub struct Token {
    pub reserve: Uint128,
    pub denom: String,
}

pub const TOKEN_FACTORY: Item<Addr> = Item::new("token_factory");
pub const LP_DENOM: Item<String> = Item::new("lp_denom");

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
