use crate::dex::utils::read_from;
use crate::dex::orca_whirlpools::WHIRLPOOL_ID;
use crate::dex::FromCache;
use crate::dex::global_cache::{DynamicCache, StaticCache};
use parking_lot::RwLockReadGuard;
use serde::{Deserialize, Serialize};
use solana_sdk::program_error::ProgramError;
use solana_sdk::pubkey::Pubkey;

/// This constant is used to scale the value of the volatility accumulator.
pub const VOLATILITY_ACCUMULATOR_SCALE_FACTOR: u16 = 10_000;

/// The denominator of the reduction factor.
pub const REDUCTION_FACTOR_DENOMINATOR: u16 = 10_000;

/// adaptive_fee_control_factor is used to map the square of the volatility accumulator to the fee rate.
pub const ADAPTIVE_FEE_CONTROL_FACTOR_DENOMINATOR: u32 = 100_000;

/// The time (in seconds) to forcibly reset the reference if it is not updated for a long time.
pub const MAX_REFERENCE_AGE: u64 = 3_600;

/// max fee rate should be controlled by max_volatility_accumulator, so this is a hard limit for safety.
/// Fee rate is represented as hundredths of a basis point.
pub const FEE_RATE_HARD_LIMIT: u32 = 100_000; // 10%

pub fn get_oracle_address(whirlpool: &Pubkey) -> Result<Pubkey, ProgramError> {
    let seeds = &[b"oracle", whirlpool.as_ref()];

    Pubkey::try_find_program_address(seeds, &WHIRLPOOL_ID)
        .map_or(Err(ProgramError::InvalidSeeds), |v| Ok(v.0))
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "print_data_after_update", derive(Serialize, Deserialize))]
pub struct Oracle {
    // 8,32
    pub whirlpool: Pubkey,
    // 40,34
    pub adaptive_fee_constants: AdaptiveFeeConstants,
    // 74,44
    pub adaptive_fee_variables: AdaptiveFeeVariables,
    // 118,128
    // reserved
}

impl FromCache for Oracle {
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
        unsafe {
            let whirlpool = read_from::<Pubkey>(&static_data[0..32]);
            // adaptive_fee_constants
            let filter_period = read_from::<u16>(&static_data[32..34]);
            let decay_period = read_from::<u16>(&static_data[34..36]);
            let reduction_factor = read_from::<u16>(&static_data[36..38]);
            let adaptive_fee_control_factor = read_from::<u32>(&static_data[38..42]);
            let max_volatility_accumulator = read_from::<u32>(&static_data[42..46]);
            let tick_group_size = read_from::<u16>(&static_data[46..48]);
            let major_swap_threshold_ticks = read_from::<u16>(&static_data[48..50]);
            let adaptive_fee_constants = AdaptiveFeeConstants {
                filter_period,
                decay_period,
                reduction_factor,
                adaptive_fee_control_factor,
                max_volatility_accumulator,
                tick_group_size,
                major_swap_threshold_ticks,
            };
            // adaptive_fee_variables
            let last_reference_update_timestamp = read_from::<u64>(&dynamic_data[0..8]);
            let last_major_swap_timestamp = read_from::<u64>(&dynamic_data[8..16]);
            let volatility_reference = read_from::<u32>(&dynamic_data[16..20]);
            let tick_group_index_reference = read_from::<i32>(&dynamic_data[20..24]);
            let volatility_accumulator = read_from::<u32>(&dynamic_data[24..28]);
            let adaptive_fee_variables = AdaptiveFeeVariables {
                last_reference_update_timestamp,
                last_major_swap_timestamp,
                volatility_reference,
                tick_group_index_reference,
                volatility_accumulator,
            };
            Some(Self {
                whirlpool,
                adaptive_fee_constants,
                adaptive_fee_variables,
            })
        }
    }
}

