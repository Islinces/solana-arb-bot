use crate::dex::orca_whirlpools::accounts::{
    AdaptiveFeeInfo, OracleFacade, TickArrays, TickFacade, WhirlpoolFacade,
};
use crate::dex::orca_whirlpools::error::{
    CoreError, AMOUNT_EXCEEDS_MAX_U64, ARITHMETIC_OVERFLOW, INVALID_ADAPTIVE_FEE_INFO,
    INVALID_SQRT_PRICE_LIMIT_DIRECTION, SQRT_PRICE_LIMIT_OUT_OF_BOUNDS, ZERO_TRADABLE_AMOUNT,
};
use crate::dex::orca_whirlpools::math::{
    sqrt_price_to_tick_index, tick_index_to_sqrt_price, try_apply_swap_fee, try_apply_transfer_fee,
    try_get_amount_delta_a, try_get_amount_delta_b, try_get_next_sqrt_price_from_a,
    try_get_next_sqrt_price_from_b, try_reverse_apply_swap_fee,
    FeeRateManager, TickArraySequence, TransferFee,
};

pub const FEE_RATE_DENOMINATOR: u32 = 1_000_000;
pub const MIN_SQRT_PRICE: u128 = 4295048016;
/// The maximum sqrt price for a whirlpool.
pub const MAX_SQRT_PRICE: u128 = 79226673515401279992447579055;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct ExactInSwapQuote {
    pub token_est_out: u64,
    pub trade_fee: u64,
}

/// Computes the exact input or output amount for a swap transaction.
///
/// # Arguments
/// - `token_in`: The input token amount.
/// - `specified_token_a`: If `true`, the input token is token A. Otherwise, it is token B.
/// - `slippage_tolerance`: The slippage tolerance in basis points.
/// - `whirlpool`: The whirlpool state.
/// - `oracle`: The oracle data for the whirlpool.
/// - `tick_arrays`: The tick arrays needed for the swap.
/// - `timestamp`: The timestamp for the swap.
/// - `transfer_fee_a`: The transfer fee for token A.
/// - `transfer_fee_b`: The transfer fee for token B.
///
/// # Returns
/// The exact input or output amount for the swap transaction.
#[allow(clippy::too_many_arguments)]
pub fn swap_quote_by_input_token(
    token_in: u64,
    specified_token_a: bool,
    whirlpool: WhirlpoolFacade,
    oracle: Option<OracleFacade>,
    tick_arrays: TickArrays,
    timestamp: u64,
    transfer_fee_a: Option<TransferFee>,
    transfer_fee_b: Option<TransferFee>,
) -> Result<ExactInSwapQuote, CoreError> {
    let (transfer_fee_in, transfer_fee_out) = if specified_token_a {
        (transfer_fee_a, transfer_fee_b)
    } else {
        (transfer_fee_b, transfer_fee_a)
    };
    // token2022 fee
    let token_in_after_fee =
        try_apply_transfer_fee(token_in.into(), transfer_fee_in.unwrap_or_default())?;
    // TickArray校验，排序，必须连续
    let tick_sequence = TickArraySequence::new(tick_arrays.into(), whirlpool.tick_spacing)?;

    let swap_result = compute_swap(
        token_in_after_fee.into(),
        0,
        whirlpool,
        tick_sequence,
        specified_token_a,
        true,
        timestamp,
        oracle.map(|oracle| oracle.into()),
    )?;

    let token_est_out_before_fee = if specified_token_a {
        swap_result.token_b
    } else {
        swap_result.token_a
    };

    let amount_out = try_apply_transfer_fee(
        token_est_out_before_fee,
        transfer_fee_out.unwrap_or_default(),
    )?;

    Ok(ExactInSwapQuote {
        token_est_out: amount_out,
        trade_fee: swap_result.trade_fee,
    })
}

pub struct SwapResult {
    pub token_a: u64,
    pub token_b: u64,
    pub trade_fee: u64,
    pub applied_fee_rate_min: u32,
    pub applied_fee_rate_max: u32,
}

