use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint128, WasmQuery,
    QueryRequest, Order, StdError,
};
use cw2::set_contract_version;
use cw20::{Cw20QueryMsg, BalanceResponse as Cw20BalanceResponse};

use crate::error::ContractError;
use crate::msg::{
    ExecuteMsg, InstantiateMsg, QueryMsg, PointsResponse, WhitelistResponse, StNibiBalanceResponse,
    AdminResponse, StNibiTokenResponse, TotalPointsResponse, WhitelistedAddressesResponse,
};
use crate::state::{ADMIN, WHITELIST, POINTS, Config, CONFIG};

const CONTRACT_NAME: &str = "nibiru-point-system";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let admin = deps.api.addr_validate(&msg.admin)?;
    ADMIN.save(deps.storage, &admin)?;

    let config = Config {
        st_nibi_token: deps.api.addr_validate(&msg.st_nibi_token)?,
    };
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("admin", msg.admin)
        .add_attribute("st_nibi_token", msg.st_nibi_token))
}

pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::AddToWhitelist { address } => execute_add_to_whitelist(deps, info, address),
        ExecuteMsg::RemoveFromWhitelist { address } => execute_remove_from_whitelist(deps, info, address),
        ExecuteMsg::AddPoints { address, points } => execute_add_points(deps, info, address, points),
        ExecuteMsg::SubtractPoints { address, points } => execute_subtract_points(deps, info, address, points),
        ExecuteMsg::TransferPoints { from, to, points } => execute_transfer_points(deps, info, from, to, points),
        ExecuteMsg::UpdateAdmin { new_admin } => execute_update_admin(deps, info, new_admin),
        ExecuteMsg::UpdateStNibiToken { new_token } => execute_update_st_nibi_token(deps, info, new_token),
    }
}

pub fn execute_add_to_whitelist(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let admin = ADMIN.load(deps.storage)?;
    if info.sender != admin {
        return Err(ContractError::Unauthorized {});
    }

    let addr = deps.api.addr_validate(&address)?;
    WHITELIST.save(deps.storage, &addr, &true)?;

    Ok(Response::new()
        .add_attribute("action", "add_to_whitelist")
        .add_attribute("address", address))
}

pub fn execute_remove_from_whitelist(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let admin = ADMIN.load(deps.storage)?;
    if info.sender != admin {
        return Err(ContractError::Unauthorized {});
    }

    let addr = deps.api.addr_validate(&address)?;
    WHITELIST.remove(deps.storage, &addr);

    Ok(Response::new()
        .add_attribute("action", "remove_from_whitelist")
        .add_attribute("address", address))
}

pub fn execute_add_points(
    deps: DepsMut,
    _info: MessageInfo,
    address: String,
    points: u64,
) -> Result<Response, ContractError> {
    let addr = deps.api.addr_validate(&address)?;
    if !WHITELIST.may_load(deps.storage, &addr)?.unwrap_or(false) {
        return Err(ContractError::NotWhitelisted {});
    }

    POINTS.update(deps.storage, &addr, |existing| -> StdResult<u64> {
        Ok(existing.unwrap_or(0) + points)
    })?;

    Ok(Response::new()
        .add_attribute("action", "add_points")
        .add_attribute("address", address)
        .add_attribute("points", points.to_string()))
}

pub fn execute_subtract_points(
    deps: DepsMut,
    _info: MessageInfo,
    address: String,
    points: u64,
) -> Result<Response, ContractError> {
    let addr = deps.api.addr_validate(&address)?;
    if !WHITELIST.may_load(deps.storage, &addr)?.unwrap_or(false) {
        return Err(ContractError::NotWhitelisted {});
    }

    POINTS.update(deps.storage, &addr, |existing| -> StdResult<u64> {
        let current = existing.unwrap_or(0);
        if current < points {
            return Err(StdError::generic_err("Not enough points"));
        }
        Ok(current - points)
    })?;

    Ok(Response::new()
        .add_attribute("action", "subtract_points")
        .add_attribute("address", address)
        .add_attribute("points", points.to_string()))
}

