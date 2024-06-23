use cosmwasm_schema::Api;
use cosmwasm_std::{
    entry_point, to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult, Uint128
};
// use cw2::set_contract_version;
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::{msg::{State,Operator,InstantiateMsg,OperatorDetails,ExecuteMsg,SignatureWithExpiry,Withdrawal}};

#[entry_point]

pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let operator_details = OperatorDetails {
        earnings_receiver: Addr::unchecked(&msg.initial_owner),
        delegation_approver: None,
        metadata_uri: "none".to_string()
    };
    OPERATOR_DETAILS.save(deps.storage, &operator_details)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender))
}


#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> StdResult<Response> {
    match msg {
        ExecuteMsg::RegisterAsOperator { details, metadata_uri } => {
            register_as_operator(deps, info, details, metadata_uri)
        }
      
    }
}

pub fn register_as_operator(
    deps: DepsMut,
    info: MessageInfo,
    details: OperatorDetails,
    metadata_uri: String,
) -> StdResult<Response> {
    let operator = Operator {
        details,
        is_registered: true,
    };
    OPERATORS.save(deps.storage, &info.sender, &operator)?;
    Ok(Response::new().add_attribute("method", "register_as_operator"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetOperatorsForDelegation {} => {
            let mut validators = query_validators(deps)?;
            validators.sort_by(|v1, v2| v1.total_delegated.cmp(&v2.total_delegated));
            to_binary(&validators)
        }
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
    }
}

fn query_validators(deps: Deps) -> StdResult<Vec<ValidatorResponse>> {
    let config = CONFIG.load(deps.storage)?;
    let hub_address = config.hub_contract;

    let mut delegations = HashMap::new();
    for delegation in deps.querier.query_all_delegations(&hub_address)? {
        delegations.insert(delegation.validator, delegation.amount.amount);
    }

    let mut validators: Vec<ValidatorResponse> = vec![];
    for item in REGISTRY.range(deps.storage, None, None, cosmwasm_std::Order::Ascending) {
        let mut validator = ValidatorResponse {
            total_delegated: Default::default(),
            address: item?.1.address,
        };
        // TODO: check that cosmos cosmwasm module has this bug or not
        // There is a bug in terra/core.
        // The bug happens when we do query_delegation() but there are no delegation pair (delegator-validator)
        // but query_delegation() fails with a parse error cause terra/core returns an empty FullDelegation struct
        // instead of a nil pointer to the struct.
        // https://github.com/terra-money/core/blob/58602320d2907814cfccdf43e9679468bb4bd8d3/x/staking/wasm/interface.go#L227
        // So we do query_all_delegations() instead of query_delegation().unwrap()
        // and try to find delegation in the returned vec
        validator.total_delegated = *delegations
            .get(&validator.address)
            .unwrap_or(&Uint128::zero());
        validators.push(validator);
    }
    Ok(validators)
}
