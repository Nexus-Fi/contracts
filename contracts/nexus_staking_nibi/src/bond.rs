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

use crate::contract::slashing;
use crate::math::decimal_division;
use crate::state::{StakerInfo, CONFIG, CURRENT_BATCH, PARAMETERS, STAKERINFO, STATE};
use basset::hub::{BondType, Parameters};
use cosmwasm_std::{
    attr, to_binary, Coin, CosmosMsg, DepsMut, Env, MessageInfo, QueryRequest, Response,
    StakingMsg, StdError, StdResult, Uint128, WasmMsg, WasmQuery,
};
use cw20::Cw20ExecuteMsg;
use nexus_validator_registary::common::calculate_delegations;
use nexus_validator_registary::msg::QueryMsg as QueryValidators;
use nexus_validator_registary::registry::ValidatorResponse;
use serde::de::IntoDeserializer;

pub fn execute_bond(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    bond_type: BondType,
) -> Result<Response, StdError> {
    let params: Parameters = PARAMETERS.load(deps.storage)?;
    if params.paused.unwrap_or(false) {
        return Err(StdError::generic_err("the contract is temporarily paused"));
    }
    let epoch_period = params.clone().epoch_period;
    let unbondng_period = params.clone().unbonding_period;
    let coin_denom = params.underlying_coin_denom;
    let config = CONFIG.load(deps.storage)?;
    let restake_coin_denom = String::from("stNIBI");
    let reward_dispatcher_addr = config.reward_dispatcher_contract.ok_or_else(|| {
        StdError::generic_err("the reward dispatcher contract must have been registered")
    })?;

    if bond_type == BondType::BondRewards && info.sender != reward_dispatcher_addr {
        return Err(StdError::generic_err("unauthorized"));
    }

    // current batch requested fee is need for accurate exchange rate computation.
    let current_batch = CURRENT_BATCH.load(deps.storage)?;
    let requested_with_fee = current_batch.requested_stnibi;

    // coin must have be sent along with transaction and it should be in underlying coin denom
    if info.funds.len() > 1usize {
        return Err(StdError::generic_err(
            "More than one coin is sent; only one asset is supported",
        ));
    }

    // coin must have be sent along with transaction and it should be in underlying coin denom
    let payment = info
        .funds
        .iter()
        .find(|x| x.denom == coin_denom && x.amount > Uint128::zero())
        .ok_or_else(|| {
            StdError::generic_err(format!("No {} assets are provided to bond", coin_denom))
        })?;
        let time = env.clone().block.time.seconds();
    // check slashing
    let state = slashing(&mut deps, env)?;

    let sender = info.sender.clone();

    // get the total supply
    let mut total_supply = state.total_stnibi_issued;

    let mint_amount = match bond_type {
        BondType::stnibi => decimal_division(payment.amount, state.stnibi_exchange_rate),
        BondType::BondRewards => Uint128::zero(),
        BondType::Restake => todo!()
        
    };

    // total supply should be updated for exchange rate calculation.
    total_supply += mint_amount;

    // exchange rate should be updated for future
    STATE.update(deps.storage, |mut prev_state| -> StdResult<_> {
        match bond_type {
            BondType::BondRewards => {
                prev_state.total_bond_stnibi_amount += payment.amount;
                prev_state.update_stnibi_exchange_rate(total_supply, requested_with_fee);
                Ok(prev_state)
            }
            BondType::stnibi => {
                prev_state.total_bond_stnibi_amount += payment.amount;
                Ok(prev_state)
            },
            BondType::Restake => todo!()

        }
    })?;

    let validators_registry_contract = if let Some(v) = config.validators_registry_contract {
        v
    } else {
        return Err(StdError::generic_err(
            "Validators registry contract address is empty",
        ));
    };
    let validators: Vec<ValidatorResponse> =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: validators_registry_contract.to_string(),
            msg: to_binary(&QueryValidators::GetValidatorsForDelegation {})?,
        }))?;

    if validators.is_empty() {
        return Err(StdError::generic_err("Validators registry is empty"));
    }

    let delegations = calculate_delegations(payment.amount, validators.as_slice())?;

    let mut external_call_msgs: Vec<cosmwasm_std::CosmosMsg> = vec![];
    for i in 0..delegations.len() {
        if delegations[i].is_zero() {
            continue;
        }
        external_call_msgs.push(cosmwasm_std::CosmosMsg::Staking(StakingMsg::Delegate {
            validator: validators[i].address.clone(),
            amount: Coin::new(delegations[i].u128(), payment.denom.as_str()),
        }));
    }

    // we don't need to mint stnibi when bonding rewards
    if bond_type == BondType::BondRewards {
        let res = Response::new()
            .add_messages(external_call_msgs)
            .add_attributes(vec![
                attr("action", "bond_rewards"),
                attr("from", sender),
                attr("bonded", payment.amount),
            ]);
        return Ok(res);
    }

    let mint_msg = Cw20ExecuteMsg::Mint {
        recipient: sender.to_string(),
        amount: mint_amount,
    };

    let token_address = config
        .stnibi_token_contract
        .ok_or_else(|| StdError::generic_err("the token contract must have been registered"))?;

    external_call_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: token_address.to_string(),
        msg: to_binary(&mint_msg)?,
        funds: vec![],
    }));
    let staker_info = STAKERINFO.may_load(deps.storage, info.sender.clone().into_string())?;
   let new_staker_info = match staker_info {
        Some(d) =>{
            d
        },
        None =>{
            StakerInfo{
                amount_staked_unibi: payment.amount,
                amount_staked_stnibi: mint_amount,
                bonding_time: time.into(),
                epoch_period: epoch_period.into(),
                validator_list: validators,
            }
        }

    };

    let _ =STAKERINFO.save(deps.storage, info.sender.into_string(), &new_staker_info);
    let res = Response::new()
        .add_messages(external_call_msgs)
        .add_attributes(vec![
            attr("action", "mint"),
            attr("from", sender),
            attr("bonded", payment.amount),
            attr("minted", mint_amount),
        ]);
    Ok(res)
}
