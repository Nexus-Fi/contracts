// Copyright 2021 Anchor Protocol. Modified by nexus
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

use crate::state::{CONFIG, PARAMETERS};
use basset::hub::Parameters;
use cosmwasm_std::{
    attr, CosmosMsg, DepsMut, DistributionMsg, Env, MessageInfo, Response, StdError, StdResult,
};

/// Update general parameters
/// Only creator/owner is allowed to execute
#[allow(clippy::too_many_arguments)]
pub fn execute_update_params(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    epoch_period: Option<u64>,
    unbonding_period: Option<u64>,
) -> StdResult<Response> {
    // only owner can send this message.
    let config = CONFIG.load(deps.storage)?;
    let sender_raw = info.sender;
    if sender_raw != config.creator {
        return Err(StdError::generic_err("unauthorized"));
    }

    let params: Parameters = PARAMETERS.load(deps.storage)?;

    let new_params = Parameters {
        epoch_period: epoch_period.unwrap_or(params.epoch_period),
        underlying_coin_denom: params.underlying_coin_denom,
        unbonding_period: unbonding_period.unwrap_or(params.unbonding_period),
        paused: params.paused,
    };

    PARAMETERS.save(deps.storage, &new_params)?;

    let res = Response::new().add_attributes(vec![attr("action", "update_params")]);
    Ok(res)
}

#[allow(clippy::too_many_arguments)]
/// Update the config. Update the owner, reward and token contracts.
/// Only creator/owner is allowed to execute
pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    owner: Option<String>,
    rewards_dispatcher_contract: Option<String>,
    // stnibi_token_contract: Option<String>,
    validators_registry_contract: Option<String>,
    stnibi_denom:Option<String>
) -> StdResult<Response> {
    // only owner must be able to send this message.
    let conf = CONFIG.load(deps.storage)?;
    let sender_raw = info.sender;
    if sender_raw != conf.creator {
        return Err(StdError::generic_err("unauthorized"));
    }

    let mut messages: Vec<CosmosMsg> = vec![];

    if let Some(o) = owner {
        let owner_raw = deps.api.addr_validate(&o)?;

        CONFIG.update(deps.storage, |mut last_config| -> StdResult<_> {
            last_config.creator = owner_raw;
            Ok(last_config)
        })?;
    }
    if let Some(reward) = rewards_dispatcher_contract {
        let reward_raw = deps.api.addr_validate(&reward)?;

        CONFIG.update(deps.storage, |mut last_config| -> StdResult<_> {
            last_config.reward_dispatcher_contract = Some(reward_raw);
            Ok(last_config)
        })?;

        // register the reward contract for automate reward withdrawal.
        let msg: CosmosMsg =
            CosmosMsg::Distribution(DistributionMsg::SetWithdrawAddress { address: reward });
        messages.push(msg);
    }

    // if let Some(token) = stnibi_token_contract {
    //     let token_raw = deps.api.addr_validate(&token)?;

    //     CONFIG.update(deps.storage, |mut last_config| -> StdResult<_> {
    //         // if last_config.rstnibi_token_contract.is_some() {
    //         //     return Err(StdError::generic_err(
    //         //         "updating stnibi token address is forbidden",
    //         //     ));
    //         // }

    //         last_config.stnibi_token_contract = Some(token_raw);
    //         Ok(last_config)
    //     })?;
    // }
    
    if let Some(token) = stnibi_denom {
        // let token_raw = deps.api.addr_validate(&token)?;

        CONFIG.update(deps.storage, |mut last_config| -> StdResult<_> {
            if last_config.stnibi_denom.is_some() {
                return Err(StdError::generic_err(
                    "updating stnibi token address is forbidden",
                ));
            }

            last_config.stnibi_denom = Some(token);
            Ok(last_config)
        })?;
    }
    
    if let Some(validators_registry) = validators_registry_contract {
        let validators_raw = deps.api.addr_validate(&validators_registry)?;
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<_> {
            last_config.validators_registry_contract = Some(validators_raw);
            Ok(last_config)
        })?;
    }

    let res = Response::new()
        .add_messages(messages)
        .add_attributes(vec![attr("action", "update_config")]);
    Ok(res)
}
