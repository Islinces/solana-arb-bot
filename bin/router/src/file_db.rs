use crate::cache::Pool;
use crate::interface::{DexType, DB};
use anyhow::anyhow;
use anyhow::Result;
use log::warn;
use serde::{Deserialize, Deserializer};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::pubkey::Pubkey;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::str::FromStr;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{error, info};

pub const FILE_DB_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/data/dex_data.json");

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
    async fn load_token_pools(&self, protocols: &[DexType]) -> anyhow::Result<Vec<Pool>> {
        let dex_jsons: Vec<DexJson> = match File::open(FILE_DB_DIR) {
            Ok(file) => serde_json::from_reader(file).expect("解析【dex_data.json】失败"),
            Err(e) => {
                error!("{}", e);
                vec![]
            }
        };
        if dex_jsons.is_empty() {
            return Err(anyhow!("文件【dex_data.json】无池子数据"));
        }
        let mut dex_pool_group = HashMap::new();
        for dex_json in dex_jsons {
            dex_pool_group
                .entry(DexType::from(dex_json.owner))
                .or_insert(Vec::new())
                .push(dex_json.pool);
        }
        let rpc_url = &self.rpc_url;
        let mut join_set = JoinSet::new();
        let rpc_client = Arc::new(RpcClient::new(rpc_url.clone()));
        for (protocol, pool_ids) in dex_pool_group {
            let snapshot_fetcher = protocol.get_snapshot_fetcher();
            if let Ok(fetcher) = snapshot_fetcher {
                let rpc_client = rpc_client.clone();
                join_set.spawn(async move {
                    let fetched_snapshots = fetcher.fetch_snapshot(pool_ids, rpc_client).await;
                    if let Some(snapshots) = fetched_snapshots.as_ref() {
                        info!(
                            "【{}】 fetch snapshot finished, fetch pool size is {}",
                            protocol,
                            snapshots.len()
                        );
                        fetched_snapshots
                    } else {
                        warn!(
                            "【{}】 fetch snapshot finished, not fetch any snapshots",
                            protocol
                        );
                        None
                    }
                });
            }
        }
        let mut all_pools = Vec::new();
        while let Some(Ok(Some(pools))) = join_set.join_next().await {
            all_pools.extend(pools);
        }
        if !all_pools.is_empty() {
            Ok(all_pools)
        } else {
            Err(anyhow!("所有DEX未找到任何池子"))
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DexJson {
    #[serde(deserialize_with = "deserialize_pubkey")]
    pub pool: Pubkey,
    #[serde(deserialize_with = "deserialize_pubkey")]
    pub owner: Pubkey,
}

fn deserialize_pubkey<'de, D>(deserializer: D) -> Result<Pubkey, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    Ok(Pubkey::from_str(s.as_str()).unwrap())
}
