use crate::dex::raydium_clmm::tick_math::{MAX_TICK, MIN_TICK};
use crate::dex::raydium_clmm::RAYDIUM_CLMM_PROGRAM_ID;
use crate::dex::subscriber::{AccountSubscriber, SubscriptionAccounts};
use crate::dex::{get_account_data, read_from, PoolState, TICK_ARRAY_SEED, TICK_ARRAY_SIZE};
use crate::dex_data::DexJson;
use crate::grpc_subscribe::POOL_TICK_ARRAY_BITMAP_SEED;
use anyhow::anyhow;
use solana_sdk::pubkey::Pubkey;
use std::future::Future;
use tracing::info;

pub struct RaydiumCLMMAccountSubscriber;

impl AccountSubscriber for RaydiumCLMMAccountSubscriber {
    fn get_subscription_accounts(&self, dex_json: &[DexJson]) -> Option<SubscriptionAccounts> {
        None
    }
}

pub fn get_pool_all_tick_array_keys(pool_id: &Pubkey) -> anyhow::Result<Vec<Pubkey>> {
    let pool_state = get_account_data::<PoolState>(pool_id).map_or(Err(anyhow!("")), |t| Ok(t))?;
    let tick_spacing = pool_state.tick_spacing;
    let (min_tick, max_tick) = tick_array_start_index_range(tick_spacing);
    Ok((min_tick..=max_tick)
        .step_by(tick_count(tick_spacing) as usize)
        .into_iter()
        .map(|index| {
            Pubkey::find_program_address(
                &[
                    TICK_ARRAY_SEED.as_bytes(),
                    pool_id.to_bytes().as_ref(),
                    &index.to_be_bytes(),
                ],
                &RAYDIUM_CLMM_PROGRAM_ID,
            )
            .0
        })
        .collect::<Vec<_>>())
}

fn tick_array_start_index_range(tick_spacing: u16) -> (i32, i32) {
    let max_tick_index = get_tick_array_index(MAX_TICK, tick_spacing) + 1;
    let min_tick_index = get_tick_array_index(MIN_TICK, tick_spacing);
    (
        min_tick_index * tick_count(tick_spacing),
        max_tick_index * tick_count(tick_spacing),
    )
}

fn get_tick_array_index(tick_index: i32, tick_spacing: u16) -> i32 {
    let ticks_in_array = tick_count(tick_spacing);
    let mut start = tick_index / ticks_in_array;
    if tick_index < 0 && tick_index % ticks_in_array != 0 {
        start = start - 1
    }
    start
}

pub fn tick_count(tick_spacing: u16) -> i32 {
    TICK_ARRAY_SIZE * i32::from(tick_spacing)
}
