use solana_sdk::pubkey::Pubkey;

#[repr(C)]
#[derive(Debug, Clone)]
pub struct LbPair {
    pub parameters: StaticParameters,
    pub v_parameters: VariableParameters,
    pub bump_seed: [u8; 1],
    pub bin_step_seed: [u8; 2],
    pub pair_type: u8,
    pub active_id: i32,
    pub bin_step: u16,
    pub status: u8,
    pub require_base_factor_seed: u8,
    pub base_factor_seed: [u8; 2],
    pub activation_type: u8,
    pub creator_pool_on_off_control: u8,
    pub token_x_mint: Pubkey,
    pub token_y_mint: Pubkey,
    pub reserve_x: Pubkey,
    pub reserve_y: Pubkey,
    pub protocol_fee: ProtocolFee,
    pub padding1: [u8; 32],
    pub reward_infos: [RewardInfo; 2],
    pub oracle: Pubkey,
    pub bin_array_bitmap: [u64; 16],
    pub last_updated_at: i64,
    pub padding2: [u8; 32],
    pub pre_activation_swap_address: Pubkey,
    pub base_key: Pubkey,
    pub activation_point: u64,
    pub pre_activation_duration: u64,
    pub padding3: [u8; 8],
    pub padding4: u64,
    pub creator: Pubkey,
    pub token_mint_x_program_flag: u8,
    pub token_mint_y_program_flag: u8,
    pub reserved: [u8; 22],
}

impl TryInto<crate::dex::meteora_dlmm::LbPair> for LbPair {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<crate::dex::LbPair, Self::Error> {
        Ok(crate::dex::LbPair {
            parameters:self.parameters.try_into()?,
            pair_type: self.pair_type,
            bin_step: self.bin_step,
            status: self.status,
            activation_type: self.activation_type,
            token_x_mint: self.token_x_mint,
            token_y_mint: self.token_y_mint,
            reserve_x: self.reserve_x,
            reserve_y: self.reserve_y,
            oracle: self.oracle,
            activation_point: self.activation_point,
            token_mint_x_program_flag: self.token_mint_x_program_flag,
            token_mint_y_program_flag: self.token_mint_y_program_flag,
            v_parameters: self.v_parameters.try_into()?,
            active_id: self.active_id,
            bin_array_bitmap: self.bin_array_bitmap,
        })
    }
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct StaticParameters {
    pub base_factor: u16,
    pub filter_period: u16,
    pub decay_period: u16,
    pub reduction_factor: u16,
    pub variable_fee_control: u32,
    pub max_volatility_accumulator: u32,
    pub min_bin_id: i32,
    pub max_bin_id: i32,
    pub protocol_share: u16,
    pub base_fee_power_factor: u8,
    pub padding: [u8; 5],
}

impl TryInto<crate::dex::meteora_dlmm::interface::StaticParameters> for StaticParameters {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<crate::dex::meteora_dlmm::interface::StaticParameters, Self::Error> {
        Ok(crate::dex::meteora_dlmm::interface::StaticParameters {
            base_factor: self.base_factor,
            filter_period: self.filter_period,
            decay_period: self.decay_period,
            reduction_factor: self.reduction_factor,
            variable_fee_control: self.variable_fee_control,
            max_volatility_accumulator: self.max_volatility_accumulator,
            protocol_share: self.protocol_share,
            base_fee_power_factor: self.base_fee_power_factor,
        })
    }
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct VariableParameters {
    pub volatility_accumulator: u32,
    pub volatility_reference: u32,
    pub index_reference: i32,
    pub padding: [u8; 4],
    pub last_update_timestamp: i64,
    pub padding1: [u8; 8],
}

impl TryInto<crate::dex::meteora_dlmm::interface::VariableParameters> for VariableParameters {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<crate::dex::meteora_dlmm::interface::VariableParameters, Self::Error> {
        Ok(crate::dex::meteora_dlmm::interface::VariableParameters {
            volatility_accumulator: self.volatility_accumulator,
            volatility_reference: self.volatility_reference,
            index_reference: self.index_reference,
            last_update_timestamp: self.last_update_timestamp,
        })
    }
}

#[repr(C)]
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProtocolFee {
    pub amount_x: u64,
    pub amount_y: u64,
}
#[repr(C)]
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RewardInfo {
    pub mint: Pubkey,
    pub vault: Pubkey,
    pub funder: Pubkey,
    pub reward_duration: u64,
    pub reward_duration_end: u64,
    pub reward_rate: u128,
    pub last_update_time: u64,
    pub cumulative_seconds_with_empty_liquidity_reward: u64,
}
