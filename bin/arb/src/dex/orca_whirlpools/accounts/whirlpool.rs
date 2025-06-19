use crate::dex::utils::read_from;
use crate::dex::FromCache;
use anyhow::anyhow;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;

#[repr(C, packed)]
#[derive(Clone, Debug)]
// #[serde_as]
// #[cfg_attr(feature = "print_data_after_update", derive(Serialize, Deserialize))]
pub struct Whirlpool {
    // 41,2
    pub tick_spacing: u16,
    // 43,2
    pub fee_tier_index_seed: [u8; 2],
    // 45,2
    pub fee_rate: u16,
    // 49,16
    // #[serde_as(as = "DisplayFromStr")]
    pub liquidity: u128,
    // 65,16
    // #[serde_as(as = "DisplayFromStr")]
    pub sqrt_price: u128,
    // 81,4
    pub tick_current_index: i32,
    // 101,32
    // #[serde_as(as = "DisplayFromStr")]
    pub token_mint_a: Pubkey,
    // 133,32
    // #[serde_as(as = "DisplayFromStr")]
    pub token_vault_a: Pubkey,
    // 181,32
    // #[serde_as(as = "DisplayFromStr")]
    pub token_mint_b: Pubkey,
    // 213,32
    // #[serde_as(as = "DisplayFromStr")]
    pub token_vault_b: Pubkey,
}

impl FromCache for Whirlpool {
    fn from_cache(
        static_cache: Option<Arc<Vec<u8>>>,
        dynamic_cache: Option<Arc<Vec<u8>>>,
    ) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let static_data = static_cache.ok_or(anyhow!(""))?;
        let dynamic_data = dynamic_cache.ok_or(anyhow!(""))?;
        Whirlpool::from_slice_data(static_data.as_slice(), dynamic_data.as_slice())
    }
}

impl Whirlpool {
    pub fn from_slice_data(static_data: &[u8], dynamic_data: &[u8]) -> anyhow::Result<Self> {
        unsafe {
            let tick_spacing = read_from::<u16>(&static_data[0..2]);
            let fee_tier_index_seed = read_from::<[u8; 2]>(&static_data[2..4]);
            let fee_rate = read_from::<u16>(&static_data[4..6]);
            let token_mint_a = read_from::<Pubkey>(&static_data[6..38]);
            let token_vault_a = read_from::<Pubkey>(&static_data[38..70]);
            let token_mint_b = read_from::<Pubkey>(&static_data[70..102]);
            let token_vault_b = read_from::<Pubkey>(&static_data[102..134]);
            let liquidity = read_from::<u128>(&dynamic_data[0..16]);
            let sqrt_price = read_from::<u128>(&dynamic_data[16..32]);
            let tick_current_index = read_from::<i32>(&dynamic_data[32..36]);
            Ok(Self {
                tick_spacing,
                fee_tier_index_seed,
                fee_rate,
                liquidity,
                sqrt_price,
                tick_current_index,
                token_mint_a,
                token_vault_a,
                token_mint_b,
                token_vault_b,
            })
        }
    }

    #[inline(always)]
    pub fn from_bytes(data: &[u8]) -> Result<Self, std::io::Error> {
        let mut data = data;
        unsafe {
            let tick_spacing = read_from::<u16>(&data[41..43]);
            let fee_tier_index_seed = read_from::<[u8; 2]>(&data[43..45]);
            let fee_rate = read_from::<u16>(&data[45..47]);
            let liquidity = read_from::<u128>(&data[49..65]);
            let sqrt_price = read_from::<u128>(&data[65..81]);
            let tick_current_index = read_from::<i32>(&data[81..85]);
            let token_mint_a = read_from::<Pubkey>(&data[101..133]);
            let token_vault_a = read_from::<Pubkey>(&data[133..165]);
            let token_mint_b = read_from::<Pubkey>(&data[181..213]);
            let token_vault_b = read_from::<Pubkey>(&data[213..245]);
            Ok(Self {
                tick_spacing,
                fee_tier_index_seed,
                fee_rate,
                liquidity,
                sqrt_price,
                tick_current_index,
                token_mint_a,
                token_vault_a,
                token_mint_b,
                token_vault_b,
            })
        }
    }

    pub fn is_initialized_with_adaptive_fee(&self) -> bool {
        self.tick_spacing != u16::from_le_bytes(self.fee_tier_index_seed)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct WhirlpoolFacade {
    pub fee_tier_index_seed: [u8; 2],
    pub tick_spacing: u16,
    pub fee_rate: u16,
    pub liquidity: u128,
    pub sqrt_price: u128,
    pub tick_current_index: i32,
    pub token_mint_a: Pubkey,
    pub token_mint_b: Pubkey,
}

impl WhirlpoolFacade {
    pub fn fee_tier_index(&self) -> u16 {
        u16::from_le_bytes(self.fee_tier_index_seed)
    }

    pub fn is_initialized_with_adaptive_fee(&self) -> bool {
        self.fee_tier_index() != self.tick_spacing
    }
}

impl From<Whirlpool> for WhirlpoolFacade {
    fn from(val: Whirlpool) -> Self {
        WhirlpoolFacade {
            fee_tier_index_seed: val.fee_tier_index_seed,
            tick_spacing: val.tick_spacing,
            fee_rate: val.fee_rate,
            liquidity: val.liquidity,
            sqrt_price: val.sqrt_price,
            tick_current_index: val.tick_current_index,
            token_mint_a: val.token_mint_a,
            token_mint_b: val.token_mint_b,
        }
    }
}
