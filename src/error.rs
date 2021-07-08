use cosmwasm_std::{StdError, Uint128};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Cw20Error(#[from] cw20_base::ContractError),

    #[error("Unauthorized")]
    Unauthorized {},
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
    #[error("MinLiquidityError")]
    MinLiquidityError {
        min_liquidity: Uint128,
        liquidity_available: Uint128,
    },

    #[error("MaxTokenError")]
    MaxTokenError {
        max_token: Uint128,
        tokens_required: Uint128,
    },

    #[error("InsufficientLiquidityError")]
    InsufficientLiquidityError {
        requested: Uint128,
        available: Uint128,
    },

    #[error("NoLiquidityError")]
    NoLiquidityError {},

    #[error("MinNativeError")]
    MinNative {
        requested: Uint128,
        available: Uint128,
    },

    #[error("MinTokenError")]
    MinToken {
        requested: Uint128,
        available: Uint128,
    },

    #[error("IncorrectNativeDenom")]
    IncorrectNativeDenom { provided: String, required: String },

    #[error("SwapMinError")]
    SwapMinError { min: Uint128, available: Uint128 },
}
