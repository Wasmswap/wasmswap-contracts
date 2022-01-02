#![cfg(test)]

use std::borrow::BorrowMut;

use cosmwasm_std::{coins, Addr, Coin, Empty, Uint128};
use cw0::Expiration;

use crate::error::ContractError;
use cw20::{Cw20Coin, Cw20Contract, Cw20ExecuteMsg, Denom};
use cw_multi_test::{App, Contract, ContractWrapper, Executor};

use crate::msg::{ExecuteMsg, InfoResponse, InstantiateMsg, QueryMsg, TokenSelect};

fn mock_app() -> App {
    App::default()
}

pub fn contract_amm() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    )
    .with_reply(crate::contract::reply);
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

fn create_amm(router: &mut App, owner: &Addr, cash: &Cw20Contract, native_denom: String) -> Addr {
    // set up amm contract
    let cw20_id = router.store_code(contract_cw20());
    let amm_id = router.store_code(contract_amm());
    let msg = InstantiateMsg {
        token1_denom: Denom::Native(native_denom),
        token2_denom: Denom::Cw20(cash.addr()),
        lp_token_code_id: cw20_id,
    };
    router
        .instantiate_contract(amm_id, owner.clone(), &msg, &[], "amm", None)
        .unwrap()
}

// CreateCW20 create new cw20 with given initial balance belonging to owner
fn create_cw20(
    router: &mut App,
    owner: &Addr,
    name: String,
    symbol: String,
    balance: Uint128,
) -> Cw20Contract {
    // set up cw20 contract with some tokens
    let cw20_id = router.store_code(contract_cw20());
    let msg = cw20_base::msg::InstantiateMsg {
        name,
        symbol,
        decimals: 2,
        initial_balances: vec![Cw20Coin {
            address: owner.to_string(),
            amount: balance,
        }],
        mint: None,
        marketing: None,
    };
    let addr = router
        .instantiate_contract(cw20_id, owner.clone(), &msg, &[], "CASH", None)
        .unwrap();
    Cw20Contract(addr)
}

fn bank_balance(router: &mut App, addr: &Addr, denom: String) -> Coin {
    router
        .wrap()
        .query_balance(addr.to_string(), denom)
        .unwrap()
}

#[test]
// receive cw20 tokens and release upon approval
fn test_instantiate() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";

    let owner = Addr::unchecked("owner");
    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let cw20_token = create_cw20(
        &mut router,
        &owner,
        "token".to_string(),
        "CWTOKEN".to_string(),
        Uint128::new(5000),
    );

    let amm_addr = create_amm(&mut router, &owner, &cw20_token, NATIVE_TOKEN_DENOM.into());

    assert_ne!(cw20_token.addr(), amm_addr);

    let info = get_info(&router, &amm_addr);
    assert_eq!(info.lp_token_address, "Contract #2".to_string())
}

