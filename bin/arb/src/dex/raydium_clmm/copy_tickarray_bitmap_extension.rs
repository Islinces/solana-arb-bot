use borsh::BorshDeserialize;
use solana_sdk::pubkey::Pubkey;

/// 为什么是14，443636/60/512
const EXTENSION_TICKARRAY_BITMAP_SIZE: usize = 14;

// #[account(zero_copy(unsafe))]
#[repr(C, packed)]
#[derive(Debug, BorshDeserialize, Clone)]
pub struct TickArrayBitmapExtension {
    pub pool_id: Pubkey,
    /// Packed initialized tick array state for start_tick_index is positive
    pub positive_tick_array_bitmap: [[u64; 8]; EXTENSION_TICKARRAY_BITMAP_SIZE],
    /// Packed initialized tick array state for start_tick_index is negitive
    pub negative_tick_array_bitmap: [[u64; 8]; EXTENSION_TICKARRAY_BITMAP_SIZE],
}

impl Default for TickArrayBitmapExtension {
    #[inline]
    fn default() -> TickArrayBitmapExtension {
        TickArrayBitmapExtension {
            pool_id: Pubkey::default(),
            positive_tick_array_bitmap: [[0; 8]; EXTENSION_TICKARRAY_BITMAP_SIZE],
            negative_tick_array_bitmap: [[0; 8]; EXTENSION_TICKARRAY_BITMAP_SIZE],
        }
    }
}
