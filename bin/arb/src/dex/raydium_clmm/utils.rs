use crate::dex::get_transfer_fee;
use crate::dex::raydium_clmm::big_num::U128;
use crate::dex::raydium_clmm::state::{
    AmmConfig, PoolState, TickArrayBitmapExtension, TickArrayState, TickState,
    FEE_RATE_DENOMINATOR_VALUE, TICK_ARRAY_SEED,
};
use crate::dex::raydium_clmm::tick_math::{MAX_TICK, MIN_TICK};
use crate::dex::raydium_clmm::{liquidity_math, swap_math, tick_math};
use crate::interface::DexType;
use anyhow::Result;
use bincode::config;
use borsh::BorshDeserialize;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::account::Account;
use solana_sdk::pubkey::Pubkey;
use spl_token_2022::extension::transfer_fee::TransferFeeConfig;
use std::collections::VecDeque;
use std::ops::{DerefMut, Neg};
use std::sync::Arc;
use tracing::{error, info};

// the top level state of the swap, the results of which are recorded in storage at the end
#[derive(Debug)]
pub struct SwapState {
    // the amount remaining to be swapped in/out of the input/output asset
    pub amount_specified_remaining: u64,
    // the amount already swapped out/in of the output/input asset
    pub amount_calculated: u64,
    // current sqrt(price)
    pub sqrt_price_x64: u128,
    // the tick associated with the current price
    pub tick: i32,
    // the current liquidity in range
    pub liquidity: u128,
    pub fee_amount: u64,
}
#[derive(Default)]
struct StepComputations {
    // the price at the beginning of the step
    sqrt_price_start_x64: u128,
    // the next tick to swap to from the current tick in the swap direction
    tick_next: i32,
    // whether tick_next is initialized or not
    initialized: bool,
    // sqrt(price) for the next tick (1/0)
    sqrt_price_next_x64: u128,
    // how much is being swapped in in this step
    amount_in: u64,
    // how much is being swapped out
    amount_out: u64,
    // how much fee is being paid in
    fee_amount: u64,
}

pub fn get_out_put_amount_and_remaining_accounts(
    mut input_amount: u64,
    sqrt_price_limit_x64: Option<u128>,
    zero_for_one: bool,
    is_base_input: bool,
    pool_config: &AmmConfig,
    pool_state: &PoolState,
    tickarray_bitmap_extension: &Option<TickArrayBitmapExtension>,
    tick_arrays: &mut VecDeque<TickArrayState>,
) -> Result<(u64, u64, VecDeque<i32>), &'static str> {
    let transfer_fee = get_transfer_fee(
        if zero_for_one {
            &pool_state.token_mint_0
        } else {
            &pool_state.token_mint_1
        },
        pool_state.recent_epoch,
        input_amount,
    );
    input_amount -= transfer_fee;
    let (is_pool_current_tick_array, current_vaild_tick_array_start_index) = pool_state
        .get_first_initialized_tick_array(tickarray_bitmap_extension, zero_for_one)
        .unwrap();

    let (amount_calculated, fee_amount, tick_array_start_index_vec) = swap_compute(
        pool_config,
        zero_for_one,
        is_base_input,
        is_pool_current_tick_array,
        pool_config.trade_fee_rate,
        input_amount,
        current_vaild_tick_array_start_index,
        sqrt_price_limit_x64.unwrap_or(0),
        pool_state,
        tickarray_bitmap_extension,
        tick_arrays,
    )?;
    // println!("tick_array_start_index:{:?}", tick_array_start_index_vec);

    Ok((amount_calculated, fee_amount, tick_array_start_index_vec))
}

