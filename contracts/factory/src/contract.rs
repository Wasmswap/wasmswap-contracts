#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Order, Reply, Response, StdResult,
    SubMsg, WasmMsg,
};
use cw0::parse_reply_instantiate_data;
use cw2::set_contract_version;
use cw20::Denom;

use crate::error::ContractError;
use crate::msg::{
    ExecuteMsg, GetSwapsDetailedResponse, GetSwapsResponse, InstantiateMsg, QueryMsg, SwapDetails,
};

use crate::state::{get_denom_primary_key, Config, Swap, CONFIG, SWAPS};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:factory";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const INSTANTIATE_SWAP_REPLY_ID: u64 = 0;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let config = Config {
        swap_code_id: msg.swap_code_id,
        lp_token_code_id: msg.lp_token_code_id,
        unstaking_duration: msg.unstaking_duration,
    };
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("sender", info.sender)
        .add_attribute("swap_code_id", msg.swap_code_id.to_string())
        .add_attribute("lp_token_code_id", msg.lp_token_code_id.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::CreateSwap { token_denom } => try_create_swap(deps, token_denom),
    }
}

pub fn try_create_swap(deps: DepsMut, token_denom: Denom) -> Result<Response, ContractError> {
    // TODO improve label
    if SWAPS
        .may_load(deps.storage, get_denom_primary_key(&token_denom))?
        .is_some()
    {
        return Err(ContractError::SwapAlreadyExists {});
    }
    let config = CONFIG.load(deps.storage)?;
    let instantiate_msg = junoswap::msg::InstantiateMsg {
        token1_denom: Denom::Native("ujuno".to_string()),
        token2_denom: token_denom,
        lp_token_code_id: config.lp_token_code_id,
        lp_token_unstaking_duration: config.unstaking_duration,
    };

    let instantiate_msg = WasmMsg::Instantiate {
        admin: None,
        code_id: config.swap_code_id,
        msg: to_binary(&instantiate_msg)?,
        funds: vec![],
        label: "TODO_improve_label".to_string(),
    };

    let reply_msg = SubMsg::reply_on_success(instantiate_msg, INSTANTIATE_SWAP_REPLY_ID);

    println!("created reply msg");
    Ok(Response::new()
        .add_submessage(reply_msg)
        .add_attribute("method", "create_swap"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    if msg.id != INSTANTIATE_SWAP_REPLY_ID {
        return Err(ContractError::UnknownReplyId { id: msg.id });
    };
    println!("reply!");
    let res = parse_reply_instantiate_data(msg);
    match res {
        Ok(res) => {
            // Validate contract address
            let swap_addr = deps.api.addr_validate(&res.contract_address)?;
            let query_msg = junoswap::msg::QueryMsg::Info {};
            let info: junoswap::msg::InfoResponse = deps
                .querier
                .query_wasm_smart(swap_addr.clone(), &query_msg)?;

            let swap = Swap {
                address: swap_addr,
                token1: info.token1_denom,
                token2: info.token2_denom.clone(),
            };

            SWAPS.save(
                deps.storage,
                get_denom_primary_key(&info.token2_denom),
                &swap,
            )?;

            Ok(Response::new())
        }
        Err(_) => Err(ContractError::InstatiateSwapError {}),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetSwaps {} => to_binary(&query_swaps(deps)?),
        QueryMsg::GetSwapsDetailed { .. } => to_binary(&query_swaps_detailed(deps)?),
    }
}

fn query_swaps(deps: Deps) -> StdResult<GetSwapsResponse> {
    let swaps: StdResult<Vec<Addr>> = SWAPS
        .range(deps.storage, None, None, Order::Ascending)
        .map(|s| Ok(s?.1.address))
        .collect();
    Ok(GetSwapsResponse { swaps: swaps? })
}

fn query_swaps_detailed(deps: Deps) -> StdResult<GetSwapsDetailedResponse> {
    let swaps: StdResult<Vec<Addr>> = SWAPS
        .range(deps.storage, None, None, Order::Ascending)
        .map(|s| Ok(s?.1.address))
        .collect();
    let details: StdResult<Vec<SwapDetails>> = swaps?
        .into_iter()
        .map(|addr| {
            let info = deps
                .querier
                .query_wasm_smart(addr.clone(), &junoswap::msg::QueryMsg::Info {})?;
            Ok(SwapDetails {
                addr,
                details: info,
            })
        })
        .collect();
    Ok(GetSwapsDetailedResponse { swaps: details? })
}

#[cfg(test)]
mod tests {
    use crate::msg::{
        ExecuteMsg, GetSwapsDetailedResponse, GetSwapsResponse, InstantiateMsg, QueryMsg,
    };
    use crate::ContractError;
    use cosmwasm_std::{coins, Addr, Empty, Uint128};
    use cw0::Duration;
    use cw20::{Cw20Coin, Cw20Contract, Denom};
    use cw_multi_test::{App, Contract, ContractWrapper, Executor};
    use std::borrow::BorrowMut;
    use std::collections::HashSet;

    fn mock_app() -> App {
        App::default()
    }

    pub fn contract_cw20() -> Box<dyn Contract<Empty>> {
        let contract = ContractWrapper::new(
            cw20_base::contract::execute,
            cw20_base::contract::instantiate,
            cw20_base::contract::query,
        );
        Box::new(contract)
    }

    pub fn contract_cw20_stakeable() -> Box<dyn Contract<Empty>> {
        let contract = ContractWrapper::new(
            cw20_stakeable::contract::execute,
            cw20_stakeable::contract::instantiate,
            cw20_stakeable::contract::query,
        );
        Box::new(contract)
    }

    pub fn contract_swap() -> Box<dyn Contract<Empty>> {
        let contract = ContractWrapper::new(
            junoswap::contract::execute,
            junoswap::contract::instantiate,
            junoswap::contract::query,
        )
        .with_reply(junoswap::contract::reply);
        Box::new(contract)
    }

    pub fn contract_factory() -> Box<dyn Contract<Empty>> {
        let contract = ContractWrapper::new(
            crate::contract::execute,
            crate::contract::instantiate,
            crate::contract::query,
        )
        .with_reply(crate::contract::reply);
        Box::new(contract)
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
            name: name,
            symbol: symbol,
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

    fn set_up_factory(app: &mut App) -> (Addr, Addr) {
        const NATIVE_TOKEN_DENOM: &str = "juno";

        let owner = Addr::unchecked("owner");
        let funds = coins(2000, NATIVE_TOKEN_DENOM);
        app.borrow_mut().init_modules(|router, _, storage| {
            router.bank.init_balance(storage, &owner, funds).unwrap()
        });

        let cw20_token = create_cw20(
            app,
            &owner,
            "token".to_string(),
            "CWTOKEN".to_string(),
            Uint128::new(5000),
        );

        let swap_code_id = app.store_code(contract_swap());
        let lp_token_code_id = app.store_code(contract_cw20_stakeable());
        let factory_code_id = app.store_code(contract_factory());

        let instatiate_msg = InstantiateMsg {
            swap_code_id,
            lp_token_code_id,
            unstaking_duration: Some(Duration::Time(100)),
        };
        let factory_addr = app
            .instantiate_contract(factory_code_id, owner, &instatiate_msg, &[], "asdf", None)
            .unwrap();
        (factory_addr, cw20_token.addr())
    }

    #[test]
    fn test_instantiate() {
        let mut app = mock_app();
        set_up_factory(&mut app);
    }

    #[test]
    fn test_create_swap() {
        let sender = Addr::unchecked("sender");
        let mut app = mock_app();
        let (factory_addr, cw20_addr) = set_up_factory(&mut app);
        println!("{}", factory_addr);

        let create_msg = ExecuteMsg::CreateSwap {
            token_denom: Denom::Cw20(cw20_addr),
        };

        app.borrow_mut()
            .execute_contract(sender, factory_addr, &create_msg, &[])
            .unwrap();
    }

    #[test]
    fn test_create_many_swaps() {
        let sender = Addr::unchecked("sender");
        let mut app = mock_app();
        let (factory_addr, cw20_addr) = set_up_factory(&mut app);
        println!("{}", factory_addr);

        let create_msg = ExecuteMsg::CreateSwap {
            token_denom: Denom::Cw20(cw20_addr),
        };
        app.borrow_mut()
            .execute_contract(sender.clone(), factory_addr.clone(), &create_msg, &[])
            .unwrap();

        let cw20_token = create_cw20(
            app.borrow_mut(),
            &sender,
            "token".to_string(),
            "CWTOKEN".to_string(),
            Uint128::new(5000),
        );
        let create_msg = ExecuteMsg::CreateSwap {
            token_denom: Denom::Cw20(cw20_token.addr()),
        };
        app.borrow_mut()
            .execute_contract(sender.clone(), factory_addr.clone(), &create_msg, &[])
            .unwrap();

        let cw20_token = create_cw20(
            app.borrow_mut(),
            &sender,
            "token".to_string(),
            "CWTOKEN".to_string(),
            Uint128::new(5000),
        );
        let create_msg = ExecuteMsg::CreateSwap {
            token_denom: Denom::Cw20(cw20_token.addr()),
        };
        app.borrow_mut()
            .execute_contract(sender.clone(), factory_addr.clone(), &create_msg, &[])
            .unwrap();
    }

    #[test]
    fn test_no_duplicate_swaps() {
        let sender = Addr::unchecked("sender");
        let mut app = mock_app();
        let (factory_addr, cw20_addr) = set_up_factory(&mut app);
        println!("{}", factory_addr);

        let create_msg = ExecuteMsg::CreateSwap {
            token_denom: Denom::Cw20(cw20_addr),
        };

        app.borrow_mut()
            .execute_contract(sender.clone(), factory_addr.clone(), &create_msg, &[])
            .unwrap();

        let err = app
            .borrow_mut()
            .execute_contract(sender, factory_addr, &create_msg, &[])
            .unwrap_err();
        assert_eq!(ContractError::SwapAlreadyExists {}, err.downcast().unwrap())
    }

    #[test]
    fn test_get_swap_details() {
        let mut cw20_addr_set = HashSet::new();
        let sender = Addr::unchecked("sender");
        let mut app = mock_app();
        let (factory_addr, cw20_addr) = set_up_factory(&mut app);
        println!("{}", factory_addr);
        cw20_addr_set.insert(cw20_addr.clone());

        let create_msg = ExecuteMsg::CreateSwap {
            token_denom: Denom::Cw20(cw20_addr),
        };
        app.borrow_mut()
            .execute_contract(sender.clone(), factory_addr.clone(), &create_msg, &[])
            .unwrap();

        let cw20_token = create_cw20(
            app.borrow_mut(),
            &sender,
            "token".to_string(),
            "CWTOKEN".to_string(),
            Uint128::new(5000),
        );
        cw20_addr_set.insert(cw20_token.addr());
        let create_msg = ExecuteMsg::CreateSwap {
            token_denom: Denom::Cw20(cw20_token.addr()),
        };
        app.borrow_mut()
            .execute_contract(sender.clone(), factory_addr.clone(), &create_msg, &[])
            .unwrap();

        let cw20_token = create_cw20(
            app.borrow_mut(),
            &sender,
            "token".to_string(),
            "CWTOKEN".to_string(),
            Uint128::new(5000),
        );
        cw20_addr_set.insert(cw20_token.addr());
        let create_msg = ExecuteMsg::CreateSwap {
            token_denom: Denom::Cw20(cw20_token.addr()),
        };
        app.borrow_mut()
            .execute_contract(sender.clone(), factory_addr.clone(), &create_msg, &[])
            .unwrap();

        let res: GetSwapsResponse = app
            .borrow_mut()
            .wrap()
            .query_wasm_smart(factory_addr.clone(), &QueryMsg::GetSwaps {})
            .unwrap();
        assert_eq!(res.swaps.len(), 3);

        let res: GetSwapsDetailedResponse = app
            .borrow_mut()
            .wrap()
            .query_wasm_smart(factory_addr, &QueryMsg::GetSwapsDetailed {})
            .unwrap();
        assert_eq!(res.swaps.len(), 3);
        let mut result_cw20_addr_set = HashSet::new();
        for s in res.swaps {
            if let Denom::Cw20(addr) = s.details.token2_denom {
                result_cw20_addr_set.insert(addr);
            }
        }
        assert!(result_cw20_addr_set.is_superset(&cw20_addr_set))
    }
}
