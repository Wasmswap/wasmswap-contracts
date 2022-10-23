use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Decimal, Uint128};
use cw_utils::Expiration;

#[cw_serde]
pub struct InstantiateMsg {
    pub token1_denom: String,
    pub token2_denom: String,
    pub token_factory: String,
    pub owner: Option<String>,
    pub protocol_fee_recipient: String,
    // NOTE: Fees percents are out of 100 e.g., 1 = 1%
    pub protocol_fee_percent: Decimal,
    pub lp_fee_percent: Decimal,
}

#[cw_serde]
pub enum TokenSelect {
    Token1,
    Token2,
}

#[cw_serde]
pub enum ExecuteMsg {
    AddLiquidity {
        token1_amount: Uint128,
        min_liquidity: Uint128,
        max_token2: Uint128,
        expiration: Option<Expiration>,
    },
    RemoveLiquidity {
        amount: Uint128,
        min_token1: Uint128,
        min_token2: Uint128,
        expiration: Option<Expiration>,
    },
    Swap {
        input_token: TokenSelect,
        input_amount: Uint128,
        min_output: Uint128,
        expiration: Option<Expiration>,
    },
    /// Chained swap converting A -> B and B -> C by leveraging two swap contracts
    PassThroughSwap {
        output_amm_address: String,
        input_token: TokenSelect,
        input_token_amount: Uint128,
        output_min_token: Uint128,
        expiration: Option<Expiration>,
    },
    SwapAndSendTo {
        input_token: TokenSelect,
        input_amount: Uint128,
        recipient: String,
        min_token: Uint128,
        expiration: Option<Expiration>,
    },
    UpdateConfig {
        owner: Option<String>,
        lp_fee_percent: Decimal,
        protocol_fee_percent: Decimal,
        protocol_fee_recipient: String,
    },
}

#[cw_serde]
pub enum QueryMsg {
    Info {},
    Token1ForToken2Price { token1_amount: Uint128 },
    Token2ForToken1Price { token2_amount: Uint128 },
}

#[cw_serde]
pub struct MigrateMsg {
    pub owner: Option<String>,
    pub protocol_fee_recipient: String,
    pub protocol_fee_percent: Decimal,
    pub lp_fee_percent: Decimal,
}

#[cw_serde]
pub struct InfoResponse {
    pub token1_reserve: Uint128,
    pub token1_denom: String,
    pub token2_reserve: Uint128,
    pub token2_denom: String,
    pub lp_token_supply: Uint128,
    pub lp_token_denom: String,
    pub owner: Option<String>,
    pub lp_fee_percent: Decimal,
    pub protocol_fee_percent: Decimal,
    pub protocol_fee_recipient: String,
}

#[cw_serde]
pub struct Token1ForToken2PriceResponse {
    pub token2_amount: Uint128,
}

#[cw_serde]
pub struct Token2ForToken1PriceResponse {
    pub token1_amount: Uint128,
}
