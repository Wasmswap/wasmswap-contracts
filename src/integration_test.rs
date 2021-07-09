#![cfg(test)]

use cosmwasm_std::testing::{mock_env, MockApi, MockStorage};
use cosmwasm_std::{coins, from_binary, Addr, BalanceResponse, BankQuery, Coin, Empty, Uint128};
use cw20::{Cw20Coin, Cw20Contract, Cw20ExecuteMsg};
use cw_multi_test::{App, Contract, ContractWrapper, SimpleBank};

use crate::msg::{ExecuteMsg, InfoResponse, InstantiateMsg, QueryMsg};

fn mock_app() -> App {
    let env = mock_env();
    let api = Box::new(MockApi::default());
    let bank = SimpleBank {};

    App::new(api, env.block, bank, || Box::new(MockStorage::new()))
}

pub fn contract_amm() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    );
    Box::new(contract)
}

pub fn contract_cw20() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    );
    Box::new(contract)
}

fn get_info(router: &App, contract_addr: &Addr) -> InfoResponse {
    router
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::Info {})
        .unwrap()
}

#[test]
// receive cw20 tokens and release upon approval
fn amm_add_and_remove_liquidity() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "token";

    let owner = Addr::unchecked("owner");
    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.set_bank_balance(&owner, funds).unwrap();

    // set up cw20 contract with some tokens
    let cw20_id = router.store_code(contract_cw20());
    let msg = cw20_base::msg::InstantiateMsg {
        name: "Cash Money".to_string(),
        symbol: "CASH".to_string(),
        decimals: 2,
        initial_balances: vec![Cw20Coin {
            address: owner.to_string(),
            amount: Uint128(5000),
        }],
        mint: None,
    };
    let cash_addr = router
        .instantiate_contract(cw20_id, owner.clone(), &msg, &[], "CASH")
        .unwrap();

    // set up amm contract
    let amm_id = router.store_code(contract_amm());
    let msg = InstantiateMsg {
        native_denom: NATIVE_TOKEN_DENOM.to_string(),
        token_address: cash_addr.clone(),
    };
    let amm_addr = router
        .instantiate_contract(amm_id, owner.clone(), &msg, &[], "amm")
        .unwrap();

    assert_ne!(cash_addr, amm_addr);

    // set up cw20 helpers
    let cash = Cw20Contract(cash_addr.clone());
    let amm = Cw20Contract(amm_addr.clone());

    // check initial balances
    let owner_balance = cash.balance(&router, owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128(5000));

    // send tokens to contract address
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::from(100u128),
        expires: None,
    };
    let res = router
        .execute_contract(owner.clone(), cash_addr.clone(), &allowance_msg, &[])
        .unwrap();
    println!("{:?}", res.attributes);

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        min_liquidity: Uint128(100),
        max_token: Uint128(100),
    };
    let res = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128(100),
            }],
        )
        .unwrap();
    println!("{:?}", res.attributes);

    // ensure balances updated
    let owner_balance = cash.balance(&router, owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128(4900));
    let amm_balance = cash.balance(&router, amm_addr.clone()).unwrap();
    assert_eq!(amm_balance, Uint128(100));
    let crust_balance = amm.balance(&router, owner.clone()).unwrap();
    assert_eq!(crust_balance, Uint128(100));

    // send tokens to contract address
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::from(51u128),
        expires: None,
    };
    let res = router
        .execute_contract(owner.clone(), cash_addr.clone(), &allowance_msg, &[])
        .unwrap();
    println!("{:?}", res.attributes);
    assert_eq!(res.attributes.len(), 4);

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        min_liquidity: Uint128(50),
        max_token: Uint128(51),
    };
    let res = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128(50),
            }],
        )
        .unwrap();
    println!("{:?}", res.attributes);

    // ensure balances updated
    let owner_balance = cash.balance(&router, owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128(4849));
    let amm_balance = cash.balance(&router, amm_addr.clone()).unwrap();
    assert_eq!(amm_balance, Uint128(151));
    let crust_balance = amm.balance(&router, owner.clone()).unwrap();
    assert_eq!(crust_balance, Uint128(150));

    let remove_liquidity_msg = ExecuteMsg::RemoveLiquidity {
        amount: Uint128(50),
        min_native: Uint128(50),
        min_token: Uint128(50),
    };
    let res = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &remove_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128(50),
            }],
        )
        .unwrap();
    println!("{:?}", res.attributes);

    // ensure balances updated
    let owner_balance = cash.balance(&router, owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128(4899));
    let amm_balance = cash.balance(&router, amm_addr.clone()).unwrap();
    assert_eq!(amm_balance, Uint128(101));
    let crust_balance = amm.balance(&router, owner.clone()).unwrap();
    assert_eq!(crust_balance, Uint128(100));
}

