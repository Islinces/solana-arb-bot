use solana_sdk::pubkey::Pubkey;

#[repr(C)]
#[derive(Clone, Debug)]
pub struct BinArray {
    pub index: i64,
    pub version: u8,
    pub padding: [u8; 7],
    pub lb_pair: Pubkey,
    pub bins: [Bin; 70],
}

impl TryInto<crate::dex::meteora_dlmm::interface::accounts::BinArray> for BinArray {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<crate::dex::BinArray, Self::Error> {
        let mut bins = [crate::dex::meteora_dlmm::interface::Bin::default(); 70];
        for (index, bin) in self.bins.into_iter().enumerate() {
            bins[index] = bin.try_into()?;
        }
        Ok(crate::dex::BinArray {
            index: self.index,
            lb_pair: self.lb_pair,
            bins,
        })
    }
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct Bin {
    pub amount_x: u64,
    pub amount_y: u64,
    pub price: u128,
    pub liquidity_supply: u128,
    pub reward_per_token_stored: [u128; 2],
    pub fee_amount_x_per_token_stored: u128,
    pub fee_amount_y_per_token_stored: u128,
    pub amount_x_in: u128,
    pub amount_y_in: u128,
}

impl TryInto<crate::dex::meteora_dlmm::interface::Bin> for Bin {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<crate::dex::meteora_dlmm::interface::Bin, Self::Error> {
        Ok(crate::dex::meteora_dlmm::interface::Bin {
            amount_x: self.amount_x,
            amount_y: self.amount_y,
            price: self.price,
        })
    }
}
