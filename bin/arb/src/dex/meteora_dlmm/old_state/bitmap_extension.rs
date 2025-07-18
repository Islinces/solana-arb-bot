use solana_sdk::pubkey::Pubkey;

#[repr(C)]
#[derive(Clone, Debug)]
pub struct BinArrayBitmapExtension {
    pub lb_pair: Pubkey,
    pub positive_bin_array_bitmap: [[u64; 8]; 12],
    pub negative_bin_array_bitmap: [[u64; 8]; 12],
}

impl TryInto<crate::dex::BinArrayBitmapExtension> for BinArrayBitmapExtension {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<crate::dex::BinArrayBitmapExtension, Self::Error> {
        Ok(crate::dex::BinArrayBitmapExtension {
            lb_pair: self.lb_pair,
            positive_bin_array_bitmap: self.positive_bin_array_bitmap,
            negative_bin_array_bitmap: self.negative_bin_array_bitmap,
        })
    }
}
