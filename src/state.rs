use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Uint128};
use cw20::Denom;
use cw_storage_plus::Item;

pub const LP_TOKEN: Item<Addr> = Item::new("lp_token");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Token {
    pub reserve: Uint128,
    pub denom: Denom,
}

pub const TOKEN1: Item<Token> = Item::new("token1");
pub const TOKEN2: Item<Token> = Item::new("token2");

pub const OWNER: Item<Option<Addr>> = Item::new("owner");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Fees {
    pub protocol_fee_recipient: Addr,
    pub protocol_fee_percent: Uint128,
    pub lp_fee_percent: Uint128,
}

pub const FEES: Item<Fees> = Item::new("fees");
