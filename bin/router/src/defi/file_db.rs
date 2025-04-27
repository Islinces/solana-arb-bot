use crate::defi::dex::Dex;
use crate::defi::raydium_amm::raydium_amm::RaydiumAmmDex;
use crate::defi::raydium_amm::state::{AmmInfo, Loadable, PoolInfo};
use crate::defi::raydium_clmm::raydium_clmm::RaydiumClmmDex;
use crate::defi::types::{Mint, Pool, PoolExtra, Protocol};
use crate::defi::DB;
use anyhow::anyhow;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::program_pack::Pack;
use solana_sdk::commitment_config::CommitmentConfig;
use spl_token::state::Account;
use std::env;
use std::fs::File;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::error;

pub const FILE_DB_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/data");

#[derive(Debug, Clone)]
pub struct RpcDB {
    rpc_url: String,
}

impl RpcDB {
    pub fn new(rpc_url: &str) -> Self {
        Self {
            rpc_url: String::from(rpc_url),
        }
    }
}

#[async_trait::async_trait]
impl DB for RpcDB {
    async fn load_token_pools(&self, protocols: &[Protocol]) -> anyhow::Result<Vec<Pool>> {
        let rpc_url = &self.rpc_url;
        let mut join_set = JoinSet::new();
        let rpc_client = Arc::new(RpcClient::new(rpc_url.clone()));
        for protocol in protocols {
            let protocol = protocol.clone();
            let rpc_client = rpc_client.clone();
            join_set.spawn(async move {
                match protocol {
                    Protocol::RaydiumAMM => RaydiumAmmDex::fetch_snapshot(rpc_client).await,
                    Protocol::RaydiumCLmm => RaydiumClmmDex::fetch_snapshot(rpc_client).await,
                    // Protocol::PumpFunAMM => {}
                    // Protocol::MeteoraDLMM => {}
                    _ => None,
                }
            });
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
