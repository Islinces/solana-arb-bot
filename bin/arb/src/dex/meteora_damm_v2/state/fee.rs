use std::u64;

use crate::dex::meteora_damm_v2::constants::BASIS_POINT_MAX;
use crate::dex::meteora_damm_v2::error::{InvalidCollectFeeMode, MathOverflow, TypeCastFailed};
use crate::dex::meteora_damm_v2::math::fee_math::get_fee_in_period;
use crate::dex::meteora_damm_v2::math::safe_math::SafeMath;
use crate::dex::meteora_damm_v2::state::pool::CollectFeeMode;
use anyhow::{anyhow, Result};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use crate::dex::meteora_damm_v2::TradeDirection;

/// Encodes all results of swapping
#[derive(Debug, PartialEq)]
pub struct FeeOnAmountResult {
    pub amount: u64,
    pub lp_fee: u64,
}

/// collect fee mode
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, IntoPrimitive, TryFromPrimitive)]
// https://www.desmos.com/calculator/oxdndn2xdx
pub enum FeeSchedulerMode {
    // fee = cliff_fee_numerator - passed_period * reduction_factor
    Linear,
    // fee = cliff_fee_numerator * (1-reduction_factor/10_000)^passed_period
    Exponential,
}

#[derive(Debug, Default)]
pub struct BaseFeeStruct {
    // 8,8
    pub cliff_fee_numerator: u64,
    // 16,1
    pub fee_scheduler_mode: u8,
    // 22,2
    pub number_of_period: u16,
    // 24,8
    pub period_frequency: u64,
    // 32,8
    pub reduction_factor: u64,
}

impl BaseFeeStruct {
    pub fn get_current_base_fee_numerator(
        &self,
        current_point: u64,
        activation_point: u64,
    ) -> Result<u64> {
        if self.period_frequency == 0 {
            return Ok(self.cliff_fee_numerator);
        }
        // can trade before activation point, so it is alpha-vault, we use min fee
        let period = if current_point < activation_point {
            self.number_of_period.into()
        } else {
            let period = current_point
                .safe_sub(activation_point)?
                .safe_div(self.period_frequency)?;
            period.min(self.number_of_period.into())
        };
        let fee_scheduler_mode = FeeSchedulerMode::try_from(self.fee_scheduler_mode)
            .map_err(|_| anyhow!(TypeCastFailed))?;

        match fee_scheduler_mode {
            FeeSchedulerMode::Linear => {
                let fee_numerator = self
                    .cliff_fee_numerator
                    .safe_sub(period.safe_mul(self.reduction_factor.into())?)?;
                Ok(fee_numerator)
            }
            FeeSchedulerMode::Exponential => {
                let period = u16::try_from(period).map_err(|_| anyhow!(MathOverflow))?;
                let fee_numerator =
                    get_fee_in_period(self.cliff_fee_numerator, self.reduction_factor, period)?;
                Ok(fee_numerator)
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct DynamicFeeStruct {
    // 56，1
    pub initialized: u8, // 0, ignore for dynamic fee
    // 68,4
    pub variable_fee_control: u32,
    // 72,2
    pub bin_step: u16,
    // 74,2
    pub filter_period: u16,
    // 76,2
    pub decay_period: u16,
    // 78,2
    pub reduction_factor: u16,
    // 80,8
    pub last_update_timestamp: u64,
    // 104,16
    pub sqrt_price_reference: u128, // reference sqrt price
    // 120,16
    pub volatility_accumulator: u128,
    // 136,16
    pub volatility_reference: u128, // decayed volatility accumulator
}

impl DynamicFeeStruct {

    pub fn update_references(
        &mut self,
        sqrt_price_current: u128,
        current_timestamp: u64,
    ) -> Result<()> {
        let elapsed = current_timestamp.safe_sub(self.last_update_timestamp)?;
        // Not high frequency trade
        if elapsed >= self.filter_period as u64 {
            // Update sqrt of last transaction
            self.sqrt_price_reference = sqrt_price_current;
            // filter period < t < decay_period. Decay time window.
            if elapsed < self.decay_period as u64 {
                let volatility_reference = self
                    .volatility_accumulator
                    .safe_mul(self.reduction_factor.into())?
                    .safe_div(BASIS_POINT_MAX.into())?;

                self.volatility_reference = volatility_reference;
            }
            // Out of decay time window
            else {
                self.volatility_reference = 0;
            }
        }
        Ok(())
    }

    pub fn is_dynamic_fee_enable(&self) -> bool {
        self.initialized != 0
    }

    pub fn get_variable_fee(&self) -> Result<u128> {
        if self.is_dynamic_fee_enable() {
            let square_vfa_bin: u128 = self
                .volatility_accumulator
                .safe_mul(self.bin_step.into())?
                .checked_pow(2)
                .unwrap();
            // Variable fee control, volatility accumulator, bin step are in basis point unit (10_000)
            // This is 1e20. Which > 1e9. Scale down it to 1e9 unit and ceiling the remaining.
            let v_fee = square_vfa_bin.safe_mul(self.variable_fee_control.into())?;

            let scaled_v_fee = v_fee.safe_add(99_999_999_999)?.safe_div(100_000_000_000)?;

            Ok(scaled_v_fee)
        } else {
            Ok(0)
        }
    }
}

#[derive(Default, Debug)]
pub struct FeeMode {
    pub fees_on_input: bool,
    pub fees_on_token_a: bool,
    pub has_referral: bool,
}

impl FeeMode {
    pub fn get_fee_mode(
        collect_fee_mode: u8,
        trade_direction: TradeDirection,
        has_referral: bool,
    ) -> Result<FeeMode> {
        let collect_fee_mode = CollectFeeMode::try_from(collect_fee_mode)
            .map_err(|_| anyhow!(InvalidCollectFeeMode))?;

        let (fees_on_input, fees_on_token_a) = match (collect_fee_mode, trade_direction) {
            // When collecting fees on output token
            (CollectFeeMode::BothToken, TradeDirection::AtoB) => (false, false),
            (CollectFeeMode::BothToken, TradeDirection::BtoA) => (false, true),

            // When collecting fees on tokenB
            (CollectFeeMode::OnlyB, TradeDirection::AtoB) => (false, false),
            (CollectFeeMode::OnlyB, TradeDirection::BtoA) => (true, false),
        };

        Ok(FeeMode {
            fees_on_input,
            fees_on_token_a,
            has_referral,
        })
    }
}
