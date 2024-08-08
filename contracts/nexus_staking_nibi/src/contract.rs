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
use nexus_validator_registary::registry::ValidatorResponse;
use nibiru_std::proto::{nibiru, NibiruStargateMsg};
use std::string::FromUtf8Error;

use cosmwasm_std::{
    attr, from_binary, to_binary, Addr, Api, Binary, CanonicalAddr, Coin, CosmosMsg, Decimal, Delegation, Deps, DepsMut, DistributionMsg, Env, MessageInfo, Order, QueryRequest, Response, StakingMsg, StdError, StdResult, Storage, Uint128, Validator, WasmMsg, WasmQuery
};

use crate::config::{self, execute_update_config, execute_update_params};
use crate::state::{
    all_unbond_history, get_unbond_requests, query_get_finished_amount, remove_and_accumulate_lock_info, StakerInfo, CONFIG, CURRENT_BATCH, GUARDIANS, LPTOKENS, PARAMETERS, STAKERINFO, STATE
};
use crate::unbond::{execute_unbond_stnibi, execute_withdraw_unbonded};

use crate::bond::execute_bond;
use basset::hub::{
    AllHistoryResponse, BondType, Config, ConfigResponse, CurrentBatch, CurrentBatchResponse, InstantiateMsg, MigrateMsg, Parameters, QueryMsg, RestakeResponse, State, StateResponse, UnbondHistoryResponse, UnbondRequestsResponse, WithdrawableUnbondedResponse
};
use basset::hub::{Cw20HookMsg, ExecuteMsg};
use cw20::{Cw20ExecuteMsg, Cw20QueryMsg, Cw20ReceiveMsg, TokenInfoResponse};
use nexus_rewards_dispatcher::msg::ExecuteMsg::DispatchRewards;
// use crate::restaking::{execute_restake_bond, execute_restake_bond_test};


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
            ExecuteMsg::CreateDenom { subdenom } => {
                let cosmos_msg: CosmosMsg = nibiru::tokenfactory::MsgCreateDenom {
                    sender: env.contract.address.into_string(),
                    subdenom,
                }
                .into_stargate_msg();
         
                Ok(Response::new()
                    // .add_event()
                    .add_message(cosmos_msg))
            }
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
                // stnibi_token_contract,
                stnibi_denom
            } => execute_update_config(
                deps,
                env,
                info,
                owner,
                rewards_dispatcher_contract,
                // stnibi_token_contract,
                validators_registry_contract,
                stnibi_denom
            ),
            ExecuteMsg::RedelegateProxy {
                src_validator,
                redelegations,
            } => execute_redelegate_proxy(deps, env, info, src_validator, redelegations),
            ExecuteMsg::PauseContracts {} => execute_pause_contracts(deps, env, info),
            ExecuteMsg::UnpauseContracts {} => execute_unpause_contracts(deps, env, info),
            ExecuteMsg::AddGuardians { addresses } => execute_add_guardians(deps, env, info, addresses),
            ExecuteMsg::RemoveGuardians { addresses } => {
                execute_remove_guardians(deps, env, info, addresses)
            },
            ExecuteMsg::DepositLiquidity { stnibi_amount, nusd_amount } => {
                todo!()
            },
            ExecuteMsg::WithdrawLiquidity { } => {
                execute_withdraw_liquidity(deps, env, info)
            },
            
           
    }
}





pub fn _withdraw_lock(
    storage: &mut dyn Storage,
    env: &Env,
    staker_addr: &Addr,
    staking_token: &Addr,
) -> StdResult<Response> {
    // execute 10 lock a time
    let unlock_amount = remove_and_accumulate_lock_info(
        storage,
        staking_token.as_bytes(),
        staker_addr.as_bytes(),
        env.block.time,
    )?;

    if unlock_amount.is_zero() {
        return Ok(Response::new());
    }

    let unbond_response = _unbond(staker_addr, staking_token, unlock_amount)?;

    Ok(unbond_response)
}

fn _unbond(staker_addr: &Addr, staking_token_addr: &Addr, amount: Uint128) -> StdResult<Response> {
    let messages: Vec<CosmosMsg> = vec![WasmMsg::Execute {
        contract_addr: staking_token_addr.to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: staker_addr.to_string(),
            amount,
        })?,
        funds: vec![],
    }
    .into()];

    Ok(Response::new().add_messages(messages).add_attributes([
        attr("action", "unbond"),
        attr("staker_addr", staker_addr.as_str()),
        attr("amount", amount.to_string()),
        attr("staking_token", staking_token_addr.as_str()),
    ]))
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


// fn _increase_bond_amount(
//     deps:DepsMut,
//     amount: Uint128,
//     staker_addr:Addr
// ) -> StdResult<()> {
//     let storage = deps.storage; 
//     let existing_balance = LPTOKENS.may_load(storage, staker_addr.clone().into_string())?;
//     existing_balance.unwrap().checked_add(amount);
//     // Update the user's balance in the map
//     LPTOKENS.save(storage, staker_addr.clone().into_string(), &existing_balance)?;
//     Ok(())
// }

// withdraw reward to pending reward
// pub fn before_share_change(pool_index: Decimal, reward_info: &mut RewardInfo) -> StdResult<()> {
//     let pending_reward = (reward_info.bond_amount * pool_index)
//         .checked_sub(reward_info.bond_amount * reward_info.index)?;
//     reward_info.index = pool_index;
//     reward_info.pending_reward += pending_reward;
//     Ok(())
// }
    
