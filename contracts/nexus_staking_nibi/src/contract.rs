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

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cw_storage_plus::Bound;
use nibiru_std::proto::{nibiru, NibiruStargateMsg};
use std::string::FromUtf8Error;
//// this is v1 

use cosmwasm_std::{
    attr, from_binary, to_binary, Addr, Binary, Coin, CosmosMsg, Decimal, Delegation, Deps, DepsMut, DistributionMsg, Env, MessageInfo, Order, QueryRequest, Response, StakingMsg, StdError, StdResult, Storage, Uint128, Validator, WasmMsg, WasmQuery
};

use crate::config::{ execute_update_config, execute_update_params};
use crate::error::BalanceError;
use crate::state::{
    all_unbond_history, get_unbond_requests, query_get_finished_amount, read_unbond_history, BalanceAction, BalanceHistory, BalanceUpdate, BalanceUpdatesResponse, BALANCE_UPDATES, CONFIG, CURRENT_BATCH, GUARDIANS, LAST_UPDATE_ID, LPTOKENS, PARAMETERS, STAKERINFO, STAKERINFO_NEW, STATE
};
use crate::unbond::{execute_unbond_stnibi, execute_withdraw_unbonded};

use crate::bond::execute_bond;
use basset::hub::{
    self, AllHistoryResponse, BondType, Config, ConfigResponse, CurrentBatch, CurrentBatchResponse, InstantiateMsg, MigrateMsg, Parameters, QueryMsg, RestakeResponse, StakerInfo, StakerInfoResponse, State, StateResponse, UnbondHistoryResponse, UnbondRequestsResponse, UnbondingInfoResponse, UnbondingRequest, WithdrawableUnbondedResponse
};
use basset::hub::{Cw20HookMsg, ExecuteMsg,COSMOS_UNBONDING_PERIOD};
use cw20::{Cw20ExecuteMsg, Cw20QueryMsg, Cw20ReceiveMsg, TokenInfoResponse};
use nexus_rewards_dispatcher::msg::ExecuteMsg::DispatchRewards;



#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let sender = info.sender;

    // store config
    let data = Config {creator:sender,reward_dispatcher_contract:None,validators_registry_contract:None,stnibi_token_contract:None, stnibi_reserve:None,total_bonded:Uint128::zero(),stnibi_denom:None};
    CONFIG.save(deps.storage, &data)?;

    // store state
    let state = State {
        stnibi_exchange_rate: Decimal::one(),
        last_unbonded_time: env.block.time.seconds(),
        last_processed_batch: 0u64,
        ..Default::default()
    };

    STATE.save(deps.storage, &state)?;

    // instantiate parameters
    let params = Parameters {
        epoch_period: msg.epoch_period,
        underlying_coin_denom: msg.underlying_coin_denom,
        unbonding_period: msg.unbonding_period,
        paused: Some(false),
    };

    PARAMETERS.save(deps.storage, &params)?;

    let batch = CurrentBatch {
        id: 1,
        requested_stnibi: Default::default(),
    };
    CURRENT_BATCH.save(deps.storage, &batch)?;

    let res = Response::new();
    Ok(res)
}


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
            ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
            ExecuteMsg::BondForstnibi {} => execute_bond(deps, env, info, BondType::stnibi),
            ExecuteMsg::BondRewards {} => execute_bond(deps, env, info, BondType::BondRewards),
            ExecuteMsg::DispatchRewards {} => execute_dispatch_rewards(deps, env, info),
            ExecuteMsg::WithdrawUnbonded {} => execute_withdraw_unbonded(deps, env, info),
            ExecuteMsg::CheckSlashing {} => execute_slashing(deps, env),
            ExecuteMsg::UpdateParams {
                epoch_period,
                unbonding_period,
            } => execute_update_params(deps, env, info, epoch_period, unbonding_period),
            ExecuteMsg::UpdateConfig {
                owner,
                rewards_dispatcher_contract,
                validators_registry_contract,
                stnibi_token_contract,
                stnibi_denom
            } => execute_update_config(
                deps,
                env,
                info,
                owner,
                rewards_dispatcher_contract,
                stnibi_token_contract,
                validators_registry_contract,
                stnibi_denom
            ),
            ExecuteMsg::RedelegateProxy {
                src_validator,
                redelegations,
            } => todo!(),
            ExecuteMsg::PauseContracts {} => execute_pause_contracts(deps, env, info),
            ExecuteMsg::UnpauseContracts {} => execute_unpause_contracts(deps, env, info),
            ExecuteMsg::AddGuardians { addresses } => execute_add_guardians(deps, env, info, addresses),
            ExecuteMsg::RemoveGuardians { addresses } => {
                execute_remove_guardians(deps, env, info, addresses)
            },
    }
}


