use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::Item;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub native_reserve: Uint128,
    pub native_denom: String,

    pub token_address: Addr,
    pub token_reserve: Uint128,
}

pub const STATE: Item<State> = Item::new("state");
