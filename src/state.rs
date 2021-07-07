use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Coin, Uint128};
use cw_storage_plus::Item;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub native_supply: Coin,

    pub token_address: Addr,
    pub token_supply: Uint128,
}

pub const STATE: Item<State> = Item::new("state");