pub fn execute_add_guardians(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    guardians: Vec<String>,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.creator {
        return Err(StdError::generic_err("unauthorized"));
    }

    for guardian in &guardians {
        GUARDIANS.save(deps.storage, guardian.clone(), &true)?;
    }

    Ok(Response::new()
        .add_attributes(vec![attr("action", "add_guardians")])
        .add_attributes(guardians.iter().map(|g| attr("address", g))))
}

pub fn execute_remove_guardians(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    guardians: Vec<String>,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.creator {
        return Err(StdError::generic_err("unauthorized"));
    }

    for guardian in &guardians {
        GUARDIANS.remove(deps.storage, guardian.clone());
    }

    Ok(Response::new()
        .add_attributes(vec![attr("action", "remove_guardians")])
        .add_attributes(guardians.iter().map(|g| attr("value", g))))
}

pub fn execute_pause_contracts(deps: DepsMut, _env: Env, info: MessageInfo) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    if !(info.sender == config.creator || GUARDIANS.has(deps.storage, info.sender.to_string())) {
        return Err(StdError::generic_err("unauthorized"));
    }

    let mut params: Parameters = PARAMETERS.load(deps.storage)?;
    params.paused = Some(true);

    PARAMETERS.save(deps.storage, &params)?;

    let res = Response::new().add_attributes(vec![attr("action", "pause_contracts")]);
    Ok(res)
}

    

pub fn execute_unpause_contracts(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.creator {
        return Err(StdError::generic_err("unauthorized"));
    }

    let mut params: Parameters = PARAMETERS.load(deps.storage)?;
    params.paused = Some(false);

    PARAMETERS.save(deps.storage, &params)?;

    let res = Response::new().add_attributes(vec![attr("action", "unpause_contracts")]);
    Ok(res)
}


    
// pub fn execute_redelegate_proxy(
//     deps: DepsMut,
//     _env: Env,
//     info: MessageInfo,
//     src_validator: String,
//     redelegations: Vec<(String, Coin)>,
// ) -> StdResult<Response> {
//     let sender_contract_addr = info.sender;
//     let conf = CONFIG.load(deps.storage)?;
//     let validators_registry_contract = conf.validators_registry_contract.ok_or_else(|| {
//         StdError::generic_err("the validator registry contract must have been registered")
//     })?;

//     if !(sender_contract_addr == validators_registry_contract
//         || sender_contract_addr == conf.creator)
//     {
//         return Err(StdError::generic_err("unauthorized"));
//     }

//     let messages: Vec<CosmosMsg> = redelegations
//         .into_iter()
//         .map(|(dst_validator, amount)| {
//             cosmwasm_std::CosmosMsg::Staking(StakingMsg::Redelegate {
//                 src_validator: src_validator.clone(),
//                 dst_validator,
//                 amount,
//             })
//         })
//         .collect();

//     let res = Response::new().add_messages(messages);

//     Ok(res)
// }

/// CW20 token receive handler.
pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response> {
    let params: Parameters = PARAMETERS.load(deps.storage)?;
    if params.paused.unwrap_or(false) {
        return Err(StdError::generic_err("the contract is temporarily paused"));
    }

    let contract_addr = deps.api.addr_validate(info.sender.as_str())?;

    // only token contract can execute this message
    let conf = CONFIG.load(deps.storage)?;

    let ststnibi_contract_addr = if let Some(st) = conf.stnibi_token_contract {
        st
    } else {
        return Err(StdError::generic_err(
            "the stnibi token contract must have been registered",
        ));
    };

    // match from_binary(&cw20_msg.msg)? {
    //     Cw20HookMsg::Unbond {} => {
            // if contract_addr == ststnibi_contract_addr {
                execute_unbond_stnibi(deps, env, cw20_msg.amount, cw20_msg.sender)
            // } else {
            //     Err(StdError::generic_err("unauthorized"))
            // }
    //     }
    //     Cw20HookMsg::Restake {  } => todo!(),
    // }
}



