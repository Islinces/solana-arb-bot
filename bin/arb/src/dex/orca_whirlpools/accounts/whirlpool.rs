use crate::dex::utils::read_from;
use crate::dex::FromCache;
use crate::dex::global_cache::{DynamicCache, StaticCache};
use parking_lot::RwLockReadGuard;
use solana_sdk::pubkey::Pubkey;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Whirlpool {
    // 41,2
    pub tick_spacing: u16,
    // 43,2
    pub fee_tier_index_seed: [u8; 2],
    // 45,2
    pub fee_rate: u16,
    // 49,16
    pub liquidity: u128,
    // 65,16
    pub sqrt_price: u128,
    // 81,4
    pub tick_current_index: i32,
    // 85,8
    pub protocol_fee_owed_a: u64,
    // 93,8
    pub protocol_fee_owed_b: u64,
    // 101,32
    pub token_mint_a: Pubkey,
    // 133,32
    pub token_vault_a: Pubkey,
    // 181,32
    pub token_mint_b: Pubkey,
    // 213,32
    pub token_vault_b: Pubkey,
}

impl FromCache for Whirlpool {
    fn from_cache(
        account_key: &Pubkey,
        static_cache: RwLockReadGuard<StaticCache>,
        dynamic_cache: &DynamicCache,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        let static_data = static_cache.get(&account_key)?;
        let dynamic_data = dynamic_cache.get(&account_key)?;
        let dynamic_data = dynamic_data.value().as_slice();
        Whirlpool::from_slice_data(static_data, dynamic_data)
            .map_or(None, |whirlpool| Some(whirlpool))
    }
}

impl Whirlpool {
    pub fn from_slice_data(static_data: &[u8], dynamic_data: &[u8]) -> anyhow::Result<Self> {
        unsafe {
            let tick_spacing = read_from::<u16>(&static_data[0..2]);
            let fee_tier_index_seed = read_from::<[u8; 2]>(&static_data[2..4]);
            let fee_rate = read_from::<u16>(&static_data[4..6]);
            let protocol_fee_owed_a = read_from::<u64>(&static_data[6..14]);
            let protocol_fee_owed_b = read_from::<u64>(&static_data[14..22]);
            let token_mint_a = read_from::<Pubkey>(&static_data[22..54]);
            let token_vault_a = read_from::<Pubkey>(&static_data[54..86]);
            let token_mint_b = read_from::<Pubkey>(&static_data[86..118]);
            let token_vault_b = read_from::<Pubkey>(&static_data[118..150]);
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
                protocol_fee_owed_a,
                protocol_fee_owed_b,
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
            let protocol_fee_owed_a = read_from::<u64>(&data[85..93]);
            let protocol_fee_owed_b = read_from::<u64>(&data[93..101]);
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
                protocol_fee_owed_a,
                protocol_fee_owed_b,
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
