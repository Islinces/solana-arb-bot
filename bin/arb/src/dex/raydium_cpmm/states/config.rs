use crate::dex::utils::read_from;
use crate::dex::FromCache;
use anyhow::anyhow;
use std::sync::Arc;

const AMM_CONFIG_SEED: &str = "amm_config";

/// Holds the current owner of the factory
#[derive(Default, Debug)]
pub struct AmmConfig {
    /// 12，8
    pub trade_fee_rate: u64,
}

impl FromCache for AmmConfig {
    fn from_cache(
        static_cache: Option<Arc<Vec<u8>>>,
        _dynamic_cache: Option<Arc<Vec<u8>>>,
    ) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let static_data = static_cache.ok_or(anyhow!(""))?;
        let trade_fee_rate = unsafe { read_from::<u64>(&static_data[0..8]) };
        Ok(Self { trade_fee_rate })
    }
}