/// Permissionless
pub fn execute_dispatch_rewards(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
) -> StdResult<Response> {
    let params: Parameters = PARAMETERS.load(deps.storage)?;
    if params.paused.unwrap_or(false) {
        return Err(StdError::generic_err("the contract is temporarily paused"));
    }

    let config = CONFIG.load(deps.storage)?;
    let reward_addr_dispatcher = config
        .reward_dispatcher_contract
        .ok_or_else(|| StdError::generic_err("the reward contract must have been registered"))?;

    // Send withdraw message
    let mut withdraw_msgs = withdraw_all_rewards(&deps, env.contract.address.to_string())?;
    let mut messages: Vec<CosmosMsg> = vec![];
    messages.append(&mut withdraw_msgs);

    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: reward_addr_dispatcher.to_string(),
        msg: to_binary(&DispatchRewards {})?,
        funds: vec![],
    }));

    let res = Response::new()
        .add_messages(messages)
        .add_attributes(vec![attr("action", "dispatch_rewards")]);
    Ok(res)
}

/// Create withdraw requests for all validators
fn withdraw_all_rewards(deps: &DepsMut, delegator: String) -> StdResult<Vec<CosmosMsg>> {
    let mut messages: Vec<CosmosMsg> = vec![];

    let delegations = deps.querier.query_all_delegations(delegator)?;

    if !delegations.is_empty() {
        for delegation in delegations {
            let msg: CosmosMsg =
                CosmosMsg::Distribution(DistributionMsg::WithdrawDelegatorReward {
                    validator: delegation.validator,
                });
            messages.push(msg);
        }
    }

    Ok(messages)
}

fn query_actual_state(deps: Deps, env: &Env) -> StdResult<State> {
    let mut state = STATE.load(deps.storage)?;
    let delegations = deps.querier.query_all_delegations(env.contract.address.clone())?;
    if delegations.is_empty() {
        return Ok(state);
    }
    
    // read params
    let params = PARAMETERS.load(deps.storage)?;
    let coin_denom = params.underlying_coin_denom;

    // Check the actual bonded amount
    let mut actual_total_bonded = Uint128::zero();
    for delegation in &delegations {
        if delegation.amount.denom == coin_denom {
            actual_total_bonded += delegation.amount.amount;
        }
    }

    // Check the amount that contract thinks is bonded
    if state.total_bond_stnibi_amount.is_zero() {
        return Ok(state);
    }

    // Need total issued for updating the exchange rate
    state.total_stnibi_issued = query_total_stnibi_issued(deps)?;
    let current_batch = CURRENT_BATCH.load(deps.storage)?;
    let current_requested_stnibi = current_batch.requested_stnibi;

    if state.total_bond_stnibi_amount.u128() > actual_total_bonded.u128() {
        state.total_bond_stnibi_amount = actual_total_bonded;
    }
    //NOT UPDATING THE EXCHANGE RATE 
    state.update_stnibi_exchange_rate(state.total_stnibi_issued, current_requested_stnibi);
    Ok(state)
}



/// Check whether slashing has happened
/// This is used for checking slashing while bonding or unbonding
pub fn slashing(deps: &mut DepsMut, env: &Env) -> StdResult<State> {
    let state = query_actual_state(deps.as_ref(), env)?;

    STATE.save(deps.storage, &state)?;

    Ok(state)
}

