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

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{from_slice, to_vec, Addr, Decimal, Order, StdError, StdResult, Storage, Timestamp, Uint128};
use cosmwasm_storage::{Bucket, PrefixedStorage, ReadonlyBucket, ReadonlyPrefixedStorage};
use nexus_validator_registary::registry::ValidatorResponse;
use cosmwasm_std::Uint256;
use cw_storage_plus::{Item, Map, SnapshotMap, Strategy};

use basset::hub::{
    CoinDenom,Config, CurrentBatch, Parameters , State, UnbondHistory, UnbondRequest, UnbondWaitEntity
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
pub const CONFIG: Item<Config> = Item::new("config");
pub const STAKERINFO:Map<String, StakerInfo> = Map::new("stakerInfo");
pub const PARAMETERS: Item<Parameters> = Item::new("parameters");
pub const CURRENT_BATCH: Item<CurrentBatch> = Item::new("current_batch");
pub const STATE: Item<State> = Item::new("state");
pub static PREFIX_STAKER: &[u8] = b"staker_v3";
pub static LOCK_INFO: &[u8] = b"locking_users";
// Contains whitelisted address which are allowed to pause (but not unpause) the contracts
pub const GUARDIANS: Map<String, bool> = Map::new("guardians");
pub const LPTOKENS: Map<String, Uint128>=Map::new("lptokens");
pub const RESTAKETOKENS: Map<String, Uint128>=Map::new("restaketokens");
pub static PREFIX_WAIT_MAP: &[u8] = b"wait";
pub static UNBOND_HISTORY_MAP: &[u8] = b"history_map";
pub const TOKEN_SUPPLY: Map<&str, Uint128> = Map::new("token_supply");
pub const DENOM:Item<CoinDenom> =Item::new("denom");
pub static PREFIX_REWARD: &[u8] = b"reward_v3";

pub const MAX_DEFAULT_RANGE_LIMIT: u32 = 1000;
pub static PREFIX_POOL_INFO: &[u8] = b"pool_info_v3";
// pub const STAKED_BALANCES: SnapshotMap<(&[u8], &Addr), Uint128> = SnapshotMap::new(
//     "staked_balances",
//     "staked_balance__checkpoints",
//     "staked_balance__changelog",
//     Strategy::EveryBlock,
// );

// pub const STAKED_TOTAL: SnapshotMap<&[u8], Uint128> = SnapshotMap::new(
//     "total_staked",
//     "total_staked__checkpoints",
//     "total_staked__changelog",
//     Strategy::EveryBlock,
// );

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StakerInfo {
    pub amount_staked_unibi: Uint128,
    pub amount_stnibi_balance: Uint128,
    pub bonding_time:Uint128,
    pub epoch_period:Uint128,
    pub validator_list:Vec<ValidatorResponse>
}


// pub fn read_pool_info(storage: &dyn Storage, asset_key: &[u8]) -> StdResult<PoolInfo> {
//     ReadonlyBucket::new(storage, PREFIX_POOL_INFO).load(asset_key)
// }


// pub fn rewards_read<'a>(storage: &'a dyn Storage, mut staker: &[u8]) -> ReadonlyBucket<'a, RewardInfo> {
//     ReadonlyBucket::multilevel(storage, &[PREFIX_REWARD,  staker])
// }
// pub fn rewards_store<'a>(storage: &'a mut dyn Storage, staker: &[u8]) -> Bucket<'a, RewardInfo> {
//     Bucket::multilevel(storage, &[PREFIX_REWARD, staker])
// }

// pub fn store_pool_info(
//     storage: &mut dyn Storage,
//     asset_key: &[u8],
//     pool_info: &PoolInfo,
// ) -> StdResult<()> {
//     Bucket::new(storage, PREFIX_POOL_INFO).save(asset_key, pool_info)
// }
// /// returns a bucket with all stakers belong by this staker (query it by staker)
// pub fn stakers_store<'a>(storage: &'a mut dyn Storage, asset_key: &[u8]) -> Bucket<'a, bool> {
//     Bucket::multilevel(storage, &[PREFIX_STAKER, asset_key])
// }

/// Store undelegation wait list per each batch
/// HashMap<user's address, <batch_id, requested_amount>
pub fn store_unbond_wait_list(
    storage: &mut dyn Storage,
    batch_id: u64,
    sender_address: String,
    amount: Uint128,
) -> StdResult<()> {
    let batch = to_vec(&batch_id)?;
    let addr = to_vec(&sender_address)?;
    let mut position_indexer: Bucket<UnbondWaitEntity> =
        Bucket::multilevel(storage, &[PREFIX_WAIT_MAP, &addr]);
    position_indexer.update(&batch, |asked_already| -> StdResult<UnbondWaitEntity> {
        let mut wl = asked_already.unwrap_or_default();
        wl.stnibi_amount += amount;
        Ok(wl)
    })?;

    Ok(())
}
// pub fn insert_lock_info(
//     storage: &mut dyn Storage,
//     asset_key: &[u8],
//     user: &[u8],
//     lock_info: LockInfo,
// ) -> StdResult<()> {
//     Bucket::multilevel(storage, &[LOCK_INFO, asset_key, user]).save(
//         &lock_info.unlock_time.seconds().to_be_bytes(),
//         &lock_info.amount,
//     )
// }
pub fn remove_and_accumulate_lock_info(
    storage: &mut dyn Storage,
    asset_key: &[u8],
    user: &[u8],
    timestamp: Timestamp,
) -> StdResult<Uint128> {
    let mut bucket = Bucket::<Uint128>::multilevel(storage, &[LOCK_INFO, asset_key, user]);
    let mut remove_timestamps = vec![];
    let mut accumulate_amount = Uint128::zero();

    // use temporay cursor
    {
        let mut cursor = bucket.range(None, None, Order::Ascending);
        let time_in_seconds = timestamp.seconds().to_be_bytes().to_vec();
        while let Some(Ok((time, amount))) = cursor.next() {
            if time.cmp(&time_in_seconds) == std::cmp::Ordering::Greater {
                break;
            }
            remove_timestamps.push(time);
            accumulate_amount += amount;
        }
    }

    // remove timestamp
    for time in remove_timestamps {
        bucket.remove(&time);
    }

    Ok(accumulate_amount)
}

/// Remove unbond batch id from user's wait list
pub fn remove_unbond_wait_list(
    storage: &mut dyn Storage,
    batch_id: Vec<u64>,
    sender_address: String,
) -> StdResult<()> {
    let addr = to_vec(&sender_address)?;
    let mut position_indexer: Bucket<UnbondWaitEntity> =
        Bucket::multilevel(storage, &[PREFIX_WAIT_MAP, &addr]);
    for b in batch_id {
        let batch = to_vec(&b)?;
        position_indexer.remove(&batch);
    }
    Ok(())
}

pub fn read_unbond_wait_list(
    storage: &dyn Storage,
    batch_id: u64,
    sender_addr: String,
) -> StdResult<UnbondWaitEntity> {
    let vec = to_vec(&sender_addr)?;
    let res: ReadonlyBucket<UnbondWaitEntity> =
        ReadonlyBucket::multilevel(storage, &[PREFIX_WAIT_MAP, &vec]);
    let batch = to_vec(&batch_id)?;
    let wl = res.load(&batch)?;
    Ok(wl)
}

pub fn get_unbond_requests(storage: &dyn Storage, sender_addr: String) -> StdResult<UnbondRequest> {
    let vec = to_vec(&sender_addr)?;
    let mut requests: UnbondRequest = vec![];
    let res: ReadonlyBucket<UnbondWaitEntity> =
        ReadonlyBucket::multilevel(storage, &[PREFIX_WAIT_MAP, &vec]);
    for item in res.range(None, None, Order::Ascending) {
        let (k, value) = item?;
        let user_batch: u64 = from_slice(&k)?;
        requests.push((user_batch, value.stnibi_amount))
    }
    Ok(requests)
}

/// Return all requested unbond amount.
/// This needs to be called after process withdraw rate function.
/// If the batch is released, this will return user's requested
/// amount proportional to withdraw rate.
pub fn get_finished_amount(
    storage: &dyn Storage,
    sender_addr: String,
) -> StdResult<(Uint128, Vec<u64>)> {
    let vec = to_vec(&sender_addr)?;
    let mut withdrawable_amount: Uint128 = Uint128::zero();
    let mut deprecated_batches: Vec<u64> = vec![];
    let res: ReadonlyBucket<UnbondWaitEntity> =
        ReadonlyBucket::multilevel(storage, &[PREFIX_WAIT_MAP, &vec]);
    for item in res.range(None, None, Order::Ascending) {
        let (k, v) = item?;
        let user_batch: u64 = from_slice(&k)?;
        let history = read_unbond_history(storage, user_batch);
        if let Ok(h) = history {
            if h.released {
                withdrawable_amount += v.stnibi_amount * h.stnibi_withdraw_rate;
                deprecated_batches.push(user_batch);
            }
        }
    }
    Ok((withdrawable_amount, deprecated_batches))
}

/// Return the finished amount for all batches that has been before the given block time.
pub fn query_get_finished_amount(
    storage: &dyn Storage,
    sender_addr: String,
    block_time: u64,
) -> StdResult<Uint128> {
    let vec = to_vec(&sender_addr)?;
    let mut withdrawable_amount: Uint128 = Uint128::zero();
    let res: ReadonlyBucket<UnbondWaitEntity> =
        ReadonlyBucket::multilevel(storage, &[PREFIX_WAIT_MAP, &vec]);
    for item in res.range(None, None, Order::Ascending) {
        let (k, v) = item?;
        let user_batch: u64 = from_slice(&k)?;
        let history = read_unbond_history(storage, user_batch);
        if let Ok(h) = history {
            if h.time < block_time {
                withdrawable_amount += v.stnibi_amount * h.stnibi_withdraw_rate;
            }
        }
    }
    Ok(withdrawable_amount)
}

/// Store unbond history map
/// Hashmap<batch_id, <UnbondHistory>>
pub fn store_unbond_history(
    storage: &mut dyn Storage,
    batch_id: u64,
    history: UnbondHistory,
) -> StdResult<()> {
    let vec = batch_id.to_be_bytes().to_vec();
    let value: Vec<u8> = to_vec(&history)?;
    PrefixedStorage::new(storage, UNBOND_HISTORY_MAP).set(&vec, &value);
    Ok(())
}

#[allow(clippy::needless_lifetimes)]
pub fn read_unbond_history(storage: &dyn Storage, epoch_id: u64) -> StdResult<UnbondHistory> {
    let vec = epoch_id.to_be_bytes().to_vec();
    let res = ReadonlyPrefixedStorage::new(storage, UNBOND_HISTORY_MAP).get(&vec);
    match res {
        Some(data) => from_slice(&data),
        None => Err(StdError::generic_err(
            "Burn requests not found for the specified time period",
        )),
    }
}

// settings for pagination
const MAX_LIMIT: u32 = 100;
const DEFAULT_LIMIT: u32 = 10;

/// Return all unbond_history from UnbondHistory map
#[allow(clippy::needless_lifetimes)]
pub fn all_unbond_history(
    storage: &dyn Storage,
    start: Option<u64>,
    limit: Option<u32>,
) -> StdResult<Vec<UnbondHistory>> {
    let vec = convert(start);

    let lim = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let res: StdResult<Vec<UnbondHistory>> =
        ReadonlyPrefixedStorage::new(storage, UNBOND_HISTORY_MAP)
            .range(vec.as_deref(), None, Order::Ascending)
            .take(lim)
            .map(|item| {
                let history: StdResult<UnbondHistory> = from_slice(&item.1);
                history
            })
            .collect();
    res
}

fn convert(start_after: Option<u64>) -> Option<Vec<u8>> {
    start_after.map(|idx| {
        let mut v = idx.to_be_bytes().to_vec();
        v.push(1);
        v
    })
}