pub fn execute_transfer_points(
    deps: DepsMut,
    info: MessageInfo,
    from: String,
    to: String,
    points: u64,
) -> Result<Response, ContractError> {
    let from_addr = deps.api.addr_validate(&from)?;
    let to_addr = deps.api.addr_validate(&to)?;

    if info.sender != from_addr {
        return Err(ContractError::Unauthorized {});
    }

    if !WHITELIST.may_load(deps.storage, &from_addr)?.unwrap_or(false) ||
       !WHITELIST.may_load(deps.storage, &to_addr)?.unwrap_or(false) {
        return Err(ContractError::NotWhitelisted {});
    }

    POINTS.update(deps.storage, &from_addr, |existing| -> StdResult<u64> {
        let current = existing.unwrap_or(0);
        if current < points {
            return Err(StdError::generic_err("Not enough points"));
        }
        Ok(current - points)
    })?;

    POINTS.update(deps.storage, &to_addr, |existing| -> StdResult<u64> {
        Ok(existing.unwrap_or(0) + points)
    })?;

    Ok(Response::new()
        .add_attribute("action", "transfer_points")
        .add_attribute("from", from)
        .add_attribute("to", to)
        .add_attribute("points", points.to_string()))
}

pub fn execute_update_admin(
    deps: DepsMut,
    info: MessageInfo,
    new_admin: String,
) -> Result<Response, ContractError> {
    let admin = ADMIN.load(deps.storage)?;
    if info.sender != admin {
        return Err(ContractError::Unauthorized {});
    }

    let new_admin_addr = deps.api.addr_validate(&new_admin)?;
    ADMIN.save(deps.storage, &new_admin_addr)?;

    Ok(Response::new()
        .add_attribute("action", "update_admin")
        .add_attribute("new_admin", new_admin))
}

pub fn execute_update_st_nibi_token(
    deps: DepsMut,
    info: MessageInfo,
    new_token: String,
) -> Result<Response, ContractError> {
    let admin = ADMIN.load(deps.storage)?;
    if info.sender != admin {
        return Err(ContractError::Unauthorized {});
    }

    let new_token_addr = deps.api.addr_validate(&new_token)?;
    CONFIG.update(deps.storage, |mut config| -> StdResult<_> {
        config.st_nibi_token = new_token_addr;
        Ok(config)
    })?;

    Ok(Response::new()
        .add_attribute("action", "update_st_nibi_token")
        .add_attribute("new_token", new_token))
}

pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetPoints { address } => to_binary(&query_points(deps, address)?),
        QueryMsg::IsWhitelisted { address } => to_binary(&query_whitelist(deps, address)?),
        QueryMsg::CheckStNibiBalance { address } => to_binary(&query_st_nibi_balance(deps, address)?),
        QueryMsg::GetAdmin {} => to_binary(&query_admin(deps)?),
        QueryMsg::GetStNibiToken {} => to_binary(&query_st_nibi_token(deps)?),
        QueryMsg::GetTotalPoints {} => to_binary(&query_total_points(deps)?),
        QueryMsg::GetWhitelistedAddresses {} => to_binary(&query_whitelisted_addresses(deps)?),
    }
}
        
fn query_points(deps: Deps, address: String) -> StdResult<PointsResponse> {
    let addr = deps.api.addr_validate(&address)?;
    let points = POINTS.may_load(deps.storage, &addr)?.unwrap_or(0);
    Ok(PointsResponse { address, points })
}

fn query_whitelist(deps: Deps, address: String) -> StdResult<WhitelistResponse> {
    let addr = deps.api.addr_validate(&address)?;
    let is_whitelisted = WHITELIST.may_load(deps.storage, &addr)?.unwrap_or(false);
    Ok(WhitelistResponse { is_whitelisted })
}

fn query_st_nibi_balance(deps: Deps, address: String) -> StdResult<StNibiBalanceResponse> {
    let addr = deps.api.addr_validate(&address)?;
    let config = CONFIG.load(deps.storage)?;
    
    let balance: Cw20BalanceResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.st_nibi_token.to_string(),
        msg: to_binary(&Cw20QueryMsg::Balance { address: addr.to_string() })?,
    }))?;

    Ok(StNibiBalanceResponse {
        address,
        has_balance: balance.balance > Uint128::zero(),
    })
}

fn query_admin(deps: Deps) -> StdResult<AdminResponse> {
    let admin = ADMIN.load(deps.storage)?;
    Ok(AdminResponse { admin: admin.to_string() })
}

fn query_st_nibi_token(deps: Deps) -> StdResult<StNibiTokenResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(StNibiTokenResponse { token: config.st_nibi_token.to_string() })
}

fn query_total_points(deps: Deps) -> StdResult<TotalPointsResponse> {
    let total: u64 = POINTS
        .range(deps.storage, None, None, Order::Ascending)
        .map(|item| item.map(|(_, points)| points))
        .sum::<StdResult<u64>>()?;
    Ok(TotalPointsResponse { total })
}