/// Handler for tracking slashing
/// NOT AUDITED
pub fn execute_slashing(mut deps: DepsMut, env: Env) -> StdResult<Response> {
    let params: Parameters = PARAMETERS.load(deps.storage)?;
    if params.paused.unwrap_or(false) {
        return Err(StdError::generic_err("the contract is temporarily paused"));
    }
///////////////////////////
    // Get previous state for comparison
    let prev_state = STATE.load(deps.storage)?;
    
    // Call slashing to get new state with updated exchange rate
    let new_state = slashing(&mut deps, &env)?;

    // Calculate slash percentage if exchange rate decreased
    if new_state.stnibi_exchange_rate < prev_state.stnibi_exchange_rate {
        let slash_percentage = Decimal::one() - (new_state.stnibi_exchange_rate / prev_state.stnibi_exchange_rate);
        
        // Get all delegations to identify affected validators
        let delegations = deps.querier.query_all_delegations(env.contract.address.clone())?;
        let mut affected_validators = Vec::new();
        
        // Identify slashed validators by comparing delegation amounts
        for delegation in delegations {
            affected_validators.push(delegation.validator);
        }

        // Update all stakers' balances
        // First, get all stakers (you might need to implement a way to track all stakers)
        let stakers = STAKERINFO
            .range(deps.storage, None, None, Order::Ascending)
            .map(|item| {
                let (staker, _) = item?;
                Ok(staker)
            })
            .collect::<StdResult<Vec<String>>>()?;

        // Update each staker's balance
        for staker in stakers {
            for validator in affected_validators.iter() {
                let a= update_balances_for_slash(
                    deps.storage,
                    &staker,
                    slash_percentage,
                    validator.clone(),
                    env.block.time.seconds(),
                    env.block.height,
                );
            }
        }

        return Ok(Response::new().add_attributes(vec![
            attr("action", "check_slashing"),
            attr("new_stnibi_exchange_rate", new_state.stnibi_exchange_rate.to_string()),
            attr("slash_percentage", slash_percentage.to_string()),
            attr("affected_validators", affected_validators.join(","))
        ]));
    }

    // call slashing and return new exchange rate
    let state = slashing(&mut deps, &env)?;
    Ok(Response::new().add_attributes(vec![
        attr("action", "check_slashing"),
        attr(
            "new_stnibi_exchange_rate",
            state.stnibi_exchange_rate.to_string(),
        ),
    ]))
}


// let a =   update_balances_for_slash(
//     deps.storage,
//     staker_address,
//     slash_percentage,
//     validator_address,
//     env.block.time.seconds(),
//     env.block.height,
// );

