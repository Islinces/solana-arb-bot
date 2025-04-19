use crate::dlmm_pool::DlmmPool;
use dex::interface::{DexInterface, DexPoolInterface};
use dex::state::FetchConfig;
use solana_client::nonblocking::rpc_client::RpcClient;
use std::sync::Arc;

pub struct MeteoraDlmmDex {
    pub pools: Vec<DlmmPool>,
}

impl MeteoraDlmmDex {
    pub fn new(pools: Vec<DlmmPool>) -> Self {
        Self { pools }
    }
}

#[async_trait::async_trait]
impl DexInterface for MeteoraDlmmDex {
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
