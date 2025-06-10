use crate::dex::orca_whirlpools::error::CoreError;
use crate::dex::orca_whirlpools::{
    get_oracle_address, get_tick_array_address, get_tick_array_start_tick_index,
    swap_quote_by_input_token, ExactInSwapQuote, Oracle, OracleFacade, TickArray, TickArrayFacade,
    TickFacade, TransferFee, Whirlpool, WhirlpoolFacade, TICK_ARRAY_SIZE,
};
use crate::global_cache::{get_account_data, get_clock, get_token2022_data};
use solana_sdk::pubkey::Pubkey;
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::error;

pub fn quote(
    amount_in: u64,
    swap_direction: bool,
    pool_id: &Pubkey,
    pool: Whirlpool,
) -> Option<u64> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let pool = WhirlpoolFacade::from(pool);
    let tick_arrays = get_tick_arrays_or_default(
        pool_id,
        pool.tick_current_index,
        pool.tick_spacing,
        swap_direction,
    )
    .unwrap();
    match swap_quote_by_input_token(
        amount_in,
        swap_direction,
        pool,
        get_oracle_account(pool_id, &pool),
        tick_arrays.into(),
        timestamp,
        get_current_transfer_fee(&pool.token_mint_a),
        get_current_transfer_fee(&pool.token_mint_b),
    ) {
        Ok(quote_result) => Some(quote_result.token_est_out),
        Err(e) => {
            error!("【OracWhirl】Quote失败，原因：{}", e);
            None
        }
    }
}

fn get_current_transfer_fee(mint: &Pubkey) -> Option<TransferFee> {
    get_token2022_data(mint).map_or(None, |transfer_fee_config| {
        let fee = transfer_fee_config.get_epoch_fee(get_clock().unwrap().epoch);
        Some(TransferFee {
            fee_bps: fee.transfer_fee_basis_points.into(),
            max_fee: fee.maximum_fee.into(),
        })
    })
}

fn get_oracle_account(pool_id: &Pubkey, pool: &WhirlpoolFacade) -> Option<OracleFacade> {
    if pool.is_initialized_with_adaptive_fee() {
        get_account_data::<Oracle>(&get_oracle_address(pool_id).unwrap())
            .map_or(None, |oracle_account| {
                Some(OracleFacade::from(oracle_account))
            })
    } else {
        None
    }
}

fn get_tick_arrays_or_default(
    whirlpool_address: &Pubkey,
    tick_current_index: i32,
    tick_spacing: u16,
    swap_direction: bool,
) -> anyhow::Result<[TickArrayFacade; 3]> {
    let tick_array_start_index = get_tick_array_start_tick_index(tick_current_index, tick_spacing);
    let offset = tick_spacing as i32 * TICK_ARRAY_SIZE as i32;

    let tick_array_indexes = if swap_direction {
        [
            tick_array_start_index,
            tick_array_start_index - offset,
            tick_array_start_index - offset * 2,
            // tick_array_start_index - offset * 3,
            // tick_array_start_index - offset * 4,
            // tick_array_start_index - offset * 5,
        ]
    } else {
        [
            tick_array_start_index,
            tick_array_start_index + offset,
            tick_array_start_index + offset * 2,
            // tick_array_start_index + offset * 3,
            // tick_array_start_index + offset * 4,
            // tick_array_start_index + offset * 5,
        ]
    };

    let tick_arrays = tick_array_indexes
        .iter()
        .zip(0..6)
        .map(|(tick_index, i)| {
            let key = get_tick_array_address(whirlpool_address, *tick_index)
                .unwrap()
                .0;
            get_account_data::<TickArray>(&key).map_or(
                uninitialized_tick_array(tick_array_indexes[i as usize]),
                |tick_array| TickArrayFacade::from(tick_array),
            )
        })
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    Ok(tick_arrays)
}

fn uninitialized_tick_array(start_tick_index: i32) -> TickArrayFacade {
    TickArrayFacade {
        start_tick_index,
        ticks: [TickFacade::default(); TICK_ARRAY_SIZE],
    }
}
