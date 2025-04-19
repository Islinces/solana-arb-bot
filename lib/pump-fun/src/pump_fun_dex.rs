use crate::pump_fun_pool::PumpFunPool;
use dex::interface::{DexInterface, DexPoolInterface};
use dex::state::FetchConfig;
use solana_client::nonblocking::rpc_client::RpcClient;
use std::sync::Arc;

pub struct PumpFunDex {
    pub pools: Vec<PumpFunPool>,
}

impl PumpFunDex {
    pub fn new(pools: Vec<PumpFunPool>) -> Self {
        Self { pools }
    }
}
#[async_trait::async_trait]
impl DexInterface for PumpFunDex {
    fn name(&self) -> String {
        todo!()
    }

    fn get_base_pools(&self) -> Vec<Arc<dyn DexPoolInterface>> {
        todo!()
    }

    async fn fetch_pool_base_info(
        _rpc_client: &RpcClient,
        _fetch_config: &FetchConfig,
    ) -> anyhow::Result<Arc<dyn DexInterface>>
    where
        Self: Sized,
    {
        todo!()
    }
}