#[test]
fn swap_tokens_happy_path() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "token";

    let owner = Addr::unchecked("owner");
    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.set_bank_balance(&owner, funds).unwrap();

    // set up cw20 contract with some tokens
    let cw20_id = router.store_code(contract_cw20());
    let msg = cw20_base::msg::InstantiateMsg {
        name: "Cash Money".to_string(),
        symbol: "CASH".to_string(),
        decimals: 2,
        initial_balances: vec![Cw20Coin {
            address: owner.to_string(),
            amount: Uint128(5000),
        }],
        mint: None,
    };
    let cash_addr = router
        .instantiate_contract(cw20_id, owner.clone(), &msg, &[], "CASH")
        .unwrap();

    // set up amm contract
    let amm_id = router.store_code(contract_amm());
    let msg = InstantiateMsg {
        native_denom: NATIVE_TOKEN_DENOM.to_string(),
        token_address: cash_addr.clone(),
    };
    let amm_addr = router
        .instantiate_contract(amm_id, owner.clone(), &msg, &[], "amm")
        .unwrap();

    assert_ne!(cash_addr, amm_addr);

    // set up cw20 helper
    let cash = Cw20Contract(cash_addr.clone());

    // check initial balances
    let owner_balance = cash.balance(&router, owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128(5000));

    // send tokens to contract address
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::from(100u128),
        expires: None,
    };
    let res = router
        .execute_contract(owner.clone(), cash_addr.clone(), &allowance_msg, &[])
        .unwrap();
    println!("{:?}", res.attributes);

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        min_liquidity: Uint128(100),
        max_token: Uint128(100),
    };
    let res = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128(100),
            }],
        )
        .unwrap();
    println!("{:?}", res.attributes);

    let info = get_info(&router, &amm_addr);
    assert_eq!(info.native_reserve, Uint128(100));
    assert_eq!(info.token_reserve, Uint128(100));

    let buyer = Addr::unchecked("buyer");
    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.set_bank_balance(&buyer, funds).unwrap();

    let add_liquidity_msg = ExecuteMsg::SwapNativeForToken {
        min_token: Uint128(9),
    };
    let res = router
        .execute_contract(
            buyer.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128(10),
            }],
        )
        .unwrap();
    println!("{:?}", res.attributes);

    let info = get_info(&router, &amm_addr);
    assert_eq!(info.native_reserve, Uint128(110));
    assert_eq!(info.token_reserve, Uint128(91));

    // ensure balances updated
    let buyer_balance = cash.balance(&router, buyer.clone()).unwrap();
    assert_eq!(buyer_balance, Uint128(9));

    // Check balances of owner and buyer reflect the sale transaction
    let query_res = router
        .query(
            cosmwasm_std::QueryRequest::Bank(BankQuery::Balance {
                address: buyer.to_string(),
                denom: NATIVE_TOKEN_DENOM.to_string(),
            })
            .into(),
        )
        .unwrap();
    let balance: BalanceResponse = from_binary(&query_res).unwrap();
    assert_eq!(balance.amount.amount, Uint128(1990));

    let swap_msg = ExecuteMsg::SwapNativeForToken {
        min_token: Uint128(7),
    };
    let res = router
        .execute_contract(
            buyer.clone(),
            amm_addr.clone(),
            &swap_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128(10),
            }],
        )
        .unwrap();
    println!("{:?}", res.attributes);

    let info = get_info(&router, &amm_addr);
    assert_eq!(info.native_reserve, Uint128(120));
    assert_eq!(info.token_reserve, Uint128(84));

    // ensure balances updated
    let buyer_balance = cash.balance(&router, buyer.clone()).unwrap();
    assert_eq!(buyer_balance, Uint128(16));

    // Check balances of owner and buyer reflect the sale transaction
    let query_res = router
        .query(
            cosmwasm_std::QueryRequest::Bank(BankQuery::Balance {
                address: buyer.to_string(),
                denom: NATIVE_TOKEN_DENOM.to_string(),
            })
            .into(),
        )
        .unwrap();
    let balance: BalanceResponse = from_binary(&query_res).unwrap();
    assert_eq!(balance.amount.amount, Uint128(1980));

    // Swap token for native

    // send tokens to contract address
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128(16),
        expires: None,
    };
    let res = router
        .execute_contract(buyer.clone(), cash_addr.clone(), &allowance_msg, &[])
        .unwrap();
    println!("{:?}", res.attributes);

    let swap_msg = ExecuteMsg::SwapTokenForNative {
        token_amount: Uint128(16),
        min_native: Uint128(19),
    };
    let res = router
        .execute_contract(buyer.clone(), amm_addr.clone(), &swap_msg, &vec![])
        .unwrap();
    println!("{:?}", res.attributes);

    let info = get_info(&router, &amm_addr);
    assert_eq!(info.native_reserve, Uint128(101));
    assert_eq!(info.token_reserve, Uint128(100));

    // ensure balances updated
    let buyer_balance = cash.balance(&router, buyer.clone()).unwrap();
    assert_eq!(buyer_balance, Uint128(0));

    // Check balances of owner and buyer reflect the sale transaction
    let query_res = router
        .query(
            cosmwasm_std::QueryRequest::Bank(BankQuery::Balance {
                address: buyer.to_string(),
                denom: NATIVE_TOKEN_DENOM.to_string(),
            })
            .into(),
        )
        .unwrap();
    let balance: BalanceResponse = from_binary(&query_res).unwrap();
    assert_eq!(balance.amount.amount, Uint128(1999));
}
