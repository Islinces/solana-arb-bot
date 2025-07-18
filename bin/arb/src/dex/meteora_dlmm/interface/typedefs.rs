use crate::dex::utils::read_from;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
// #[serde_as]
// #[cfg_attr(feature = "print_data_after_update", derive(Serialize, Deserialize))]
pub struct Bin {
    pub amount_x: u64,
    pub amount_y: u64,
    // #[serde_as(as = "DisplayFromStr")]
    pub price: u128,
    // pub liquidity_supply: u128,
}

pub(crate) const S_PARAMETER_LEN: usize = 2 + 2 + 2 + 2 + 4 + 4 + 2 + 1;
#[repr(C)]
#[derive(Clone, Debug)]
#[cfg_attr(feature = "print_data_after_update", derive(Serialize, Deserialize))]
pub struct StaticParameters {
    pub base_factor: u16,
    pub filter_period: u16,
    pub decay_period: u16,
    pub reduction_factor: u16,
    pub variable_fee_control: u32,
    pub max_volatility_accumulator: u32,
    pub protocol_share: u16,
    pub base_fee_power_factor: u8,
}

impl StaticParameters {
    pub(crate) fn from_slice_data(data: &[u8]) -> Self {
        unsafe {
            let base_factor = read_from::<u16>(&data[0..2]);
            let filter_period = read_from::<u16>(&data[2..4]);
            let decay_period = read_from::<u16>(&data[4..6]);
            let reduction_factor = read_from::<u16>(&data[6..8]);
            let variable_fee_control = read_from::<u32>(&data[8..12]);
            let max_volatility_accumulator = read_from::<u32>(&data[12..16]);
            let protocol_share = read_from::<u16>(&data[16..18]);
            let base_fee_power_factor = read_from::<u8>(&data[18..19]);
            Self {
                base_factor,
                filter_period,
                decay_period,
                reduction_factor,
                variable_fee_control,
                max_volatility_accumulator,
                protocol_share,
                base_fee_power_factor,
            }
        }
    }
}

pub(crate) const V_PARAMETER_LEN: usize = 4 + 4 + 4 + 8;
#[repr(C)]
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "print_data_after_update", derive(Serialize, Deserialize))]
pub struct VariableParameters {
    pub volatility_accumulator: u32,
    pub volatility_reference: u32,
    pub index_reference: i32,
    pub last_update_timestamp: i64,
}

impl VariableParameters {
    pub(crate) fn from_slice_data(data: &[u8]) -> Self {
        unsafe {
            let volatility_accumulator = read_from::<u32>(&data[0..4]);
            let volatility_reference = read_from::<u32>(&data[4..8]);
            let index_reference = read_from::<i32>(&data[8..12]);
            let last_update_timestamp = read_from::<i64>(&data[12..20]);
            Self {
                volatility_accumulator,
                volatility_reference,
                index_reference,
                last_update_timestamp,
            }
        }
    }
}
#[derive(Clone, Debug, BorshDeserialize, BorshSerialize, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ActivationType {
    Slot,
    Timestamp,
}
#[derive(Clone, Debug, BorshDeserialize, BorshSerialize, PartialEq)]
pub enum PairType {
    Permissionless,
    Permission,
    CustomizablePermissionless,
    PermissionlessV2,
}
#[derive(Clone, Debug, BorshDeserialize, BorshSerialize, PartialEq)]
pub enum PairStatus {
    Enabled,
    Disabled,
}
#[derive(Clone, Debug, BorshDeserialize, BorshSerialize, PartialEq)]
pub enum TokenProgramFlags {
    TokenProgram,
    TokenProgram2022,
}

#[derive(Clone, Debug, BorshDeserialize, BorshSerialize, PartialEq)]
pub enum Rounding {
    Up,
    Down,
}
