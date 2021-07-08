use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Uint128};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub native_denom: String,
    pub token_address: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    AddLiquidity {
        min_liquidity: Uint128,
        max_token: Uint128,
    },
    RemoveLiquidity {
        amount: Uint128,
        min_native: Uint128,
        min_token: Uint128,
    },
    NativeForTokenSwapInput {
        min_token: Uint128,
    },
    TokenForNativeSwapInput {
        token_amount: Uint128,
        min_native: Uint128,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Implements CW20. Returns the current balance of the given address, 0 if unset.
    Balance {
        address: String,
    },
    Info {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InfoResponse {
    pub native_supply: Uint128,
    pub native_denom: String,
    pub token_supply: Uint128,
}