#[test]
// receive cw20 tokens and release upon approval
fn amm_add_and_remove_liquidity() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";

    let owner = Addr::unchecked("owner");
    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let cw20_token = create_cw20(
        &mut router,
        &owner,
        "token".to_string(),
        "CWTOKEN".to_string(),
        Uint128::new(5000),
    );

    let amm_addr = create_amm(&mut router, &owner, &cw20_token, NATIVE_TOKEN_DENOM.into());

    assert_ne!(cw20_token.addr(), amm_addr);

    let info = get_info(&router, &amm_addr);
    // set up cw20 helpers
    let lp_token = Cw20Contract(Addr::unchecked(info.lp_token_address));

    // check initial balances
    let owner_balance = cw20_token.balance(&router, owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::new(5000));

    // send tokens to contract address
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::new(100u128),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw20_token.addr(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        token1_amount: Uint128::new(100),
        min_liquidity: Uint128::new(100),
        max_token2: Uint128::new(100),
        expiration: None,
    };
    let _res = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(100),
            }],
        )
        .unwrap();

    // ensure balances updated
    let owner_balance = cw20_token.balance(&router, owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::new(4900));
    let amm_balance = cw20_token.balance(&router, amm_addr.clone()).unwrap();
    assert_eq!(amm_balance, Uint128::new(100));
    let crust_balance = lp_token.balance(&router, owner.clone()).unwrap();
    assert_eq!(crust_balance, Uint128::new(100));

    // send tokens to contract address
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::new(51u128),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw20_token.addr(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        token1_amount: Uint128::new(50),
        min_liquidity: Uint128::new(50),
        max_token2: Uint128::new(51),
        expiration: None,
    };
    let _res = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(50),
            }],
        )
        .unwrap();

    // ensure balances updated
    let owner_balance = cw20_token.balance(&router, owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::new(4849));
    let amm_balance = cw20_token.balance(&router, amm_addr.clone()).unwrap();
    assert_eq!(amm_balance, Uint128::new(151));
    let crust_balance = lp_token.balance(&router, owner.clone()).unwrap();
    assert_eq!(crust_balance, Uint128::new(150));

    // too low max token error
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::new(51u128),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw20_token.addr(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        token1_amount: Uint128::new(50),
        min_liquidity: Uint128::new(50),
        max_token2: Uint128::new(45),
        expiration: None,
    };
    let err = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(50),
            }],
        )
        .unwrap_err();

    assert_eq!(
        ContractError::MaxTokenError {
            max_token: Uint128::new(45),
            tokens_required: Uint128::new(51)
        },
        err.downcast().unwrap()
    );

    // too high min liquidity
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::new(51u128),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw20_token.addr(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        token1_amount: Uint128::new(50),
        min_liquidity: Uint128::new(500),
        max_token2: Uint128::new(50),
        expiration: None,
    };
    let err = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(50),
            }],
        )
        .unwrap_err();

    assert_eq!(
        ContractError::MinLiquidityError {
            min_liquidity: Uint128::new(500),
            liquidity_available: Uint128::new(50)
        },
        err.downcast().unwrap()
    );

    // Expired message
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::new(51u128),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw20_token.addr(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        token1_amount: Uint128::new(50),
        min_liquidity: Uint128::new(50),
        max_token2: Uint128::new(50),
        expiration: Some(Expiration::AtHeight(0)),
    };
    let err = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(50),
            }],
        )
        .unwrap_err();

    assert_eq!(
        ContractError::MsgExpirationError {},
        err.downcast().unwrap()
    );

    // Remove more liquidity then owned
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::new(50u128),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), lp_token.addr(), &allowance_msg, &[])
        .unwrap();

    let remove_liquidity_msg = ExecuteMsg::RemoveLiquidity {
        amount: Uint128::new(151),
        min_token1: Uint128::new(0),
        min_token2: Uint128::new(0),
        expiration: None,
    };
    let err = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &remove_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(50),
            }],
        )
        .unwrap_err();

    assert_eq!(
        ContractError::InsufficientLiquidityError {
            requested: Uint128::new(151),
            available: Uint128::new(150)
        },
        err.downcast().unwrap()
    );

    // Remove some liquidity
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::new(50u128),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), lp_token.addr(), &allowance_msg, &[])
        .unwrap();

    let remove_liquidity_msg = ExecuteMsg::RemoveLiquidity {
        amount: Uint128::new(50),
        min_token1: Uint128::new(50),
        min_token2: Uint128::new(50),
        expiration: None,
    };
    let _res = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &remove_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(50),
            }],
        )
        .unwrap();

    // ensure balances updated
    let owner_balance = cw20_token.balance(&router, owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::new(4899));
    let amm_balance = cw20_token.balance(&router, amm_addr.clone()).unwrap();
    assert_eq!(amm_balance, Uint128::new(101));
    let crust_balance = lp_token.balance(&router, owner.clone()).unwrap();
    assert_eq!(crust_balance, Uint128::new(100));

    // Remove rest of liquidity
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::new(100u128),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), lp_token.addr(), &allowance_msg, &[])
        .unwrap();

    let remove_liquidity_msg = ExecuteMsg::RemoveLiquidity {
        amount: Uint128::new(100),
        min_token1: Uint128::new(100),
        min_token2: Uint128::new(100),
        expiration: None,
    };
    let _res = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &remove_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(50),
            }],
        )
        .unwrap();

    // ensure balances updated
    let owner_balance = cw20_token.balance(&router, owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::new(5000));
    let amm_balance = cw20_token.balance(&router, amm_addr).unwrap();
    assert_eq!(amm_balance, Uint128::new(0));
    let crust_balance = lp_token.balance(&router, owner.clone()).unwrap();
    assert_eq!(crust_balance, Uint128::new(0));
}

