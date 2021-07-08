use cosmwasm_std::{
    attr, entry_point, to_binary, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo,
    Response, StdError, StdResult, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, MinterResponse};
use cw20_base::contract::{
    execute_burn, execute_mint, instantiate as cw20_instantiate, query_balance,
};
use cw20_base::state::{BALANCES as LIQUIDITY_BALANCES, TOKEN_INFO as LIQUIDITY_INFO};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InfoResponse, InstantiateMsg, QueryMsg};
use crate::state::{State, STATE};

// Note, you can use StdResult in some functions where you do not
// make use of the custom errors
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        native_supply: Coin {
            denom: msg.native_denom,
            amount: Uint128(0),
        },
        token_address: msg.token_address,
        token_supply: Uint128(0),
    };
    STATE.save(deps.storage, &state)?;

    cw20_instantiate(
        deps,
        _env.clone(),
        info,
        cw20_base::msg::InstantiateMsg {
            name: "CRUST_LIQUIDITY_TOKEN".into(),
            symbol: "CRUST".into(),
            decimals: 18,
            initial_balances: vec![],
            mint: Some(MinterResponse {
                minter: _env.contract.address.into(),
                cap: None,
            }),
        },
    )?;

    Ok(Response::default())
}

// And declare a custom Error variant for the ones where you will want to make use of it
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::AddLiquidity {
            min_liquidity,
            max_token,
        } => execute_add_liquidity(deps, info, _env, min_liquidity, max_token),
        ExecuteMsg::RemoveLiquidity {
            amount,
            min_native,
            min_token,
        } => execute_remove_liquidity(deps, info, _env, amount, min_native, min_token),
        ExecuteMsg::NativeForTokenSwapInput { min_token } => {
            execute_native_for_token_swap_input(deps, info, _env, min_token)
        }
        ExecuteMsg::TokenForNativeSwapInput {
            token_amount,
            min_native,
        } => execute_token_for_native_swap_input(deps, info, _env, token_amount, min_native),
    }
}

fn get_liquidity_amount(
    native_token_amount: Uint128,
    liquidity_supply: Uint128,
    native_supply: Uint128,
) -> Result<Uint128, ContractError> {
    if liquidity_supply == Uint128(0) {
        Ok(native_token_amount)
    } else {
        Ok(native_token_amount
            .checked_mul(liquidity_supply)
            .map_err(StdError::overflow)?
            .checked_div(native_supply)
            .map_err(StdError::divide_by_zero)?)
    }
}

fn get_token_amount(
    max_token: Uint128,
    native_token_amount: Uint128,
    liquidity_supply: Uint128,
    token_supply: Uint128,
    native_supply: Uint128,
) -> Result<Uint128, ContractError> {
    if liquidity_supply == Uint128(0) {
        Ok(max_token)
    } else {
        Ok(native_token_amount
            .checked_mul(token_supply)
            .map_err(StdError::overflow)?
            .checked_div(native_supply)
            .map_err(StdError::divide_by_zero)?
            .checked_add(Uint128(1))
            .map_err(StdError::overflow)?)
    }
}

