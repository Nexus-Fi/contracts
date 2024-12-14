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
use crate::error::BalanceError;
use crate::math::decimal_division;
use crate::state::{ validate_balance_update, BalanceAction, BalanceUpdate, BALANCE_UPDATES, CONFIG, CURRENT_BATCH, LAST_UPDATE_ID, PARAMETERS, STAKERINFO, STAKERINFO_NEW, STATE, TOKEN_SUPPLY};
use basset::hub::{BondType, Parameters,StakerInfo};
use cosmwasm_std::{
    attr, to_binary, Coin, CosmosMsg, Decimal, DepsMut, Env, MessageInfo, QueryRequest, Response, StakingMsg, StdError, StdResult, Storage, Uint128, Uint256, WasmMsg, WasmQuery
};
use cw20::Cw20ExecuteMsg;
use nexus_validator_registary::common::calculate_delegations;
use nexus_validator_registary::msg::QueryMsg as QueryValidators;
use nexus_validator_registary::registry::ValidatorResponse;
use nibiru_std::proto::{cosmos, nibiru, NibiruStargateMsg};

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

    let coin_denom = params.underlying_coin_denom;
    let config = CONFIG.load(deps.storage)?;

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
    let state = slashing(&mut deps, env.clone())?;

    let sender = info.sender.clone();

    // get the total supply
    let mut total_supply = state.total_stnibi_issued;

    let mint_amount = match bond_type {
        BondType::stnibi => decimal_division(payment.amount, state.stnibi_exchange_rate),
        BondType::BondRewards => Uint128::zero(),
    };

    // total supply should be updated for exchange rate calculation.
    total_supply += mint_amount;


    let a = update_balances_for_bond(
        deps.storage,
        info.sender.as_str(),
        payment.amount,
        mint_amount,
        env.clone().block.time.seconds(),
        state.stnibi_exchange_rate,
        env.block.height,
        None, // or pass validator if you have it
    );
    


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
            }
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
        let supply_key ="";
        // update token supply 
        let token_supply =
    TOKEN_SUPPLY.may_load(deps.storage, supply_key)?;
    match token_supply {
        Some(supply) => {
            let new_supply = supply + Uint128::from(mint_amount);
            total_supply += mint_amount;
            TOKEN_SUPPLY.save(deps.storage, supply_key, &new_supply)
        }?,
        None => {
            total_supply = mint_amount; 
            TOKEN_SUPPLY.save(
            deps.storage,
            supply_key,
            &Uint128::from(mint_amount),
        )?
    }
    }

        let token_address = config
            .stnibi_token_contract
            .ok_or_else(|| StdError::generic_err("the token contract must have been registered"))?;

        external_call_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: token_address.to_string(),
            msg: to_binary(&mint_msg)?,
            funds: vec![],
        }));

        // update staker info 
        let staker_info = STAKERINFO.may_load(deps.storage, info.sender.clone().into_string())?;
        let new_staker_info = match staker_info {
            Some(mut d) =>{
                    d.amount_staked_unibi += payment.amount;
                    d.amount_stnibi_balance += mint_amount;
                    d            
            },
            None =>{
                StakerInfo{
                    amount_staked_unibi: payment.amount,
                    amount_stnibi_balance: mint_amount,
                    bonding_time: time.into(),
                    unbonding_period:None,
                    validator_list: None,
                    last_update_time:0 // need to update
                }
            }
    
        };
        let _  = STAKERINFO.save(deps.storage, info.sender.into_string().clone(),&new_staker_info );

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