impl Oracle {
    #[inline(always)]
    pub fn from_bytes(data: &[u8]) -> Result<Self, std::io::Error> {
        let mut data = data;
        unsafe {
            let whirlpool = read_from::<Pubkey>(&data[8..40]);
            // adaptive_fee_constants
            let filter_period = read_from::<u16>(&data[40..42]);
            let decay_period = read_from::<u16>(&data[42..44]);
            let reduction_factor = read_from::<u16>(&data[44..46]);
            let adaptive_fee_control_factor = read_from::<u32>(&data[46..50]);
            let max_volatility_accumulator = read_from::<u32>(&data[50..54]);
            let tick_group_size = read_from::<u16>(&data[54..56]);
            let major_swap_threshold_ticks = read_from::<u16>(&data[56..58]);
            let adaptive_fee_constants = AdaptiveFeeConstants {
                filter_period,
                decay_period,
                reduction_factor,
                adaptive_fee_control_factor,
                max_volatility_accumulator,
                tick_group_size,
                major_swap_threshold_ticks,
            };
            // adaptive_fee_variables
            let last_reference_update_timestamp = read_from::<u64>(&data[70..78]);
            let last_major_swap_timestamp = read_from::<u64>(&data[78..86]);
            let volatility_reference = read_from::<u32>(&data[86..90]);
            let tick_group_index_reference = read_from::<i32>(&data[90..94]);
            let volatility_accumulator = read_from::<u32>(&data[94..98]);
            let adaptive_fee_variables = AdaptiveFeeVariables {
                last_reference_update_timestamp,
                last_major_swap_timestamp,
                volatility_reference,
                tick_group_index_reference,
                volatility_accumulator,
            };
            Ok(Self {
                whirlpool,
                adaptive_fee_constants,
                adaptive_fee_variables,
            })
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "print_data_after_update", derive(Serialize, Deserialize))]
pub struct AdaptiveFeeConstants {
    // 40,2
    pub filter_period: u16,
    // 42,2
    pub decay_period: u16,
    // 44,2
    pub reduction_factor: u16,
    // 46,4
    pub adaptive_fee_control_factor: u32,
    // 50,4
    pub max_volatility_accumulator: u32,
    // 54,2
    pub tick_group_size: u16,
    // 56,2
    pub major_swap_threshold_ticks: u16,
    // 58,16
    // reserved
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "print_data_after_update", derive(Serialize, Deserialize))]
pub struct AdaptiveFeeVariables {
    // 74,8
    pub last_reference_update_timestamp: u64,
    // 82,8
    pub last_major_swap_timestamp: u64,
    // 90,4
    pub volatility_reference: u32,
    // 94,4
    pub tick_group_index_reference: i32,
    // 98,4
    pub volatility_accumulator: u32,
    // 102,16
    //reserved
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct OracleFacade {
    pub adaptive_fee_constants: AdaptiveFeeConstantsFacade,
    pub adaptive_fee_variables: AdaptiveFeeVariablesFacade,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct AdaptiveFeeConstantsFacade {
    pub filter_period: u16,
    pub decay_period: u16,
    pub reduction_factor: u16,
    pub adaptive_fee_control_factor: u32,
    pub max_volatility_accumulator: u32,
    pub tick_group_size: u16,
    pub major_swap_threshold_ticks: u16,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct AdaptiveFeeVariablesFacade {
    pub last_reference_update_timestamp: u64,
    pub last_major_swap_timestamp: u64,
    pub volatility_reference: u32,
    pub tick_group_index_reference: i32,
    pub volatility_accumulator: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct AdaptiveFeeInfo {
    pub constants: AdaptiveFeeConstantsFacade,
    pub variables: AdaptiveFeeVariablesFacade,
}

impl From<OracleFacade> for AdaptiveFeeInfo {
    fn from(oracle: OracleFacade) -> Self {
        AdaptiveFeeInfo {
            constants: oracle.adaptive_fee_constants,
            variables: oracle.adaptive_fee_variables,
        }
    }
}

impl From<Oracle> for OracleFacade {
    fn from(val: Oracle) -> Self {
        OracleFacade {
            adaptive_fee_constants: val.adaptive_fee_constants.into(),
            adaptive_fee_variables: val.adaptive_fee_variables.into(),
        }
    }
}

impl From<AdaptiveFeeConstants> for AdaptiveFeeConstantsFacade {
    fn from(val: AdaptiveFeeConstants) -> Self {
        AdaptiveFeeConstantsFacade {
            filter_period: val.filter_period,
            decay_period: val.decay_period,
            reduction_factor: val.reduction_factor,
            adaptive_fee_control_factor: val.adaptive_fee_control_factor,
            max_volatility_accumulator: val.max_volatility_accumulator,
            tick_group_size: val.tick_group_size,
            major_swap_threshold_ticks: val.major_swap_threshold_ticks,
        }
    }
}

impl From<AdaptiveFeeVariables> for AdaptiveFeeVariablesFacade {
    fn from(val: AdaptiveFeeVariables) -> Self {
        AdaptiveFeeVariablesFacade {
            last_reference_update_timestamp: val.last_reference_update_timestamp,
            last_major_swap_timestamp: val.last_major_swap_timestamp,
            volatility_reference: val.volatility_reference,
            tick_group_index_reference: val.tick_group_index_reference,
            volatility_accumulator: val.volatility_accumulator,
        }
    }
}
