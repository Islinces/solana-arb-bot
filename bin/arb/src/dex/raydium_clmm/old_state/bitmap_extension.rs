use solana_sdk::pubkey::Pubkey;

const EXTENSION_TICKARRAY_BITMAP_SIZE: usize = 14;

#[repr(C, packed)]
#[derive(Debug, Clone)]
pub struct TickArrayBitmapExtension {
    pub pool_id: Pubkey,
    /// Packed initialized tick array state for start_tick_index is positive
    pub positive_tick_array_bitmap: [[u64; 8]; EXTENSION_TICKARRAY_BITMAP_SIZE],
    /// Packed initialized tick array state for start_tick_index is negitive
    pub negative_tick_array_bitmap: [[u64; 8]; EXTENSION_TICKARRAY_BITMAP_SIZE],
}

impl TryInto<crate::dex::raydium_clmm::TickArrayBitmapExtension> for TickArrayBitmapExtension {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<crate::dex::TickArrayBitmapExtension, Self::Error> {
        Ok(crate::dex::TickArrayBitmapExtension {
            pool_id: self.pool_id,
            positive_tick_array_bitmap: self.positive_tick_array_bitmap,
            negative_tick_array_bitmap: self.negative_tick_array_bitmap,
        })
    }
}
