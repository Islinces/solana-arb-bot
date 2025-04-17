use crate::sdk::commons::quote::quote_exact_in;
use crate::sdk::conversions::token_program_flag::TokenProgramFlagWrapper;
use crate::sdk::interface::accounts::{BinArray, BinArrayBitmapExtension, LbPair};
use crate::sdk::interface::typedefs::TokenProgramFlags;
use anchor_spl::token_2022::spl_token_2022::extension::transfer_fee::TransferFeeConfig;
use dex::account_write::AccountWrite;
use dex::interface::Pool;
use solana_program::clock::Clock;
use solana_program::msg;
use solana_program::pubkey::Pubkey;
use std::collections::HashMap;
use std::ops::Deref;

#[derive(Debug, Clone)]
pub struct DlmmPool {
    pub lb_pair_pubkey: Pubkey,
    pub active_id: i32,
    // 1，5，8，10，16，80，100
    pub bin_step: u16,
    pub status: u8,
    pub activation_point: u64,
    pub pair_type: u8,
    pub activation_type: u8,
    pub token_x_mint: Pubkey,
    pub token_y_mint: Pubkey,
    pub bin_array_bitmap: [u64; 16],
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
    // v_parameters
    pub volatility_accumulator: u32,
    pub volatility_reference: u32,
    pub index_reference: i32,
    pub last_update_timestamp: i64,

    pub left_bin_arrays: HashMap<Pubkey, BinArray>,
    pub right_bin_arrays: HashMap<Pubkey, BinArray>,

    pub bitmap_extension: Option<BinArrayBitmapExtension>,

    // mint 可能没有这个配置
    pub mint_x_transfer_fee_config: Option<TransferFeeConfig>,
    pub mint_y_transfer_fee_config: Option<TransferFeeConfig>,
    pub clock: Clock,
}

impl DlmmPool {
    pub fn new(
        lb_pair_pubkey: Pubkey,
        lb_pair_state: LbPair,
        left_bin_arrays: HashMap<Pubkey, BinArray>,
        right_bin_arrays: HashMap<Pubkey, BinArray>,
        bitmap_extension: Option<BinArrayBitmapExtension>,
        mint_x_transfer_fee_config: Option<TransferFeeConfig>,
        mint_y_transfer_fee_config: Option<TransferFeeConfig>,
        clock: Clock,
    ) -> Self {
        let mint_x_transfer_fee_config = Self::mint_transfer_fee_config(
            lb_pair_state.token_mint_x_program_flag,
            mint_x_transfer_fee_config,
        );
        let mint_y_transfer_fee_config = Self::mint_transfer_fee_config(
            lb_pair_state.token_mint_y_program_flag,
            mint_y_transfer_fee_config,
        );
        Self {
            lb_pair_pubkey,
            active_id: lb_pair_state.active_id,
            bin_step: lb_pair_state.bin_step,
            status: lb_pair_state.status,
            activation_point: lb_pair_state.activation_point,
            pair_type: lb_pair_state.pair_type,
            activation_type: lb_pair_state.activation_type,
            token_x_mint: lb_pair_state.token_x_mint,
            token_y_mint: lb_pair_state.token_y_mint,
            bin_array_bitmap: lb_pair_state.bin_array_bitmap,
            base_factor: lb_pair_state.parameters.base_factor,
            filter_period: lb_pair_state.parameters.filter_period,
            decay_period: lb_pair_state.parameters.decay_period,
            reduction_factor: lb_pair_state.parameters.reduction_factor,
            max_volatility_accumulator: lb_pair_state.parameters.max_volatility_accumulator,
            base_fee_power_factor: lb_pair_state.parameters.base_fee_power_factor,
            variable_fee_control: lb_pair_state.parameters.variable_fee_control,
            volatility_accumulator: lb_pair_state.v_parameters.volatility_accumulator,
            volatility_reference: lb_pair_state.v_parameters.volatility_reference,
            index_reference: lb_pair_state.v_parameters.index_reference,
            last_update_timestamp: lb_pair_state.v_parameters.last_update_timestamp,
            left_bin_arrays,
            right_bin_arrays,
            bitmap_extension,
            mint_x_transfer_fee_config,
            mint_y_transfer_fee_config,
            clock,
        }
    }

