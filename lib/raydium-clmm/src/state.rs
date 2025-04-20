use crate::sdk::pool::PoolState;
use crate::sdk::tick_array::TickArrayState;
use crate::sdk::tickarray_bitmap_extension::TickArrayBitmapExtension;
use solana_program::pubkey::Pubkey;

pub(crate) struct PoolSnapshotInfo {
    pub pool_id: Pubkey,
    pub pool_state: PoolState,
    pub tick_array_bitmap_extension_key: Pubkey,
    pub tick_array_bitmap_extension: TickArrayBitmapExtension,
    pub trade_fee_rate: u32,
    pub tick_array_states: Vec<TickArrayState>,
}