#[test]
fn swap_tokens_happy_path() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";

    let owner = Addr::unchecked("owner");
    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let cw20_token = create_cw20(
        &mut router,
        &owner,
        "token".to_string(),
        "CWTOKEN".to_string(),
        Uint128::new(5000),
    );

    let amm_addr = create_amm(
        &mut router,
        &owner,
        &cw20_token,
        NATIVE_TOKEN_DENOM.to_string(),
    );

    assert_ne!(cw20_token.addr(), amm_addr);

    // check initial balances
    let owner_balance = cw20_token.balance(&router, owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::new(5000));

    // send tokens to contract address
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::new(100u128),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw20_token.addr(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        token1_amount: Uint128::new(100),
        min_liquidity: Uint128::new(100),
        max_token2: Uint128::new(100),
        expiration: None,
    };
    let _res = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(100),
            }],
        )
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(info.token1_reserve, Uint128::new(100));
    assert_eq!(info.token2_reserve, Uint128::new(100));

    let buyer = Addr::unchecked("buyer");
    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &buyer, funds).unwrap()
    });

    let add_liquidity_msg = ExecuteMsg::Swap {
        input_token: TokenSelect::Token1,
        input_amount: Uint128::new(10),
        min_output: Uint128::new(9),
        expiration: None,
    };
    let _res = router
        .execute_contract(
            buyer.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(10),
            }],
        )
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(info.token1_reserve, Uint128::new(110));
    assert_eq!(info.token2_reserve, Uint128::new(91));

    // ensure balances updated
    let buyer_balance = cw20_token.balance(&router, buyer.clone()).unwrap();
    assert_eq!(buyer_balance, Uint128::new(9));

    // Check balances of owner and buyer reflect the sale transaction
    let balance: Coin = bank_balance(&mut router, &buyer, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(balance.amount, Uint128::new(1990));

    let swap_msg = ExecuteMsg::Swap {
        input_token: TokenSelect::Token1,
        input_amount: Uint128::new(10),
        min_output: Uint128::new(7),
        expiration: None,
    };
    let _res = router
        .execute_contract(
            buyer.clone(),
            amm_addr.clone(),
            &swap_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(10),
            }],
        )
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(info.token1_reserve, Uint128::new(120));
    assert_eq!(info.token2_reserve, Uint128::new(84));

    // ensure balances updated
    let buyer_balance = cw20_token.balance(&router, buyer.clone()).unwrap();
    assert_eq!(buyer_balance, Uint128::new(16));

    // Check balances of owner and buyer reflect the sale transaction
    let balance: Coin = bank_balance(&mut router, &buyer, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(balance.amount, Uint128::new(1980));

    // Swap token for native

    // send tokens to contract address
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::new(16),
        expires: None,
    };
    let _res = router
        .execute_contract(buyer.clone(), cw20_token.addr(), &allowance_msg, &[])
        .unwrap();

    let swap_msg = ExecuteMsg::Swap {
        input_token: TokenSelect::Token2,
        input_amount: Uint128::new(16),
        min_output: Uint128::new(19),
        expiration: None,
    };
    let _res = router
        .execute_contract(buyer.clone(), amm_addr.clone(), &swap_msg, &[])
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(info.token1_reserve, Uint128::new(101));
    assert_eq!(info.token2_reserve, Uint128::new(100));

    // ensure balances updated
    let buyer_balance = cw20_token.balance(&router, buyer.clone()).unwrap();
    assert_eq!(buyer_balance, Uint128::new(0));

    // Check balances of owner and buyer reflect the sale transaction
    let balance: Coin = bank_balance(&mut router, &buyer, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(balance.amount, Uint128::new(1999));

    // check owner balance
    let owner_balance = cw20_token.balance(&router, owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::new(4900));

    let swap_msg = ExecuteMsg::SwapAndSendTo {
        input_token: TokenSelect::Token1,
        input_amount: Uint128::new(10),
        recipient: owner.clone(),
        min_token: Uint128::new(3),
        expiration: None,
    };
    let _res = router
        .execute_contract(
            buyer.clone(),
            amm_addr.clone(),
            &swap_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(10),
            }],
        )
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(info.token1_reserve, Uint128::new(111));
    assert_eq!(info.token2_reserve, Uint128::new(92));

    // ensure balances updated
    let owner_balance = cw20_token.balance(&router, owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::new(4908));

    // Check balances of owner and buyer reflect the sale transaction
    let balance = bank_balance(&mut router, &buyer, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(balance.amount, Uint128::new(1989));
}

