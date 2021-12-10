use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::Addr;
use cw20::Denom;
use cw_storage_plus::{Item, Map};

pub const SWAP_CODE_ID: Item<u64> = Item::new("swap_code_id");
pub const LP_TOKEN_CODE_ID: Item<u64> = Item::new("lp_token_code_id");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Swap {
    pub token1: Denom,
    pub token2: Denom,
}

pub const SWAPS: Map<String, Swap> = Map::new("swaps");

pub fn getDenomPrimaryKey(denom: &Denom) -> String {
    match denom {
        Denom::Native(denom) => format!("{}_{}","native",denom),
        Denom::Cw20(addr) => format!("{}_{}","cw20",addr.to_string()),
    }
}
