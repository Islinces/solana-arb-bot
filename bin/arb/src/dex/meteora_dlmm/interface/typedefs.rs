use borsh::{BorshDeserialize, BorshSerialize};
#[repr(C)]
#[derive(Clone, Debug, Default)]
pub struct Bin {
    pub amount_x: u64,
    pub amount_y: u64,
    pub price: u128,
    // pub liquidity_supply: u128,
}

pub(crate) const S_PARAMETER_LEN: usize = 2 + 2 + 2 + 2 + 4 + 4 + 2 + 1;
#[repr(C)]
#[derive(Clone, Debug, Default)]
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

pub(crate) const V_PARAMETER_LEN: usize = 4 + 4 + 4 + 8;
#[repr(C)]
#[derive(Clone, Debug, Default)]
pub struct VariableParameters {
    pub volatility_accumulator: u32,
    pub volatility_reference: u32,
    pub index_reference: i32,
    pub last_update_timestamp: i64,
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
