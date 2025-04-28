use crate::cache::Pool;
use crate::interface::{Protocol, DB};
use anyhow::anyhow;
use solana_client::nonblocking::rpc_client::RpcClient;
use std::env;
use std::sync::Arc;
use tokio::task::JoinSet;

pub const FILE_DB_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/data");

#[derive(Debug, Clone)]
pub struct FileDB {
    rpc_url: String,
}

impl FileDB {
    pub fn new(rpc_url: &str) -> Self {
        Self {
            rpc_url: String::from(rpc_url),
        }
    }
}

#[async_trait::async_trait]
impl DB for FileDB {
    async fn load_token_pools(&self, protocols: &[Protocol]) -> anyhow::Result<Vec<Pool>> {
        let rpc_url = &self.rpc_url;
        let mut join_set = JoinSet::new();
        let rpc_client = Arc::new(RpcClient::new(rpc_url.clone()));
        for protocol in protocols {
            let protocol = protocol.clone();
            let snapshot_fetcher = protocol.get_snapshot_fetcher();
            if let Some(fetcher) = snapshot_fetcher {
                let rpc_client = rpc_client.clone();
                join_set.spawn(async move { fetcher.fetch_snapshot(rpc_client).await });
            }
        }
        let mut all_pools = Vec::new();
        while let Some(Ok(Some(pools))) = join_set.join_next().await {
            all_pools.extend(pools);
        }
        if !all_pools.is_empty() {
            Ok(all_pools)
        } else {
            Err(anyhow!("未找到任何池子"))
        }
    }
}
