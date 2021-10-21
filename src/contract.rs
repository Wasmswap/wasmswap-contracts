use cosmwasm_std::{
    attr, entry_point, to_binary, Addr, Binary, BlockInfo, Coin, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, Response, StdError, StdResult, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Expiration, MinterResponse};
use cw20_base::contract::{
    execute_burn, execute_mint, instantiate as cw20_instantiate, query_balance,
};
use cw20_base::state::{BALANCES as LIQUIDITY_BALANCES, TOKEN_INFO as LIQUIDITY_INFO};

use crate::error::ContractError;
use crate::msg::{
    ExecuteMsg, InfoResponse, InstantiateMsg, QueryMsg, Token1ForToken2PriceResponse,
    Token2ForToken1PriceResponse, TokenSelect,
};
use crate::state::{Token, TOKEN1, TOKEN2};
use cw_storage_plus::Item;

// Note, you can use StdResult in some functions where you do not
// make use of the custom errors
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let token1 = Token {
        reserve: Uint128(0),
        denom: msg.token1_denom,
        address: msg.token1_address,
    };

    TOKEN1.save(deps.storage, &token1)?;

    let token2 = Token {
        address: msg.token2_address,
        denom: msg.token2_denom,
        reserve: Uint128(0),
    };

    TOKEN2.save(deps.storage, &token2)?;

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
            token1_amount,
            min_liquidity,
            max_token2,
            expiration,
        } => execute_add_liquidity(
            deps,
            &info,
            _env,
            min_liquidity,
            token1_amount,
            max_token2,
            expiration,
        ),
        ExecuteMsg::RemoveLiquidity {
            amount,
            min_token1: min_native,
            min_token2: min_token,
            expiration,
        } => execute_remove_liquidity(deps, info, _env, amount, min_native, min_token, expiration),
        ExecuteMsg::SwapToken1ForToken2 {
            token1_amount,
            min_token2,
            expiration,
        } => execute_swap(
            deps,
            &info,
            token1_amount,
            _env,
            TOKEN1,
            TOKEN2,
            &info.sender,
            min_token2,
            expiration,
        ),
        ExecuteMsg::SwapToken2ForToken1 {
            token2_amount,
            min_token1,
            expiration,
        } => execute_swap(
            deps,
            &info,
            token2_amount,
            _env,
            TOKEN2,
            TOKEN1,
            &info.sender,
            min_token1,
            expiration,
        ),
        ExecuteMsg::SwapTokenForToken {
            output_amm_address,
            input_token_amount,
            output_min_token,
            expiration,
        } => execute_multi_contract_swap(
            deps,
            info,
            _env,
            output_amm_address,
            TokenSelect::Token2,
            input_token_amount,
            TokenSelect::Token2,
            output_min_token,
            expiration,
        ),
        ExecuteMsg::SwapTo {
            input_token,
            input_amount,
            recipient,
            min_token,
            expiration,
        } => execute_swap(
            deps,
            &info,
            input_amount,
            _env,
            match input_token{
                TokenSelect::Token1 => TOKEN1,
                TokenSelect::Token2 => TOKEN2,
            },
            match input_token{
                TokenSelect::Token1 => TOKEN2,
                TokenSelect::Token2 => TOKEN1,
            },
            &recipient,
            min_token,
            expiration,
        ),
    }
}

fn check_expiration(
    expiration: &Option<Expiration>,
    block: &BlockInfo,
) -> Result<(), ContractError> {
    match expiration {
        Some(e) => {
            if e.is_expired(block) {
                return Err(ContractError::MsgExpirationError {});
            }
            Ok(())
        }
        None => Ok(()),
    }
}

fn get_liquidity_amount(
    native_amount: Uint128,
    liquidity_supply: Uint128,
    native_reserve: Uint128,
) -> Result<Uint128, ContractError> {
    if liquidity_supply == Uint128(0) {
        Ok(native_amount)
    } else {
        Ok(native_amount
            .checked_mul(liquidity_supply)
            .map_err(StdError::overflow)?
            .checked_div(native_reserve)
            .map_err(StdError::divide_by_zero)?)
    }
}

fn get_token_amount(
    max_token: Uint128,
    native_amount: Uint128,
    liquidity_supply: Uint128,
    token_reserve: Uint128,
    native_reserve: Uint128,
) -> Result<Uint128, StdError> {
    if liquidity_supply == Uint128(0) {
        Ok(max_token)
    } else {
        Ok(native_amount
            .checked_mul(token_reserve)
            .map_err(StdError::overflow)?
            .checked_div(native_reserve)
            .map_err(StdError::divide_by_zero)?
            .checked_add(Uint128(1))
            .map_err(StdError::overflow)?)
    }
}

