use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Uint128};
use cw20::Denom;
use cw_storage_plus::Item;

pub const LP_TOKEN: Item<Addr> = Item::new("lp_token");
pub const TWAP_PRECISION: Uint128 = Uint128::new(1_000_000u128);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Token {
    pub reserve: Uint128,
    pub denom: Denom,
}

pub const TOKEN1: Item<Token> = Item::new("token1");
pub const TOKEN2: Item<Token> = Item::new("token2");

pub const PRICE_HISTORY: Item<Vec<PriceSnapShot>> = Item::new("twap_price_history");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PriceSnapShot {
    pub token1_price: Uint128,
    pub token2_price: Uint128,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolPricesResponse {
    /// The last cumulative price 0 asset in pool
    pub price1_current: Uint128,
    /// The last cumulative price 1 asset in pool
    pub price2_current: Uint128,
}