fn swap_compute(
    amm_config: &AmmConfig,
    zero_for_one: bool,
    is_base_input: bool,
    is_pool_current_tick_array: bool,
    fee: u32,
    amount_specified: u64,
    current_vaild_tick_array_start_index: i32,
    sqrt_price_limit_x64: u128,
    pool_state: &PoolState,
    tickarray_bitmap_extension: &Option<TickArrayBitmapExtension>,
    tick_arrays: &mut VecDeque<TickArrayState>,
) -> Result<(u64, u64, VecDeque<i32>), &'static str> {
    if amount_specified == 0 {
        return Result::Err("amountSpecified must not be 0");
    }
    let sqrt_price_limit_x64 = if sqrt_price_limit_x64 == 0 {
        if zero_for_one {
            tick_math::MIN_SQRT_PRICE_X64 + 1
        } else {
            tick_math::MAX_SQRT_PRICE_X64 - 1
        }
    } else {
        sqrt_price_limit_x64
    };
    if zero_for_one {
        if sqrt_price_limit_x64 < tick_math::MIN_SQRT_PRICE_X64 {
            return Result::Err("sqrt_price_limit_x64 must greater than MIN_SQRT_PRICE_X64");
        }
        if sqrt_price_limit_x64 >= pool_state.sqrt_price_x64 {
            return Result::Err("sqrt_price_limit_x64 must smaller than current");
        }
    } else {
        if sqrt_price_limit_x64 > tick_math::MAX_SQRT_PRICE_X64 {
            return Result::Err("sqrt_price_limit_x64 must smaller than MAX_SQRT_PRICE_X64");
        }
        if sqrt_price_limit_x64 <= pool_state.sqrt_price_x64 {
            return Result::Err("sqrt_price_limit_x64 must greater than current");
        }
    }
    let mut tick_match_current_tick_array = is_pool_current_tick_array;

    let mut state = SwapState {
        amount_specified_remaining: amount_specified,
        amount_calculated: 0,
        sqrt_price_x64: pool_state.sqrt_price_x64,
        tick: pool_state.tick_current,
        liquidity: pool_state.liquidity,
        fee_amount: 0,
    };

    let mut tick_array_current = tick_arrays.pop_front().unwrap();
    if tick_array_current.start_tick_index != current_vaild_tick_array_start_index {
        return Result::Err("tick array start tick index does not match");
    }
    let mut current_vaild_tick_array_start_index = current_vaild_tick_array_start_index;
    let mut tick_array_start_index_vec = VecDeque::new();
    tick_array_start_index_vec.push_back(tick_array_current.start_tick_index);
    let mut loop_count = 0;
    // loop across ticks until input liquidity is consumed, or the limit price is reached
    while state.amount_specified_remaining != 0
        && state.sqrt_price_x64 != sqrt_price_limit_x64
        && state.tick < tick_math::MAX_TICK
        && state.tick > tick_math::MIN_TICK
    {
        if loop_count > 100 {
            return Result::Err("loop_count limit");
        }
        let mut step = StepComputations::default();
        step.sqrt_price_start_x64 = state.sqrt_price_x64;
        // save the bitmap, and the tick account if it is initialized
        let mut next_initialized_tick = if let Some(tick_state) = tick_array_current
            .next_initialized_tick(state.tick, pool_state.tick_spacing, zero_for_one)
            .unwrap()
        {
            Box::new(*tick_state)
        } else {
            if !tick_match_current_tick_array {
                tick_match_current_tick_array = true;
                Box::new(
                    *tick_array_current
                        .first_initialized_tick(zero_for_one)
                        .unwrap(),
                )
            } else {
                Box::new(TickState::default())
            }
        };
        if !next_initialized_tick.is_initialized() {
            let next_vaild_tick_array_start_index = pool_state
                .next_initialized_tick_array_start_index(
                    tickarray_bitmap_extension,
                    current_vaild_tick_array_start_index,
                    zero_for_one,
                )
                .unwrap();
            let option = tick_arrays.pop_front();

            if let Some(next_vaild_tick_array) = option {
                tick_array_current = next_vaild_tick_array;
            } else {
                // error!("tick_arrays is empty");
            }

            if next_vaild_tick_array_start_index.is_none() {
                return Result::Err("tick array start tick index out of range limit");
            }
            if tick_array_current.start_tick_index != next_vaild_tick_array_start_index.unwrap() {
                return Result::Err("tick array start tick index does not match");
            }
            tick_array_start_index_vec.push_back(tick_array_current.start_tick_index);
            current_vaild_tick_array_start_index = next_vaild_tick_array_start_index.unwrap();
            let mut first_initialized_tick = tick_array_current
                .first_initialized_tick(zero_for_one)
                .unwrap();

            next_initialized_tick = Box::new(*first_initialized_tick.deref_mut());
        }
        step.tick_next = next_initialized_tick.tick;
        step.initialized = next_initialized_tick.is_initialized();
        if step.tick_next < MIN_TICK {
            step.tick_next = MIN_TICK;
        } else if step.tick_next > MAX_TICK {
            step.tick_next = MAX_TICK;
        }

        step.sqrt_price_next_x64 = tick_math::get_sqrt_price_at_tick(step.tick_next).unwrap();

        let target_price = if (zero_for_one && step.sqrt_price_next_x64 < sqrt_price_limit_x64)
            || (!zero_for_one && step.sqrt_price_next_x64 > sqrt_price_limit_x64)
        {
            sqrt_price_limit_x64
        } else {
            step.sqrt_price_next_x64
        };
        let swap_step = swap_math::compute_swap_step(
            state.sqrt_price_x64,
            target_price,
            state.liquidity,
            state.amount_specified_remaining,
            fee,
            is_base_input,
            zero_for_one,
            1,
        )
        .unwrap();
        state.sqrt_price_x64 = swap_step.sqrt_price_next_x64;
        step.amount_in = swap_step.amount_in;
        step.amount_out = swap_step.amount_out;
        step.fee_amount = swap_step.fee_amount;

        if is_base_input {
            state.amount_specified_remaining = state
                .amount_specified_remaining
                .checked_sub(step.amount_in + step.fee_amount)
                .unwrap();
            state.amount_calculated = state
                .amount_calculated
                .checked_add(step.amount_out)
                .unwrap();
        } else {
            state.amount_specified_remaining = state
                .amount_specified_remaining
                .checked_sub(step.amount_out)
                .unwrap();
            state.amount_calculated = state
                .amount_calculated
                .checked_add(step.amount_in + step.fee_amount)
                .unwrap();
        }
        let step_fee_amount = step.fee_amount;
        if amm_config.protocol_fee_rate > 0 {
            let delta = U128::from(step_fee_amount)
                .checked_mul(amm_config.protocol_fee_rate.into())
                .unwrap()
                .checked_div(FEE_RATE_DENOMINATOR_VALUE.into())
                .unwrap()
                .as_u64();
            step.fee_amount = step.fee_amount.checked_sub(delta).unwrap();
        }
        if amm_config.fund_fee_rate > 0 {
            let delta = U128::from(step_fee_amount)
                .checked_mul(amm_config.fund_fee_rate.into())
                .unwrap()
                .checked_div(FEE_RATE_DENOMINATOR_VALUE.into())
                .unwrap()
                .as_u64();
            step.fee_amount = step.fee_amount.checked_sub(delta).unwrap();
        }
        if state.liquidity > 0 {
            state.fee_amount = state.fee_amount.checked_add(step.fee_amount).unwrap();
        }
        if state.sqrt_price_x64 == step.sqrt_price_next_x64 {
            // if the tick is initialized, run the tick transition
            if step.initialized {
                let mut liquidity_net = next_initialized_tick.liquidity_net;
                if zero_for_one {
                    liquidity_net = liquidity_net.neg();
                }
                let result = liquidity_math::add_delta(state.liquidity, liquidity_net);
                match result {
                    Ok(result) => state.liquidity = result,
                    Err(e) => {
                        error!(
                            "clmm quote error: {}, pool_id:{}",
                            e,
                            tickarray_bitmap_extension.as_ref().unwrap().pool_id
                        );
                    }
                };
            }

            state.tick = if zero_for_one {
                step.tick_next - 1
            } else {
                step.tick_next
            };
        } else if state.sqrt_price_x64 != step.sqrt_price_start_x64 {
            // recompute unless we're on a lower tick boundary (i.e. already transitioned ticks), and haven't moved
            state.tick = tick_math::get_tick_at_sqrt_price(state.sqrt_price_x64).unwrap();
        }
        loop_count += 1;
    }

    Ok((
        state.amount_calculated,
        state.fee_amount,
        tick_array_start_index_vec,
    ))
}

