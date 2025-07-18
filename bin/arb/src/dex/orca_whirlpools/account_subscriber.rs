use crate::dex::oracle::get_oracle_address;
use crate::dex::orca_whirlpools::WHIRLPOOL_ID;
use crate::dex::subscriber::{AccountSubscriber, SubscriptionAccounts};
use crate::dex::tick_array::{
    get_tick_array_address, MAX_TICK_INDEX, MIN_TICK_INDEX, TICK_ARRAY_SIZE,
};
use crate::dex::whirlpool::Whirlpool;
use crate::dex::{get_account_data, CLOCK_ID};
use crate::dex_data::DexJson;
use solana_sdk::pubkey::Pubkey;

pub struct OrcaWhirlAccountSubscriber;

impl AccountSubscriber for OrcaWhirlAccountSubscriber {
    fn get_subscription_accounts(&self, dex_json: &[DexJson]) -> Option<SubscriptionAccounts> {
        None
    }
}

fn get_single_pool_all_tick_array_keys(pool_id: &Pubkey) -> anyhow::Result<Vec<Pubkey>> {
    let tick_spacing = get_account_data::<Whirlpool>(pool_id).unwrap().tick_spacing as i32;
    let (min, max) = get_tick_array_start_tick_index_range(tick_spacing);
    let tick_array_keys = (min..=max)
        .step_by((tick_spacing as usize) * TICK_ARRAY_SIZE)
        .into_iter()
        .map(|index| get_tick_array_address(pool_id, index).map(|y| y.0).unwrap())
        .collect::<Vec<_>>();
    Ok(tick_array_keys)
}

fn get_tick_array_start_tick_index_range(tick_spacing: i32) -> (i32, i32) {
    let tick_array_size_i32 = TICK_ARRAY_SIZE as i32;
    let min_index = MIN_TICK_INDEX
        .div_euclid(tick_spacing)
        .div_euclid(tick_array_size_i32);
    let max_index = MAX_TICK_INDEX
        .div_euclid(tick_spacing)
        .div_euclid(tick_array_size_i32);
    (
        min_index * tick_spacing * tick_array_size_i32,
        max_index * tick_spacing * tick_array_size_i32,
    )
}