/// Computes the amounts of tokens A and B based on the current Whirlpool state and tick sequence.
///
/// # Arguments
/// - `token_amount`: The input or output amount specified for the swap. Must be non-zero.
/// - `sqrt_price_limit`: The price limit for the swap represented as a square root.
///    If set to `0`, it defaults to the minimum or maximum sqrt price based on the direction of the swap.
/// - `whirlpool`: The current state of the Whirlpool AMM, including liquidity, price, and tick information.
/// - `tick_sequence`: A sequence of ticks used to determine price levels during the swap process.
/// - `a_to_b`: Indicates the direction of the swap:
///    - `true`: Swap from token A to token B.
///    - `false`: Swap from token B to token A.
/// - `specified_input`: Determines if the input amount is specified:
///    - `true`: `token_amount` represents the input amount.
///    - `false`: `token_amount` represents the output amount.
/// - `timestamp`: A timestamp used to calculate the adaptive fee rate.
/// - `adaptive_fee_info`: An optional `AdaptiveFeeInfo` struct containing information about the adaptive fee rate.
/// # Returns
/// A `Result` containing a `SwapResult` struct if the swap is successful, or an `ErrorCode` if the computation fails.
/// # Notes
/// - This function doesn't take into account slippage tolerance.
/// - This function doesn't take into account transfer fee extension.
#[allow(clippy::too_many_arguments)]
pub fn compute_swap<const SIZE: usize>(
    token_amount: u64,
    sqrt_price_limit: u128,
    whirlpool: WhirlpoolFacade,
    tick_sequence: TickArraySequence<SIZE>,
    a_to_b: bool,
    specified_input: bool,
    timestamp: u64,
    adaptive_fee_info: Option<AdaptiveFeeInfo>,
) -> Result<SwapResult, CoreError> {
    let sqrt_price_limit = if sqrt_price_limit == 0 {
        if a_to_b {
            MIN_SQRT_PRICE
        } else {
            MAX_SQRT_PRICE
        }
    } else {
        sqrt_price_limit
    };

    if !(MIN_SQRT_PRICE..=MAX_SQRT_PRICE).contains(&sqrt_price_limit) {
        return Err(SQRT_PRICE_LIMIT_OUT_OF_BOUNDS);
    }

    if a_to_b && sqrt_price_limit >= whirlpool.sqrt_price
        || !a_to_b && sqrt_price_limit <= whirlpool.sqrt_price
    {
        return Err(INVALID_SQRT_PRICE_LIMIT_DIRECTION);
    }

    if token_amount == 0 {
        return Err(ZERO_TRADABLE_AMOUNT);
    }
    // 交换后剩余amount
    let mut amount_remaining = token_amount;
    // 可交换出的amount
    let mut amount_calculated = 0u64;
    let mut current_sqrt_price = whirlpool.sqrt_price;
    let mut current_tick_index = whirlpool.tick_current_index;
    let mut current_liquidity = whirlpool.liquidity;
    // fee amount
    let mut trade_fee = 0u64;

    let base_fee_rate = whirlpool.fee_rate;
    let mut applied_fee_rate_min: Option<u32> = None;
    let mut applied_fee_rate_max: Option<u32> = None;
    // 校验开启自适应fee时，adaptive_fee_info是否传了
    if whirlpool.is_initialized_with_adaptive_fee() != adaptive_fee_info.is_some() {
        return Err(INVALID_ADAPTIVE_FEE_INFO);
    }

    let mut fee_rate_manager = FeeRateManager::new(
        a_to_b,
        whirlpool.tick_current_index, // note:  -1 shift is acceptable
        timestamp,
        base_fee_rate,
        &adaptive_fee_info,
    )?;

    while amount_remaining > 0 && sqrt_price_limit != current_sqrt_price {
        // 下一个有流动性的tick
        let (next_tick, next_tick_index) = if a_to_b {
            tick_sequence.prev_initialized_tick(current_tick_index)?
        } else {
            tick_sequence.next_initialized_tick(current_tick_index)?
        };
        // 下一个tick对应价格
        let next_tick_sqrt_price: u128 = tick_index_to_sqrt_price(next_tick_index.into()).into();
        let target_sqrt_price = if a_to_b {
            next_tick_sqrt_price.max(sqrt_price_limit)
        } else {
            next_tick_sqrt_price.min(sqrt_price_limit)
        };

        loop {
            // 更新波动累加器
            fee_rate_manager.update_volatility_accumulator();
            // 费率
            // 静态，静态费率
            // 动态，费率=静态费率+动态费率，最大10w
            let total_fee_rate = fee_rate_manager.get_total_fee_rate();
            applied_fee_rate_min = Some(
                applied_fee_rate_min
                    .unwrap_or(total_fee_rate)
                    .min(total_fee_rate),
            );
            applied_fee_rate_max = Some(
                applied_fee_rate_max
                    .unwrap_or(total_fee_rate)
                    .max(total_fee_rate),
            );

            let (bounded_sqrt_price_target, adaptive_fee_update_skipped) = fee_rate_manager
                .get_bounded_sqrt_price_target(target_sqrt_price, current_liquidity);

            let step_quote = compute_swap_step(
                amount_remaining,
                total_fee_rate,
                current_liquidity,
                current_sqrt_price,
                bounded_sqrt_price_target,
                a_to_b,
                specified_input,
            )?;
            // 累加fee
            trade_fee += step_quote.fee_amount;

            if specified_input {
                // 扣减需要的amount_in
                amount_remaining = amount_remaining
                    .checked_sub(step_quote.amount_in)
                    .ok_or(ARITHMETIC_OVERFLOW)?
                    .checked_sub(step_quote.fee_amount)
                    .ok_or(ARITHMETIC_OVERFLOW)?;
                // 累加amount_out
                amount_calculated = amount_calculated
                    .checked_add(step_quote.amount_out)
                    .ok_or(ARITHMETIC_OVERFLOW)?;
            } else {
                amount_remaining = amount_remaining
                    .checked_sub(step_quote.amount_out)
                    .ok_or(ARITHMETIC_OVERFLOW)?;
                amount_calculated = amount_calculated
                    .checked_add(step_quote.amount_in)
                    .ok_or(ARITHMETIC_OVERFLOW)?
                    .checked_add(step_quote.fee_amount)
                    .ok_or(ARITHMETIC_OVERFLOW)?;
            }
            // 当前tick换完了
            if step_quote.next_sqrt_price == next_tick_sqrt_price {
                // 累加流动性
                current_liquidity = get_next_liquidity(current_liquidity, next_tick, a_to_b);
                // 移动tick
                current_tick_index = if a_to_b {
                    next_tick_index - 1
                } else {
                    next_tick_index
                }
            }
            // 当前tick未换完时，根据价格重新计算当前的tick index
            else if step_quote.next_sqrt_price != current_sqrt_price {
                current_tick_index =
                    sqrt_price_to_tick_index(step_quote.next_sqrt_price.into()).into();
            }
            // 修改价格
            current_sqrt_price = step_quote.next_sqrt_price;

            if !adaptive_fee_update_skipped {
                fee_rate_manager.advance_tick_group();
            } else {
                fee_rate_manager.advance_tick_group_after_skip(
                    current_sqrt_price,
                    next_tick_sqrt_price,
                    next_tick_index,
                );
            }

            // do while loop
            if amount_remaining == 0 || current_sqrt_price == target_sqrt_price {
                break;
            }
        }
    }

    let swapped_amount = token_amount - amount_remaining;

    let token_a = if a_to_b == specified_input {
        swapped_amount
    } else {
        amount_calculated
    };
    let token_b = if a_to_b == specified_input {
        amount_calculated
    } else {
        swapped_amount
    };
    // 价格变动剧烈时记录下时间
    fee_rate_manager.update_major_swap_timestamp(
        timestamp,
        whirlpool.sqrt_price,
        current_sqrt_price,
    );

    Ok(SwapResult {
        token_a,
        token_b,
        trade_fee,
        applied_fee_rate_min: applied_fee_rate_min.unwrap_or(base_fee_rate as u32),
        applied_fee_rate_max: applied_fee_rate_max.unwrap_or(base_fee_rate as u32),
    })
}

