use crate::defi::dex::Dex;
use crate::defi::file_db::RpcDB;
use crate::defi::raydium_amm::raydium_amm::RaydiumAmmDex;
use crate::defi::types::{Pool, PoolCache, Protocol, TokenPairPools, TokenPools};
use crate::strategy::grpc_message_processor::GrpcMessage;
use dashmap::DashMap;
use solana_program::pubkey::Pubkey;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tokio::task::JoinSet;
use tracing::info;

pub mod common;
pub mod dex;
pub mod file_db;
pub mod raydium_amm;
pub mod raydium_clmm;
pub mod types;
// pub mod pump_fun;

static INDEXER: OnceCell<Arc<DexIndexer>> = OnceCell::const_new();

#[derive(Clone)]
pub struct Defi {
    indexer: Arc<DexIndexer>,
}

impl Defi {
    pub async fn new(rpc_url: &str, subscribe_mints: &Vec<Pubkey>) -> anyhow::Result<Self> {
        let indexer = INDEXER
            .get_or_init(|| async {
                let indexer = DexIndexer::new(rpc_url, subscribe_mints).await;
                Arc::new(indexer)
            })
            .await
            .clone();
        Ok(Self { indexer })
    }

    pub fn get_pool_data(&self, pool_id: &Pubkey) -> Option<Pool> {
        let option = self.indexer.pool_cache.pool_map.get(pool_id);
        option.map(|pool| pool.clone())
    }

    pub fn get_support_protocols(&self) -> Vec<Protocol> {
        self.indexer
            .pool_cache
            .pool_map
            .iter()
            .map(|pool| pool.protocol.clone())
            .collect::<HashSet<Protocol>>()
            .into_iter()
            .collect::<Vec<Protocol>>()
    }

    pub fn get_all_pools(&self) -> Option<HashMap<Protocol, Vec<Pool>>> {
        let mut protocol_pool_map: HashMap<Protocol, Vec<Pool>> = HashMap::new();
        for pool in self.indexer.pool_cache.pool_map.iter() {
            protocol_pool_map
                .entry(pool.protocol.clone())
                .or_default()
                .push(pool.value().clone());
        }
        Some(protocol_pool_map)
    }

    pub async fn update_and_find_route(
        &self,
        grpc_message: GrpcMessage,
    ) -> Option<Vec<(Pool, Pool)>> {
        let indexer = self.indexer.clone();
        if let Some(_update_pool) = indexer.pool_cache.update_cache(grpc_message) {
            //TODO:支持配置
            let amount_in_mint =
                Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
            if let Some(paths) = indexer.find_path(&amount_in_mint).await {
                let path_result = Self::find_best_path(Arc::new(paths)).await;
                info!("path_result {:?}", path_result);
            };
            None
        } else {
            None
        }
    }

    fn create_dex(amount_in_mint: Pubkey, pool: Pool) -> Option<Box<dyn Dex>> {
        match pool.protocol {
            // Protocol::RaydiumAMM => Some(Box::new(RaydiumAmmDex::new(pool, amount_in_mint))),
            _ => None,
        }
    }

    async fn find_best_path(paths: Arc<Vec<(Box<dyn Dex>, Box<dyn Dex>)>>) -> Vec<(u64, u64)> {
        let mut join_set = JoinSet::new();
        let mut start_amount_in = 1_000_000_u64;
        for i in 1..4 {
            start_amount_in = start_amount_in.checked_mul(10u64.pow(i)).unwrap();
            let paths = paths.clone();
            join_set.spawn(async move {
                paths
                    .iter()
                    .filter_map(|(pool1, pool2)| {
                        if let Some(f_amount_out) = pool1.quote(start_amount_in) {
                            if let Some(s_amount_out) = pool2.quote(f_amount_out) {
                                if s_amount_out <= start_amount_in {
                                    return None;
                                } else {
                                    Some((f_amount_out, s_amount_out))
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            });
        }
        let mut res = Vec::new();
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(dexes) => res.extend(dexes),
                Err(_error) => {}
            }
        }
        res
    }
}

pub fn supported_protocols() -> Vec<Protocol> {
    vec![Protocol::RaydiumAMM, Protocol::RaydiumCLmm]
}

#[derive(Clone)]
pub struct DexIndexer {
    pool_cache: PoolCache,
    // db: Arc<dyn DB>,
}

impl DexIndexer {
    pub async fn new(rpc_url: &str, subscribe_mints: &[Pubkey]) -> Self {
        let rpc_db = RpcDB::new(rpc_url);
        let pools = rpc_db
            .load_token_pools(supported_protocols().as_slice())
            .await
            .unwrap();
        let pool_map = DashMap::new();
        let token_pools = TokenPools::new();
        let token_pair_pools = TokenPairPools::new();
        let mut edges = HashMap::new();
        for pool in pools.into_iter().filter(|pool| {
            subscribe_mints.contains(&pool.tokens.first().unwrap().mint)
                && subscribe_mints.contains(&pool.tokens.last().unwrap().mint)
        }) {
            let pool_id = pool.pool_id;
            let mint_pair = pool.token_pair();
            // for token in &pool.tokens {
            //     let mint = token.mint;
            //     token_pools
            //         .entry(mint)
            //         .or_insert_with(HashSet::new)
            //         .insert(pool_id);
            // }
            // token_pair_pools
            //     .entry(mint_pair)
            //     .or_insert_with(HashSet::new)
            //     .insert(pool_id);
            pool_map.insert(pool_id, pool);
            edges
                .entry(mint_pair.0)
                .or_insert(vec![])
                .push((mint_pair.1, pool_id));
            edges
                .entry(mint_pair.1)
                .or_insert(vec![])
                .push((mint_pair.0, pool_id));
        }
        Self {
            pool_cache: PoolCache::new(edges, pool_map),
            // db: Arc::new(rpc_db),
        }
    }

    pub async fn find_path(
        &self,
        amount_in_mint: &Pubkey,
    ) -> Option<Vec<(Box<dyn Dex>, Box<dyn Dex>)>> {
        let pool_cache = self.pool_cache.clone();
        let edges = pool_cache.edges;
        let pool_map = pool_cache.pool_map;
        let mut paths = Vec::new();
        if let Some(first_pools) = edges.get(amount_in_mint) {
            for (f_pool_out_mint, f_pool) in first_pools {
                if let Some(second_pools) = edges.get(f_pool_out_mint) {
                    for (s_pool_out_mint, s_pool) in second_pools {
                        if amount_in_mint != s_pool_out_mint || f_pool == s_pool {
                            continue;
                        }
                        if let Some(f_dex) = Defi::create_dex(
                            amount_in_mint.clone(),
                            pool_map.get(f_pool).unwrap().clone(),
                        ) {
                            if let Some(s_dex) = Defi::create_dex(
                                f_pool_out_mint.clone(),
                                pool_map.get(s_pool).unwrap().clone(),
                            ) {
                                paths.push((f_dex, s_dex))
                            }
                        }
                    }
                }
            }
        }

        if paths.is_empty() {
            None
        } else {
            Some(paths)
        }
    }
}

#[async_trait::async_trait]
pub trait DB: Debug + Send + Sync {
    async fn load_token_pools(&self, protocols: &[Protocol]) -> anyhow::Result<Vec<Pool>>;
}
