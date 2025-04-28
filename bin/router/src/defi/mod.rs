use crate::cache::{Pool, PoolCache};
use crate::defi::raydium_amm::raydium_amm::RaydiumAmmDex;
use crate::file_db::FileDB;
use crate::interface::{Dex, GrpcMessage, Protocol, DB};
use anyhow::anyhow;
use dashmap::{DashMap, Entry};
use solana_program::pubkey::Pubkey;
use std::collections::{HashMap, HashSet};
use std::ops::Sub;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tokio::task::JoinSet;
use tracing::info;

pub mod common;
mod json_state;
pub mod raydium_amm;
pub mod raydium_clmm;
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
        if let Some(_pool) = indexer.update_cache(grpc_message) {
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
            Protocol::RaydiumAMM => Some(Box::new(RaydiumAmmDex::new(pool, amount_in_mint))),
            _ => None,
        }
    }

    async fn find_best_path(
        paths: Arc<Vec<(Box<dyn Dex>, Box<dyn Dex>)>>,
    ) -> Option<DexQuoteResult> {
        let mut join_set = JoinSet::new();
        let mut start_amount_in = 1_000_000_u64;
        for i in 1..4 {
            start_amount_in = start_amount_in.checked_mul(10u64.pow(i)).unwrap();
            let paths = paths.clone();
            join_set.spawn(async move {
                let mut local_best: Option<(usize, u64, u64, u64)> = None;
                for (index, (f_dex, s_dex)) in paths.iter().enumerate() {
                    if let Some(f_amount_out) = f_dex.quote(start_amount_in).await {
                        if let Some(s_amount_out) = s_dex.quote(f_amount_out).await {
                            if s_amount_out < start_amount_in {
                                continue;
                            }
                            if local_best.is_none() || s_amount_out > local_best.unwrap().2 {
                                local_best = Some((
                                    index,
                                    start_amount_in,
                                    s_amount_out,
                                    s_amount_out.sub(start_amount_in),
                                ))
                            }
                        }
                    }
                }
                local_best
            });
        }
        let mut global_best: Option<(usize, u64, u64, u64)> = None;
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(local_best) => {
                    if let Some((path_index, amount_in, amount_out, profit)) = local_best {
                        if global_best.is_none() || profit >= global_best.unwrap().3 {
                            global_best = Some((path_index, amount_in, amount_out, profit));
                        }
                    }
                }
                Err(_error) => {}
            }
        }
        if let Some((path_index, amount_in, amount_out, profit)) = global_best {
            let option = paths.get(path_index).unwrap();
            Some(DexQuoteResult {
                path: vec![option.0.clone_self(), option.1.clone_self()],
                amount_in,
                amount_out,
                profit,
            })
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct DexQuoteResult {
    pub path: Vec<Box<dyn Dex>>,
    pub amount_in: u64,
    pub amount_out: u64,
    pub profit: u64,
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
        let rpc_db = FileDB::new(rpc_url);
        let pools = rpc_db
            .load_token_pools(supported_protocols().as_slice())
            .await
            .unwrap();
        let pool_map = DashMap::new();
        let mut edges = HashMap::new();
        for pool in pools.into_iter().filter(|pool| {
            subscribe_mints.contains(&pool.tokens.first().unwrap().mint)
                && subscribe_mints.contains(&pool.tokens.last().unwrap().mint)
        }) {
            let pool_id = pool.pool_id;
            let mint_pair = pool.token_pair();
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

    pub fn update_cache(&self, grpc_message: GrpcMessage) -> Option<Pubkey> {
        let pool_id = match grpc_message {
            GrpcMessage::RaydiumAmmData { pool_id, .. } => pool_id,
            GrpcMessage::RaydiumClmmData { pool_id, .. } => pool_id,
        };
        let arc = self.pool_cache.clone().pool_map;
        let successful = match arc.clone().entry(pool_id) {
            Entry::Occupied(ref mut exists) => {
                let pool = exists.get_mut();
                if pool.extra.try_change(grpc_message).is_ok() {
                    Ok(pool_id)
                } else {
                    Err(anyhow!(""))
                }
            }
            Entry::Vacant(_) => Err(anyhow!("")),
        };
        if successful.is_ok() {
            info!("更新缓存：成功 {:#?}", pool_id);
            Some(successful.unwrap())
        } else {
            info!("更新缓存：未发生变化{:#?}", pool_id);
            None
        }
    }
}
