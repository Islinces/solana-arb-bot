use parking_lot::RwLockReadGuard;
use crate::account_cache::{DynamicCache, StaticCache};
use solana_sdk::pubkey::Pubkey;

mod amm_math;
mod byte_utils;
pub mod meteora_dlmm;
pub mod pump_fun;
pub mod raydium_amm;
pub mod raydium_clmm;

pub trait FromCache {
    fn from_cache(
        account_key: &Pubkey,
        static_cache: RwLockReadGuard<StaticCache>,
        dynamic_cache: &DynamicCache,
    ) -> Option<Self> where Self: Sized;
}
