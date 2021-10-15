use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::Item;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub token1_address: Option<Addr>,
    pub token1_reserve: Uint128,
    pub token1_denom: String,

    pub token2_address: Option<Addr>,
    pub token2_denom: String,
    pub token2_reserve: Uint128,
}

pub const STATE: Item<State> = Item::new("state");
