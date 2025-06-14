use crate::dex::utils::read_from;
use crate::dex::{DynamicCache, FromCache, StaticCache};
use parking_lot::RwLockReadGuard;
use solana_sdk::pubkey::Pubkey;

const AMM_CONFIG_SEED: &str = "amm_config";

/// Holds the current owner of the factory
#[derive(Default, Debug)]
pub struct AmmConfig {
    /// 12，8
    pub trade_fee_rate: u64,
}

impl FromCache for AmmConfig {
    fn from_cache(
        account_key: &Pubkey,
        static_cache: RwLockReadGuard<StaticCache>,
        _dynamic_cache: &DynamicCache,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        let static_data = static_cache.get(account_key)?;
        let trade_fee_rate = unsafe { read_from::<u64>(&static_data[0..8]) };
        Some(Self { trade_fee_rate })
    }
}
