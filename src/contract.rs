use std::path::Prefix::DeviceNS;
use cosmwasm_std::{
    attr, entry_point, to_binary, Addr, Binary, BlockInfo, Coin, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, Reply, Response, StdError, StdResult, SubMsg, Uint128, WasmMsg,
};
use cw0::parse_reply_instantiate_data;
use cw20::{Cw20ExecuteMsg, Denom, Expiration, MinterResponse};
use cw20::Denom::Cw20;
use cw20_base::contract::query_balance;

use crate::error::ContractError;
use crate::msg::{
    ExecuteMsg, InfoResponse, InstantiateMsg, QueryMsg, Token1ForToken2PriceResponse,
    Token2ForToken1PriceResponse, TokenSelect,
};
use crate::state::{Token, LP_TOKEN, TOKEN1, TOKEN2};
use cw_storage_plus::Item;

const INSTANTIATE_LP_TOKEN_REPLY_ID: u64 = 0;
// Note, you can use StdResult in some functions where you do not
// make use of the custom errors
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let token1 = Token {
        reserve: Uint128::zero(),
        denom: msg.token1_denom,
    };

    TOKEN1.save(deps.storage, &token1)?;

    let token2 = Token {
        denom: msg.token2_denom,
        reserve: Uint128::zero(),
    };

    TOKEN2.save(deps.storage, &token2)?;

    let instantiate_lp_token_msg = WasmMsg::Instantiate {
        code_id: msg.lp_token_code_id,
        funds: vec![],
        admin: None,
        label: "lp_token".to_string(),
        msg: to_binary(&cw20_stakeable::msg::InstantiateMsg {
            cw20_base: cw20_base::msg::InstantiateMsg {
                name: "CRUST_LIQUIDITY_TOKEN".into(),
                symbol: "CRUST".into(),
                decimals: 18,
                initial_balances: vec![],
                mint: Some(MinterResponse {
                    minter: _env.contract.address.into(),
                    cap: None,
                }),
                marketing: None,
            },
            unstaking_duration: None,
        })?,
    };

    let reply_msg =
        SubMsg::reply_on_success(instantiate_lp_token_msg, INSTANTIATE_LP_TOKEN_REPLY_ID);

    Ok(Response::new().add_submessage(reply_msg))
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
        ExecuteMsg::MultiContractSwap {
            output_amm_address,
            input_token,
            output_token,
            input_token_amount,
            output_min_token,
            expiration,
        } => execute_multi_contract_swap(
            deps,
            info,
            _env,
            output_amm_address,
            input_token,
            input_token_amount,
            output_token,
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
            match input_token {
                TokenSelect::Token1 => TOKEN1,
                TokenSelect::Token2 => TOKEN2,
            },
            match input_token {
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
    if liquidity_supply == Uint128::zero() {
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
    if liquidity_supply == Uint128::zero() {
        Ok(max_token)
    } else {
        Ok(native_amount
            .checked_mul(token_reserve)
            .map_err(StdError::overflow)?
            .checked_div(native_reserve)
            .map_err(StdError::divide_by_zero)?
            .checked_add(Uint128::new(1))
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
    let lp_token_addr = LP_TOKEN.load(deps.storage)?;

    // validate funds
    validate_input_amount(&info.funds, token1_amount, &token1.denom)?;
    validate_input_amount(&info.funds, max_token2, &token2.denom)?;

    let lp_token_supply = get_lp_token_supply(deps.as_ref(), &lp_token_addr)?;
    let liquidity_amount = get_liquidity_amount(token1_amount, lp_token_supply, token1.reserve)?;

    let token_amount = get_token_amount(
        max_token2,
        token1_amount,
        lp_token_supply,
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
    if let Cw20(addr) = token1.denom {
        cw20_transfer_msgs.push(get_cw20_transfer_from_msg(
            &info.sender,
            &_env.contract.address,
            &addr,
            token1_amount,
        )?)
    }
    if let Cw20(addr) = token2.denom {
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

    let mint_msg = mint_lp_tokens(&info.sender, liquidity_amount, &lp_token_addr)?;

    Ok(Response::new()
        .add_messages(cw20_transfer_msgs)
        .add_message(mint_msg)
        .add_attributes(vec![
            attr("native_amount", info.funds[0].clone().amount),
            attr("token_amount", token_amount),
            attr("liquidity_received", liquidity_amount),
        ]))
}

fn get_lp_token_supply(deps: Deps, lp_token_addr: &Addr) -> StdResult<Uint128> {
    let resp: cw20::TokenInfoResponse = deps
        .querier
        .query_wasm_smart(lp_token_addr, &cw20_base::msg::QueryMsg::TokenInfo {})?;
    Ok(resp.total_supply)
}

fn mint_lp_tokens(
    recipient: &Addr,
    liquidity_amount: Uint128,
    lp_token_address: &Addr,
) -> StdResult<CosmosMsg> {
    let mint_msg = cw20_base::msg::ExecuteMsg::Mint {
        recipient: recipient.into(),
        amount: liquidity_amount,
    };
    Ok(WasmMsg::Execute {
        contract_addr: lp_token_address.to_string(),
        msg: to_binary(&mint_msg)?,
        funds: vec![],
    }
    .into())
}

fn get_token_balance(deps: Deps, contract: &Addr, addr: &Addr) -> StdResult<Uint128> {
    let resp: cw20::BalanceResponse = deps.querier.query_wasm_smart(
        contract,
        &cw20_base::msg::QueryMsg::Balance {
            address: addr.to_string(),
        },
    )?;
    Ok(resp.balance)
}

fn validate_input_amount(
    actual_funds: &[Coin],
    given_amount: Uint128,
    given_denom: &Denom,
) -> Result<(), ContractError> {
    match given_denom {
        Denom::Cw20(_) => Ok(()),
        Denom::Native(denom) => {let actual = get_amount_for_denom(actual_funds, &denom);
        if actual.amount != given_amount {
            return Err(ContractError::InsufficientFunds {
        });
    }
    if &actual.denom != denom {
        return Err(ContractError::IncorrectNativeDenom {
            provided: actual.denom,
            required: denom.to_string(),
        });
    };
    Ok(())}
}
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
        funds: vec![],
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
        funds: vec![],
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

    let lp_token_addr = LP_TOKEN.load(deps.storage)?;
    let balance = get_token_balance(deps.as_ref(), &lp_token_addr, &info.sender)?;
    let lp_token_supply = get_lp_token_supply(deps.as_ref(), &lp_token_addr)?;
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
        .checked_div(lp_token_supply)
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
        .checked_div(lp_token_supply)
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

    let token1_transfer_msg = match token1.denom {
        Denom::Cw20(addr) => get_cw20_transfer_to_msg(&info.sender, &addr, native_amount)?,
        Denom::Native(denom)=> get_bank_transfer_to_msg(&info.sender, &denom, native_amount),
    };
    let token2_transfer_msg = match token2.denom {
        Denom::Cw20(addr) => get_cw20_transfer_to_msg(&info.sender, &addr, token_amount)?,
        Denom::Native(denom) => get_bank_transfer_to_msg(&info.sender, &denom, token_amount),
    };

    let lp_token_burn_msg = get_burn_msg(&lp_token_addr, &info.sender, amount)?;

    Ok(Response::new()
        .add_messages(vec![
            token1_transfer_msg,
            token2_transfer_msg,
            lp_token_burn_msg,
        ])
        .add_attributes(vec![
            attr("liquidity_burned", amount),
            attr("native_returned", native_amount),
            attr("token_returned", token_amount),
        ]))
}

fn get_burn_msg(contract: &Addr, owner: &Addr, amount: Uint128) -> StdResult<CosmosMsg> {
    let msg = cw20_base::msg::ExecuteMsg::BurnFrom {
        owner: owner.to_string(),
        amount,
    };
    Ok(WasmMsg::Execute {
        contract_addr: contract.to_string(),
        msg: to_binary(&msg)?,
        funds: vec![],
    }
    .into())
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
        funds: vec![],
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
    if input_reserve == Uint128::zero() || output_reserve == Uint128::zero() {
        return Err(ContractError::NoLiquidityError {});
    };

    let input_amount_with_fee = input_amount
        .checked_mul(Uint128::new(997))
        .map_err(StdError::overflow)?;
    let numerator = input_amount_with_fee
        .checked_mul(output_reserve)
        .map_err(StdError::overflow)?;
    let denominator = input_reserve
        .checked_mul(Uint128::new(1000))
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
    validate_input_amount(&info.funds, input_amount, &input_token.denom)?;

    let token_bought = get_input_price(input_amount, input_token.reserve, output_token.reserve)?;

    if min_token > token_bought {
        return Err(ContractError::SwapMinError {
            min: min_token,
            available: token_bought,
        });
    }

    // Create transfer from message
    let mut transfer_msgs = match input_token.denom {
        Denom::Cw20(addr) => vec![get_cw20_transfer_from_msg(
            &info.sender,
            &_env.contract.address,
            &addr,
            input_amount,
        )?],
        Denom::Native(_) => vec![],
    };

    // Create transfer to message
    transfer_msgs.push(match output_token.denom {
        Denom::Cw20(addr) => get_cw20_transfer_to_msg(recipient, &addr, token_bought)?,
        Denom::Native(denom) => get_bank_transfer_to_msg(recipient, &denom, token_bought),
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

    Ok(Response::new()
        .add_messages(transfer_msgs)
        .add_attributes(vec![
            attr("native_sold", input_amount),
            attr("token_bought", token_bought),
        ]))
}

#[allow(clippy::too_many_arguments)]
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

    validate_input_amount(&info.funds, input_token_amount, &input_token.denom)?;

    let amount_to_transfer = get_input_price(
        input_token_amount,
        input_token.reserve,
        transfer_token.reserve,
    )?;

    // Transfer tokens to contract
    let mut msgs: Vec<CosmosMsg> = vec![];
    if let Denom::Cw20(addr) = &input_token.denom {
        msgs.push(get_cw20_transfer_from_msg(
            &info.sender,
            &_env.contract.address,
            addr,
            input_token_amount,
        )?)
    };

    // Increase allowance of output contract is transfer token is cw20
    if let Denom::Cw20(addr) = &transfer_token.denom {
        msgs.push(get_cw20_increase_allowance_msg(
            addr,
            &output_amm_address,
            amount_to_transfer,
            Some(Expiration::AtHeight(_env.block.height + 1)),
        )?)
    };

    let swap_msg = ExecuteMsg::SwapTo {
        input_token: match output_token_enum {
            TokenSelect::Token1 => TokenSelect::Token2,
            TokenSelect::Token2 => TokenSelect::Token1,
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
            funds: match transfer_token.denom {
                Denom::Cw20(_) => vec![],
                Denom::Native(denom) => vec![Coin {
                    denom,
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

    Ok(Response::new().add_messages(msgs).add_attributes(vec![
        attr("input_token_amount", input_token_amount),
        attr("native_transferred", amount_to_transfer),
    ]))
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
    let lp_token_address = LP_TOKEN.load(deps.storage)?.to_string();
    // TODO get total supply
    Ok(InfoResponse {
        token1_reserve: token1.reserve,
        token1_denom: token1.denom,
        token2_reserve: token2.reserve,
        token2_denom: token2.denom,
        lp_token_supply: Uint128::new(100),
        lp_token_address,
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

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    if msg.id != INSTANTIATE_LP_TOKEN_REPLY_ID {
        return Err(ContractError::UnknownReplyId { id: msg.id });
    };
    let res = parse_reply_instantiate_data(msg);
    match res {
        Ok(res) => {
            // Validate contract address
            let cw20_addr = deps.api.addr_validate(&res.contract_address)?;

            // Save gov token
            LP_TOKEN.save(deps.storage, &cw20_addr)?;

            Ok(Response::new())
        }
        Err(_) => Err(ContractError::InstantiateLpTokenError {}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_liquidity_amount() {
        let liquidity =
            get_liquidity_amount(Uint128::new(100), Uint128::zero(), Uint128::zero()).unwrap();
        assert_eq!(liquidity, Uint128::new(100));

        let liquidity =
            get_liquidity_amount(Uint128::new(100), Uint128::new(50), Uint128::new(25)).unwrap();
        assert_eq!(liquidity, Uint128::new(200));
    }

    #[test]
    fn test_get_token_amount() {
        let liquidity = get_token_amount(
            Uint128::new(100),
            Uint128::new(50),
            Uint128::zero(),
            Uint128::zero(),
            Uint128::zero(),
        )
        .unwrap();
        assert_eq!(liquidity, Uint128::new(100));

        let liquidity = get_token_amount(
            Uint128::new(200),
            Uint128::new(50),
            Uint128::new(50),
            Uint128::new(100),
            Uint128::new(25),
        )
        .unwrap();
        assert_eq!(liquidity, Uint128::new(201));
    }

    #[test]
    fn test_get_input_price() {
        // Base case
        assert_eq!(
            get_input_price(Uint128::new(10), Uint128::new(100), Uint128::new(100)).unwrap(),
            Uint128::new(9)
        );

        // No input reserve error
        let err =
            get_input_price(Uint128::new(10), Uint128::new(0), Uint128::new(100)).unwrap_err();
        assert_eq!(err, ContractError::NoLiquidityError {});

        // No output reserve error
        let err =
            get_input_price(Uint128::new(10), Uint128::new(100), Uint128::new(0)).unwrap_err();
        assert_eq!(err, ContractError::NoLiquidityError {});

        // No reserve error
        let err = get_input_price(Uint128::new(10), Uint128::new(0), Uint128::new(0)).unwrap_err();
        assert_eq!(err, ContractError::NoLiquidityError {});
    }
}
