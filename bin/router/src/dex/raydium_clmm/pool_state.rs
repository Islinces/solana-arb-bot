use crate::dex::raydium_clmm::sdk::tick_array::TickArrayState;
use crate::dex::raydium_clmm::sdk::tickarray_bitmap_extension::TickArrayBitmapExtension;
use crate::interface::DexType;
use solana_program::address_lookup_table::AddressLookupTableAccount;
use solana_program::pubkey::Pubkey;
use std::collections::VecDeque;
use std::fmt::{Debug, Display, Formatter};

#[derive(Debug, Clone)]
pub struct RaydiumCLMMPoolState {
    pub amm_config: Pubkey,
    pub mint_0_vault: Pubkey,
    pub mint_1_vault: Pubkey,
    pub observation_key: Pubkey,
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

#[derive(Debug, Clone)]
pub struct RaydiumCLMMInstructionItem {
    pub pool_id: Pubkey,
    pub amm_config: Pubkey,
    pub mint_0: Pubkey,
    pub mint_1: Pubkey,
    pub mint_0_vault: Pubkey,
    pub mint_1_vault: Pubkey,
    pub observation_key: Pubkey,
    pub tick_arrays: Vec<Pubkey>,
    pub alt: AddressLookupTableAccount,
    pub zero_to_one: bool,
}

impl Display for RaydiumCLMMInstructionItem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}: {},{:?}",
            DexType::RaydiumCLmm,
            self.pool_id,
            self.zero_to_one
        )
    }
}
