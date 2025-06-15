#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct PoolFeesStruct {
    /// Trade fees are extra token amounts that are held inside the token
    /// accounts during a trade, making the value of liquidity tokens rise.
    /// Trade fee numerator
    pub base_fee: BaseFeeStruct,

    /// Protocol trading fees are extra token amounts that are held inside the token
    /// accounts during a trade, with the equivalent in pool tokens minted to
    /// the protocol of the program.
    /// Protocol trade fee numerator
    pub protocol_fee_percent: u8,
    /// partner fee
    pub partner_fee_percent: u8,
    /// referral fee
    pub referral_fee_percent: u8,
    /// padding
    pub padding_0: [u8; 5],

    /// dynamic fee
    pub dynamic_fee: DynamicFeeStruct,

    /// padding
    pub padding_1: [u64; 2],
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct BaseFeeStruct {
    pub cliff_fee_numerator: u64,
    pub fee_scheduler_mode: u8,
    pub padding_0: [u8; 5],
    pub number_of_period: u16,
    pub period_frequency: u64,
    pub reduction_factor: u64,
    pub padding_1: u64,
}

impl TryInto<crate::dex::meteora_damm_v2::state::fee::BaseFeeStruct> for BaseFeeStruct {
    type Error = anyhow::Error;

    fn try_into(
        self,
    ) -> Result<crate::dex::meteora_damm_v2::state::fee::BaseFeeStruct, Self::Error> {
        Ok(crate::dex::meteora_damm_v2::state::fee::BaseFeeStruct {
            cliff_fee_numerator: self.cliff_fee_numerator,
            fee_scheduler_mode: self.fee_scheduler_mode,
            number_of_period: self.number_of_period,
            period_frequency: self.period_frequency,
            reduction_factor: self.reduction_factor,
        })
    }
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct DynamicFeeStruct {
    pub initialized: u8, // 0, ignore for dynamic fee
    pub padding: [u8; 7],
    pub max_volatility_accumulator: u32,
    pub variable_fee_control: u32,
    pub bin_step: u16,
    pub filter_period: u16,
    pub decay_period: u16,
    pub reduction_factor: u16,
    pub last_update_timestamp: u64,
    pub bin_step_u128: u128,
    pub sqrt_price_reference: u128, // reference sqrt price
    pub volatility_accumulator: u128,
    pub volatility_reference: u128, // decayed volatility accumulator
}

impl TryInto<crate::dex::meteora_damm_v2::state::fee::DynamicFeeStruct> for DynamicFeeStruct {
    type Error = anyhow::Error;

    fn try_into(
        self,
    ) -> Result<crate::dex::meteora_damm_v2::state::fee::DynamicFeeStruct, Self::Error> {
        Ok(crate::dex::meteora_damm_v2::state::fee::DynamicFeeStruct {
            initialized: self.initialized,
            variable_fee_control: self.variable_fee_control,
            bin_step: self.bin_step,
            filter_period: self.filter_period,
            decay_period: self.decay_period,
            reduction_factor: self.reduction_factor,
            last_update_timestamp: self.last_update_timestamp,
            sqrt_price_reference: self.sqrt_price_reference,
            volatility_accumulator: self.volatility_accumulator,
            volatility_reference: self.volatility_reference,
        })
    }
}