// Enhanced update functions with validation
pub fn update_balances_for_bond(
    storage: &mut dyn Storage,
    staker: &str,
    nibi_amount: Uint128,
    stnibi_amount: Uint128,
    timestamp: u64,
    exchange_rate: Decimal,
    block_height: u64,
    validator: Option<String>,
) -> Result<(), BalanceError> {
    let old_info = STAKERINFO_NEW.may_load(storage, staker)
        .map_err(|_| BalanceError::StakerNotFound {})?;

    match old_info{
        Some(data) =>{
            // Validate the update
                validate_balance_update(
                    &data,
                    nibi_amount,
                    stnibi_amount,
                    true,
                    timestamp,
                    exchange_rate,
                )?;

                let new_info = StakerInfo {
                    amount_staked_unibi: data.amount_staked_unibi + nibi_amount,
                    amount_stnibi_balance: data.amount_stnibi_balance + stnibi_amount,
                    bonding_time: data.bonding_time,
                    unbonding_period: data.unbonding_period,
                    validator_list: data.validator_list,
                    last_update_time: timestamp,
                };

                let update_id = LAST_UPDATE_ID
        .may_load(storage, staker)?
        .unwrap_or_default() + 1;

        let update = BalanceUpdate {
            action: BalanceAction::Bond {
                nibi_amount,
                stnibi_minted: stnibi_amount,
                validator,
            },
            timestamp,
            exchange_rate,
            resulting_nibi_balance: new_info.amount_staked_unibi,
            resulting_stnibi_balance: new_info.amount_stnibi_balance,
            block_height,
        };
       let _=  STAKERINFO_NEW.save(storage, staker, &new_info)?;
        BALANCE_UPDATES.save(storage, (staker, update_id), &update)?;
        LAST_UPDATE_ID.save(storage, staker, &update_id)?;

        },
        None=>{
          let staker_info =  StakerInfo{
                amount_staked_unibi: nibi_amount,
                amount_stnibi_balance: stnibi_amount,
                bonding_time: timestamp.into(),
                unbonding_period:None,
                validator_list: None,
                last_update_time:0 // need to update
            };

            let update_id = LAST_UPDATE_ID
            .may_load(storage, staker)?
            .unwrap_or_default() + 1;
        let update = BalanceUpdate {
            action: BalanceAction::Bond {
                nibi_amount,
                stnibi_minted: stnibi_amount,
                validator,
            },
            timestamp,
            exchange_rate,
            resulting_nibi_balance: staker_info.amount_staked_unibi,
            resulting_stnibi_balance: staker_info.amount_stnibi_balance,
            block_height,
        };
       let _=  STAKERINFO_NEW.save(storage, staker, &staker_info)?;
        BALANCE_UPDATES.save(storage, (staker, update_id), &update)?;
        LAST_UPDATE_ID.save(storage, staker, &update_id)?;

        }
    }

    
    // // Validate the update
    // validate_balance_update(
    //     &old_info,
    //     nibi_amount,
    //     stnibi_amount,
    //     true,
    //     timestamp,
    //     exchange_rate,
    // )?;

    // Update staker info
    

    // Record the update
    

    
  

    Ok(())
}



// Migration function
pub fn migrate_staker_balances(
    storage: &mut dyn Storage,
    staker: &str,
    old_info: StakerInfo,
    block_height: u64,
    timestamp: u64,
) -> StdResult<Response> {
    let new_info = StakerInfo {
        amount_staked_unibi: old_info.amount_staked_unibi,
        amount_stnibi_balance: old_info.amount_stnibi_balance,
        bonding_time: old_info.bonding_time,
        unbonding_period: old_info.unbonding_period,
        validator_list: old_info.validator_list,
        last_update_time: timestamp,
    };

    // Create initial balance update record
    let update = BalanceUpdate {
        action: BalanceAction::Bond {
            nibi_amount: old_info.amount_staked_unibi,
            stnibi_minted: old_info.amount_stnibi_balance,
            validator: None,
        },
        timestamp,
        exchange_rate: Decimal::one(), // Use current exchange rate if available
        resulting_nibi_balance: old_info.amount_staked_unibi,
        resulting_stnibi_balance: old_info.amount_stnibi_balance,
        block_height,
    };

    STAKERINFO.save(storage, staker.to_owned(), &new_info)?;
    BALANCE_UPDATES.save(storage, (staker, 1), &update)?;
    LAST_UPDATE_ID.save(storage, staker, &1u64)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "migrate_staker_balance"),
        attr("staker", staker),
        attr("nibi_balance", old_info.amount_staked_unibi),
        attr("stnibi_balance", old_info.amount_stnibi_balance),
    ]))
}