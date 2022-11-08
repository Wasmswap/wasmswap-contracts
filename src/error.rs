use cosmwasm_std::{Decimal, StdError, Uint128};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Cw20Error(#[from] cw20_base::ContractError),

    #[error("None Error")]
    NoneError {},

    #[error("Unauthorized")]
    Unauthorized {},
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
    #[error("Min liquidity error: requested: {min_liquidity}, available: {liquidity_available}")]
    MinLiquidityError {
        min_liquidity: Uint128,
        liquidity_available: Uint128,
    },

    #[error("Max token error: max_token: {max_token}, tokens_required: {tokens_required}")]
    MaxTokenError {
        max_token: Uint128,
        tokens_required: Uint128,
    },

    #[error("Insufficient liquidity error: requested: {requested}, available: {available}")]
    InsufficientLiquidityError {
        requested: Uint128,
        available: Uint128,
    },

    #[error("Min token1 error: requested: {requested}, available: {available}")]
    MinToken1Error {
        requested: Uint128,
        available: Uint128,
    },

    #[error("Min token2 error: requested: {requested}, available: {available}")]
    MinToken2Error {
        requested: Uint128,
        available: Uint128,
    },

    #[error("Incorrect native denom: provided: {provided}, required: {required}")]
    IncorrectNativeDenom { provided: String, required: String },

    #[error("Swap min error: min: {min}, available: {available}")]
    SwapMinError { min: Uint128, available: Uint128 },

    #[error("MsgExpirationError")]
    MsgExpirationError {},

    #[error("Total fee ({total_fee_percent}) percent is higher than max ({max_fee_percent})")]
    FeesTooHigh {
        max_fee_percent: Decimal,
        total_fee_percent: Decimal,
    },

    #[error("InsufficientFunds")]
    InsufficientFunds {},

    #[error("Uknown reply id: {id}")]
    UnknownReplyId { id: u64 },

    #[error("Failed to instantiate lp token")]
    InstantiateLpTokenError {},

    #[error("The output amm provided is invalid")]
    InvalidOutputPool {},
}
