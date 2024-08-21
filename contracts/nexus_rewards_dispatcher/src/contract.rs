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

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    attr, to_binary, Attribute, BankMsg, Binary, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, Response, StdError, StdResult, Uint128, WasmMsg,
};

use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::state::{Config, CONFIG};
use basset::hub::{is_paused, ExecuteMsg::BondRewards};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let conf = Config {
        owner: info.sender,
        hub_contract: deps.api.addr_validate(&msg.hub_contract)?,
        stnibi_reward_denom: msg.stnibi_reward_denom,
        nexus_fee_address: deps.api.addr_validate(&msg.nexus_fee_address)?,
        nexus_fee_rate: msg.nexus_fee_rate,
    };

    CONFIG.save(deps.storage, &conf)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::DispatchRewards {} => execute_dispatch_rewards(deps, env, info),
        ExecuteMsg::UpdateConfig {
            owner,
            hub_contract,
            stnibi_reward_denom,
            nexus_fee_address,
            nexus_fee_rate,
        } => execute_update_config(
            deps,
            env,
            info,
            owner,
            hub_contract,
            stnibi_reward_denom,
            nexus_fee_address,
            nexus_fee_rate,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    owner: Option<String>,
    hub_contract: Option<String>,
    stnibi_reward_denom: Option<String>,
    nexus_fee_address: Option<String>,
    nexus_fee_rate: Option<Decimal>,
) -> StdResult<Response> {
    let conf: Config = CONFIG.load(deps.storage)?;
    let sender_raw = deps.api.addr_validate(info.sender.as_str())?;
    if sender_raw != conf.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    if let Some(o) = owner {
        let owner_raw = deps.api.addr_validate(&o)?;

        CONFIG.update(deps.storage, |mut last_config| -> StdResult<_> {
            last_config.owner = owner_raw;
            Ok(last_config)
        })?;
    }

    if let Some(h) = hub_contract {
        let hub_raw = deps.api.addr_validate(&h)?;

        CONFIG.update(deps.storage, |mut last_config| -> StdResult<_> {
            last_config.hub_contract = hub_raw;
            Ok(last_config)
        })?;
    }

    if let Some(_s) = stnibi_reward_denom {
        return Err(StdError::generic_err(
            "updating stnibi reward denom is forbidden",
        ));
    }

    if let Some(r) = nexus_fee_rate {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<_> {
            last_config.nexus_fee_rate = r;
            Ok(last_config)
        })?;
    }

    if let Some(a) = nexus_fee_address {
        let address_raw = deps.api.addr_validate(&a)?;

        CONFIG.update(deps.storage, |mut last_config| -> StdResult<_> {
            last_config.nexus_fee_address = address_raw;
            Ok(last_config)
        })?;
    }

    Ok(Response::default())
}

pub fn execute_dispatch_rewards(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;
    if is_paused(deps.as_ref(), config.hub_contract.clone().into_string())? {
        return Err(StdError::generic_err("the contract is temporarily paused"));
    }

    let hub_addr = config.hub_contract;
    if info.sender != hub_addr {
        return Err(StdError::generic_err("unauthorized"));
    }
    
    let contr_addr = env.contract.address;
    let mut stnibi_rewards = deps
        .querier
        .query_balance(contr_addr, config.stnibi_reward_denom.clone())?;

    let nexus_stnibi_fee_amount = compute_nexus_fee(stnibi_rewards.amount, config.nexus_fee_rate);
    stnibi_rewards.amount = stnibi_rewards.amount.checked_sub(nexus_stnibi_fee_amount)?;

    let mut fees_attrs: Vec<Attribute> = vec![];

    let mut nexus_fees: Vec<Coin> = vec![];
    if !nexus_stnibi_fee_amount.is_zero() {
        let stnibi_fee = Coin {
            amount: nexus_stnibi_fee_amount,
            denom: config.stnibi_reward_denom.clone(),
        };
        nexus_fees.push(stnibi_fee.clone());
        fees_attrs.push(attr("nexus_stnibi_fee", stnibi_fee.to_string()));
    }
    let mut messages: Vec<CosmosMsg> = vec![];
    if !stnibi_rewards.amount.is_zero() {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: hub_addr.to_string(),
            msg: to_binary(&BondRewards {}).unwrap(),
            funds: vec![stnibi_rewards.clone()],
        }));
    }
    if !nexus_fees.is_empty() {
        messages.push(
            BankMsg::Send {
                to_address: config.nexus_fee_address.to_string(),
                amount: nexus_fees,
            }
            .into(),
        )
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attributes(vec![
            attr("action", "claim_reward"),
            attr("stnibi_rewards", stnibi_rewards.to_string()),
        ])
        .add_attributes(fees_attrs))
}

fn query_config(deps: Deps) -> StdResult<Config> {
    let config = CONFIG.load(deps.storage)?;
    Ok(config)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::GetBufferedRewards {} => unimplemented!(),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}

pub fn compute_nexus_fee(amount: Uint128, fee_rate: Decimal) -> Uint128 {
    amount * fee_rate
}