pub fn execute_add_liquidity(
    deps: DepsMut,
    info: &MessageInfo,
    _env: Env,
    min_liquidity: Uint128,
    token1_amount: Uint128,
    max_token2: Uint128,
    expiration: Option<Expiration>,
) -> Result<Response, ContractError> {
    check_expiration(&expiration, &_env.block)?;

    let token1 = TOKEN1.load(deps.storage).unwrap();
    let token2 = TOKEN2.load(deps.storage).unwrap();

    let liquidity = LIQUIDITY_INFO.load(deps.storage)?;

    // validate funds if native input token
    match token1.address {
        Some(_) => Ok(()),
        None => validate_native_input_amount(&info.funds, token1_amount, &token1.denom),
    }?;
    match token2.address {
        Some(_) => Ok(()),
        None => validate_native_input_amount(&info.funds, max_token2, &token2.denom),
    }?;

    let liquidity_amount =
        get_liquidity_amount(token1_amount, liquidity.total_supply, token1.reserve)?;

    let token_amount = get_token_amount(
        max_token2,
        token1_amount,
        liquidity.total_supply,
        token2.reserve,
        token1.reserve,
    )?;

    if liquidity_amount < min_liquidity {
        return Err(ContractError::MinLiquidityError {
            min_liquidity,
            liquidity_available: liquidity_amount,
        });
    }

    if token_amount > max_token2 {
        return Err(ContractError::MaxTokenError {
            max_token: max_token2,
            tokens_required: token_amount,
        });
    }

    // Generate cw20 transfer messages if necessary
    let mut cw20_transfer_msgs: Vec<CosmosMsg> = vec![];
    if let Some(addr) = token1.address {
        cw20_transfer_msgs.push(get_cw20_transfer_from_msg(
            &info.sender,
            &_env.contract.address,
            &addr,
            token1_amount,
        )?)
    }
    if let Some(addr) = token2.address {
        cw20_transfer_msgs.push(get_cw20_transfer_from_msg(
            &info.sender,
            &_env.contract.address,
            &addr,
            token_amount,
        )?)
    }

    TOKEN1.update(deps.storage, |mut token1| -> Result<_, ContractError> {
        token1.reserve += token1_amount;
        Ok(token1)
    })?;
    TOKEN2.update(deps.storage, |mut token2| -> Result<_, ContractError> {
        token2.reserve += token_amount;
        Ok(token2)
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
        messages: cw20_transfer_msgs,
        submessages: vec![],
        attributes: vec![
            attr("native_amount", info.funds[0].clone().amount),
            attr("token_amount", token_amount),
            attr("liquidity_received", liquidity_amount),
        ],
        data: None,
    })
}

fn validate_native_input_amount(
    actual_funds: &[Coin],
    given_amount: Uint128,
    given_denom: &str,
) -> Result<(), ContractError> {
    let actual = get_amount_for_denom(actual_funds, given_denom);
    if actual.amount != given_amount {
        return Err(ContractError::InsufficientFunds {});
    }
    if actual.denom != given_denom {
        return Err(ContractError::IncorrectNativeDenom {
            provided: actual.denom,
            required: given_denom.to_string(),
        });
    };
    Ok(())
}

fn get_cw20_transfer_from_msg(
    owner: &Addr,
    recipient: &Addr,
    token_addr: &Addr,
    token_amount: Uint128,
) -> StdResult<CosmosMsg> {
    // create transfer cw20 msg
    let transfer_cw20_msg = Cw20ExecuteMsg::TransferFrom {
        owner: owner.into(),
        recipient: recipient.into(),
        amount: token_amount,
    };
    let exec_cw20_transfer = WasmMsg::Execute {
        contract_addr: token_addr.into(),
        msg: to_binary(&transfer_cw20_msg)?,
        send: vec![],
    };
    let cw20_transfer_cosmos_msg: CosmosMsg = exec_cw20_transfer.into();
    Ok(cw20_transfer_cosmos_msg)
}

fn get_cw20_increase_allowance_msg(
    token_addr: &Addr,
    spender: &Addr,
    amount: Uint128,
    expires: Option<Expiration>,
) -> StdResult<CosmosMsg> {
    // create transfer cw20 msg
    let increase_allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: spender.to_string(),
        amount,
        expires,
    };
    let exec_allowance = WasmMsg::Execute {
        contract_addr: token_addr.into(),
        msg: to_binary(&increase_allowance_msg)?,
        send: vec![],
    };
    Ok(exec_allowance.into())
}