// Private functions

fn get_next_liquidity(
    current_liquidity: u128,
    next_tick: Option<&TickFacade>,
    a_to_b: bool,
) -> u128 {
    let liquidity_net = next_tick.map(|tick| tick.liquidity_net).unwrap_or(0);
    let liquidity_net_unsigned = liquidity_net.unsigned_abs();
    if a_to_b {
        if liquidity_net < 0 {
            current_liquidity + liquidity_net_unsigned
        } else {
            current_liquidity - liquidity_net_unsigned
        }
    } else if liquidity_net < 0 {
        current_liquidity - liquidity_net_unsigned
    } else {
        current_liquidity + liquidity_net_unsigned
    }
}

struct SwapStepQuote {
    amount_in: u64,
    amount_out: u64,
    next_sqrt_price: u128,
    fee_amount: u64,
}

fn compute_swap_step(
    amount_remaining: u64,
    fee_rate: u32,
    current_liquidity: u128,
    current_sqrt_price: u128,
    target_sqrt_price: u128,
    a_to_b: bool,
    specified_input: bool,
) -> Result<SwapStepQuote, CoreError> {
    // Any error that is not AMOUNT_EXCEEDS_MAX_U64 is not recoverable
    let initial_amount_fixed_delta = try_get_amount_fixed_delta(
        current_sqrt_price,
        target_sqrt_price,
        current_liquidity,
        a_to_b,
        specified_input,
    );
    let is_initial_amount_fixed_overflow =
        initial_amount_fixed_delta == Err(AMOUNT_EXCEEDS_MAX_U64);
    // 扣减fee
    let amount_calculated = if specified_input {
        try_apply_swap_fee(amount_remaining.into(), fee_rate)?
    } else {
        amount_remaining
    };

    let next_sqrt_price =
        if !is_initial_amount_fixed_overflow && initial_amount_fixed_delta? <= amount_calculated {
            // 可以把当前tick换完
            target_sqrt_price
        } else {
            // 换不完计算剩余amount可以达到的价格
            try_get_next_sqrt_price(
                current_sqrt_price,
                current_liquidity,
                amount_calculated,
                a_to_b,
                specified_input,
            )?
        };
    // 当前tick换完了
    let is_max_swap = next_sqrt_price == target_sqrt_price;
    // 换出amount
    let amount_unfixed_delta = try_get_amount_unfixed_delta(
        current_sqrt_price,
        next_sqrt_price,
        current_liquidity,
        a_to_b,
        specified_input,
    )?;

    // If the swap is not at the max, we need to readjust the amount of the fixed token we are using
    let amount_fixed_delta = if !is_max_swap || is_initial_amount_fixed_overflow {
        // 换不完时，根据价格重新计算下需要多少amount
        try_get_amount_fixed_delta(
            current_sqrt_price,
            next_sqrt_price,
            current_liquidity,
            a_to_b,
            specified_input,
        )?
    } else {
        initial_amount_fixed_delta?
    };

    let (amount_in, mut amount_out) = if specified_input {
        (amount_fixed_delta, amount_unfixed_delta)
    } else {
        (amount_unfixed_delta, amount_fixed_delta)
    };

    // Cap output amount if using output
    if !specified_input && amount_out > amount_remaining {
        amount_out = amount_remaining;
    }

    let fee_amount =
        // 没换完的话剩下的都是fee
        if specified_input && !is_max_swap {
            amount_remaining - amount_in
        } else {
            // 当前tick需要的fee
            let pre_fee_amount = try_reverse_apply_swap_fee(amount_in.into(), fee_rate)?;
            pre_fee_amount - amount_in
        };

    Ok(SwapStepQuote {
        amount_in,
        amount_out,
        next_sqrt_price,
        fee_amount,
    })
}

