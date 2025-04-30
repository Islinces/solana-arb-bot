use crate::cache::{Pool, PoolCache};
use crate::file_db::FileDB;
use crate::interface::{Dex, GrpcMessage, Protocol, DB};
use anyhow::anyhow;
use dashmap::{DashMap, Entry};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::clock::Clock;
use solana_program::pubkey::Pubkey;
use solana_program::sysvar::SysvarId;
use std::collections::{HashMap, HashSet};
use std::ops::Sub;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tokio::task::JoinSet;
use tracing::info;

pub mod common;
pub mod meteora_dlmm;
pub mod pump_fun;
pub mod raydium_amm;
pub mod raydium_clmm;

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

    pub async fn update_cache_and_find_route(
        &self,
        grpc_message: GrpcMessage,
    ) -> Option<Vec<(Pool, Pool)>> {
        let indexer = self.indexer.clone();
        if let Some(update_pool) = indexer.update_cache(grpc_message) {
            //TODO:支持配置
            let amount_in_mint =
                Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
            if let Some(paths) = indexer.find_path(&amount_in_mint, &update_pool).await {
                let reverse_path = paths
                    .iter()
                    .map(|p| {
                        let mut path = p.clone();
                        path.path.reverse();
                        path
                    })
                    .collect::<Vec<_>>();
                let path_result =
                    Self::find_best_path(Arc::new(paths), Arc::new(reverse_path)).await;
                if path_result.is_some() {
                    info!("套利路径: {:?}", path_result.unwrap());
                }
            };
            None
        } else {
            None
        }
    }

    async fn create_dex(
        amount_in_mint: Pubkey,
        pool: &Pool,
        clock: &Clock,
    ) -> Option<Box<dyn Dex>> {
        let protocol = &pool.protocol;
        protocol
            .create_dex(amount_in_mint, pool.clone(), clock.clone())
            .await
    }

    async fn find_best_path(
        paths: Arc<Vec<Path>>,
        reverse_path: Arc<Vec<Path>>,
    ) -> Option<DexQuoteResult> {
        let mut join_set = JoinSet::new();
        let mut start_amount_in = 1_000_000_u64;
        for i in 1..3 {
            start_amount_in = start_amount_in.checked_mul(10u64.pow(i)).unwrap();
            let paths = paths.clone();
            join_set.spawn(async move {
                (
                    true,
                    Defi::find_best_path_for_single_direction_path(paths, start_amount_in).await,
                )
            });
            let reverse_path = reverse_path.clone();
            join_set.spawn(async move {
                (
                    false,
                    Defi::find_best_path_for_single_direction_path(reverse_path, start_amount_in)
                        .await,
                )
            });
        }

        let mut global_best: Option<(bool, (usize, u64, u64, u64))> = None;
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(local_best) => {
                    if let (is_positive, Some((path_index, amount_in, amount_out, profit))) =
                        local_best
                    {
                        if global_best.is_none() || profit >= global_best.unwrap().1 .3 {
                            global_best =
                                Some((is_positive, (path_index, amount_in, amount_out, profit)));
                        }
                    }
                }
                Err(_error) => {}
            }
        }
        if let Some((is_positive, (path_index, amount_in, amount_out, profit))) = global_best {
            let option = if is_positive {
                paths.get(path_index).unwrap()
            } else {
                reverse_path.get(path_index).unwrap()
            };
            Some(DexQuoteResult {
                path: vec![
                    option.path.first().unwrap().clone_self(),
                    option.path.last().unwrap().clone_self(),
                ],
                amount_in,
                amount_out,
                profit,
            })
        } else {
            None
        }
    }

    async fn find_best_path_for_single_direction_path(
        paths: Arc<Vec<Path>>,
        start_amount_in: u64,
    ) -> Option<(usize, u64, u64, u64)> {
        let mut local_best: Option<(usize, u64, u64, u64)> = None;
        for (index, path) in paths.iter().enumerate() {
            if let Some(f_amount_out) = path.path[0].quote(start_amount_in).await {
                if let Some(s_amount_out) = path.path[1].quote(f_amount_out).await {
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
    vec![
        Protocol::RaydiumAMM,
        Protocol::RaydiumCLmm,
        Protocol::PumpFunAMM,
        Protocol::MeteoraDLMM,
    ]
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
        let clock: Clock = bincode::deserialize(
            RpcClient::new(rpc_url.to_string())
                .get_account(&Clock::id())
                .await
                .unwrap()
                .data
                .as_ref(),
        )
        .unwrap();
        Self {
            pool_cache: PoolCache::new(edges, pool_map, clock),
        }
    }

    pub async fn find_path(
        &self,
        amount_in_mint: &Pubkey,
        update_pool: &Pubkey,
    ) -> Option<Vec<Path>> {
        let pool_cache = self.pool_cache.clone();
        let edges = pool_cache.edges;
        let pool_map = pool_cache.pool_map;
        let clock = pool_cache.clock;
        let mut paths = Vec::new();
        if let Some(first_pools) = edges.get(amount_in_mint) {
            for (f_pool_out_mint, f_pool) in first_pools {
                if let Some(second_pools) = edges.get(f_pool_out_mint) {
                    for (s_pool_out_mint, s_pool) in second_pools {
                        if amount_in_mint != s_pool_out_mint || f_pool == s_pool {
                            continue;
                        }
                        // 要包含更新的池子
                        if f_pool != update_pool && s_pool != update_pool {
                            continue;
                        }
                        if let Some(f_dex) = Defi::create_dex(
                            *amount_in_mint,
                            &pool_map.get(f_pool).unwrap(),
                            &clock,
                        )
                        .await
                        {
                            if let Some(s_dex) = Defi::create_dex(
                                *f_pool_out_mint,
                                &pool_map.get(s_pool).unwrap().clone(),
                                &clock,
                            )
                            .await
                            {
                                paths.push(Path {
                                    path: vec![f_dex, s_dex],
                                });
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
        if let GrpcMessage::Clock(clock) = grpc_message {
            self.pool_cache.clone().clock = clock;
            return None;
        }
        let pool_id = grpc_message.pool_id()?;
        let arc = self.pool_cache.clone().pool_map;
        let successful = match arc.entry(pool_id) {
            Entry::Occupied(ref mut exists) => {
                let pool = exists.get_mut();
                match pool.protocol.get_cache_updater(grpc_message) {
                    Ok(updater) => {
                        if updater.update_cache(pool).is_ok() {
                            Ok(pool_id)
                        } else {
                            Err(anyhow!(""))
                        }
                    }
                    Err(e) => Err(anyhow!("更新缓存: 失败，原因: {}", e)),
                }
            }
            Entry::Vacant(_) => Err(anyhow!("")),
        };
        if successful.is_ok() {
            info!("更新缓存: 成功 {:?}", pool_id);
            Some(successful.unwrap())
        } else {
            info!("更新缓存: 未发生变化{:?}", pool_id);
            None
        }
    }
}

pub struct Path {
    path: Vec<Box<dyn Dex>>,
}

impl Clone for Path {
    fn clone(&self) -> Self {
        Self {
            path: self.path.iter().map(|path| path.clone_self()).collect(),
        }
    }
}
