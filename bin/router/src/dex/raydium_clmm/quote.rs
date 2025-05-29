use crate::dex::raydium_clmm::state::{
    pda_bit_map_extension_key, AmmConfig, PoolState, TickArrayBitmapExtension, TickArrayState,
};
use crate::dex::raydium_clmm::utils;
use crate::dex::raydium_clmm::utils::load_cur_and_next_specify_count_tick_array_key;
use solana_sdk::pubkey::Pubkey;
use std::collections::VecDeque;
use tracing::info;

pub fn quote(
    amount_in: u64,
    swap_direction: bool,
    pool_id: &Pubkey,
    pool_state: &PoolState,
) -> Option<u64> {
    let bitmap_extension = Some(get_bitmap_extension(pool_id)?);
    info!("{:#?}", bitmap_extension);
    let mut tick_arrays =
        get_tick_arrays(pool_id, pool_state, &bitmap_extension, swap_direction, 3)?;
    utils::get_out_put_amount_and_remaining_accounts(
        amount_in,
        None,
        swap_direction,
        true,
        &get_amm_config(&pool_state.amm_config)?,
        &pool_state,
        &bitmap_extension,
        &mut tick_arrays,
    )
    .map_or(None, |(amount_out, _, _)| Some(amount_out))
}

fn get_amm_config(amm_config_key: &Pubkey) -> Option<AmmConfig> {
    crate::account_cache::get_account_data::<AmmConfig>(amm_config_key)
}

fn get_bitmap_extension(pool_id: &Pubkey) -> Option<TickArrayBitmapExtension> {
    crate::account_cache::get_account_data::<TickArrayBitmapExtension>(&pda_bit_map_extension_key(
        pool_id,
    ))
}

fn get_tick_arrays(
    pool_id: &Pubkey,
    pool_state: &PoolState,
    tick_array_bitmap_extension: &Option<TickArrayBitmapExtension>,
    swap_direction: bool,
    take_count: u8,
) -> Option<VecDeque<TickArrayState>> {
    let tick_array_keys = load_cur_and_next_specify_count_tick_array_key(
        take_count,
        pool_id,
        pool_state,
        tick_array_bitmap_extension,
        swap_direction,
    );
    if tick_array_keys.as_ref()?.is_empty() {
        return None;
    }
    let expect_count = tick_array_keys.as_ref()?.len();
    let deque = tick_array_keys?
        .into_iter()
        .filter_map(|key| {
            let account_data = crate::account_cache::get_account_data::<TickArrayState>(&key);
            info!("key : {:?}\n{:#?}", key, account_data);
            account_data
        })
        .collect::<VecDeque<_>>();
    if expect_count != deque.len() {
        None
    } else {
        Some(deque)
    }
}
