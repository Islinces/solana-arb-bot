use crate::dex::raydium_clmm::sdk::tick_array::TickArrayState;
use crate::dex::raydium_clmm::sdk::tickarray_bitmap_extension::TickArrayBitmapExtension;
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct RaydiumCLMMPoolState {
    pub tick_spacing: u16,
    pub trade_fee_rate: u32,
    pub liquidity: u128,
    pub sqrt_price_x64: u128,
    pub tick_current: i32,
    pub tick_array_bitmap: [u64; 16],
    pub tick_array_bitmap_extension: TickArrayBitmapExtension,
    pub zero_to_one_tick_array_states: Option<VecDeque<TickArrayState>>,
    pub one_to_zero_tick_array_states: Option<VecDeque<TickArrayState>>,
}

impl RaydiumCLMMPoolState {
    pub fn new(
        tick_spacing: u16,
        trade_fee_rate: u32,
        liquidity: u128,
        sqrt_price_x64: u128,
        tick_current: i32,
        tick_array_bitmap: [u64; 16],
        tick_array_bitmap_extension: TickArrayBitmapExtension,
        zero_to_one_tick_array_states: Option<VecDeque<TickArrayState>>,
        one_to_zero_tick_array_states: Option<VecDeque<TickArrayState>>,
    ) -> Self {
        Self {
            tick_spacing,
            trade_fee_rate,
            liquidity,
            sqrt_price_x64,
            tick_current,
            tick_array_bitmap,
            tick_array_bitmap_extension,
            zero_to_one_tick_array_states,
            one_to_zero_tick_array_states,
        }
    }
}