// Function to handle slashing events
pub fn update_balances_for_slash(
    storage: &mut dyn Storage,
    staker: &str,
    slash_percentage: Decimal,
    validator: String,
    timestamp: u64,
    block_height: u64,
) -> Result<(), BalanceError> {
    let old_info = STAKERINFO_NEW.load(storage, staker)
        .map_err(|_| BalanceError::StakerNotFound {})?;

    // Calculate slashed amounts
    let nibi_slashed = old_info.amount_staked_unibi * slash_percentage;
    let stnibi_adjusted = old_info.amount_stnibi_balance * slash_percentage;

    // Update staker info
    let new_info = StakerInfo {
        amount_staked_unibi: old_info.amount_staked_unibi.checked_sub(nibi_slashed)
            .map_err(|_| BalanceError::NegativeBalance {})?,
        amount_stnibi_balance: old_info.amount_stnibi_balance.checked_sub(stnibi_adjusted)
            .map_err(|_| BalanceError::NegativeBalance {})?,
        ..old_info
    };

    // Record the slashing event
    let update_id = LAST_UPDATE_ID
        .may_load(storage, staker)?
        .unwrap_or_default() + 1;

    let update = BalanceUpdate {
        action: BalanceAction::Slash {
            nibi_slashed,
            stnibi_adjusted,
            validator,
        },
        timestamp,
        exchange_rate: Decimal::one(), // Slashing doesn't use exchange rate
        resulting_nibi_balance: new_info.amount_staked_unibi,
        resulting_stnibi_balance: new_info.amount_stnibi_balance,
        block_height,
    };

    STAKERINFO.save(storage, staker.to_owned(), &new_info)?;
    BALANCE_UPDATES.save(storage, (staker, update_id), &update)?;
    LAST_UPDATE_ID.save(storage, staker, &update_id)?;

    Ok(())
}


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps, env)?),
        QueryMsg::CurrentBatch {} => to_binary(&query_current_batch(deps)?),
        QueryMsg::WithdrawableUnbonded { address } => {
            to_binary(&query_withdrawable_unbonded(deps, address, env)?)
        }
        QueryMsg::Parameters {} => to_binary(&query_params(deps)?),
        QueryMsg::UnbondRequests { address } => to_binary(&query_unbond_requests(deps, address)?),
        QueryMsg::AllHistory { start_from, limit } => {
            to_binary(&query_unbond_requests_limitation(deps, start_from, limit)?)
        }
        QueryMsg::Guardians => to_binary(&query_guardians(deps)?),
        QueryMsg::Restake { staker } => to_binary(&query_restake(deps,staker)?),
        QueryMsg::Staker { staker } => to_binary(&query_staker(deps,staker)?),
        QueryMsg::DelegationData{delegator}=> to_binary(&query_delegation(deps,delegator)?),
        QueryMsg::HubBalance{contract_address} => to_binary(&query_hub_balance(deps,contract_address)?),
        QueryMsg::GetUnbondingInfo { user_address } => {
            to_binary(&query_unbonding_info(deps, env, user_address)?)
        },
        QueryMsg::BalanceHistory { staker,start_after,limit } => {
            to_binary(&query_balance_history(deps,  staker,start_after,limit)?)

        },
        QueryMsg::BalanceUpdates { staker,start_after,limit } => {
            to_binary(&query_balance_updates(deps,  staker,start_after,limit)?)
            
        },
        QueryMsg::StakerInfo { staker } => {
            to_binary(&query_staker_info(deps,  staker)?)
            
        },
        QueryMsg::AllStakers { start_after,limit } => {
            unimplemented!()
        },

    }
}


pub fn query_balance_updates(
    deps: Deps,
    staker: String,
    start_after: Option<u64>,
    limit: Option<u64>,
) -> StdResult<BalanceUpdatesResponse> {

   // First check if staker exists
   let _ = match STAKERINFO_NEW.may_load(deps.storage, staker.as_str())? {
    Some(_) => (), // Staker exists, continue
    None => {
        // Return empty response for non-existent staker
        return Ok(BalanceUpdatesResponse {
            updates: vec![],
            last_update_id: 0,
        });
    }
};

    
    let limit = limit.unwrap_or(10).min(30) as usize;
    
    let start = start_after.map(|id| Bound::exclusive(id));
    
    let updates: Vec<BalanceUpdate> = BALANCE_UPDATES
        .prefix(&staker)
        .range(deps.storage, start, None, Order::Descending)
        .take(limit)
        .map(|item| item.map(|(_, update)| update))
        .collect::<StdResult<Vec<_>>>()?;

    let last_update_id = LAST_UPDATE_ID
        .may_load(deps.storage, &staker)?
        .unwrap_or_default();

    Ok(BalanceUpdatesResponse {
        updates,
        last_update_id,
    })
}




pub fn query_staker_info(deps: Deps, staker: String) -> StdResult<StakerInfoResponse> {
    let info = match STAKERINFO_NEW.may_load(deps.storage, &staker)? {
        Some(info) => info,
        None => return Ok(StakerInfoResponse {
            amount_staked_unibi: Uint128::zero(),
            amount_stnibi_balance: Uint128::zero(),
            bonding_time: Uint128::zero(),
            unbonding_period: None,
            validator_list: None,
            last_update_time: 0,
            total_rewards_earned: None,
        })
    };

    // Get last update time from balance updates
    let last_update_id = LAST_UPDATE_ID
        .may_load(deps.storage, &staker)?
        .unwrap_or_default();

    let last_update_time = if last_update_id > 0 {
        BALANCE_UPDATES
            .may_load(deps.storage, (&staker, last_update_id))?
            .map(|update| update.timestamp)
            .unwrap_or(0)
    } else {
        0
    };

    // Calculate total rewards earned from history
    let total_rewards = calculate_total_rewards(deps.storage, &staker)?;

    Ok(StakerInfoResponse {
        amount_staked_unibi: info.amount_staked_unibi,
        amount_stnibi_balance: info.amount_stnibi_balance,
        bonding_time: info.bonding_time,
        unbonding_period: info.unbonding_period,
        validator_list: info.validator_list,
        last_update_time,
        total_rewards_earned: Some(total_rewards),
    })
}