pub fn execute_add_liquidity(
    deps: DepsMut,
    info: MessageInfo,
    _env: Env,
    min_liquidity: Uint128,
    max_token: Uint128,
) -> Result<Response, ContractError> {
    let state = STATE.load(deps.storage).unwrap();

    let liquidity = LIQUIDITY_INFO.load(deps.storage)?;

    if info.funds[0].denom != state.native_supply.denom {
        return Err(ContractError::IncorrectNativeDenom {
            provided: info.funds[0].denom.clone(),
            required: state.native_supply.denom,
        });
    }

    let liquidity_amount = get_liquidity_amount(
        info.funds[0].clone().amount,
        liquidity.total_supply,
        state.native_supply.amount,
    )?;

    let token_amount = get_token_amount(
        max_token,
        info.funds[0].clone().amount,
        liquidity.total_supply,
        state.token_supply,
        state.native_supply.amount,
    )?;

    if liquidity_amount < min_liquidity {
        return Err(ContractError::MinLiquidityError {
            min_liquidity,
            liquidity_available: liquidity_amount,
        });
    }

    if token_amount > max_token {
        return Err(ContractError::MaxTokenError {
            max_token,
            tokens_required: token_amount,
        });
    }

    // create transfer cw20 msg
    let transfer_cw20_msg = Cw20ExecuteMsg::TransferFrom {
        owner: info.sender.clone().into(),
        recipient: _env.contract.address.clone().into(),
        amount: token_amount,
    };
    let exec_cw20_transfer = WasmMsg::Execute {
        contract_addr: state.token_address.into(),
        msg: to_binary(&transfer_cw20_msg)?,
        send: vec![],
    };
    let cw20_transfer_cosmos_msg: CosmosMsg = exec_cw20_transfer.into();

    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        state.token_supply += token_amount;
        state.native_supply.amount += info.funds[0].amount;
        Ok(state)
    })?;

    let sub_info = MessageInfo {
        sender: _env.contract.address.clone(),
        funds: vec![],
    };
    execute_mint(
        deps,
        _env,
        sub_info,
        info.sender.clone().into(),
        liquidity_amount,
    )?;

    Ok(Response {
        messages: vec![cw20_transfer_cosmos_msg],
        submessages: vec![],
        attributes: vec![
            attr("native_amount", info.funds[0].clone().amount),
            attr("token_amount", token_amount),
            attr("liquidity_received", liquidity_amount),
        ],
        data: None,
    })
}

pub fn execute_remove_liquidity(
    deps: DepsMut,
    info: MessageInfo,
    _env: Env,
    amount: Uint128,
    min_native: Uint128,
    min_token: Uint128,
) -> Result<Response, ContractError> {
    let balance = LIQUIDITY_BALANCES.load(deps.storage, &info.sender)?;
    let token = LIQUIDITY_INFO.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    if amount > balance {
        return Err(ContractError::InsufficientLiquidityError {
            requested: amount,
            available: balance,
        });
    }

    let native_amount = amount
        .checked_mul(state.native_supply.amount)
        .map_err(StdError::overflow)?
        .checked_div(token.total_supply)
        .map_err(StdError::divide_by_zero)?;
    if native_amount < min_native {
        return Err(ContractError::MinNative {
            requested: min_native,
            available: native_amount,
        });
    }

    let token_amount = amount
        .checked_mul(state.token_supply)
        .map_err(StdError::overflow)?
        .checked_div(token.total_supply)
        .map_err(StdError::divide_by_zero)?;
    if token_amount < min_token {
        return Err(ContractError::MinToken {
            requested: min_token,
            available: token_amount,
        });
    }

    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        state.token_supply = state
            .token_supply
            .checked_sub(token_amount)
            .map_err(StdError::overflow)?;
        state.native_supply.amount = state
            .native_supply
            .amount
            .checked_sub(native_amount)
            .map_err(StdError::overflow)?;
        Ok(state)
    })?;

    let transfer_bank_msg = cosmwasm_std::BankMsg::Send {
        to_address: info.sender.clone().into(),
        amount: vec![Coin {
            denom: state.native_supply.denom,
            amount: native_amount,
        }],
    };

    let transfer_bank_cosmos_msg: CosmosMsg = transfer_bank_msg.into();

    // create transfer cw20 msg
    let transfer_cw20_msg = Cw20ExecuteMsg::Transfer {
        recipient: info.sender.clone().into(),
        amount: token_amount,
    };
    let exec_cw20_transfer = WasmMsg::Execute {
        contract_addr: state.token_address.into(),
        msg: to_binary(&transfer_cw20_msg)?,
        send: vec![],
    };
    let cw20_transfer_cosmos_msg: CosmosMsg = exec_cw20_transfer.into();

    execute_burn(deps, _env, info, amount)?;

    Ok(Response {
        messages: vec![transfer_bank_cosmos_msg, cw20_transfer_cosmos_msg],
        submessages: vec![],
        attributes: vec![
            attr("liquidity_burned", amount),
            attr("native_returned", native_amount),
            attr("token_returned", token_amount),
        ],
        data: None,
    })
}