pub fn load_cur_and_next_specify_count_tick_array_key(
    load_count: u8,
    pool_id: &Pubkey,
    pool_state: &PoolState,
    tickarray_bitmap_extension: &Option<TickArrayBitmapExtension>,
    zero_for_one: bool,
) -> Option<Vec<Pubkey>> {
    // 获取当前可用 tick array 的 start index
    let (_, mut current_vaild_tick_array_start_index) = pool_state
        .get_first_initialized_tick_array(tickarray_bitmap_extension, zero_for_one)
        .unwrap();
    let mut tick_array_keys = Vec::with_capacity(load_count as usize);

    tick_array_keys.push(
        Pubkey::find_program_address(
            &[
                TICK_ARRAY_SEED.as_bytes(),
                pool_id.to_bytes().as_ref(),
                &current_vaild_tick_array_start_index.to_be_bytes(),
            ],
            DexType::RaydiumCLMM.get_ref_program_id(),
        )
        .0,
    );
    let mut max_array_size = load_count;
    while max_array_size != 0 {
        let next_tick_array_index = pool_state
            .next_initialized_tick_array_start_index(
                tickarray_bitmap_extension,
                current_vaild_tick_array_start_index,
                zero_for_one,
            )
            .unwrap();
        if next_tick_array_index.is_none() {
            break;
        }
        current_vaild_tick_array_start_index = next_tick_array_index.unwrap();
        tick_array_keys.push(
            Pubkey::find_program_address(
                &[
                    TICK_ARRAY_SEED.as_bytes(),
                    pool_id.to_bytes().as_ref(),
                    &current_vaild_tick_array_start_index.to_be_bytes(),
                ],
                DexType::RaydiumCLMM.get_ref_program_id(),
            )
            .0,
        );
        max_array_size -= 1;
    }
    (!tick_array_keys.is_empty()).then_some(tick_array_keys)
}