pub fn execute_redelegate_proxy(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    src_validator: String,
    redelegations: Vec<(String, Coin)>,
) -> StdResult<Response> {
    let sender_contract_addr = info.sender;
    let conf = CONFIG.load(deps.storage)?;
    let validators_registry_contract = conf.validators_registry_contract.ok_or_else(|| {
        StdError::generic_err("the validator registry contract must have been registered")
    })?;

    if !(sender_contract_addr == validators_registry_contract
        || sender_contract_addr == conf.creator)
    {
        return Err(StdError::generic_err("unauthorized"));
    }

    let messages: Vec<CosmosMsg> = redelegations
        .into_iter()
        .map(|(dst_validator, amount)| {
            cosmwasm_std::CosmosMsg::Staking(StakingMsg::Redelegate {
                src_validator: src_validator.clone(),
                dst_validator,
                amount,
            })
        })
        .collect();

    let res = Response::new().add_messages(messages);

    Ok(res)
}

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

    let stnibi_contract_addr = if let Some(st) = conf.stnibi_token_contract {
        st
    } else {
        return Err(StdError::generic_err(
            "the stnibi token contract must have been registered",
        ));
    };
    // 
    // STAKERINFO.remove(deps.storage, info.sender.into_string().clone());

    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::Unbond {} => {
            if contract_addr == stnibi_contract_addr {
                execute_unbond_stnibi(deps, env, cw20_msg.amount, cw20_msg.sender)
            } else {
                Err(StdError::generic_err("unauthorized"))
            }
        },
        Cw20HookMsg::Restake {  } => todo!()
    }
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

fn query_actual_state(deps: Deps, env: Env) -> StdResult<State> {
    let mut state = STATE.load(deps.storage)?;
    let delegations = deps.querier.query_all_delegations(env.contract.address)?;
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
    state.update_stnibi_exchange_rate(state.total_stnibi_issued, current_requested_stnibi);
    Ok(state)
}

/// Check whether slashing has happened
/// This is used for checking slashing while bonding or unbonding
pub fn slashing(deps: &mut DepsMut, env: Env) -> StdResult<State> {
    let state = query_actual_state(deps.as_ref(), env)?;

    STATE.save(deps.storage, &state)?;

    Ok(state)
}

/// Handler for tracking slashing
pub fn execute_slashing(mut deps: DepsMut, env: Env) -> StdResult<Response> {
    let params: Parameters = PARAMETERS.load(deps.storage)?;
    if params.paused.unwrap_or(false) {
        return Err(StdError::generic_err("the contract is temporarily paused"));
    }

    // call slashing and return new exchange rate
    let state = slashing(&mut deps, env)?;
    Ok(Response::new().add_attributes(vec![
        attr("action", "check_slashing"),
        attr(
            "new_stnibi_exchange_rate",
            state.stnibi_exchange_rate.to_string(),
        ),
    ]))
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
        QueryMsg::DelegationData{delegator}=> to_binary(&query_delegation(deps,delegator)?)
    }
}

fn query_delegation(deps:Deps,delegator:String) -> StdResult<Vec<Delegation>> {
    let delegations = deps.querier.query_all_delegations(delegator)?;
    Ok(delegations)
        
}


fn query_staker(deps:Deps,staker:String) -> StdResult<StakerInfo>{
    let restake = STAKERINFO.may_load(deps.storage, staker.clone()).unwrap();
    
    Ok(restake.unwrap())
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
    let state = query_actual_state(deps, env)?;
    let res = StateResponse {
        stnibi_exchange_rate: state.stnibi_exchange_rate,
        total_bond_stnibi_amount: state.total_bond_stnibi_amount,
        prev_hub_balance: state.prev_hub_balance,
        last_unbonded_time: state.last_unbonded_time,
        last_processed_batch: state.last_processed_batch,
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
    let token_address = CONFIG
        .load(deps.storage)?
        .stnibi_token_contract
        .ok_or_else(|| StdError::generic_err("token contract must have been registered"))?;
    let token_info: TokenInfoResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: token_address.to_string(),
            msg: to_binary(&Cw20QueryMsg::TokenInfo {})?,
        }))?;
    Ok(token_info.total_supply)
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

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::new())
}

/// transfer stnibi cw20 version 
// Withdraw liquidity function
fn execute_withdraw_liquidity(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> StdResult<Response> {

    // // Get user's LP token balance
    // let user_lp_balance = LPTOKENS.may_load(deps.storage, info.sender.to_string())?.unwrap_or_default();
    // if user_lp_balance < lp_tokens {
    //     return Err(StdError::generic_err("Insufficient LP token balance"));
    // }
    let sender = info.sender.to_string();

    let conf = CONFIG.load(deps.storage)?;
    let storage = deps.storage; 
    let staking_token = conf.stnibi_token_contract.unwrap().to_string();
    let amount = conf.total_bonded;
    let st = STAKERINFO.may_load(storage, sender.clone()).unwrap();
    // Transfer stNIBI tokens from contract to user
    let balance = LPTOKENS.may_load(storage, info.sender.clone().to_string())?.unwrap_or_else(Uint128::zero);
    
    
    let transfer_stnibi_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr:staking_token,
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: info.sender.to_string(),
            amount: balance,
        })?,
        funds: vec![],
    });
    // messages.push(transfer_stnibi_msg);
    LPTOKENS.remove(storage, info.sender.into_string());
    let new_staker_info = match st {
        Some(mut d) =>{
                d.amount_stnibi_balance +=balance;
                d            
        },
        None =>{
        return Err(StdError::generic_err("Stake is not available"));
           
        }

    };
    let _= STAKERINFO.save(storage, sender.clone(), &new_staker_info);
    let res = Response::new()
        .add_messages(vec![transfer_stnibi_msg])
        .add_attribute("from", sender)
        .add_attribute("action", "withdraw_liquidity")
        .add_attribute("stnibi_amount",  amount);
    Ok(res)
}