    fn mint_transfer_fee_config(
        token_mint_program_flag: u8,
        mint_transfer_fee_config: Option<TransferFeeConfig>,
    ) -> Option<TransferFeeConfig> {
        // 0: TokenProgramFlags::TokenProgram , 没有TransferFeeConfig，不需要计算
        // 1: TokenProgramFlags::TokenProgram2022
        let flag: TokenProgramFlagWrapper = token_mint_program_flag.try_into().unwrap();
        match flag.deref() {
            TokenProgramFlags::TokenProgram => None,
            TokenProgramFlags::TokenProgram2022 => mint_transfer_fee_config,
        }
    }

    fn into_lb_pair(&self) -> LbPair {
        let mut lb_pair_state = LbPair::default();
        lb_pair_state.active_id = self.active_id;
        lb_pair_state.bin_step = self.bin_step;
        lb_pair_state.activation_point = self.activation_point;
        lb_pair_state.pair_type = self.pair_type;
        lb_pair_state.activation_type = self.activation_type;
        lb_pair_state.token_x_mint = self.token_x_mint;
        lb_pair_state.token_y_mint = self.token_y_mint;
        lb_pair_state.bin_array_bitmap = self.bin_array_bitmap;
        lb_pair_state.parameters.base_factor = self.base_factor;
        lb_pair_state.parameters.filter_period = self.filter_period;
        lb_pair_state.parameters.decay_period = self.decay_period;
        lb_pair_state.parameters.reduction_factor = self.reduction_factor;
        lb_pair_state.parameters.max_volatility_accumulator = self.max_volatility_accumulator;
        lb_pair_state.parameters.base_fee_power_factor = self.base_fee_power_factor;
        lb_pair_state.parameters.variable_fee_control = self.variable_fee_control;
        lb_pair_state.v_parameters.volatility_accumulator = self.volatility_accumulator;
        lb_pair_state.v_parameters.volatility_reference = self.volatility_reference;
        lb_pair_state.v_parameters.index_reference = self.index_reference;
        lb_pair_state.v_parameters.last_update_timestamp = self.last_update_timestamp;
        lb_pair_state
    }
}

impl Pool for DlmmPool {
    fn get_pool_id(&self) -> Pubkey {
        self.lb_pair_pubkey
    }

    fn get_mint_0(&self) -> Pubkey {
        self.token_x_mint
    }

    fn get_mint_1(&self) -> Pubkey {
        self.token_y_mint
    }

    fn quote(&self, amount_in: u64, amount_in_mint: Pubkey) -> Option<u64> {
        if amount_in_mint != self.token_x_mint && amount_in_mint != self.token_y_mint {
            return None;
        }
        let swap_for_y = self.token_x_mint == amount_in_mint;
        let lp_pair_state = self.into_lb_pair();
        let result = quote_exact_in(
            self.lb_pair_pubkey,
            lp_pair_state,
            amount_in,
            swap_for_y,
            if swap_for_y {
                self.left_bin_arrays.clone()
            } else {
                self.right_bin_arrays.clone()
            },
            self.bitmap_extension.clone(),
            self.clock.clone(),
            self.mint_x_transfer_fee_config.clone(),
            self.mint_y_transfer_fee_config.clone(),
        );
        match result {
            Ok(quote) => {
                // msg!("dlmm swap fee amount : {:?}", quote.fee);
                Some(quote.amount_out)
            }
            Err(e) => {
                msg!("dlmm swap error : {:?}", e);
                None
            }
        }
    }

    fn clone_box(&self) -> Box<dyn Pool> {
        Box::new(self.clone())
    }

    fn update_data(&self, account_write: AccountWrite) {
        todo!()
    }
}
