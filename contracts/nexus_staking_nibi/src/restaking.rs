
use crate::contract::slashing;
use crate::math::decimal_division;
use crate::state::{CONFIG, CURRENT_BATCH, PARAMETERS, STATE};
use basset::hub::{BondType, Parameters};
use cosmwasm_std::{
   attr, to_binary, Addr, Api, Coin, CosmosMsg, DepsMut, Env, MessageInfo, QueryRequest, Response, StakingMsg, StdError, StdResult, Storage, Uint128, WasmMsg, WasmQuery
};
use cw20::Cw20ExecuteMsg;
use nexus_validator_registary::common::calculate_delegations;
use nexus_validator_registary::msg::QueryMsg as QueryValidators;
use nexus_validator_registary::registry::ValidatorResponse;

pub fn execute_restake_bond(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    bond_type: BondType,
) -> Result<Response,StdError> {
    let params: Parameters = PARAMETERS.load(deps.storage)?;
    if params.paused.unwrap_or(false) {
        return Err(StdError::generic_err("the contract is temporarily paused"));
    }
    let config = CONFIG.load(deps.storage)?;
    let restake_coin_denom = String::from("stNIBI");
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
        .find(|x| x.denom == restake_coin_denom && x.amount > Uint128::zero())
        .ok_or_else(|| {
            StdError::generic_err(format!("No {} assets are provided to bond", restake_coin_denom))
        })?;

        let sender = info.sender.clone();

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
        let mut external_call_msgs: Vec<cosmwasm_std::CosmosMsg> = vec![];
        let delegations = calculate_delegations(payment.amount, validators.as_slice())?;
        for i in 0..delegations.len() {
            if delegations[i].is_zero() {
                continue;
            }
            external_call_msgs.push(cosmwasm_std::CosmosMsg::Staking(StakingMsg::Delegate {
                validator: validators[i].address.clone(),
                amount: Coin::new(delegations[i].u128(), payment.denom.as_str()),
            }));
        }
        let res = Response::new()
        .add_messages(external_call_msgs)
        .add_attributes(vec![
            attr("from", sender),
            attr("bonded", payment.amount),
        ]);
    Ok(res)  
}



pub fn execute_restake_bond_test(
    mut deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response,StdError> {
   
   
    // coin must have be sent along with transaction and it should be in underlying coin denom
   

        let sender = info.sender.clone();

   
        let mut external_call_msgs: Vec<cosmwasm_std::CosmosMsg> = vec![];
        
            external_call_msgs.push(cosmwasm_std::CosmosMsg::Staking(StakingMsg::Delegate {
                validator: "nibivaloper1lq3ktemm9rhpu0je850rnlrny752v6yuv4jc6d".to_string(),
                amount: Coin::new(1, "nexusnibif"),
            }));
        // }
        let res = Response::new()
        .add_messages(external_call_msgs)
        .add_attributes(vec![
            attr("from", sender),
            attr("bonded", "1"),
        ]);
    Ok(res)  
}