#[test]
fn swap_native_to_native_tokens_happy_path() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";
    const IBC_TOKEN_DENOM: &str = "atom";

    let owner = Addr::unchecked("owner");
    let funds = vec![
        Coin {
            denom: NATIVE_TOKEN_DENOM.into(),
            amount: Uint128::new(2000),
        },
        Coin {
            denom: IBC_TOKEN_DENOM.into(),
            amount: Uint128::new(5000),
        },
    ];
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let amm_id = router.store_code(contract_amm());
    let lp_token_id = router.store_code(contract_cw20());
    let msg = InstantiateMsg {
        token1_denom: Denom::Native(NATIVE_TOKEN_DENOM.into()),
        token2_denom: Denom::Native(IBC_TOKEN_DENOM.into()),
        lp_token_code_id: lp_token_id,
    };
    let amm_addr = router
        .instantiate_contract(amm_id, owner.clone(), &msg, &[], "amm", None)
        .unwrap();

    // send tokens to contract address
    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        token1_amount: Uint128::new(100),
        min_liquidity: Uint128::new(100),
        max_token2: Uint128::new(100),
        expiration: None,
    };
    let _res = router
        .execute_contract(
            owner,
            amm_addr.clone(),
            &add_liquidity_msg,
            &[
                Coin {
                    denom: NATIVE_TOKEN_DENOM.into(),
                    amount: Uint128::new(100),
                },
                Coin {
                    denom: IBC_TOKEN_DENOM.into(),
                    amount: Uint128::new(100),
                },
            ],
        )
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(info.token1_reserve, Uint128::new(100));
    assert_eq!(info.token2_reserve, Uint128::new(100));

    let buyer = Addr::unchecked("buyer");
    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &buyer, funds).unwrap()
    });

    let add_liquidity_msg = ExecuteMsg::Swap {
        input_token: TokenSelect::Token1,
        input_amount: Uint128::new(10),
        min_output: Uint128::new(9),
        expiration: None,
    };
    let _res = router
        .execute_contract(
            buyer.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(10),
            }],
        )
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(info.token1_reserve, Uint128::new(110));
    assert_eq!(info.token2_reserve, Uint128::new(91));

    // Check balances of owner and buyer reflect the sale transaction
    let native_balance: Coin = bank_balance(&mut router, &buyer, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(native_balance.amount, Uint128::new(1990));
    let ibc_balance: Coin = bank_balance(&mut router, &buyer, IBC_TOKEN_DENOM.to_string());
    assert_eq!(ibc_balance.amount, Uint128::new(9));

    let swap_msg = ExecuteMsg::Swap {
        input_token: TokenSelect::Token1,
        input_amount: Uint128::new(10),
        min_output: Uint128::new(7),
        expiration: None,
    };
    let _res = router
        .execute_contract(
            buyer.clone(),
            amm_addr.clone(),
            &swap_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(10),
            }],
        )
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(info.token1_reserve, Uint128::new(120));
    assert_eq!(info.token2_reserve, Uint128::new(84));

    // Check balances of owner and buyer reflect the sale transaction
    let native_balance: Coin = bank_balance(&mut router, &buyer, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(native_balance.amount, Uint128::new(1980));
    let ibc_balance: Coin = bank_balance(&mut router, &buyer, IBC_TOKEN_DENOM.to_string());
    assert_eq!(ibc_balance.amount, Uint128::new(16));

    // Swap token for native
    let swap_msg = ExecuteMsg::Swap {
        input_token: TokenSelect::Token2,
        input_amount: Uint128::new(16),
        min_output: Uint128::new(19),
        expiration: None,
    };
    let _res = router
        .execute_contract(
            buyer.clone(),
            amm_addr.clone(),
            &swap_msg,
            &[Coin {
                denom: IBC_TOKEN_DENOM.into(),
                amount: Uint128::new(16),
            }],
        )
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(info.token1_reserve, Uint128::new(101));
    assert_eq!(info.token2_reserve, Uint128::new(100));

    // Check balances of owner and buyer reflect the sale transaction
    let native_balance: Coin = bank_balance(&mut router, &buyer, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(native_balance.amount, Uint128::new(1999));
    let ibc_balance: Coin = bank_balance(&mut router, &buyer, IBC_TOKEN_DENOM.to_string());
    assert_eq!(ibc_balance.amount, Uint128::new(0));

    // TODO: implement
    /*
    // check owner balance
    let owner_balance = bank_balance(&mut router, &owner, IBC_TOKEN_DENOM.to_string());
    assert_eq!(owner_balance, Uint128::new(4900));

    let swap_msg = ExecuteMsg::SwapNativeForTokenTo {
        recipient: owner.clone(),
        min_token: Uint128::new(3),
        expiration: None,
    };
    let res = router
        .execute_contract(
            buyer.clone(),
            amm_addr.clone(),
            &swap_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(10),
            }],
        )
        .unwrap();
    println!("{:?}", res.attributes);

    let info = get_info(&router, &amm_addr);
    assert_eq!(info.native_reserve, Uint128::new(111));
    assert_eq!(info.token_reserve, Uint128::new(92));

    // ensure balances updated
    let owner_balance = cw20_token.balance(&router, owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::new(4908));

    // Check balances of owner and buyer reflect the sale transaction
    let balance = bank_balance(&mut router, &buyer, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(balance.amount.amount, Uint128::new(1989));

     */
}

#[test]
fn token_to_token_swap() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";

    let owner = Addr::unchecked("owner");
    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let token1 = create_cw20(
        &mut router,
        &owner,
        "token1".to_string(),
        "TOKENONE".to_string(),
        Uint128::new(5000),
    );
    let token2 = create_cw20(
        &mut router,
        &owner,
        "token2".to_string(),
        "TOKENTWO".to_string(),
        Uint128::new(5000),
    );

    let amm1 = create_amm(&mut router, &owner, &token1, NATIVE_TOKEN_DENOM.to_string());
    let amm2 = create_amm(&mut router, &owner, &token2, NATIVE_TOKEN_DENOM.to_string());

    // Add initial liquidity to both pools
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm1.to_string(),
        amount: Uint128::new(100),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), token1.addr(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        token1_amount: Uint128::new(100),
        min_liquidity: Uint128::new(100),
        max_token2: Uint128::new(100),
        expiration: None,
    };
    router
        .execute_contract(
            owner.clone(),
            amm1.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(100),
            }],
        )
        .unwrap();

    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm2.to_string(),
        amount: Uint128::new(100),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), token2.addr(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        token1_amount: Uint128::new(100),
        min_liquidity: Uint128::new(100),
        max_token2: Uint128::new(100),
        expiration: None,
    };
    router
        .execute_contract(
            owner.clone(),
            amm2.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(100),
            }],
        )
        .unwrap();

    // Swap token1 for token2
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm1.to_string(),
        amount: Uint128::new(10),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), token1.addr(), &allowance_msg, &[])
        .unwrap();

    let swap_msg = ExecuteMsg::PassThroughSwap {
        output_amm_address: amm2.clone(),
        input_token: TokenSelect::Token2,
        input_token_amount: Uint128::new(10),
        output_min_token: Uint128::new(8),
        expiration: None,
    };
    let _res = router
        .execute_contract(owner.clone(), amm1.clone(), &swap_msg, &[])
        .unwrap();

    // ensure balances updated
    let token1_balance = token1.balance(&router, owner.clone()).unwrap();
    assert_eq!(token1_balance, Uint128::new(4890));

    let token2_balance = token2.balance(&router, owner.clone()).unwrap();
    assert_eq!(token2_balance, Uint128::new(4908));

    let amm1_native_balance = bank_balance(&mut router, &amm1, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(amm1_native_balance.amount, Uint128::new(91));

    let amm2_native_balance = bank_balance(&mut router, &amm2, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(amm2_native_balance.amount, Uint128::new(109));

    // Swap token2 for token1
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm2.to_string(),
        amount: Uint128::new(10),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), token2.addr(), &allowance_msg, &[])
        .unwrap();

    let swap_msg = ExecuteMsg::PassThroughSwap {
        output_amm_address: amm1.clone(),
        input_token: TokenSelect::Token2,
        input_token_amount: Uint128::new(10),
        output_min_token: Uint128::new(1),
        expiration: None,
    };
    let _res = router
        .execute_contract(owner.clone(), amm2.clone(), &swap_msg, &[])
        .unwrap();

    // ensure balances updated
    let token1_balance = token1.balance(&router, owner.clone()).unwrap();
    assert_eq!(token1_balance, Uint128::new(4900));

    let token2_balance = token2.balance(&router, owner.clone()).unwrap();
    assert_eq!(token2_balance, Uint128::new(4898));

    let amm1_native_balance = bank_balance(&mut router, &amm1, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(amm1_native_balance.amount, Uint128::new(101));

    let amm2_native_balance = bank_balance(&mut router, &amm2, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(amm2_native_balance.amount, Uint128::new(99));

    // assert internal state is consistent
    let info_amm1 = get_info(&router, &amm1);
    let token1_balance = token1.balance(&router, amm1.clone()).unwrap();
    assert_eq!(info_amm1.token2_reserve, token1_balance);
    assert_eq!(info_amm1.token1_reserve, amm1_native_balance.amount);

    let info_amm2 = get_info(&router, &amm2);
    let token2_balance = token2.balance(&router, amm2.clone()).unwrap();
    assert_eq!(info_amm2.token2_reserve, token2_balance);
    assert_eq!(info_amm2.token1_reserve, amm2_native_balance.amount);
}