fn get_input_price(
    input_amount: Uint128,
    input_supply: Uint128,
    output_supply: Uint128,
) -> Result<Uint128, ContractError> {
    if input_supply == Uint128(0) || output_supply == Uint128(0) {
        return Err(ContractError::NoLiquidityError {});
    };

    let input_amount_with_fee = input_amount
        .checked_mul(Uint128(997))
        .map_err(StdError::overflow)?;
    let numerator = input_amount_with_fee
        .checked_mul(output_supply)
        .map_err(StdError::overflow)?;
    let denominator = input_supply
        .checked_mul(Uint128(1000))
        .map_err(StdError::overflow)?
        .checked_add(input_amount_with_fee)
        .map_err(StdError::overflow)?;

    Ok(numerator
        .checked_div(denominator)
        .map_err(StdError::divide_by_zero)?)
}

pub fn execute_native_for_token_swap_input(
    deps: DepsMut,
    info: MessageInfo,
    _env: Env,
    min_token: Uint128,
) -> Result<Response, ContractError> {
    let state = STATE.load(deps.storage)?;

    if info.funds[0].denom != state.native_supply.denom {
        return Err(ContractError::IncorrectNativeDenom {
            provided: info.funds[0].denom.clone(),
            required: state.native_supply.denom,
        });
    }

    let native_amount = info.funds[0].amount;

    let token_bought = get_input_price(
        native_amount,
        state.native_supply.amount,
        state.token_supply,
    )?;

    if min_token > token_bought {
        return Err(ContractError::SwapMinError {
            min: min_token,
            available: token_bought,
        });
    }

    // create transfer cw20 msg
    let transfer_cw20_msg = Cw20ExecuteMsg::Transfer {
        recipient: info.sender.into(),
        amount: token_bought,
    };
    let exec_cw20_transfer = WasmMsg::Execute {
        contract_addr: state.token_address.into(),
        msg: to_binary(&transfer_cw20_msg)?,
        send: vec![],
    };
    let cw20_transfer_cosmos_msg: CosmosMsg = exec_cw20_transfer.into();

    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        state.token_supply = state
            .token_supply
            .checked_sub(token_bought)
            .map_err(StdError::overflow)?;
        state.native_supply.amount = state
            .native_supply
            .amount
            .checked_add(native_amount)
            .map_err(StdError::overflow)?;
        Ok(state)
    })?;

    Ok(Response {
        messages: vec![cw20_transfer_cosmos_msg],
        submessages: vec![],
        attributes: vec![
            attr("native_sold", native_amount),
            attr("token_bought", token_bought),
        ],
        data: None,
    })
}

pub fn execute_token_for_native_swap_input(
    deps: DepsMut,
    info: MessageInfo,
    _env: Env,
    token_amount: Uint128,
    min_native: Uint128,
) -> Result<Response, ContractError> {
    let state = STATE.load(deps.storage)?;

    let native_bought =
        get_input_price(token_amount, state.token_supply, state.native_supply.amount)?;

    if min_native > native_bought {
        return Err(ContractError::SwapMinError {
            min: min_native,
            available: native_bought,
        });
    }

    // Transfer tokens to contract
    let transfer_cw20_msg = Cw20ExecuteMsg::TransferFrom {
        owner: info.sender.clone().into(),
        recipient: _env.contract.address.into(),
        amount: token_amount,
    };
    let exec_cw20_transfer = WasmMsg::Execute {
        contract_addr: state.token_address.into(),
        msg: to_binary(&transfer_cw20_msg)?,
        send: vec![],
    };
    let cw20_transfer_cosmos_msg: CosmosMsg = exec_cw20_transfer.into();

    // Send native tokens to buyer
    let transfer_bank_msg = cosmwasm_std::BankMsg::Send {
        to_address: info.sender.into(),
        amount: vec![Coin {
            denom: state.native_supply.denom,
            amount: native_bought,
        }],
    };
    let transfer_bank_cosmos_msg: CosmosMsg = transfer_bank_msg.into();

    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        state.token_supply = state
            .token_supply
            .checked_add(token_amount)
            .map_err(StdError::overflow)?;
        state.native_supply.amount = state
            .native_supply
            .amount
            .checked_sub(native_bought)
            .map_err(StdError::overflow)?;
        Ok(state)
    })?;

    Ok(Response {
        messages: vec![cw20_transfer_cosmos_msg, transfer_bank_cosmos_msg],
        submessages: vec![],
        attributes: vec![
            attr("token_sold", token_amount),
            attr("native_bought", native_bought),
        ],
        data: None,
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Balance { address } => to_binary(&query_balance(deps, address)?),
        QueryMsg::Info {} => to_binary(&query_info(deps)?),
    }
}

