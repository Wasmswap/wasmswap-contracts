use std::ops::Add;
use cosmwasm_std::Addr;
use cw0::Duration;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cw20::Denom;
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub swap_code_id: u64,
    pub lp_token_code_id: u64,
    pub unstaking_duration: Option<Duration>,
}

pub const CONFIG: Item<Config> = Item::new("config");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Swap {
    pub address: Addr,
    pub token1: Denom,
    pub token2: Denom,
}

pub const SWAPS: Map<String, Swap> = Map::new("swaps");

pub fn get_denom_primary_key(denom: &Denom) -> String {
    match denom {
        Denom::Native(denom) => format!("{}_{}", "native", denom),
        Denom::Cw20(addr) => format!("{}_{}", "cw20", addr.to_string()),
    }
}
