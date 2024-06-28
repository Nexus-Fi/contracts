// Copyright 2021 nexus
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::env::consts::EXE_EXTENSION;

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{attr, to_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult, SubMsg, Uint128, WasmMsg};

use cw20_base::allowances::{execute_decrease_allowance, execute_increase_allowance};
use cw20_base::contract::instantiate as cw20_init;
use cw20_base::contract::query as cw20_query;
use cw20_base::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};

use crate::handler::*;
use crate::msg::TokenInitMsg;
use crate::state::HUB_CONTRACT;
use basset::hub::{is_paused, Cw20HookMsg,ExecuteMsg as HubExecutemsg};
use cw20::{Cw20ReceiveMsg, MinterResponse};
use cw20_base::ContractError;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: TokenInitMsg,
) -> Result<Response, ContractError> {
    HUB_CONTRACT.save(deps.storage, &deps.api.addr_validate(&msg.hub_contract)?)?;

    cw20_init(
        deps,
        env,
        info,
        InstantiateMsg {
            name: msg.name,
            symbol: msg.symbol,
            decimals: msg.decimals,
            initial_balances: msg.initial_balances,
            mint: Some(MinterResponse {
                minter: msg.hub_contract.clone(),
                cap: None,
            }),
            marketing: msg.marketing,
        },
    )?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let hub_addr: Addr = HUB_CONTRACT.load(deps.storage)?;
    if is_paused(deps.as_ref(), hub_addr.into_string())? {
        return Err(ContractError::Std(StdError::generic_err(
            "The contract is temporarily paused",
        )));
    }

    match msg {
        ExecuteMsg::Transfer { recipient, amount } => {
            execute_transfer(deps, env, info, recipient, amount)
        }
        ExecuteMsg::Burn { amount } => execute_burn(deps, env, info, amount),
        ExecuteMsg::Send {
            contract,
            amount,
            msg,
        } => execute_send(deps, env, info, contract, amount, msg),
        ExecuteMsg::Mint { recipient, amount } => execute_mint(deps, env, info, recipient, amount),
        ExecuteMsg::IncreaseAllowance {
            spender,
            amount,
            expires,
        } => restake_stnibi_token(deps, env, info, amount),
        ExecuteMsg::DecreaseAllowance {
            spender,
            amount,
            expires,
        } => execute_decrease_allowance(deps, env, info, spender, amount, expires),
        ExecuteMsg::TransferFrom {
            owner,
            recipient,
            amount,
        } => execute_transfer_from(deps, env, info, owner, recipient, amount),
        ExecuteMsg::BurnFrom { owner, amount } => execute_burn_from(deps, env, info, owner, amount),
        ExecuteMsg::SendFrom {
            owner,
            contract,
            amount,
            msg,
        } => transfer_stnibi_token(deps, env, info,amount),
        ExecuteMsg::UpdateMarketing {
            project,
            /// A longer description of the token and it's utility. Designed for tooltips or such
            description,
            /// The address (if any) who can update this data structure
            marketing,
        } => execute_update_marketing(deps, env, info, project, description, marketing),
        ExecuteMsg::UploadLogo(logo) => execute_upload_logo(deps, env, info, logo),
        ExecuteMsg::UpdateMinter { new_minter } => todo!()
    }
}

pub fn transfer_stnibi_token( 
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    amount:Uint128
)->Result<Response,ContractError> 
{
    let hub_contract = HUB_CONTRACT.load(deps.storage)?;
    let mut messages: Vec<CosmosMsg> = vec![];
    let msg_cw20_recieve = Cw20ReceiveMsg{
        sender:info.sender.clone().into_string(),
        amount:amount,
        msg:to_binary(&Cw20HookMsg::Unbond { })?
    }   ;

    // let transfer_stnibi_msg:CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
    //     contract_addr: env.contract.address.to_string(),
    //     msg: to_binary(&ExecuteMsg::Transfer {
    //         recipient: hub_contract.to_string(),
    //         amount: amount,
    //     })?,
    //     funds: vec![],
    // });
    let message = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: hub_contract.to_string(),
        msg: to_binary(&HubExecutemsg::Receive(msg_cw20_recieve))?,
        funds: vec![],
    });
    // messages.push(transfer_stnibi_msg);

    messages.push(message);
    let res = Response::new()
    .add_messages(messages)
    .add_attributes(vec![attr("action", "receive")]);
    Ok(res)
}

pub fn restake_stnibi_token( 
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    amount:Uint128
)->Result<Response,ContractError> 
{
    let hub_contract = HUB_CONTRACT.load(deps.storage)?;
    let mut messages: Vec<CosmosMsg> = vec![];
    let msg_cw20_recieve = Cw20ReceiveMsg{
        sender:info.sender.clone().into(),
        amount:amount,
        msg:to_binary(&Cw20HookMsg::Restake { })?
    }   ;

    let message = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: hub_contract.to_string(),
        msg: to_binary(&HubExecutemsg::Restake{cwmsg:msg_cw20_recieve})?,
        funds: vec![],
    });
    
    // let transfer_stnibi_msg:CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
    //     contract_addr: env.contract.address.to_string(),
    //     msg: to_binary(&ExecuteMsg::Transfer {
    //         recipient: hub_contract.to_string(),
    //         amount: amount,
    //     })?,
    //     funds: vec![],
    // });
    // messages.push(transfer_stnibi_msg);

    messages.push(message);
    let res = Response::new()
    .add_messages(messages)
    .add_attributes(vec![attr("action", "restake")]);
    Ok(res)
}


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    cw20_query(deps, env, msg)
}
