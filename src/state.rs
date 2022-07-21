use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Decimal256, Uint128};
use cw20::Denom;
use cw_storage_plus::Item;

pub const LP_TOKEN: Item<Addr> = Item::new("lp_token");
pub const TWAP_PRECISION: Uint128 = Uint128::new(1_000_000_000u128);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Token {
    pub reserve: Uint128,
    pub denom: Denom,
}

pub const TOKEN1: Item<Token> = Item::new("token1");
pub const TOKEN2: Item<Token> = Item::new("token2");

pub const PRICE_LAST: Item<PriceCumulativeLast> = Item::new("twap_price_last");
/// ## Description
/// This structure stores the latest cumulative and average token prices for the target pool
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PriceCumulativeLast {
    /// The last cumulative price 0 asset in pool
    pub price0_cumulative_last: Uint128,
    /// The last cumulative price 1 asset in pool
    pub price1_cumulative_last: Uint128,
    /// The average price 0 asset in pool
    pub price_0_average: Decimal256,
    /// The average price 1 asset in pool
    pub price_1_average: Decimal256,
    /// The last timestamp block in pool
    pub block_timestamp_last: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolPricesResponse {
    /// The last cumulative price 0 asset in pool
    pub price0_current: Uint128,
    /// The last cumulative price 1 asset in pool
    pub price1_current: Uint128,
}
