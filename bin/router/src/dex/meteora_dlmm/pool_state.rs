use crate::dex::meteora_dlmm::sdk::interface::accounts::{
    BinArray, BinArrayBitmapExtension, LbPair,
};
use crate::interface::DexType;
use solana_program::address_lookup_table::AddressLookupTableAccount;
use solana_program::pubkey::Pubkey;
use spl_token_2022::extension::transfer_fee::TransferFeeConfig;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};

#[derive(Debug, Clone)]
pub struct MeteoraDLMMPoolState {
    // ======================启动时初始化即可==============================
    pub mint_0_vault: Pubkey,
    pub mint_1_vault: Pubkey,
    // 1，5，8，10，16，80，100
    pub bin_step: u16,
    pub status: u8,
    pub activation_point: u64,
    pub pair_type: u8,
    pub activation_type: u8,
    // parameters
    // 0.01%*10000，0.02%*10000，0.03%*10000，0.04%*10000，0.05%*10000，
    // 0.1%*10000，0.15%*10000，0.2%*10000，0.25%*10000，
    // 0.3%*10000，0.4%*10000，0.6%*10000，0.8%*10000，
    // 1%*10000，2%*10000，5%*10000，
    pub base_factor: u16,
    pub filter_period: u16,
    pub decay_period: u16,
    pub reduction_factor: u16,
    pub max_volatility_accumulator: u32,
    pub base_fee_power_factor: u8,
    pub variable_fee_control: u32,
    // ======================每个MINT不一定有这个配置==============================
    pub mint_x_transfer_fee_config: Option<TransferFeeConfig>,
    pub mint_y_transfer_fee_config: Option<TransferFeeConfig>,
    // =====================需要订阅===============================
    pub active_id: i32,
    pub bin_array_bitmap: [u64; 16],
    // v_parameters
    pub volatility_accumulator: u32,
    pub volatility_reference: u32,
    pub index_reference: i32,
    pub last_update_timestamp: i64,
    // ======================考虑需要订阅==============================
    pub swap_for_y_bin_array_map: HashMap<Pubkey, BinArray>,
    pub swap_for_x_bin_array_map: HashMap<Pubkey, BinArray>,
    pub bin_array_bitmap_extension: Option<BinArrayBitmapExtension>,
    // ======================不确定是否需要订阅==============================
    // pub clock: Clock,
}

impl MeteoraDLMMPoolState {
    pub fn new(
        lb_pair: LbPair,
        bin_array_bitmap_extension: Option<BinArrayBitmapExtension>,
        swap_for_y_bin_array_map: HashMap<Pubkey, BinArray>,
        swap_for_x_bin_array_map: HashMap<Pubkey, BinArray>,
        mint_x_transfer_fee_config: Option<TransferFeeConfig>,
        mint_y_transfer_fee_config: Option<TransferFeeConfig>,
    ) -> Self {
        Self {
            mint_0_vault: lb_pair.reserve_x,
            mint_1_vault: lb_pair.reserve_y,
            bin_step: lb_pair.bin_step,
            status: lb_pair.status,
            activation_point: lb_pair.activation_point,
            pair_type: lb_pair.pair_type,
            activation_type: lb_pair.activation_type,
            base_factor: lb_pair.parameters.base_factor,
            filter_period: lb_pair.parameters.filter_period,
            decay_period: lb_pair.parameters.decay_period,
            reduction_factor: lb_pair.parameters.reduction_factor,
            max_volatility_accumulator: lb_pair.parameters.max_volatility_accumulator,
            base_fee_power_factor: lb_pair.parameters.base_fee_power_factor,
            variable_fee_control: lb_pair.parameters.variable_fee_control,
            mint_x_transfer_fee_config,
            mint_y_transfer_fee_config,
            active_id: lb_pair.active_id,
            bin_array_bitmap: lb_pair.bin_array_bitmap,
            volatility_accumulator: lb_pair.v_parameters.volatility_accumulator,
            volatility_reference: lb_pair.v_parameters.volatility_reference,
            index_reference: lb_pair.v_parameters.index_reference,
            last_update_timestamp: lb_pair.v_parameters.last_update_timestamp,
            bin_array_bitmap_extension,
            swap_for_x_bin_array_map,
            swap_for_y_bin_array_map,
        }
    }
}

impl Into<LbPair> for MeteoraDLMMPoolState {
    fn into(self) -> LbPair {
        let mut lb_pair = LbPair::default();
        lb_pair.bin_step = self.bin_step;
        lb_pair.status = self.status;
        lb_pair.activation_point = self.activation_point;
        lb_pair.pair_type = self.pair_type;
        lb_pair.activation_type = self.activation_type;
        lb_pair.active_id = self.active_id;
        lb_pair.bin_array_bitmap = self.bin_array_bitmap;
        lb_pair.parameters.base_factor = self.base_factor;
        lb_pair.parameters.filter_period = self.filter_period;
        lb_pair.parameters.decay_period = self.decay_period;
        lb_pair.parameters.reduction_factor = self.reduction_factor;
        lb_pair.parameters.max_volatility_accumulator = self.max_volatility_accumulator;
        lb_pair.parameters.base_fee_power_factor = self.base_fee_power_factor;
        lb_pair.parameters.variable_fee_control = self.variable_fee_control;
        lb_pair.v_parameters.volatility_accumulator = self.volatility_accumulator;
        lb_pair.v_parameters.volatility_reference = self.volatility_reference;
        lb_pair.v_parameters.index_reference = self.index_reference;
        lb_pair.v_parameters.last_update_timestamp = self.last_update_timestamp;
        lb_pair
    }
}

#[derive(Debug, Clone)]
pub struct MeteoraDLMMInstructionItem {
    pub pool_id: Pubkey,
    pub mint_0: Pubkey,
    pub mint_1: Pubkey,
    pub mint_0_vault: Pubkey,
    pub mint_1_vault: Pubkey,
    pub bitmap_extension: Pubkey,
    pub bin_arrays: Vec<Pubkey>,
    pub alt: AddressLookupTableAccount,
    pub zero_to_one: bool,
}

impl Display for MeteoraDLMMInstructionItem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}: {},{:?}",
            DexType::MeteoraDLMM,
            self.pool_id,
            self.zero_to_one
        )
    }
}
