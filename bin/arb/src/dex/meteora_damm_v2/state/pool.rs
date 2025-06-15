use std::u64;

use super::fee::{BaseFeeStruct, DynamicFeeStruct, FeeMode, FeeOnAmountResult};
use crate::dex::meteora_damm_v2::constants::fee::{FEE_DENOMINATOR, MAX_FEE_NUMERATOR};
use crate::dex::meteora_damm_v2::error::PriceRangeViolation;
use crate::dex::meteora_damm_v2::math::curve::{
    get_delta_amount_a_unsigned, get_delta_amount_b_unsigned, get_next_sqrt_price_from_input,
};
use crate::dex::meteora_damm_v2::math::safe_math::SafeMath;
use crate::dex::meteora_damm_v2::math::u128x128_math::Rounding;
use crate::dex::meteora_damm_v2::math::utils_math::safe_mul_div_cast_u64;
use crate::dex::meteora_damm_v2::TradeDirection;
use crate::dex::utils::read_from;
use crate::dex::{DynamicCache, FromCache, StaticCache};
use anyhow::{anyhow, Result};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use parking_lot::RwLockReadGuard;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

/// collect fee mode
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, IntoPrimitive, TryFromPrimitive)]
pub enum CollectFeeMode {
    /// Both token, in this mode only out token is collected
    BothToken,
    /// Only token B, we just need token B, because if user want to collect fee in token A, they just need to flip order of tokens
    OnlyB,
}

/// pool status
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, IntoPrimitive, TryFromPrimitive)]
pub enum PoolStatus {
    Enable,
    Disable,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, IntoPrimitive, TryFromPrimitive)]
pub enum PoolType {
    Permissionless,
    Customizable,
}

#[derive(Debug)]
#[cfg_attr(feature = "print_data_after_update", derive(Serialize, Deserialize))]
pub struct Pool {
    /// Pool fee
    // 8,40
    pub base_fee: BaseFeeStruct,
    // 56,96
    pub dynamic_fee: DynamicFeeStruct,
    // 168,32
    pub token_a_mint: Pubkey,
    // 200,32
    pub token_b_mint: Pubkey,
    // 232,32
    pub token_a_vault: Pubkey,
    // 264,32
    pub token_b_vault: Pubkey,
    // 360,16
    pub liquidity: u128,
    // 424,16
    pub sqrt_min_price: u128,
    // 440,16
    pub sqrt_max_price: u128,
    // 456,16
    pub sqrt_price: u128,
    // 472,8
    pub activation_point: u64,
    // 480,1
    pub activation_type: u8,
    // 481,1
    pub pool_status: u8,
    // 482,1
    pub token_a_flag: u8,
    // 483,1
    pub token_b_flag: u8,
    /// 0 is collect fee in both token, 1 only collect fee in token a, 2 only collect fee in token b
    // 484,1
    pub collect_fee_mode: u8,
}

impl FromCache for Pool {
    fn from_cache(
        account_key: &Pubkey,
        static_cache: RwLockReadGuard<StaticCache>,
        dynamic_cache: &DynamicCache,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        let static_data = static_cache.get(account_key)?;
        let dynamic_data = dynamic_cache.get(account_key)?;
        let dynamic_data = dynamic_data.value().as_slice();
        Some(Self::from_slice_data(static_data, dynamic_data))
    }
}

impl Pool {
    pub fn from_slice_data(static_data: &[u8], dynamic_data: &[u8]) -> Self {
        unsafe {
            let base_fee = {
                let cliff_fee_numerator = read_from::<u64>(&static_data[0..8]);
                let fee_scheduler_mode = read_from::<u8>(&static_data[8..9]);
                let number_of_period = read_from::<u16>(&static_data[9..11]);
                let period_frequency = read_from::<u64>(&static_data[11..19]);
                let reduction_factor = read_from::<u64>(&static_data[19..27]);
                BaseFeeStruct {
                    cliff_fee_numerator,
                    fee_scheduler_mode,
                    number_of_period,
                    period_frequency,
                    reduction_factor,
                }
            };
            let dynamic_fee = {
                let initialized = read_from::<u8>(&static_data[27..28]);
                let variable_fee_control = read_from::<u32>(&static_data[28..32]);
                let bin_step = read_from::<u16>(&static_data[32..34]);
                let filter_period = read_from::<u16>(&static_data[34..36]);
                let decay_period = read_from::<u16>(&static_data[36..38]);
                let reduction_factor = read_from::<u16>(&static_data[38..40]);
                let last_update_timestamp = read_from::<u64>(&dynamic_data[0..8]);
                let sqrt_price_reference = read_from::<u128>(&dynamic_data[8..24]);
                let volatility_accumulator = read_from::<u128>(&dynamic_data[24..40]);
                let volatility_reference = read_from::<u128>(&dynamic_data[40..56]);
                DynamicFeeStruct {
                    initialized,
                    variable_fee_control,
                    bin_step,
                    filter_period,
                    decay_period,
                    reduction_factor,
                    last_update_timestamp,
                    sqrt_price_reference,
                    volatility_accumulator,
                    volatility_reference,
                }
            };
            let token_a_mint = read_from::<Pubkey>(&static_data[40..72]);
            let token_b_mint = read_from::<Pubkey>(&static_data[72..104]);
            let token_a_vault = read_from::<Pubkey>(&static_data[104..136]);
            let token_b_vault = read_from::<Pubkey>(&static_data[136..168]);
            let liquidity = read_from::<u128>(&dynamic_data[56..72]);
            let sqrt_min_price = read_from::<u128>(&static_data[168..184]);
            let sqrt_max_price = read_from::<u128>(&static_data[184..200]);
            let sqrt_price = read_from::<u128>(&dynamic_data[72..88]);
            let activation_point = read_from::<u64>(&static_data[200..208]);
            let activation_type = read_from::<u8>(&static_data[208..209]);
            let pool_status = read_from::<u8>(&dynamic_data[88..89]);
            let token_a_flag = read_from::<u8>(&static_data[209..210]);
            let token_b_flag = read_from::<u8>(&static_data[210..211]);
            let collect_fee_mode = read_from::<u8>(&static_data[211..212]);

            Self {
                base_fee,
                dynamic_fee,
                token_a_mint,
                token_b_mint,
                token_a_vault,
                token_b_vault,
                liquidity,
                sqrt_min_price,
                sqrt_max_price,
                sqrt_price,
                activation_point,
                activation_type,
                pool_status,
                token_a_flag,
                token_b_flag,
                collect_fee_mode,
            }
        }
    }

