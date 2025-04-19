use crate::clmm_pool::ClmmPool;
use dex::interface::{DexInterface, DexPoolInterface};
use dex::state::FetchConfig;
use solana_client::nonblocking::rpc_client::RpcClient;
use std::sync::Arc;

pub struct RaydiumClmmDex {
    pub amm_pool: Vec<ClmmPool>,
}

impl RaydiumClmmDex {
    pub fn new(amm_pool: Vec<ClmmPool>) -> Self {
        Self { amm_pool }
    }
}

#[async_trait::async_trait]
impl DexInterface for RaydiumClmmDex {
    fn name(&self) -> String {
        todo!()
    }

    fn get_base_pools(&self) -> Vec<Arc<dyn DexPoolInterface>> {
        todo!()
    }

    async fn fetch_pool_base_info(
        rpc_client: &RpcClient,
        fetch_config: &FetchConfig,
    ) -> anyhow::Result<Arc<dyn DexInterface>>
    where
        Self: Sized,
    {
        todo!()
    }
}