// Helper function to calculate total rewards
fn calculate_total_rewards(storage: &dyn Storage, staker: &str) -> StdResult<Uint128> {
    let mut total_rewards = Uint128::zero();

    // Iterate through all balance updates
    let updates: Vec<BalanceUpdate> = BALANCE_UPDATES
        .prefix(staker)
        .range(storage, None, None, Order::Ascending)
        .map(|item| item.map(|(_, update)| update))
        .collect::<StdResult<Vec<_>>>()?;

    for update in updates {
        match update.action {
            BalanceAction::BondRewards { nibi_amount } => {
                total_rewards += nibi_amount;
            }
            _ => {} // Ignore other types of updates
        }
    }

    Ok(total_rewards)
}


// pub fn query_all_stakers(
//     deps: Deps,
//     start_after: Option<String>,
//     limit: Option<u32>,x x   
// ) -> StdResult<AllStakersResponse> {
//     let limit = limit.unwrap_or(10).min(30) as usize;
    
//     let start = start_after.map(|addr| Bound::exclusive(addr.as_bytes()));
    
//     let stakers: Vec<StakerSummary> = STAKERINFO
//         .range(deps.storage, start, None, Order::Ascending)
//         .take(limit)
//         .map(|item| {
//             let (address, info) = item?;
//             Ok(StakerSummary {
//                 address: String::from_utf8(address)?,
//                 staked_unibi: info.amount_staked_unibi,
//                 stnibi_balance: info.amount_stnibi_balance,
//             })
//         })
//         .collect::<StdResult<Vec<_>>>()?;

//     // Count total stakers - Note: This might be expensive for large numbers
//     let total_stakers = STAKERINFO
//         .range(deps.storage, None, None, Order::Ascending)
//         .count() as u64;

//     Ok(AllStakersResponse {
//         stakers,
//         total_stakers,
//     })
// }

// query balances
pub fn query_balance_history(
    deps: Deps,
    staker:String,
    start_after: Option<u64>,
    limit: Option<u64>,
) -> StdResult<BalanceHistory> {


    let current_info = match STAKERINFO_NEW.may_load(deps.storage, staker.as_str())? {
        Some(info) => info,
        None => {
            return Ok(BalanceHistory {
                updates: vec![],
                total_bonded: Uint128::zero(),
                total_unbonded: Uint128::zero(),
                current_stnibi: Uint128::zero(),
            });
        }
    };

    

    let limit = limit.unwrap_or(10).min(30) as usize;
    
    let updates: Vec<BalanceUpdate> = {
        let bound = match start_after {
            Some(id) => Some(Bound::exclusive(id)),
            None => None
        };

        BALANCE_UPDATES
            .prefix(&staker)
            .range(deps.clone().storage, bound, None, Order::Descending)
            .take(limit)
            .map(|item| {
                let (_, update) = item?;
                Ok(update)
            })
            .collect::<StdResult<Vec<_>>>()?
    };

    let mut total_bonded = Uint128::zero();
    let mut total_unbonded = Uint128::zero();
    let current_info = STAKERINFO_NEW.load(deps.storage, &staker)?;

    for update in updates.iter() {
        match &update.action {
            BalanceAction::Bond { nibi_amount, .. } => {
                total_bonded += nibi_amount;
            },
            BalanceAction::Unbond { nibi_unbonded, .. } => {
                total_unbonded += nibi_unbonded;
            },
            _ => {}
        }
    }

    Ok(BalanceHistory {
        updates,
        total_bonded,
        total_unbonded,
        current_stnibi: current_info.amount_stnibi_balance,
    })
}



fn query_delegation(deps:Deps,delegator:String) -> StdResult<Vec<Delegation>> {
    let delegations = deps.querier.query_all_delegations(delegator)?;
    Ok(delegations)
        
}



