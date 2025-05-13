use crate::cache::Pool;
use crate::interface::{DexType, DB};
use anyhow::anyhow;
use anyhow::Result;
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::str::FromStr;
use std::sync::Arc;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use tokio::task::JoinSet;
use tracing::{error, info, warn};

pub struct FileDB {
    rpc_client: Arc<RpcClient>,
    dex_json_path: String,
}

impl FileDB {
    pub fn new(rpc_client: Arc<RpcClient>, dex_json_path: String) -> Self {
        Self {
            rpc_client,
            dex_json_path,
        }
    }

    pub fn load_dex_json(&self) -> Result<Vec<DexJson>> {
        let dex_jsons: Vec<DexJson> = match File::open(self.dex_json_path.clone().as_str()) {
            Ok(file) => serde_json::from_reader(file).expect("解析【dex_data.json】失败"),
            Err(e) => {
                error!("{}", e);
                vec![]
            }
        };
        if dex_jsons.is_empty() {
            Err(anyhow!("文件【dex_data.json】无池子数据"))
        } else {
            Ok(dex_jsons)
        }
    }
}

#[async_trait::async_trait]
impl DB for FileDB {
    async fn load_token_pools(&self) -> Result<Vec<Pool>> {
        let dex_jsons = self.load_dex_json()?;
        let mut dex_pool_group = HashMap::new();
        for dex_json in dex_jsons {
            if dex_json.address_lookup_table_address.is_none() {
                continue;
            }
            dex_pool_group
                .entry(DexType::from(dex_json.owner))
                .or_insert(Vec::new())
                .push(dex_json);
        }
        let mut join_set = JoinSet::new();
        for (protocol, pool_ids) in dex_pool_group {
            let snapshot_fetcher = protocol.get_snapshot_fetcher();
            if let Ok(fetcher) = snapshot_fetcher {
                let rpc_client = self.rpc_client.clone();
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
    #[serde(
        deserialize_with = "deserialize_option_pubkey",
        rename = "addressLookupTableAddress"
    )]
    pub address_lookup_table_address: Option<Pubkey>,
}

fn deserialize_pubkey<'de, D>(deserializer: D) -> Result<Pubkey, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    Ok(Pubkey::from_str(s.as_str()).unwrap())
}

fn deserialize_option_pubkey<'de, D>(deserializer: D) -> Result<Option<Pubkey>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Result<String, _> = Deserialize::deserialize(deserializer);
    if s.is_err() {
        return Ok(None);
    }
    Ok(Some(Pubkey::from_str(s?.as_str()).unwrap()))
}
