use crate::config::AmmConfig;
use crate::pool::PoolState;
use crate::tick_array::TickArrayState;
use crate::tickarray_bitmap_extension::TickArrayBitmapExtension;
use crate::utils;
use dex::interface::DexPoolInterface;
use solana_program::pubkey::Pubkey;
use std::any::Any;
use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub struct ClmmPool {
    pub pool_id: Pubkey,
    pub owner_id: Pubkey,
    pub amm_config: Pubkey,
    /// mint
    pub mint_0: Pubkey,
    pub mint_1: Pubkey,
    /// mint 精度
    pub mint_0_decimals: u8,
    pub mint_1_decimals: u8,
    /// 发起交易时需要。计算amount_out不需要
    // pub mint_0_vault: u64,
    // pub mint_1_vault: u64,
    /// 发起交易时需要。计算amount_out不需要
    // pub observation_key: Pubkey,
    /// tick间隔
    pub tick_spacing: u16,
    pub liquidity: u128,
    pub sqrt_price_x64: u128,
    pub tick_current: i32,
    /// Packed initialized tick array state
    pub tick_array_bitmap: [u64; 16],
    pub tick_array_bitmap_extension: TickArrayBitmapExtension,
    pub zero_to_one_tick_arays: VecDeque<TickArrayState>,
    pub one_to_zero_tick_arays: VecDeque<TickArrayState>,
    /// amm_config
    /// 交易费率
    pub trade_fee_rate: u32,
    // 仅影响fee_amount,暂时不需要
    // pub protocol_fee_rate: u32,
    // pub fund_fee_rate: u32,
}

impl ClmmPool {
    pub fn new(
        pool_id: Pubkey,
        owner_id: Pubkey,
        amm_config: Pubkey,
        mint_0: Pubkey,
        mint_1: Pubkey,
        mint_0_decimals: u8,
        mint_1_decimals: u8,
        liquidity: u128,
        sqrt_price_x64: u128,
        tick_current: i32,
        tick_spacing: u16,
        tick_array_bitmap: [u64; 16],
        tick_array_bitmap_extension: TickArrayBitmapExtension,
        zero_to_one_tick_arays: VecDeque<TickArrayState>,
        one_to_zero_tick_arays: VecDeque<TickArrayState>,
        trade_fee_rate: u32,
    ) -> Self {
        Self {
            pool_id,
            owner_id,
            amm_config,
            mint_0,
            mint_1,
            mint_0_decimals,
            mint_1_decimals,
            liquidity,
            sqrt_price_x64,
            tick_current,
            tick_spacing,
            tick_array_bitmap,
            tick_array_bitmap_extension,
            zero_to_one_tick_arays,
            one_to_zero_tick_arays,
            trade_fee_rate,
        }
    }
}

impl DexPoolInterface for ClmmPool {
    fn quote(&self, amount_in: u64, amount_in_mint: Pubkey) -> Option<u64> {
        if amount_in_mint != self.mint_0 && amount_in_mint != self.mint_1 {
            return None;
        }
        let zero_for_one = amount_in_mint == self.mint_0;
        let mut amm_config = AmmConfig::default();
        amm_config.trade_fee_rate = self.trade_fee_rate;
        // amm_config.protocol_fee_rate = self.protocol_fee_rate;
        // amm_config.fund_fee_rate = self.fund_fee_rate;
        let mut pool_state = PoolState::default();
        pool_state.tick_current = self.tick_current;
        pool_state.tick_spacing = self.tick_spacing;
        pool_state.tick_array_bitmap = self.tick_array_bitmap;
        pool_state.liquidity = self.liquidity;
        pool_state.sqrt_price_x64 = self.sqrt_price_x64;
        let mut tick_arrays = match zero_for_one {
            true => self.zero_to_one_tick_arays.clone(),
            false => self.one_to_zero_tick_arays.clone(),
        };
        let result = get_out_put_amount_and_remaining_accounts(
            amount_in,
            None,
            zero_for_one,
            true,
            &amm_config,
            &pool_state,
            &Some(self.tick_array_bitmap_extension),
            &mut tick_arrays,
        );
        match result {
            Ok((amount_out, _, _)) => Some(amount_out),
            Err(e) => {
                println!("get_out_put_amount_and_remaining_accounts error: {:?}", e);
                None
            }
        }
    }

    fn get_pool_id(&self) -> Pubkey {
        self.pool_id
    }

    fn get_mint_0(&self) -> Pubkey {
        self.mint_1
    }

    fn get_mint_1(&self) -> Pubkey {
        self.mint_0
    }

    fn get_mint_0_vault(&self) -> Option<Pubkey> {
        None
    }

    fn get_mint_1_vault(&self) -> Option<Pubkey> {
        None
    }

    fn as_any(&self) -> &dyn Any {
        todo!()
    }

    fn update_data(&mut self, changed_pool: Box<dyn DexPoolInterface>) -> anyhow::Result<Pubkey> {
        todo!()
    }
}

fn get_out_put_amount_and_remaining_accounts(
    input_amount: u64,
    sqrt_price_limit_x64: Option<u128>,
    zero_for_one: bool,
    is_base_input: bool,
    pool_config: &AmmConfig,
    pool_state: &PoolState,
    tickarray_bitmap_extension: &Option<TickArrayBitmapExtension>,
    tick_arrays: &mut VecDeque<TickArrayState>,
) -> Result<(u64, u64, VecDeque<i32>), &'static str> {
    utils::get_out_put_amount_and_remaining_accounts(
        input_amount,
        sqrt_price_limit_x64,
        zero_for_one,
        is_base_input,
        pool_config,
        pool_state,
        tickarray_bitmap_extension,
        tick_arrays,
    )
}