    pub fn get_swap_result(
        &self,
        amount_in: u64,
        fee_mode: &FeeMode,
        trade_direction: TradeDirection,
        current_point: u64,
    ) -> Result<SwapResult> {
        let mut actual_protocol_fee = 0;
        let mut actual_lp_fee = 0;
        let mut actual_referral_fee = 0;
        let mut actual_partner_fee = 0;

        let actual_amount_in = if fee_mode.fees_on_input {
            let FeeOnAmountResult { amount, lp_fee } =
                self.get_fee_on_amount(amount_in, current_point, self.activation_point)?;

            actual_lp_fee = lp_fee;

            amount
        } else {
            amount_in
        };

        let SwapAmount {
            output_amount,
            next_sqrt_price,
        } = match trade_direction {
            TradeDirection::AtoB => self.get_swap_result_from_a_to_b(actual_amount_in),
            TradeDirection::BtoA => self.get_swap_result_from_b_to_a(actual_amount_in),
        }?;

        let actual_amount_out = if fee_mode.fees_on_input {
            output_amount
        } else {
            let FeeOnAmountResult { amount, lp_fee } =
                self.get_fee_on_amount(output_amount, current_point, self.activation_point)?;
            actual_lp_fee = lp_fee;
            amount
        };

        Ok(SwapResult {
            output_amount: actual_amount_out,
            next_sqrt_price,
            lp_fee: actual_lp_fee,
            protocol_fee: actual_protocol_fee,
            partner_fee: actual_partner_fee,
            referral_fee: actual_referral_fee,
        })
    }
    fn get_swap_result_from_a_to_b(&self, amount_in: u64) -> Result<SwapAmount> {
        // finding new target price
        let next_sqrt_price =
            get_next_sqrt_price_from_input(self.sqrt_price, self.liquidity, amount_in, true)?;

        if next_sqrt_price < self.sqrt_min_price {
            return Err(anyhow!(PriceRangeViolation));
        }

        // finding output amount
        let output_amount = get_delta_amount_b_unsigned(
            next_sqrt_price,
            self.sqrt_price,
            self.liquidity,
            Rounding::Down,
        )?;

        Ok(SwapAmount {
            output_amount,
            next_sqrt_price,
        })
    }

    fn get_swap_result_from_b_to_a(&self, amount_in: u64) -> Result<SwapAmount> {
        // finding new target price
        let next_sqrt_price =
            get_next_sqrt_price_from_input(self.sqrt_price, self.liquidity, amount_in, false)?;

        if next_sqrt_price > self.sqrt_max_price {
            return Err(anyhow!(PriceRangeViolation));
        }
        // finding output amount
        let output_amount = get_delta_amount_a_unsigned(
            self.sqrt_price,
            next_sqrt_price,
            self.liquidity,
            Rounding::Down,
        )?;

        Ok(SwapAmount {
            output_amount,
            next_sqrt_price,
        })
    }

    pub fn update_pre_swap(&mut self, current_timestamp: u64) -> Result<()> {
        if self.dynamic_fee.is_dynamic_fee_enable() {
            self.dynamic_fee
                .update_references(self.sqrt_price, current_timestamp)?;
        }
        Ok(())
    }

    // in numerator
    pub fn get_total_trading_fee(&self, current_point: u64, activation_point: u64) -> Result<u128> {
        let base_fee_numerator = self
            .base_fee
            .get_current_base_fee_numerator(current_point, activation_point)?;
        let total_fee_numerator = self
            .dynamic_fee
            .get_variable_fee()?
            .safe_add(base_fee_numerator.into())?;
        Ok(total_fee_numerator)
    }

    pub fn get_fee_on_amount(
        &self,
        amount: u64,
        current_point: u64,
        activation_point: u64,
    ) -> Result<FeeOnAmountResult> {
        let trade_fee_numerator = self.get_total_trading_fee(current_point, activation_point)?;
        let trade_fee_numerator = if trade_fee_numerator > MAX_FEE_NUMERATOR as u128 {
            MAX_FEE_NUMERATOR
        } else {
            trade_fee_numerator.try_into().unwrap()
        };
        let lp_fee: u64 =
            safe_mul_div_cast_u64(amount, trade_fee_numerator, FEE_DENOMINATOR, Rounding::Up)?;
        // update amount
        let amount = amount.safe_sub(lp_fee)?;

        Ok(FeeOnAmountResult { amount, lp_fee })
    }
}

/// Encodes all results of swapping
#[derive(Debug, PartialEq)]
pub struct SwapResult {
    pub output_amount: u64,
    pub next_sqrt_price: u128,
    pub lp_fee: u64,
    pub protocol_fee: u64,
    pub partner_fee: u64,
    pub referral_fee: u64,
}

pub struct SwapAmount {
    output_amount: u64,
    next_sqrt_price: u128,
}

#[derive(Debug, PartialEq)]
pub struct ModifyLiquidityResult {
    pub token_a_amount: u64,
    pub token_b_amount: u64,
}
