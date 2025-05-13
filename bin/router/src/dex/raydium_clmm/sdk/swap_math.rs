use crate::dex::raydium_clmm::sdk::config::FEE_RATE_DENOMINATOR_VALUE;
use crate::dex::raydium_clmm::sdk::full_math::MulDiv;
use crate::dex::raydium_clmm::sdk::liquidity_math;
use crate::dex::raydium_clmm::sdk::sqrt_price_math;
use anyhow::{anyhow, Result};

/// Result of a swap step
#[derive(Default, Debug)]
pub struct SwapStep {
    /// The price after swapping the amount in/out, not to exceed the price target
    pub sqrt_price_next_x64: u128,
    pub amount_in: u64,
    pub amount_out: u64,
    pub fee_amount: u64,
}

/// Computes the result of swapping some amount in, or amount out, given the parameters of the swap
pub fn compute_swap_step(
    // 池子当前价格
    sqrt_price_current_x64: u128,
    // 目标价格
    sqrt_price_target_x64: u128,
    // 池子当前流动性
    liquidity: u128,
    // 交换数量
    amount_remaining: u64,
    // 手续费
    fee_rate: u32,
    is_base_input: bool,
    zero_for_one: bool,
    block_timestamp: u32,
) -> Result<SwapStep> {
    // let exact_in = amount_remaining >= 0;
    let mut swap_step = SwapStep::default();
    if is_base_input {
        // round up amount_in
        // In exact input case, amount_remaining is positive
        // 扣除手续费后的交换数量
        let amount_remaining_less_fee = (amount_remaining as u64)
            .mul_div_floor(
                (FEE_RATE_DENOMINATOR_VALUE - fee_rate).into(),
                u64::from(FEE_RATE_DENOMINATOR_VALUE),
            )
            .unwrap();
        // 根据当前价格和期望价格计算出一个交换数量，可能不准确
        // Δx
        let amount_in = calculate_amount_in_range(
            sqrt_price_current_x64,
            sqrt_price_target_x64,
            liquidity,
            zero_for_one,
            is_base_input,
            block_timestamp,
        )?;
        if amount_in.is_some() {
            swap_step.amount_in = amount_in.unwrap();
        }
        // 交易后的价格
        swap_step.sqrt_price_next_x64 =
            // 预计算后，若还有可交换的数量，则本次交换后再继续交换，目标价格不变
            if amount_in.is_some() && amount_remaining_less_fee >= swap_step.amount_in {
                sqrt_price_target_x64
            } else {
                // 否则将所有交换数量都交易完
                sqrt_price_math::get_next_sqrt_price_from_input(
                    sqrt_price_current_x64,
                    liquidity,
                    amount_remaining_less_fee,
                    zero_for_one,
                )
            };
    } else {
        let amount_out = calculate_amount_in_range(
            sqrt_price_current_x64,
            sqrt_price_target_x64,
            liquidity,
            zero_for_one,
            is_base_input,
            block_timestamp,
        )?;
        if amount_out.is_some() {
            swap_step.amount_out = amount_out.unwrap();
        }
        // In exact output case, amount_remaining is negative
        swap_step.sqrt_price_next_x64 =
            if amount_out.is_some() && amount_remaining >= swap_step.amount_out {
                sqrt_price_target_x64
            } else {
                sqrt_price_math::get_next_sqrt_price_from_output(
                    sqrt_price_current_x64,
                    liquidity,
                    amount_remaining,
                    zero_for_one,
                )
            }
    }

    // whether we reached the max possible price for the given ticks
    let max = sqrt_price_target_x64 == swap_step.sqrt_price_next_x64;
    // get the input / output amounts when target price is not reached
    if zero_for_one {
        // if max is reached for exact input case, entire amount_in is needed
        if !(max && is_base_input) {
            swap_step.amount_in = liquidity_math::get_delta_amount_0_unsigned(
                swap_step.sqrt_price_next_x64,
                sqrt_price_current_x64,
                liquidity,
                true,
            )?
        };
        // if max is reached for exact output case, entire amount_out is needed
        if !(max && !is_base_input) {
            swap_step.amount_out = liquidity_math::get_delta_amount_1_unsigned(
                swap_step.sqrt_price_next_x64,
                sqrt_price_current_x64,
                liquidity,
                false,
            )?;
        };
    } else {
        if !(max && is_base_input) {
            swap_step.amount_in = liquidity_math::get_delta_amount_1_unsigned(
                sqrt_price_current_x64,
                swap_step.sqrt_price_next_x64,
                liquidity,
                true,
            )?
        };
        if !(max && !is_base_input) {
            swap_step.amount_out = liquidity_math::get_delta_amount_0_unsigned(
                sqrt_price_current_x64,
                swap_step.sqrt_price_next_x64,
                liquidity,
                false,
            )?
        };
    }

    // For exact output case, cap the output amount to not exceed the remaining output amount
    if !is_base_input && swap_step.amount_out > amount_remaining {
        swap_step.amount_out = amount_remaining;
    }

    // 交易费用
    swap_step.fee_amount =
        if is_base_input && swap_step.sqrt_price_next_x64 != sqrt_price_target_x64 {
            // we didn't reach the target, so take the remainder of the maximum input as fee
            // swap dust is granted as fee
            u64::from(amount_remaining)
                .checked_sub(swap_step.amount_in)
                .unwrap()
        } else {
            // take pip percentage as fee
            // 根据手续费比例计算交易费用
            swap_step
                .amount_in
                .mul_div_ceil(
                    fee_rate.into(),
                    (FEE_RATE_DENOMINATOR_VALUE - fee_rate).into(),
                )
                .unwrap()
        };

    Ok(swap_step)
}

/// Pre calcumate amount_in or amount_out for the specified price range
/// The amount maybe overflow of u64 due to the `sqrt_price_target_x64` maybe unreasonable.
/// Therefore, this situation needs to be handled in `compute_swap_step` to recalculate the price that can be reached based on the amount.
#[cfg(not(test))]
fn calculate_amount_in_range(
    sqrt_price_current_x64: u128,
    sqrt_price_target_x64: u128,
    liquidity: u128,
    zero_for_one: bool,
    is_base_input: bool,
    _block_timestamp: u32,
) -> Result<Option<u64>> {
    if is_base_input {
        let result = if zero_for_one {
            liquidity_math::get_delta_amount_0_unsigned(
                sqrt_price_target_x64,
                sqrt_price_current_x64,
                liquidity,
                true,
            )
        } else {
            liquidity_math::get_delta_amount_1_unsigned(
                sqrt_price_current_x64,
                sqrt_price_target_x64,
                liquidity,
                true,
            )
        };

        if result.is_ok() {
            return Ok(Some(result.unwrap()));
        } else {
            if let Some(err) = result.as_ref().err() {
                if err.to_string().eq("Max token overflow") {
                    return Ok(None);
                }
            }
            return Err(anyhow!("Square root price limit overflow"));
        }
    } else {
        let result = if zero_for_one {
            liquidity_math::get_delta_amount_1_unsigned(
                sqrt_price_target_x64,
                sqrt_price_current_x64,
                liquidity,
                false,
            )
        } else {
            liquidity_math::get_delta_amount_0_unsigned(
                sqrt_price_current_x64,
                sqrt_price_target_x64,
                liquidity,
                false,
            )
        };
        if result.is_ok() {
            Ok(Some(result?))
        } else {
            if let Some(err) = result.as_ref().err() {
                if err.to_string().eq("Max token overflow") {
                    return Ok(None);
                }
            }
            return Err(anyhow!("Square root price limit overflow"));
        }
    }
}
