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
use crate::state::{ validate_balance_update, BalanceAction, BalanceUpdate, BalanceUpdateData, BALANCE_UPDATES, CONFIG, CURRENT_BATCH, LAST_UPDATE_ID, PARAMETERS, STAKERINFO, STAKERINFO_NEW, STATE, TOKEN_SUPPLY};
use basset::hub::{BondType, Parameters,StakerInfo};
use cosmwasm_std::{
    attr, to_binary, Coin, CosmosMsg, Decimal, DepsMut, Env, MessageInfo, QueryRequest, Response, StakingMsg, StdError, StdResult, Storage, Uint128, Uint256, WasmMsg, WasmQuery
};
use cw20::Cw20ExecuteMsg;
use nexus_validator_registary::common::calculate_delegations;
use nexus_validator_registary::msg::QueryMsg as QueryValidators;
use nexus_validator_registary::registry::ValidatorResponse;



pub fn execute_bond(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    bond_type: BondType,
) -> Result<Response, StdError> {
    // Load states first
    let params: Parameters = PARAMETERS.load(deps.storage)?;
    let config = CONFIG.load(deps.storage)?;
    let current_batch = CURRENT_BATCH.load(deps.storage)?;
    let state = slashing(&mut deps, &env)?;
    
    // Initial validations
    if params.paused.unwrap_or(false) {
        return Err(StdError::generic_err("the contract is temporarily paused"));
    }

    let coin_denom = params.underlying_coin_denom;
    
    // Authorization check
    let reward_dispatcher_addr = config.reward_dispatcher_contract.ok_or_else(|| {
        StdError::generic_err("the reward dispatcher contract must have been registered")
    })?;

    if bond_type == BondType::BondRewards && info.sender != reward_dispatcher_addr {
        return Err(StdError::generic_err("unauthorized"));
    }

    // Validate payment
    if info.funds.len() > 1usize {
        return Err(StdError::generic_err(
            "More than one coin is sent; only one asset is supported",
        ));
    }

    let payment = info
        .funds
        .iter()
        .find(|x| x.denom == coin_denom && x.amount > Uint128::zero())
        .ok_or_else(|| {
            StdError::generic_err(format!("No {} assets are provided to bond", coin_denom))
        })?;

    // Calculate amounts
    let block_time = env.block.time.seconds();
    let sender_addr = info.sender.to_string();
    let mut total_supply = state.total_stnibi_issued;
    let mint_amount = match bond_type {
        BondType::stnibi => decimal_division(payment.amount, state.stnibi_exchange_rate),
        BondType::BondRewards => Uint128::zero(),
    };

    // Prepare all state updates first
    total_supply = total_supply.checked_add(mint_amount).or_else(|a| Err(StdError::generic_err("supply overflow")))?;
    
    // Update state
    STATE.update(deps.storage, |mut prev_state| -> StdResult<_> {
        match bond_type {
            BondType::BondRewards => {
                prev_state.total_bond_stnibi_amount = prev_state.total_bond_stnibi_amount
                    .checked_add(payment.amount)
                    .map_err(|_| StdError::generic_err("Bond amount overflow"))?;
                prev_state.update_stnibi_exchange_rate(total_supply, current_batch.requested_stnibi);
                Ok(prev_state)
            }
            BondType::stnibi => {
                prev_state.total_bond_stnibi_amount = prev_state.total_bond_stnibi_amount
                    .checked_add(payment.amount)
                    .map_err(|_| StdError::generic_err("Bond amount overflow"))?;
                prev_state.total_stnibi_issued = prev_state.total_stnibi_issued
                .checked_add(mint_amount)
                .map_err(|_| StdError::generic_err("Supply overflow"))?;
                Ok(prev_state)
            }
        }
    })?;
    
    // Update balances
    update_balances_for_bond(
        deps.storage,
        &sender_addr,
        payment.amount,
        mint_amount,
        block_time,
        state.stnibi_exchange_rate,
        env.block.height,
        None,
        &bond_type,
    )?;

    // After all state updates, prepare external messages
    let mut messages: Vec<CosmosMsg> = vec![];

    // Prepare delegation messages
    let validators_registry_contract = config.validators_registry_contract
        .ok_or_else(|| StdError::generic_err("Validators registry contract address is empty"))?;
    
    let validators: Vec<ValidatorResponse> =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: validators_registry_contract.to_string(),
            msg: to_binary(&QueryValidators::GetValidatorsForDelegation {})?,
        }))?;

    if validators.is_empty() {
        return Err(StdError::generic_err("Validators registry is empty"));
    }

    let delegations = calculate_delegations(payment.amount, validators.as_slice())?;

    // Add delegation messages
    for (i, amount) in delegations.iter().enumerate() {
        if amount.is_zero() {
            continue;
        }
        messages.push(CosmosMsg::Staking(StakingMsg::Delegate {
            validator: validators[i].address.clone(),
            amount: Coin::new(amount.u128(), payment.denom.as_str()),
        }));
    }

    // Handle rewards bonding separately
    if bond_type == BondType::BondRewards {
        return Ok(Response::new()
            .add_messages(messages)
            .add_attributes(vec![
                attr("action", "bond_rewards"),
                attr("from", sender_addr),
                attr("bonded", payment.amount),
            ]));
    }

    // Update token supply
    let supply_key = "";
    TOKEN_SUPPLY.update(
        deps.storage,
        supply_key,
        |token_supply: Option<Uint128>| -> StdResult<_> {
            match token_supply {
                Some(supply) => {
                    let new_supply = supply.checked_add(Uint128::from(mint_amount))
                        .or_else(|a| Err(StdError::generic_err("supply overflow")))?;
                    Ok(new_supply)
                }
                None => Ok(Uint128::from(mint_amount))
            }
        },
    )?;

    // Add mint message
    let token_address = config.stnibi_token_contract
        .ok_or_else(|| StdError::generic_err("the token contract must have been registered"))?;

    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: token_address.to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Mint {
            recipient: sender_addr.clone(),
            amount: mint_amount,
        })?,
        funds: vec![],
    }));

    Ok(Response::new()
        .add_messages(messages)
        .add_attributes(vec![
            attr("action", "mint"),
            attr("from", sender_addr),
            attr("bonded", payment.amount),
            attr("minted", mint_amount),
        ]))
}





