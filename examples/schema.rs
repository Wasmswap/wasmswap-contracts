use std::env::current_dir;
use std::fs::create_dir_all;

use cosmwasm_schema::{export_schema, remove_schemas, schema_for};

use cw20::BalanceResponse;
use wasmswap::msg::{
    ExecuteMsg, InfoResponse, InstantiateMsg, MigrateMsg, QueryMsg, Token1ForToken2PriceResponse,
    Token2ForToken1PriceResponse,
};
use wasmswap::state::Token;

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("../schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    export_schema(&schema_for!(InstantiateMsg), &out_dir);
    export_schema(&schema_for!(ExecuteMsg), &out_dir);
    export_schema(&schema_for!(QueryMsg), &out_dir);
    export_schema(&schema_for!(Token), &out_dir);
    export_schema(&schema_for!(BalanceResponse), &out_dir);
    export_schema(&schema_for!(InfoResponse), &out_dir);
    export_schema(&schema_for!(Token1ForToken2PriceResponse), &out_dir);
    export_schema(&schema_for!(Token2ForToken1PriceResponse), &out_dir);
    export_schema(&schema_for!(MigrateMsg), &out_dir);
}