pub fn execute_remove_liquidity(
    deps: DepsMut,
    info: MessageInfo,
    _env: Env,
    amount: Uint128,
    min_native: Uint128,
    min_token: Uint128,
    expiration: Option<Expiration>,
) -> Result<Response, ContractError> {
    check_expiration(&expiration, &_env.block)?;

    let balance = LIQUIDITY_BALANCES.load(deps.storage, &info.sender)?;
    let token = LIQUIDITY_INFO.load(deps.storage)?;
    let token1 = TOKEN1.load(deps.storage)?;
    let token2 = TOKEN2.load(deps.storage)?;

    if amount > balance {
        return Err(ContractError::InsufficientLiquidityError {
            requested: amount,
            available: balance,
        });
    }

    let native_amount = amount
        .checked_mul(token1.reserve)
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
        .checked_mul(token2.reserve)
        .map_err(StdError::overflow)?
        .checked_div(token.total_supply)
        .map_err(StdError::divide_by_zero)?;
    if token_amount < min_token {
        return Err(ContractError::MinToken {
            requested: min_token,
            available: token_amount,
        });
    }

    TOKEN1.update(deps.storage, |mut token1| -> Result<_, ContractError> {
        token1.reserve = token1
            .reserve
            .checked_sub(native_amount)
            .map_err(StdError::overflow)?;
        Ok(token1)
    })?;

    TOKEN2.update(deps.storage, |mut token2| -> Result<_, ContractError> {
        token2.reserve = token2
            .reserve
            .checked_sub(token_amount)
            .map_err(StdError::overflow)?;
        Ok(token2)
    })?;

    let token1_transfer_msg = match token1.address {
        Some(addr) => get_cw20_transfer_to_msg(&info.sender, &addr, native_amount)?,
        None => get_bank_transfer_to_msg(&info.sender, &token1.denom, native_amount),
    };
    let token2_transfer_msg = match token2.address {
        Some(addr) => get_cw20_transfer_to_msg(&info.sender, &addr, token_amount)?,
        None => get_bank_transfer_to_msg(&info.sender, &token1.denom, token_amount),
    };

    execute_burn(deps, _env, info, amount)?;

    Ok(Response {
        messages: vec![token1_transfer_msg, token2_transfer_msg],
        submessages: vec![],
        attributes: vec![
            attr("liquidity_burned", amount),
            attr("native_returned", native_amount),
            attr("token_returned", token_amount),
        ],
        data: None,
    })
}

fn get_cw20_transfer_to_msg(
    recipient: &Addr,
    token_addr: &Addr,
    token_amount: Uint128,
) -> StdResult<CosmosMsg> {
    // create transfer cw20 msg
    let transfer_cw20_msg = Cw20ExecuteMsg::Transfer {
        recipient: recipient.into(),
        amount: token_amount,
    };
    let exec_cw20_transfer = WasmMsg::Execute {
        contract_addr: token_addr.into(),
        msg: to_binary(&transfer_cw20_msg)?,
        send: vec![],
    };
    let cw20_transfer_cosmos_msg: CosmosMsg = exec_cw20_transfer.into();
    Ok(cw20_transfer_cosmos_msg)
}

fn get_bank_transfer_to_msg(recipient: &Addr, denom: &str, native_amount: Uint128) -> CosmosMsg {
    let transfer_bank_msg = cosmwasm_std::BankMsg::Send {
        to_address: recipient.into(),
        amount: vec![Coin {
            denom: denom.to_string(),
            amount: native_amount,
        }],
    };

    let transfer_bank_cosmos_msg: CosmosMsg = transfer_bank_msg.into();
    transfer_bank_cosmos_msg
}

fn get_input_price(
    input_amount: Uint128,
    input_reserve: Uint128,
    output_reserve: Uint128,
) -> Result<Uint128, ContractError> {
    if input_reserve == Uint128(0) || output_reserve == Uint128(0) {
        return Err(ContractError::NoLiquidityError {});
    };

    let input_amount_with_fee = input_amount
        .checked_mul(Uint128(997))
        .map_err(StdError::overflow)?;
    let numerator = input_amount_with_fee
        .checked_mul(output_reserve)
        .map_err(StdError::overflow)?;
    let denominator = input_reserve
        .checked_mul(Uint128(1000))
        .map_err(StdError::overflow)?
        .checked_add(input_amount_with_fee)
        .map_err(StdError::overflow)?;

    Ok(numerator
        .checked_div(denominator)
        .map_err(StdError::divide_by_zero)?)
}