pub fn update_balances_for_bond(
    storage: &mut dyn Storage,
    staker: &str,
    nibi_amount: Uint128,
    stnibi_amount: Uint128,
    timestamp: u64,
    exchange_rate: Decimal,
    block_height: u64,
    validator: Option<String>,
    bond_type: &BondType
) -> Result<(), StdError> {
    // 1. First load and validate all data
    let current_info = STAKERINFO_NEW.may_load(storage, staker)
        .map_err(|_| StdError::generic_err("StakerNotFound"))?;

    let update_data = BalanceUpdateData {
        nibi_amount,
        stnibi_amount,
        timestamp,
        exchange_rate,
        block_height,
        validator,
    };

    // 2. Prepare new state with validation
    let new_info = match current_info {
        Some(data) => {
            match bond_type {
                BondType::BondRewards => {
                    // Validate before any modifications
                    validate_balance_update(
                        &data,
                        update_data.nibi_amount,
                        update_data.stnibi_amount,
                        true,
                        update_data.timestamp,
                        update_data.exchange_rate,
                    )?;

                    StakerInfo {
                        amount_staked_unibi: data.amount_staked_unibi.checked_add(update_data.nibi_amount)
                            .or_else(|a| Err(StdError::generic_err("supply overflow")))?,
                        amount_stnibi_balance: data.amount_stnibi_balance,
                        bonding_time: data.bonding_time,
                        unbonding_period: data.unbonding_period,
                        validator_list: data.validator_list,
                        last_update_time: update_data.timestamp,
                    }
                },
                BondType::stnibi => {
                    validate_balance_update(
                        &data,
                        update_data.nibi_amount,
                        update_data.stnibi_amount,
                        true,
                        update_data.timestamp,
                        update_data.exchange_rate,
                    )?;

                    StakerInfo {
                        amount_staked_unibi: data.amount_staked_unibi.checked_add(update_data.nibi_amount)
                            .or_else(|a| Err(StdError::generic_err("Overflow when adding staked amount")))?,
                        amount_stnibi_balance: data.amount_stnibi_balance.checked_add(update_data.stnibi_amount)
                            .or_else(|a| Err(StdError::generic_err("Overflow when adding stnibi balance")))?,
                        bonding_time: data.bonding_time,
                        unbonding_period: data.unbonding_period,
                        validator_list: data.validator_list,
                        last_update_time: update_data.timestamp,
                    }
                }
            }
        },
        None => {
            // For new stakers, no validation needed
            StakerInfo {
                amount_staked_unibi: update_data.nibi_amount,
                amount_stnibi_balance: update_data.stnibi_amount,
                bonding_time: update_data.timestamp.into(),
                unbonding_period: None,
                validator_list: None,
                last_update_time: 0
            }
        }
    };

    

        let update_id = LAST_UPDATE_ID
        .may_load(storage, staker)?
        .unwrap_or_default()
        .checked_add(1)
        .ok_or_else(|| StdError::generic_err("Overflow when incrementing update ID"))?;

    // 4. Prepare balance update record
    let balance_update = BalanceUpdate {
        action: match bond_type {
            BondType::BondRewards => BalanceAction::BondRewards {
                nibi_amount: update_data.nibi_amount,
            },
            BondType::stnibi => BalanceAction::Bond {
                nibi_amount: update_data.nibi_amount,
                stnibi_minted: update_data.stnibi_amount,
                validator: update_data.validator,
            },
        },
        timestamp: update_data.timestamp,
        exchange_rate: update_data.exchange_rate,
        resulting_nibi_balance: new_info.amount_staked_unibi,
        resulting_stnibi_balance: new_info.amount_stnibi_balance,
        block_height: update_data.block_height,
    };

    // 5. Perform storage operations in order of importance
    // Save the core state first
    STAKERINFO_NEW.save(storage, staker, &new_info)?;
    
    // Then save the auxiliary data
    LAST_UPDATE_ID.save(storage, staker, &update_id)?;
    BALANCE_UPDATES.save(storage, (staker, update_id), &balance_update)?;

    Ok(())
}