pub fn query_info(deps: Deps) -> StdResult<InfoResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(InfoResponse {
        native_supply: state.native_supply.amount,
        native_denom: state.native_supply.denom,
        token_supply: state.token_supply,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, Addr};

    fn get_info(deps: Deps) -> InfoResponse {
        query_info(deps).unwrap()
    }

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies(&[]);

        let msg = InstantiateMsg {
            native_denom: "test".to_string(),
            token_address: Addr::unchecked("asdf"),
        };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());
    }

    #[test]
    fn add_liquidity() {
        let mut deps = mock_dependencies(&coins(2, "token"));

        let msg = InstantiateMsg {
            native_denom: "test".to_string(),
            token_address: Addr::unchecked("asdf"),
        };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Add initial liquidity
        let info = mock_info("anyone", &coins(2, "test"));
        let msg = ExecuteMsg::AddLiquidity {
            min_liquidity: Uint128(2),
            max_token: Uint128(1),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        assert_eq!(3, res.attributes.len());
        assert_eq!("2", res.attributes[0].value);
        assert_eq!("1", res.attributes[1].value);
        assert_eq!("2", res.attributes[2].value);

        // Add more liquidity
        let info = mock_info("anyone", &coins(4, "test"));
        let msg = ExecuteMsg::AddLiquidity {
            min_liquidity: Uint128(4),
            max_token: Uint128(3),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        assert_eq!(3, res.attributes.len());
        assert_eq!("4", res.attributes[0].value);
        assert_eq!("3", res.attributes[1].value);
        assert_eq!("4", res.attributes[2].value);

        // Too low max_token
        let info = mock_info("anyone", &coins(100, "test"));
        let msg = ExecuteMsg::AddLiquidity {
            min_liquidity: Uint128(100),
            max_token: Uint128(1),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg);
        assert!(res.is_err());

        // Too high min liquidity
        let info = mock_info("anyone", &coins(100, "test"));
        let msg = ExecuteMsg::AddLiquidity {
            min_liquidity: Uint128(500),
            max_token: Uint128(500),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg);
        assert!(res.is_err());

        // Incorrect native denom throws error
        let info = mock_info("anyone", &coins(100, "wrong"));
        let msg = ExecuteMsg::AddLiquidity {
            min_liquidity: Uint128(1),
            max_token: Uint128(500),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg);
        assert!(res.is_err());
    }

    #[test]
    fn remove_liquidity() {
        let mut deps = mock_dependencies(&coins(2, "token"));

        let msg = InstantiateMsg {
            native_denom: "test".to_string(),
            token_address: Addr::unchecked("asdf"),
        };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Add initial liquidity
        let info = mock_info("anyone", &coins(100, "test"));
        let msg = ExecuteMsg::AddLiquidity {
            min_liquidity: Uint128(100),
            max_token: Uint128(50),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        assert_eq!(3, res.attributes.len());
        assert_eq!("100", res.attributes[0].value);
        assert_eq!("50", res.attributes[1].value);
        assert_eq!("100", res.attributes[2].value);

        // Remove half liquidity
        let info = mock_info("anyone", &vec![]);
        let msg = ExecuteMsg::RemoveLiquidity {
            amount: Uint128(50),
            min_native: Uint128(50),
            min_token: Uint128(25),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!("50", res.attributes[0].value);
        assert_eq!("50", res.attributes[1].value);
        assert_eq!("25", res.attributes[2].value);

        // Remove half again with not proper division
        let info = mock_info("anyone", &vec![]);
        let msg = ExecuteMsg::RemoveLiquidity {
            amount: Uint128(25),
            min_native: Uint128(25),
            min_token: Uint128(12),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!("25", res.attributes[0].value);
        assert_eq!("25", res.attributes[1].value);
        assert_eq!("12", res.attributes[2].value);

        // Remove more than owned
        let info = mock_info("anyone", &vec![]);
        let msg = ExecuteMsg::RemoveLiquidity {
            amount: Uint128(26),
            min_native: Uint128(1),
            min_token: Uint128(1),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg);
        assert!(res.is_err());

        // Remove rest of liquidity
        let info = mock_info("anyone", &vec![]);
        let msg = ExecuteMsg::RemoveLiquidity {
            amount: Uint128(25),
            min_native: Uint128(1),
            min_token: Uint128(1),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!("25", res.attributes[0].value);
        assert_eq!("25", res.attributes[1].value);
        assert_eq!("13", res.attributes[2].value);
    }

    #[test]
    fn test_get_input_price() {
        // Base case
        assert_eq!(
            Uint128(9),
            get_input_price(Uint128(10), Uint128(100), Uint128(100)).unwrap()
        );

        // No input supply error
        assert!(get_input_price(Uint128(10), Uint128(0), Uint128(100)).is_err());

        // No output supply error
        assert!(get_input_price(Uint128(10), Uint128(100), Uint128(0)).is_err());

        // No supply error
        assert!(get_input_price(Uint128(10), Uint128(0), Uint128(0)).is_err());
    }

    #[test]
    fn swap_native_for_token() {
        let mut deps = mock_dependencies(&coins(2, "token"));

        let msg = InstantiateMsg {
            native_denom: "test".to_string(),
            token_address: Addr::unchecked("asdf"),
        };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Add initial liquidity
        let info = mock_info("anyone", &coins(100, "test"));
        let msg = ExecuteMsg::AddLiquidity {
            min_liquidity: Uint128(100),
            max_token: Uint128(100),
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Swap tokens
        let info = mock_info("anyone", &coins(10, "test"));
        let msg = ExecuteMsg::NativeForTokenSwapInput {
            min_token: Uint128(9),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(2, res.attributes.len());
        assert_eq!("10", res.attributes[0].value);
        assert_eq!("9", res.attributes[1].value);

        let info = get_info(deps.as_ref());
        assert_eq!(Uint128(110), info.native_supply);
        assert_eq!(Uint128(91), info.token_supply);

        // Second purchase at higher price
        let info = mock_info("anyone", &coins(10, "test"));
        let msg = ExecuteMsg::NativeForTokenSwapInput {
            min_token: Uint128(7),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(2, res.attributes.len());
        assert_eq!("10", res.attributes[0].value);
        assert_eq!("7", res.attributes[1].value);

        let info = get_info(deps.as_ref());
        assert_eq!(Uint128(120), info.native_supply);
        assert_eq!(Uint128(84), info.token_supply);

        // min_token error
        let info = mock_info("anyone", &coins(10, "test"));
        let msg = ExecuteMsg::NativeForTokenSwapInput {
            min_token: Uint128(100),
        };
        let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(
            err,
            ContractError::SwapMinError {
                min: Uint128(100),
                available: Uint128(6)
            }
        );
    }

    #[test]
    fn swap_token_for_native() {
        let mut deps = mock_dependencies(&coins(2, "token"));

        let msg = InstantiateMsg {
            native_denom: "test".to_string(),
            token_address: Addr::unchecked("asdf"),
        };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Add initial liquidity
        let info = mock_info("anyone", &coins(100, "test"));
        let msg = ExecuteMsg::AddLiquidity {
            min_liquidity: Uint128(100),
            max_token: Uint128(100),
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Swap tokens
        let info = mock_info("anyone", &vec![]);
        let msg = ExecuteMsg::TokenForNativeSwapInput {
            token_amount: Uint128(10),
            min_native: Uint128(9),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(2, res.attributes.len());
        assert_eq!("10", res.attributes[0].value);
        assert_eq!("9", res.attributes[1].value);

        let info = get_info(deps.as_ref());
        assert_eq!(Uint128(110), info.token_supply);
        assert_eq!(Uint128(91), info.native_supply);

        // Second purchase at higher price
        let info = mock_info("anyone", &vec![]);
        let msg = ExecuteMsg::TokenForNativeSwapInput {
            token_amount: Uint128(10),
            min_native: Uint128(7),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(2, res.attributes.len());
        assert_eq!("10", res.attributes[0].value);
        assert_eq!("7", res.attributes[1].value);

        let info = get_info(deps.as_ref());
        assert_eq!(Uint128(120), info.token_supply);
        assert_eq!(Uint128(84), info.native_supply);

        // min_token error
        let info = mock_info("anyone", &vec![]);
        let msg = ExecuteMsg::TokenForNativeSwapInput {
            token_amount: Uint128(10),
            min_native: Uint128(100),
        };
        let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(
            err,
            ContractError::SwapMinError {
                min: Uint128(100),
                available: Uint128(6)
            }
        );
    }
}