fn try_get_amount_fixed_delta(
    current_sqrt_price: u128,
    target_sqrt_price: u128,
    current_liquidity: u128,
    a_to_b: bool,
    specified_input: bool,
) -> Result<u64, CoreError> {
    if a_to_b == specified_input {
        try_get_amount_delta_a(
            current_sqrt_price.into(),
            target_sqrt_price.into(),
            current_liquidity.into(),
            specified_input,
        )
    } else {
        try_get_amount_delta_b(
            current_sqrt_price.into(),
            target_sqrt_price.into(),
            current_liquidity.into(),
            specified_input,
        )
    }
}

fn try_get_amount_unfixed_delta(
    current_sqrt_price: u128,
    target_sqrt_price: u128,
    current_liquidity: u128,
    a_to_b: bool,
    specified_input: bool,
) -> Result<u64, CoreError> {
    if specified_input == a_to_b {
        try_get_amount_delta_b(
            current_sqrt_price.into(),
            target_sqrt_price.into(),
            current_liquidity.into(),
            !specified_input,
        )
    } else {
        try_get_amount_delta_a(
            current_sqrt_price.into(),
            target_sqrt_price.into(),
            current_liquidity.into(),
            !specified_input,
        )
    }
}

fn try_get_next_sqrt_price(
    current_sqrt_price: u128,
    current_liquidity: u128,
    amount_calculated: u64,
    a_to_b: bool,
    specified_input: bool,
) -> Result<u128, CoreError> {
    if specified_input == a_to_b {
        try_get_next_sqrt_price_from_a(
            current_sqrt_price.into(),
            current_liquidity.into(),
            amount_calculated.into(),
            specified_input,
        )
        .map(|x| x.into())
    } else {
        try_get_next_sqrt_price_from_b(
            current_sqrt_price.into(),
            current_liquidity.into(),
            amount_calculated.into(),
            specified_input,
        )
        .map(|x| x.into())
    }
}