fn query_whitelisted_addresses(deps: Deps) -> StdResult<WhitelistedAddressesResponse> {
    let addresses: StdResult<Vec<String>> = WHITELIST
        .range(deps.storage, None, None, Order::Ascending)
        .map(|item| item.map(|(addr, _)| addr.to_string()))
        .collect();
    Ok(WhitelistedAddressesResponse { addresses: addresses? })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();
        let info = mock_info("creator", &coins(1000, "earth"));
        let msg = InstantiateMsg { 
            admin: "admin".to_string(),
            st_nibi_token: "st_nibi_token".to_string(),
        };
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(2, res.attributes.len());
    }

    #[test]
    fn add_to_whitelist() {
        let mut deps = mock_dependencies();
        let info = mock_info("admin", &[]);
        let msg = InstantiateMsg { 
            admin: "admin".to_string(),
            st_nibi_token: "st_nibi_token".to_string(),
        };
        let _ = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        let msg = ExecuteMsg::AddToWhitelist { address: "user".to_string() };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(2, res.attributes.len());

        let res = query(deps.as_ref(), mock_env(), QueryMsg::IsWhitelisted { address: "user".to_string() }).unwrap();
        let value: WhitelistResponse = from_binary(&res).unwrap();
        assert!(value.is_whitelisted);
    }

    #[test]
    fn add_and_transfer_points() {
        let mut deps = mock_dependencies();
        let info = mock_info("admin", &[]);
        let msg = InstantiateMsg { 
            admin: "admin".to_string(),
            st_nibi_token: "st_nibi_token".to_string(),
        };
        let _ = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        // Add users to whitelist
        let _ = execute(deps.as_mut(), mock_env(), info.clone(), ExecuteMsg::AddToWhitelist { address: "user1".to_string() }).unwrap();
        let _ = execute(deps.as_mut(), mock_env(), info.clone(), ExecuteMsg::AddToWhitelist { address: "user2".to_string() }).unwrap();

        // Add points to user1
        let msg = ExecuteMsg::AddPoints { address: "user1".to_string(), points: 100 };
        let _ = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        // Transfer points from user1 to user2
        let msg = ExecuteMsg::TransferPoints { 
            from: "user1".to_string(), 
            to: "user2".to_string(), 
            points: 50 
        };
        let info = mock_info("user1", &[]);
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(4, res.attributes.len());

        // Check points for both users
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetPoints { address: "user1".to_string() }).unwrap();
        let value: PointsResponse = from_binary(&res).unwrap();
        assert_eq!(50, value.points);

        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetPoints { address: "user2".to_string() }).unwrap();
        let value: PointsResponse = from_binary(&res).unwrap();
        assert_eq!(50, value.points);
    }

    #[test]
    fn update_admin() {
        let mut deps = mock_dependencies();
        let info = mock_info("admin", &[]);
        let msg = InstantiateMsg { 
            admin: "admin".to_string(),
            st_nibi_token: "st_nibi_token".to_string(),
        };
        let _ = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        let msg = ExecuteMsg::UpdateAdmin { new_admin: "new_admin".to_string() };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(2, res.attributes.len());

        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetAdmin {}).unwrap();
        let value: AdminResponse = from_binary(&res).unwrap();
        assert_eq!("new_admin", value.admin);
    }

    #[test]
    fn query_total_points_and_whitelisted_addresses() {
        let mut deps = mock_dependencies();
        let info = mock_info("admin", &[]);
        let msg = InstantiateMsg { 
            admin: "admin".to_string(),
            st_nibi_token: "st_nibi_token".to_string(),
        };
        let _ = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        // Add users to whitelist and give them points
        let _ = execute(deps.as_mut(), mock_env(), info.clone(), ExecuteMsg::AddToWhitelist { address: "user1".to_string() }).unwrap();
        let _ = execute(deps.as_mut(), mock_env(), info.clone(), ExecuteMsg::AddToWhitelist { address: "user2".to_string() }).unwrap();
        let _ = execute(deps.as_mut(), mock_env(), info.clone(), ExecuteMsg::AddPoints { address: "user1".to_string(), points: 100 }).unwrap();
        let _ = execute(deps.as_mut(), mock_env(), info.clone(), ExecuteMsg::AddPoints { address: "user2".to_string(), points: 150 }).unwrap();

        // Query total points
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetTotalPoints {}).unwrap();
        let value: TotalPointsResponse = from_binary(&res).unwrap();
        assert_eq!(250, value.total);

        // Query whitelisted addresses
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetWhitelistedAddresses {}).unwrap();
        let value: WhitelistedAddressesResponse = from_binary(&res).unwrap();
        assert_eq!(2, value.addresses.len());
        assert!(value.addresses.contains(&"user1".to_string()));
        assert!(value.addresses.contains(&"user2".to_string()));
    }
}