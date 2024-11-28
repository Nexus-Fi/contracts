#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    coin, to_binary, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult, Uint128, WasmMsg
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::msg::{InstantiateMsg,ExecuteMsg} ;
use crate::state::{State,STATE,USER_INFO,UserInfo};
#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let state = State {
        stnibi_denom: msg.stnibi_denom,
    };
    STATE.save(deps.storage, &state)?;
    Ok(Response::new().add_attribute("method", "instantiate"))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Lock {} => execute_lock(deps, env, info),
        ExecuteMsg::Unlock { amount } => execute_unlock(deps, env, info, amount),
    }
}


pub fn execute_lock(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> StdResult<Response> {
    let state = STATE.load(deps.storage)?;
    let stnibi_amount = info
        .funds
        .iter()
        .find(|c| c.denom == state.stnibi_denom)
        .ok_or_else(|| StdError::generic_err("No stNIBI token sent"))?
        .amount;

    let sender = info.sender.to_string();
    let mut user_info = USER_INFO.may_load(deps.storage, &sender)?.unwrap_or(UserInfo { locked_amount: Uint128::zero() });
    user_info.locked_amount += stnibi_amount;
    USER_INFO.save(deps.storage, &sender, &user_info)?;

    Ok(Response::new()
        .add_attribute("action", "lock")
        .add_attribute("user", sender)
        .add_attribute("amount", stnibi_amount.to_string()))
}

pub fn execute_unlock(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> StdResult<Response> {
    let state = STATE.load(deps.storage)?;
    let sender = info.sender.to_string();
    let mut user_info = USER_INFO.load(deps.storage, &sender)?;
    
    if user_info.locked_amount < amount {
        return Err(StdError::generic_err("Insufficient locked balance"));
    }

    user_info.locked_amount -= amount;
    USER_INFO.save(deps.storage, &sender, &user_info)?;

    let msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: sender.clone(),
        amount: vec![Coin {
            denom: state.stnibi_denom,
            amount,
        }],
    });

    Ok(Response::new()
        .add_message(msg)
        .add_attribute("action", "unlock")
        .add_attribute("user", sender)
        .add_attribute("amount", amount.to_string()))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetLockedAmount { user } => to_binary(&query_locked_amount(deps, user)?),
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GetLockedAmount { user: String },
}

fn query_locked_amount(deps: Deps, user: String) -> StdResult<Uint128> {
    let user_info = USER_INFO.may_load(deps.storage, &user)?.unwrap_or(UserInfo { locked_amount: Uint128::zero() });
    Ok(user_info.locked_amount)
}