fn get_amount_for_denom(coins: &[Coin], denom: &str) -> Coin {
    let amount: Uint128 = coins
        .iter()
        .filter(|c| c.denom == denom)
        .map(|c| c.amount)
        .sum();
    Coin {
        amount,
        denom: denom.to_string(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn execute_swap(
    deps: DepsMut,
    info: &MessageInfo,
    input_amount: Uint128,
    _env: Env,
    input_token_item: Item<Token>,
    output_token_item: Item<Token>,
    recipient: &Addr,
    min_token: Uint128,
    expiration: Option<Expiration>,
) -> Result<Response, ContractError> {
    check_expiration(&expiration, &_env.block)?;

    let input_token = input_token_item.load(deps.storage)?;
    let output_token = output_token_item.load(deps.storage)?;

    // validate input_amount if native input token
    match input_token.address {
        Some(_) => Ok(()),
        None => validate_native_input_amount(&info.funds, input_amount, &input_token.denom),
    }?;

    let token_bought = get_input_price(input_amount, input_token.reserve, output_token.reserve)?;

    if min_token > token_bought {
        return Err(ContractError::SwapMinError {
            min: min_token,
            available: token_bought,
        });
    }

    // Create transfer from message
    let mut transfer_msgs = match input_token.address {
        Some(addr) => vec![get_cw20_transfer_from_msg(
            &info.sender,
            &_env.contract.address,
            &addr,
            input_amount,
        )?],
        None => vec![],
    };

    // Create transfer to message
    transfer_msgs.push(match output_token.address {
        Some(addr) => get_cw20_transfer_to_msg(&recipient, &addr, token_bought)?,
        None => get_bank_transfer_to_msg(&recipient, &output_token.denom, token_bought),
    });

    input_token_item.update(
        deps.storage,
        |mut input_token| -> Result<_, ContractError> {
            input_token.reserve = input_token
                .reserve
                .checked_add(input_amount)
                .map_err(StdError::overflow)?;
            Ok(input_token)
        },
    )?;

    output_token_item.update(
        deps.storage,
        |mut output_token| -> Result<_, ContractError> {
            output_token.reserve = output_token
                .reserve
                .checked_sub(token_bought)
                .map_err(StdError::overflow)?;
            Ok(output_token)
        },
    )?;

    Ok(Response {
        messages: transfer_msgs,
        submessages: vec![],
        attributes: vec![
            attr("native_sold", input_amount),
            attr("token_bought", token_bought),
        ],
        data: None,
    })
}

pub fn execute_multi_contract_swap(
    deps: DepsMut,
    info: MessageInfo,
    _env: Env,
    output_amm_address: Addr,
    input_token_enum: TokenSelect,
    input_token_amount: Uint128,
    output_token_enum: TokenSelect,
    output_min_token: Uint128,
    expiration: Option<Expiration>,
) -> Result<Response, ContractError> {
    check_expiration(&expiration, &_env.block)?;

    let input_token_state = match input_token_enum {
        TokenSelect::Token1 => TOKEN1,
        TokenSelect::Token2 => TOKEN2,
    };
    let input_token = input_token_state.load(deps.storage)?;
    let transfer_token_state = match input_token_enum {
        TokenSelect::Token1 => TOKEN2,
        TokenSelect::Token2 => TOKEN1,
    };
    let transfer_token = transfer_token_state.load(deps.storage)?;

    // validate input_amount if native input token
    match input_token.address {
        Some(_) => Ok(()),
        None => validate_native_input_amount(&info.funds, input_token_amount, &input_token.denom),
    }?;

    let amount_to_transfer = get_input_price(
        input_token_amount,
        input_token.reserve,
        transfer_token.reserve,
    )?;

    // Transfer tokens to contract
    let mut msgs: Vec<CosmosMsg> = vec![];
    if let Some(addr) = &input_token.address {
        msgs.push(get_cw20_transfer_from_msg(
            &info.sender,
            &_env.contract.address,
            addr,
            input_token_amount,
        )?)
    };

    // Increase allowance of output contract is transfer token is cw20
    if let Some(addr) = &transfer_token.address {
        msgs.push(get_cw20_increase_allowance_msg(
            addr,
            &output_amm_address,
            amount_to_transfer,
            Some(Expiration::AtHeight(_env.block.height + 1)),
        )?)
    };

    let swap_msg = ExecuteMsg::SwapTo {
        input_token: match output_token_enum {
            TokenSelect::Token1=> TokenSelect::Token2,
            TokenSelect::Token2=> TokenSelect::Token1,
        },
        input_amount: amount_to_transfer,
        recipient: info.sender,
        min_token: output_min_token,
        expiration,
    };

    msgs.push(
        WasmMsg::Execute {
            contract_addr: output_amm_address.into(),
            msg: to_binary(&swap_msg)?,
            send: match transfer_token.address {
                Some(_) => vec![],
                None => vec![Coin {
                    denom: transfer_token.denom,
                    amount: amount_to_transfer,
                }],
            },
        }
        .into(),
    );

    input_token_state.update(deps.storage, |mut token| -> Result<_, ContractError> {
        token.reserve = token
            .reserve
            .checked_add(input_token_amount)
            .map_err(StdError::overflow)?;
        Ok(token)
    })?;

    transfer_token_state.update(deps.storage, |mut token| -> Result<_, ContractError> {
        token.reserve = token
            .reserve
            .checked_sub(amount_to_transfer)
            .map_err(StdError::overflow)?;
        Ok(token)
    })?;

    Ok(Response {
        messages: msgs,
        submessages: vec![],
        attributes: vec![
            attr("input_token_amount", input_token_amount),
            attr("native_transferred", amount_to_transfer),
        ],
        data: None,
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Balance { address } => to_binary(&query_balance(deps, address)?),
        QueryMsg::Info {} => to_binary(&query_info(deps)?),
        QueryMsg::Token1ForToken2Price {
            token1_amount: native_amount,
        } => to_binary(&query_native_for_token_price(deps, native_amount)?),
        QueryMsg::Token2ForToken1Price {
            token2_amount: token_amount,
        } => to_binary(&query_token_for_native_price(deps, token_amount)?),
    }
}

pub fn query_info(deps: Deps) -> StdResult<InfoResponse> {
    let token1 = TOKEN1.load(deps.storage)?;
    let token2 = TOKEN2.load(deps.storage)?;
    let liquidity = LIQUIDITY_INFO.load(deps.storage)?;
    Ok(InfoResponse {
        token1_reserve: token1.reserve,
        token1_denom: token1.denom,
        token1_address: token1.address.map(|a| a.to_string()),
        token2_reserve: token2.reserve,
        token2_denom: token2.denom,
        token2_address: token2.address.map(|a| a.to_string()),
        lp_token_supply: liquidity.total_supply,
    })
}

pub fn query_native_for_token_price(
    deps: Deps,
    native_amount: Uint128,
) -> StdResult<Token1ForToken2PriceResponse> {
    let token1 = TOKEN1.load(deps.storage)?;
    let token2 = TOKEN2.load(deps.storage)?;
    let token_amount = get_input_price(native_amount, token1.reserve, token2.reserve).unwrap();
    Ok(Token1ForToken2PriceResponse {
        token2_amount: token_amount,
    })
}

pub fn query_token_for_native_price(
    deps: Deps,
    token_amount: Uint128,
) -> StdResult<Token2ForToken1PriceResponse> {
    let token1 = TOKEN1.load(deps.storage)?;
    let token2 = TOKEN2.load(deps.storage)?;
    let native_amount = get_input_price(token_amount, token2.reserve, token1.reserve).unwrap();
    Ok(Token2ForToken1PriceResponse {
        token1_amount: native_amount,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary, Addr};

    fn get_info(deps: Deps) -> InfoResponse {
        query_info(deps).unwrap()
    }

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies(&[]);

        let msg = InstantiateMsg {
            token1_denom: "test".to_string(),
            token1_address: None,
            token2_denom: "coin".to_string(),
            token2_address: Some(Addr::unchecked("token_address")),
        };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.messages.len(), 0);

        let info = query_info(deps.as_ref()).unwrap();
        assert_eq!(info.token1_reserve, Uint128(0));
        assert_eq!(info.token1_denom, "test");
        assert_eq!(info.token2_reserve, Uint128(0));
        assert_eq!(info.token2_denom, "coin");
        assert_eq!(info.token2_address, Some("token_address".to_string()))
    }

    #[test]
    fn test_get_liquidity_amount() {
        let liquidity = get_liquidity_amount(Uint128(100), Uint128(0), Uint128(0)).unwrap();
        assert_eq!(liquidity, Uint128(100));

        let liquidity = get_liquidity_amount(Uint128(100), Uint128(50), Uint128(25)).unwrap();
        assert_eq!(liquidity, Uint128(200));
    }

    #[test]
    fn test_get_token_amount() {
        let liquidity = get_token_amount(
            Uint128(100),
            Uint128(50),
            Uint128(0),
            Uint128(0),
            Uint128(0),
        )
        .unwrap();
        assert_eq!(liquidity, Uint128(100));

        let liquidity = get_token_amount(
            Uint128(200),
            Uint128(50),
            Uint128(50),
            Uint128(100),
            Uint128(25),
        )
        .unwrap();
        assert_eq!(liquidity, Uint128(201));
    }

    #[test]
    fn add_liquidity() {
        let mut deps = mock_dependencies(&coins(2, "token"));

        let msg = InstantiateMsg {
            token1_denom: "test".to_string(),
            token1_address: None,
            token2_denom: "coin".to_string(),
            token2_address: Some(Addr::unchecked("asdf")),
        };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Add initial liquidity
        let info = mock_info("anyone", &coins(2, "test"));
        let msg = ExecuteMsg::AddLiquidity {
            token1_amount: Uint128(2),
            min_liquidity: Uint128(2),
            max_token2: Uint128(1),
            expiration: None,
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        assert_eq!(res.attributes.len(), 3);
        assert_eq!(res.attributes[0].value, "2");
        assert_eq!(res.attributes[1].value, "1");
        assert_eq!(res.attributes[2].value, "2");

        let info = get_info(deps.as_ref());
        assert_eq!(info.token1_reserve, Uint128(2));
        assert_eq!(info.token2_reserve, Uint128(1));

        // Add more liquidity
        let info = mock_info("anyone", &coins(4, "test"));
        let msg = ExecuteMsg::AddLiquidity {
            token1_amount: Uint128(4),
            min_liquidity: Uint128(4),
            max_token2: Uint128(3),
            expiration: None,
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        assert_eq!(res.attributes.len(), 3);
        assert_eq!(res.attributes[0].value, "4");
        assert_eq!(res.attributes[1].value, "3");
        assert_eq!(res.attributes[2].value, "4");

        let info = get_info(deps.as_ref());
        assert_eq!(info.token1_reserve, Uint128(6));
        assert_eq!(info.token2_reserve, Uint128(4));

        // Too low max_token
        let info = mock_info("anyone", &coins(100, "test"));
        let msg = ExecuteMsg::AddLiquidity {
            token1_amount: Uint128(100),
            min_liquidity: Uint128(100),
            max_token2: Uint128(1),
            expiration: None,
        };
        let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(
            err,
            ContractError::MaxTokenError {
                max_token: Uint128(1),
                tokens_required: Uint128(67)
            }
        );

        // Too high min liquidity
        let info = mock_info("anyone", &coins(100, "test"));
        let msg = ExecuteMsg::AddLiquidity {
            token1_amount: Uint128(100),
            min_liquidity: Uint128(500),
            max_token2: Uint128(500),
            expiration: None,
        };
        let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(
            err,
            ContractError::MinLiquidityError {
                min_liquidity: Uint128(500),
                liquidity_available: Uint128(100)
            }
        );

        // Incorrect native denom throws error
        let info = mock_info("anyone", &coins(100, "wrong"));
        let msg = ExecuteMsg::AddLiquidity {
            token1_amount: Uint128(100),
            min_liquidity: Uint128(1),
            max_token2: Uint128(500),
            expiration: None,
        };
        let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(err, ContractError::InsufficientFunds {});

        // Expired Message
        let info = mock_info("anyone", &coins(100, "test"));
        let mut env = mock_env();
        env.block.height = 20;
        let msg = ExecuteMsg::AddLiquidity {
            token1_amount: Uint128(100),
            min_liquidity: Uint128(100),
            max_token2: Uint128(50),
            expiration: Some(Expiration::AtHeight(19)),
        };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::MsgExpirationError {})
    }

    #[test]
    fn remove_liquidity() {
        let mut deps = mock_dependencies(&coins(2, "token"));

        let msg = InstantiateMsg {
            token1_denom: "test".to_string(),
            token1_address: None,
            token2_denom: "coin".to_string(),
            token2_address: Some(Addr::unchecked("asdf")),
        };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Add initial liquidity
        let info = mock_info("anyone", &coins(100, "test"));
        let msg = ExecuteMsg::AddLiquidity {
            token1_amount: Uint128(100),
            min_liquidity: Uint128(100),
            max_token2: Uint128(50),
            expiration: None,
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        assert_eq!(res.attributes.len(), 3);
        assert_eq!(res.attributes[0].value, "100");
        assert_eq!(res.attributes[1].value, "50");
        assert_eq!(res.attributes[2].value, "100");

        let info = get_info(deps.as_ref());
        assert_eq!(info.token1_reserve, Uint128(100));
        assert_eq!(info.token2_reserve, Uint128(50));

        // Remove half liquidity
        let info = mock_info("anyone", &vec![]);
        let msg = ExecuteMsg::RemoveLiquidity {
            amount: Uint128(50),
            min_token1: Uint128(50),
            min_token2: Uint128(25),
            expiration: None,
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.attributes[0].value, "50");
        assert_eq!(res.attributes[1].value, "50");
        assert_eq!(res.attributes[2].value, "25");

        let info = get_info(deps.as_ref());
        assert_eq!(info.token1_reserve, Uint128(50));
        assert_eq!(info.token2_reserve, Uint128(25));

        // Remove half again with not proper division
        let info = mock_info("anyone", &vec![]);
        let msg = ExecuteMsg::RemoveLiquidity {
            amount: Uint128(25),
            min_token1: Uint128(25),
            min_token2: Uint128(12),
            expiration: None,
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.attributes[0].value, "25");
        assert_eq!(res.attributes[1].value, "25");
        assert_eq!(res.attributes[2].value, "12");

        let info = get_info(deps.as_ref());
        assert_eq!(info.token1_reserve, Uint128(25));
        assert_eq!(info.token2_reserve, Uint128(13));

        // Remove more than owned
        let info = mock_info("anyone", &vec![]);
        let msg = ExecuteMsg::RemoveLiquidity {
            amount: Uint128(26),
            min_token1: Uint128(1),
            min_token2: Uint128(1),
            expiration: None,
        };
        let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(
            err,
            ContractError::InsufficientLiquidityError {
                requested: Uint128(26),
                available: Uint128(25)
            }
        );

        // Remove rest of liquidity
        let info = mock_info("anyone", &vec![]);
        let msg = ExecuteMsg::RemoveLiquidity {
            amount: Uint128(25),
            min_token1: Uint128(1),
            min_token2: Uint128(1),
            expiration: None,
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.attributes[0].value, "25");
        assert_eq!(res.attributes[1].value, "25");
        assert_eq!(res.attributes[2].value, "13");

        let info = get_info(deps.as_ref());
        assert_eq!(info.token1_reserve, Uint128(0));
        assert_eq!(info.token2_reserve, Uint128(0));

        // Expired Message
        let info = mock_info("anyone", &coins(100, "test"));
        let mut env = mock_env();
        env.block.height = 20;
        let msg = ExecuteMsg::RemoveLiquidity {
            amount: Uint128(25),
            min_token1: Uint128(1),
            min_token2: Uint128(1),
            expiration: Some(Expiration::AtHeight(19)),
        };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::MsgExpirationError {})
    }

    #[test]
    fn test_get_input_price() {
        // Base case
        assert_eq!(
            get_input_price(Uint128(10), Uint128(100), Uint128(100)).unwrap(),
            Uint128(9)
        );

        // No input reserve error
        let err = get_input_price(Uint128(10), Uint128(0), Uint128(100)).unwrap_err();
        assert_eq!(err, ContractError::NoLiquidityError {});

        // No output reserve error
        let err = get_input_price(Uint128(10), Uint128(100), Uint128(0)).unwrap_err();
        assert_eq!(err, ContractError::NoLiquidityError {});

        // No reserve error
        let err = get_input_price(Uint128(10), Uint128(0), Uint128(0)).unwrap_err();
        assert_eq!(err, ContractError::NoLiquidityError {});
    }

    #[test]
    fn swap_native_for_token() {
        let mut deps = mock_dependencies(&coins(2, "token"));

        let msg = InstantiateMsg {
            token1_denom: "test".to_string(),
            token1_address: None,
            token2_denom: "coin".to_string(),
            token2_address: Some(Addr::unchecked("asdf")),
        };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Add initial liquidity
        let info = mock_info("anyone", &coins(100, "test"));
        let msg = ExecuteMsg::AddLiquidity {
            token1_amount: Uint128(100),
            min_liquidity: Uint128(100),
            max_token2: Uint128(100),
            expiration: None,
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Swap tokens
        let info = mock_info("anyone", &coins(10, "test"));
        let msg = ExecuteMsg::SwapToken1ForToken2 {
            token1_amount: Uint128(10),
            min_token2: Uint128(9),
            expiration: None,
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.attributes.len(), 2);
        assert_eq!(res.attributes[0].value, "10");
        assert_eq!(res.attributes[1].value, "9");

        let info = get_info(deps.as_ref());
        assert_eq!(info.token1_reserve, Uint128(110));
        assert_eq!(info.token2_reserve, Uint128(91));

        // Second purchase at higher price
        let info = mock_info("anyone", &coins(10, "test"));
        let msg = ExecuteMsg::SwapToken1ForToken2 {
            token1_amount: Uint128(10),
            min_token2: Uint128(7),
            expiration: None,
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.attributes.len(), 2);
        assert_eq!(res.attributes[0].value, "10");
        assert_eq!(res.attributes[1].value, "7");

        let info = get_info(deps.as_ref());
        assert_eq!(info.token1_reserve, Uint128(120));
        assert_eq!(info.token2_reserve, Uint128(84));

        // min_token error
        let info = mock_info("anyone", &coins(10, "test"));
        let msg = ExecuteMsg::SwapToken1ForToken2 {
            token1_amount: Uint128(10),
            min_token2: Uint128(100),
            expiration: None,
        };
        let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(
            err,
            ContractError::SwapMinError {
                min: Uint128(100),
                available: Uint128(6)
            }
        );

        // Expired Message
        let info = mock_info("anyone", &coins(100, "test"));
        let mut env = mock_env();
        env.block.height = 20;
        let msg = ExecuteMsg::SwapToken1ForToken2 {
            token1_amount: Uint128(100),
            min_token2: Uint128(100),
            expiration: Some(Expiration::AtHeight(19)),
        };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::MsgExpirationError {})
    }

    #[test]
    fn swap_token_for_native() {
        let mut deps = mock_dependencies(&coins(2, "token"));

        let msg = InstantiateMsg {
            token1_denom: "test".to_string(),
            token1_address: None,
            token2_denom: "coin".to_string(),
            token2_address: Some(Addr::unchecked("asdf")),
        };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Add initial liquidity
        let info = mock_info("anyone", &coins(100, "test"));
        let msg = ExecuteMsg::AddLiquidity {
            token1_amount: Uint128(100),
            min_liquidity: Uint128(100),
            max_token2: Uint128(100),
            expiration: None,
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Swap tokens
        let info = mock_info("anyone", &vec![]);
        let msg = ExecuteMsg::SwapToken2ForToken1 {
            token2_amount: Uint128(10),
            min_token1: Uint128(9),
            expiration: None,
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.attributes.len(), 2);
        assert_eq!(res.attributes[0].value, "10");
        assert_eq!(res.attributes[1].value, "9");

        let info = get_info(deps.as_ref());
        assert_eq!(info.token2_reserve, Uint128(110));
        assert_eq!(info.token1_reserve, Uint128(91));

        // Second purchase at higher price
        let info = mock_info("anyone", &vec![]);
        let msg = ExecuteMsg::SwapToken2ForToken1 {
            token2_amount: Uint128(10),
            min_token1: Uint128(7),
            expiration: None,
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.attributes.len(), 2);
        assert_eq!(res.attributes[0].value, "10");
        assert_eq!(res.attributes[1].value, "7");

        let info = get_info(deps.as_ref());
        assert_eq!(info.token2_reserve, Uint128(120));
        assert_eq!(info.token1_reserve, Uint128(84));

        // min_token error
        let info = mock_info("anyone", &vec![]);
        let msg = ExecuteMsg::SwapToken2ForToken1 {
            token2_amount: Uint128(10),
            min_token1: Uint128(100),
            expiration: None,
        };
        let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(
            err,
            ContractError::SwapMinError {
                min: Uint128(100),
                available: Uint128(6)
            }
        );

        // Expired Message
        let info = mock_info("anyone", &coins(100, "test"));
        let mut env = mock_env();
        env.block.height = 20;
        let msg = ExecuteMsg::SwapToken2ForToken1 {
            token2_amount: Uint128(10),
            min_token1: Uint128(100),
            expiration: Some(Expiration::AtHeight(19)),
        };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::MsgExpirationError {})
    }

    #[test]
    fn query_price() {
        let mut deps = mock_dependencies(&coins(2, "token"));

        let msg = InstantiateMsg {
            token1_denom: "test".to_string(),
            token1_address: None,
            token2_denom: "coin".to_string(),
            token2_address: Some(Addr::unchecked("asdf")),
        };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Add initial liquidity
        let info = mock_info("anyone", &coins(100, "test"));
        let msg = ExecuteMsg::AddLiquidity {
            token1_amount: Uint128(100),
            min_liquidity: Uint128(100),
            max_token2: Uint128(50),
            expiration: None,
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Query Native for Token Price
        let msg = QueryMsg::Token1ForToken2Price {
            token1_amount: Uint128(10),
        };
        let data = query(deps.as_ref(), mock_env(), msg).unwrap();
        let res: Token1ForToken2PriceResponse = from_binary(&data).unwrap();
        assert_eq!(res.token2_amount, Uint128(4));

        // Query Token for Native Price
        let msg = QueryMsg::Token2ForToken1Price {
            token2_amount: Uint128(10),
        };
        let data = query(deps.as_ref(), mock_env(), msg).unwrap();
        let res: Token2ForToken1PriceResponse = from_binary(&data).unwrap();
        assert_eq!(res.token1_amount, Uint128(16));
    }

    #[test]
    fn swap_native_for_token_to() {
        let mut deps = mock_dependencies(&coins(2, "token"));

        let msg = InstantiateMsg {
            token1_denom: "test".to_string(),
            token1_address: None,
            token2_denom: "coin".to_string(),
            token2_address: Some(Addr::unchecked("asdf")),
        };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Add initial liquidity
        let info = mock_info("anyone", &coins(100, "test"));
        let msg = ExecuteMsg::AddLiquidity {
            token1_amount: Uint128(100),
            min_liquidity: Uint128(100),
            max_token2: Uint128(100),
            expiration: None,
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Swap tokens
        let info = mock_info("anyone", &coins(10, "test"));
        let msg = ExecuteMsg::SwapTo {
            input_token: TokenSelect::Token1,
            input_amount: Uint128(10),
            recipient: Addr::unchecked("test"),
            min_token: Uint128(9),
            expiration: None,
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.attributes.len(), 2);
        assert_eq!(res.attributes[0].value, "10");
        assert_eq!(res.attributes[1].value, "9");

        let info = get_info(deps.as_ref());
        assert_eq!(info.token1_reserve, Uint128(110));
        assert_eq!(info.token2_reserve, Uint128(91));
    }
}