fn query_hub_balance(deps:Deps,contract_address:String) -> StdResult<Uint128> {
    let params = PARAMETERS.load(deps.storage)?;
    let coin_denom = params.underlying_coin_denom;

    let hub_balance = deps
        .querier
        .query_balance(contract_address, &*coin_denom)?
        .amount;

    Ok(hub_balance)
        
}


fn query_staker(deps:Deps,staker:String) -> StdResult<StakerInfo>{
    let restake = STAKERINFO.may_load(deps.storage, staker.clone()).unwrap();
    match restake{
        Some(data) =>{
            return Ok(data);
        },
        None=>{
            let staker_info = StakerInfo{
                amount_staked_unibi: Uint128::zero(),
                amount_stnibi_balance:Uint128::zero(),
                bonding_time:Uint128::zero(),
                unbonding_period:None,
                validator_list:None,
                last_update_time:0
            };
            return Ok(staker_info)
        }
    }
    // return Err(cosmwasm_std::StdError::generic_err("non staker called"));
   
}



fn query_restake(deps:Deps,staker:String) -> StdResult<RestakeResponse> {
    let restake = LPTOKENS.may_load(deps.storage, staker.clone()).unwrap();
    let responce = RestakeResponse{
        Staker: staker,
        stnibi_amount: restake.unwrap(),
    };
    Ok(responce)
}

fn query_guardians(deps: Deps) -> StdResult<Vec<String>> {
    let guardians = GUARDIANS.keys(deps.storage, None, None, Order::Ascending);
    let guardians_decoded: Result<Vec<String>, FromUtf8Error> =
        guardians.map(|arg0: Result<String, StdError>| String::from_utf8(arg0.unwrap().into())).collect();
    Ok(guardians_decoded?)
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;

    let reward_dispatcher: Option<String> = config.reward_dispatcher_contract.map(|s| s.into());
    let stnibi_token: Option<String> = config.stnibi_token_contract.map(|s| s.into());
    let validators_contract: Option<String> = config.validators_registry_contract.map(|s| s.into());

    Ok(ConfigResponse {
        owner: config.creator.to_string(),
        reward_dispatcher_contract: reward_dispatcher,
        validators_registry_contract: validators_contract,
        stnibi_token_contract: stnibi_token,
    })
}

fn query_state(deps: Deps, env: Env) -> StdResult<StateResponse> {
    let state = query_actual_state(deps, &env)?;
    let res = StateResponse {
        stnibi_exchange_rate: state.stnibi_exchange_rate,
        total_bond_stnibi_amount: state.total_bond_stnibi_amount,
        prev_hub_balance: state.prev_hub_balance,
        last_unbonded_time: state.last_unbonded_time,
        last_processed_batch: state.last_processed_batch,
        total_stnibi_burned:state.total_stnibi_burned
    };
    Ok(res)
}



fn query_current_batch(deps: Deps) -> StdResult<CurrentBatchResponse> {
    let current_batch = CURRENT_BATCH.load(deps.storage)?;
    Ok(CurrentBatchResponse {
        id: current_batch.id,
        requested_stnibi: current_batch.requested_stnibi,
    })
}

fn query_withdrawable_unbonded(
    deps: Deps,
    address: String,
    env: Env,
) -> StdResult<WithdrawableUnbondedResponse> {
    let params = PARAMETERS.load(deps.storage)?;
    let historical_time = env.block.time.seconds() - params.unbonding_period;
    let all_requests = query_get_finished_amount(deps.storage, address, historical_time)?;

    let withdrawable = WithdrawableUnbondedResponse {
        withdrawable: all_requests,
    };
    Ok(withdrawable)
}

fn query_params(deps: Deps) -> StdResult<Parameters> {
    PARAMETERS.load(deps.storage)
}

