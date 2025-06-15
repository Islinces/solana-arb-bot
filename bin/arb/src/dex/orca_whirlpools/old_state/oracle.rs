use serde::Deserialize;
use serde::Serialize;
use serde_with::serde_as;
use solana_sdk::pubkey::Pubkey;

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
#[serde_as]
#[cfg_attr(feature = "print_data_after_update", derive(Serialize, Deserialize))]
pub struct Oracle {
    pub discriminator: [u8; 8],
    pub whirlpool: Pubkey,
    pub trade_enable_timestamp: u64,
    pub adaptive_fee_constants: AdaptiveFeeConstants,
    pub adaptive_fee_variables: AdaptiveFeeVariables,
    #[serde_as(as = "[_; 128]")]
    pub reserved: [u8; 128],
}

impl TryInto<crate::dex::orca_whirlpools::accounts::oracle::Oracle> for Oracle {
    type Error = anyhow::Error;

    fn try_into(
        self,
    ) -> Result<crate::dex::orca_whirlpools::accounts::oracle::Oracle, Self::Error> {
        Ok(crate::dex::orca_whirlpools::accounts::oracle::Oracle {
            whirlpool: self.whirlpool,
            adaptive_fee_constants: self.adaptive_fee_constants.try_into()?,
            adaptive_fee_variables: self.adaptive_fee_variables.try_into()?,
        })
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "print_data_after_update", derive(Serialize, Deserialize))]
pub struct AdaptiveFeeConstants {
    pub filter_period: u16,
    pub decay_period: u16,
    pub reduction_factor: u16,
    pub adaptive_fee_control_factor: u32,
    pub max_volatility_accumulator: u32,
    pub tick_group_size: u16,
    pub major_swap_threshold_ticks: u16,
    pub reserved: [u8; 16],
}

impl TryInto<crate::dex::orca_whirlpools::accounts::oracle::AdaptiveFeeConstants>
    for AdaptiveFeeConstants
{
    type Error = anyhow::Error;

    fn try_into(self) -> Result<crate::dex::oracle::AdaptiveFeeConstants, Self::Error> {
        Ok(crate::dex::oracle::AdaptiveFeeConstants {
            filter_period: self.filter_period,
            decay_period: self.decay_period,
            reduction_factor: self.reduction_factor,
            adaptive_fee_control_factor: self.adaptive_fee_control_factor,
            max_volatility_accumulator: self.max_volatility_accumulator,
            tick_group_size: self.tick_group_size,
            major_swap_threshold_ticks: self.major_swap_threshold_ticks,
        })
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "print_data_after_update", derive(Serialize, Deserialize))]
pub struct AdaptiveFeeVariables {
    pub last_reference_update_timestamp: u64,
    pub last_major_swap_timestamp: u64,
    pub volatility_reference: u32,
    pub tick_group_index_reference: i32,
    pub volatility_accumulator: u32,
    pub reserved: [u8; 16],
}

impl TryInto<crate::dex::orca_whirlpools::accounts::oracle::AdaptiveFeeVariables>
    for AdaptiveFeeVariables
{
    type Error = anyhow::Error;

    fn try_into(self) -> Result<crate::dex::oracle::AdaptiveFeeVariables, Self::Error> {
        Ok(crate::dex::oracle::AdaptiveFeeVariables {
            last_reference_update_timestamp: self.last_reference_update_timestamp,
            last_major_swap_timestamp: self.last_major_swap_timestamp,
            volatility_reference: self.volatility_reference,
            tick_group_index_reference: self.tick_group_index_reference,
            volatility_accumulator: self.volatility_accumulator,
        })
    }
}
