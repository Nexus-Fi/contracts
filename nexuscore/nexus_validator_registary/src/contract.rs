
use std::collections::HashMap;

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    to_binary, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult, Uint128, WasmMsg,
};

use crate::common::calculate_delegations;
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::registry::{Config, Validator, ValidatorResponse, CONFIG, REGISTRY};
use basset::hub::ExecuteMsg::{DispatchRewards, RedelegateProxy};



#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    CONFIG.save(
        deps.storage,
        &Config {
            owner: info.sender,
            staking_nibi_contract: deps.api.addr_validate(msg.staking_nibi_contract.as_str())?,
        },
    )?;

    for v in msg.registry {
        REGISTRY.save(deps.storage, v.address.as_str().as_bytes(), &v)?;
    }

    Ok(Response::default())
}


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::AddValidator { validator } => add_validator(deps, env, info, validator),
        ExecuteMsg::RemoveValidator { address } => remove_validator(deps, env, info, address),
        ExecuteMsg::UpdateConfig {
            owner,
            hub_contract,
        } => execute_update_config(deps, env, info, owner, hub_contract),
    }
}


/// Update the config. Update the owner and hub contract address.
/// Only creator/owner is allowed to execute
pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    owner: Option<String>,
    staking_nibi_contract: Option<String>,
) -> StdResult<Response> {
    // only owner must be able to send this message.
    let config = CONFIG.load(deps.storage)?;
    let owner_address = config.owner;
    if info.sender != owner_address {
        return Err(StdError::generic_err("unauthorized"));
    }

    if let Some(o) = owner {
        let owner_raw = deps.api.addr_validate(&o)?;

        CONFIG.update(deps.storage, |mut last_config| -> StdResult<_> {
            last_config.owner = owner_raw;
            Ok(last_config)
        })?;
    }

    if let Some(hub) = staking_nibi_contract {
        let hub_raw = deps.api.addr_validate(&hub)?;

        CONFIG.update(deps.storage, |mut last_config| -> StdResult<_> {
            last_config.staking_nibi_contract = hub_raw;
            Ok(last_config)
        })?;
    }

    Ok(Response::default())
}


pub fn add_validator(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    validator: Validator,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let owner_address = config.owner;
    let hub_address = config.staking_nibi_contract;
    if !(info.sender == owner_address || info.sender == hub_address) {
        return Err(StdError::generic_err("unauthorized"));
    }

    REGISTRY.save(
        deps.storage,
        validator.address.as_str().as_bytes(),
        &validator,
    )?;
    Ok(Response::default())
}


pub fn remove_validator(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    validator_address: String,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let owner_address = config.owner;
    if info.sender != owner_address {
        return Err(StdError::generic_err("unauthorized"));
    }

    REGISTRY.remove(deps.storage, validator_address.as_str().as_bytes());

    let mut validators = query_validators(deps.as_ref())?;
    if validators.is_empty() {
        return Err(StdError::generic_err(
            "Cannot remove the last validator in the registry",
        ));
    }
    validators.sort_by(|v1, v2| v1.total_delegated.cmp(&v2.total_delegated));

    let hub_address = config.hub_contract;

    let query = deps
        .querier
        .query_delegation(hub_address.clone(), validator_address.clone());

    let mut messages: Vec<CosmosMsg> = vec![];
    if let Ok(q) = query {
        let delegated_amount = q;

        let mut redelegations: Vec<(String, Coin)> = vec![];
        if let Some(delegation) = delegated_amount {
            // Terra core returns zero if there is another active redelegation
            // That means we cannot start a new redelegation, so we only remove a validator from
            // the registry.
            // We'll do a redelegation manually later by sending RedelegateProxy to the hub
            if delegation.can_redelegate.amount < delegation.amount.amount {
                return StdResult::Ok(Response::new());
            }

            let delegations =
                calculate_delegations(delegation.amount.amount, validators.as_slice())?;

            for i in 0..delegations.len() {
                if delegations[i].is_zero() {
                    continue;
                }
                redelegations.push((
                    validators[i].address.clone(),
                    Coin::new(delegations[i].u128(), delegation.amount.denom.as_str()),
                ));
            }

            let regelegate_msg = RedelegateProxy {
                src_validator: validator_address,
                redelegations,
            };
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: hub_address.clone().into_string(),
                msg: to_binary(&regelegate_msg)?,
                funds: vec![],
            }));

            let msg = DispatchRewards {};
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: hub_address.into_string(),
                msg: to_binary(&msg)?,
                funds: vec![],
            }));
        }
    }

    let res = Response::new().add_messages(messages);
    Ok(res)
}