pub(crate) fn query_total_stnibi_issued(deps: Deps) -> StdResult<Uint128> {
    // let token_address = CONFIG
    //     .load(deps.storage)?
    //     .stnibi_token_contract
    //     .ok_or_else(|| StdError::generic_err("token contract must have been registered"))?;
    // let token_info: TokenInfoResponse =
    //     deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
    //         contract_addr: token_address.to_string(),
    //         msg: to_binary(&Cw20QueryMsg::TokenInfo {})?,
    //     }))?;
    let mut state = STATE.load(deps.storage)?;
        let total_issued = state.total_stnibi_issued; 
    Ok(total_issued)
}



fn query_unbond_requests(deps: Deps, address: String) -> StdResult<UnbondRequestsResponse> {
    let requests = get_unbond_requests(deps.storage, address.clone())?;
    let res = UnbondRequestsResponse { address, requests };
    Ok(res)
}

fn query_unbond_requests_limitation(
    deps: Deps,
    start: Option<u64>,
    limit: Option<u32>,
) -> StdResult<AllHistoryResponse> {
    let requests = all_unbond_history(deps.storage, start, limit)?;
    let requests_responses = requests
        .iter()
        .map(|r| UnbondHistoryResponse {
            batch_id: r.batch_id,
            time: r.time,

            stnibi_amount: r.stnibi_amount,
            stnibi_applied_exchange_rate: r.stnibi_applied_exchange_rate,
            stnibi_withdraw_rate: r.stnibi_withdraw_rate,

            released: r.released,
        })
        .collect();

    let res = AllHistoryResponse {
        history: requests_responses,
    };
    Ok(res)
}


// In contract.rs
pub fn query_unbonding_info(deps: Deps, env: Env, user_address: String) -> StdResult<UnbondingInfoResponse> {
    let config: Config = CONFIG.load(deps.storage)?;
    let params: Parameters = PARAMETERS.load(deps.storage)?;
    
    let contract_period = params.unbonding_period;
    let effective_period = std::cmp::max(contract_period, COSMOS_UNBONDING_PERIOD);

    // Get contract-level unbond requests
    let requests = get_unbond_requests(deps.storage, user_address.clone())?;
    let mut unbonding_details: Vec<UnbondingRequest> = vec![];
    let mut total_unbonding = Uint128::zero();

    // Check if there are any active protocol-level unbondings
    let delegator_addr = deps.api.addr_validate(&user_address)?;
    let unbonding_responses = deps.querier.query_all_delegations(delegator_addr)?;
    let is_protocol_unbonding = !unbonding_responses.is_empty();

    // Process each request
    for (batch_id, amount) in requests {
        if let Ok(history) = read_unbond_history(deps.storage, batch_id) {
            total_unbonding += amount;
            
            let contract_release = history.time + contract_period;
            let protocol_release = history.time + COSMOS_UNBONDING_PERIOD;
            let final_release = std::cmp::max(contract_release, protocol_release);

            unbonding_details.push(UnbondingRequest {
                batch_id,
                amount,
                time_requested: history.time,
                contract_release_time: contract_release,
                protocol_release_time: protocol_release,
                final_release_time: final_release,
            });
        }
    }

    Ok(UnbondingInfoResponse {
        contract_unbonding_period: contract_period,
        protocol_unbonding_period: COSMOS_UNBONDING_PERIOD,
        effective_unbonding_period: effective_period,
        unbonding_requests: unbonding_details,  // Fixed: using unbonding_details instead of unbonding_requests
        total_unbonding,
        is_unbonding_protocol_locked: is_protocol_unbonding,
    })
}


// Add documentation
/// # Unbonding Process in Cosmos-SDK Based Chains
/// 
/// This contract interacts with two levels of unbonding:
/// 
/// 1. Contract Level:
///    - Configurable through `params.unbonding_period`
///    - Controls when users can withdraw from the contract
///    - Can be set to any value including 0 // 
/// 
/// 2. Protocol Level (Cosmos SDK):
///    - Fixed 21-day unbonding period
///    - Hardcoded in the Cosmos SDK staking module
///    - Cannot be modified by contracts or the chain
///    - Required for network security
///     
/// The effective unbonding period will always be at least 21 days due to 
/// the protocol-level requirement, regardless of contract settings.
/// 
pub fn document_unbonding_process() -> &'static str {
    "See function documentation for unbonding process details"
}



#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::new())
}

