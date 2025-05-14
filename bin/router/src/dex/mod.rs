use crate::cache::{Pool, PoolCache};
use crate::file_db::FileDB;
use crate::interface::{DexType, GrpcMessage, InstructionItem, DB};
use anyhow::anyhow;
use arrayref::{array_ref, array_refs};
use bincode::config;
use dashmap::{DashMap, Entry};
use moka::sync::{Cache, CacheBuilder};
use num_traits::Pow;
use rayon::prelude::*;
use solana_account_decoder_client_types::UiDataSliceConfig;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client_api::config::RpcAccountInfoConfig;
use solana_sdk::clock::Clock;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::SysvarId;
use std::collections::HashMap;
use std::ops::{Add, Mul, Sub};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::OnceCell;
use tokio::task::JoinSet;
use tracing::{info, instrument};

pub mod common;
pub mod meteora_dlmm;
pub mod pump_fun;
pub mod raydium_amm;
pub mod raydium_clmm;

const MINT_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";
const ATA_PROGRAM: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

pub fn get_mint_program() -> Pubkey {
    Pubkey::from_str(MINT_PROGRAM).unwrap()
}

pub fn get_ata_program() -> Pubkey {
    Pubkey::from_str(ATA_PROGRAM).unwrap()
}

pub fn get_system_program() -> Pubkey {
    Pubkey::from_str(SYSTEM_PROGRAM).unwrap()
}

pub static POOL_CACHE_HOLDER: OnceCell<Arc<PoolCacheHolder>> = OnceCell::const_new();

#[derive(Clone, Debug)]
pub struct DexData {
    pub pool_cache_holder: Arc<PoolCacheHolder>,
    pub increase_step: u64,
}

impl DexData {
    pub async fn new_only_cache_holder(
        rpc_client: Arc<RpcClient>,
        dex_json_path: String,
    ) -> anyhow::Result<Self> {
        // 单例
        let pool_cache_holder = POOL_CACHE_HOLDER
            .get_or_init(|| async {
                Arc::new(PoolCacheHolder::new(rpc_client, dex_json_path).await)
            })
            .await
            .clone();
        Ok(Self {
            pool_cache_holder,
            increase_step: 0,
        })
    }

    pub async fn new(rpc_client: Arc<RpcClient>, dex_json_path: String) -> anyhow::Result<Self> {
        // 单例
        let pool_cache_holder = POOL_CACHE_HOLDER
            .get_or_init(|| async {
                Arc::new(PoolCacheHolder::new(rpc_client, dex_json_path).await)
            })
            .await
            .clone();
        Ok(Self {
            pool_cache_holder,
            increase_step: 0.5_f64.mul(10.0.pow(9)) as u64,
        })
    }

    pub fn get_all_pools(&self) -> Option<HashMap<DexType, Vec<Pool>>> {
        let mut protocol_pool_map: HashMap<DexType, Vec<Pool>> = HashMap::new();
        for pool in self.pool_cache_holder.pool_cache.pool_map.iter() {
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
        profit_threshold: u64,
        start_amount_in: u64,
        sol_ata_amount: u64,
    ) -> Option<DexQuoteResult> {
        let grpc_cost = grpc_message.instant();
        let start_time = Instant::now();
        let indexer = self.pool_cache_holder.clone();
        //TODO：不一定是以修改的池子作为开始
        if let Some(update_pool) = indexer.update_cache(grpc_message) {
            //TODO:支持配置
            let amount_in_mint = spl_token::native_mint::id();
            if let Some((positive_paths, reverse_paths)) =
                indexer.build_graph(&amount_in_mint, &update_pool)
            {
                let all_path_size = positive_paths.len() + reverse_paths.len();
                let quote_start = Instant::now();
                let mut dex_quote_result = self
                    .find_best_route(
                        amount_in_mint,
                        Arc::new(positive_paths),
                        Arc::new(reverse_paths),
                        profit_threshold,
                        start_amount_in,
                        sol_ata_amount,
                    )
                    .await;
                if let Some(result) = &mut dex_quote_result {
                    result.start_time = Some(start_time);
                    result.route_calculate_cost = Some(quote_start.elapsed().as_nanos());
                    result.grop_cost = Some(grpc_cost);
                }
                dex_quote_result
            } else {
                None
            }
        } else {
            None
        }
    }

    pub async fn find_best_route(
        &self,
        amount_in_mint: Pubkey,
        path0: Arc<Vec<Path>>,
        path1: Arc<Vec<Path>>,
        profit_threshold: u64,
        start_amount_in: u64,
        sol_ata_amount: u64,
    ) -> Option<DexQuoteResult> {
        let pool_cache = self.pool_cache_holder.pool_cache.clone();
        let pool_map = pool_cache.pool_map;
        let clock = Arc::new(pool_cache.clock);
        let clock = clock.clone();
        let pool_map = pool_map.clone();
        let positive_local_best = DexData::find_best_path_for_positive_path(
            path0.clone(),
            pool_map.clone(),
            start_amount_in,
            amount_in_mint,
            clock.clone(),
        );
        let reverse_local_best = DexData::find_best_path_for_reverse_path(
            path1.clone(),
            pool_map.clone(),
            start_amount_in,
            amount_in_mint,
            clock.clone(),
        );
        let mut global_best = None;
        if let Some(local_best) = positive_local_best {
            global_best = Some(local_best);
        }
        if let Some(local_best) = reverse_local_best {
            if let Some(ref global) = global_best {
                if global.profit < local_best.profit {
                    global_best = Some(local_best);
                }
            } else {
                global_best = Some(local_best);
            }
        }
        // 构建executor用于生成指令的参数
        if let Some(best_quote_result) = global_best {
            // 过低的获利
            if best_quote_result.profit < profit_threshold {
                return None;
            }
            let mut instruction_items = Vec::with_capacity(2);
            // 用于确定每次池子的in_mint，上一个的out_mint是下一个池子的in_mint
            let mut previous_out_mint: Option<Pubkey> = None;
            for pool_id in &best_quote_result.path.path {
                let pool = pool_map.get(pool_id).unwrap();
                if previous_out_mint.is_none() {
                    previous_out_mint = Some(best_quote_result.in_mint);
                } else {
                    previous_out_mint = Some(pool.another_mint(&previous_out_mint?));
                }
                match pool.to_instruction_item(&previous_out_mint?) {
                    None => return None,
                    Some(item) => {
                        instruction_items.push(item);
                    }
                }
            }
            Some(DexQuoteResult {
                instruction_items,
                amount_in_mint: best_quote_result.in_mint,
                amount_in: best_quote_result.amount_in,
                first_amount_out: best_quote_result.first_amount_out,
                amount_out: best_quote_result.amount_out,
                profit: best_quote_result.profit,
                ..Default::default()
            })
        } else {
            None
        }
    }

    fn find_best_path_for_positive_path(
        paths: Arc<Vec<Path>>,
        pool_map: Arc<DashMap<Pubkey, Pool>>,
        start_amount_in: u64,
        amount_in_mint: Pubkey,
        clock: Arc<Clock>,
    ) -> Option<PathQuoteResult> {
        let first_pool = pool_map
            .get(paths.get(0).unwrap().path.first().unwrap())
            .unwrap();
        let another_mint = Some(first_pool.another_mint(&amount_in_mint)).unwrap();
        let first_pool_amount_out =
            first_pool.quote(start_amount_in, amount_in_mint, another_mint, clock.clone());
        if first_pool_amount_out.as_ref().is_none() {
            return None;
        }
        paths
            .par_iter()
            .filter_map(|path| {
                let second_pool = pool_map.get(path.path.last().unwrap()).unwrap();
                let second_pool_amount_out = second_pool.quote(
                    first_pool_amount_out.unwrap(),
                    another_mint,
                    amount_in_mint,
                    clock.clone(),
                );
                if second_pool_amount_out.unwrap_or(0).gt(&start_amount_in) {
                    Some((
                        path,
                        second_pool_amount_out.unwrap_or(0),
                        second_pool_amount_out.unwrap_or(0).sub(&start_amount_in),
                    ))
                } else {
                    None
                }
            })
            .max_by(|(_, _, profit_a), (_, _, profit_b)| profit_a.partial_cmp(profit_b).unwrap())
            .map(|(path, second_amount_out, profit)| PathQuoteResult {
                path: path.clone(),
                in_mint: amount_in_mint,
                amount_in: start_amount_in,
                first_amount_out: first_pool_amount_out.unwrap(),
                amount_out: second_amount_out,
                profit,
            })
    }

    fn find_best_path_for_reverse_path(
        paths: Arc<Vec<Path>>,
        pool_map: Arc<DashMap<Pubkey, Pool>>,
        start_amount_in: u64,
        amount_in_mint: Pubkey,
        clock: Arc<Clock>,
    ) -> Option<PathQuoteResult> {
        paths
            .par_iter()
            .filter_map(|path| {
                let first_pool = pool_map.get(path.path.first().unwrap())?;
                let another_mint = first_pool.another_mint(&amount_in_mint);
                let first_pool_amount_out =
                    first_pool.quote(start_amount_in, amount_in_mint, another_mint, clock.clone());
                if let Some(f_amount_out) = first_pool_amount_out {
                    let second_pool = pool_map.get(path.path.last().unwrap())?;
                    let second_pool_amount_out = second_pool.quote(
                        f_amount_out,
                        another_mint,
                        amount_in_mint,
                        clock.clone(),
                    );
                    if second_pool_amount_out.is_none() {
                        return None;
                    }
                    let second_pool_amount_out = second_pool_amount_out.unwrap();
                    if second_pool_amount_out <= start_amount_in {
                        return None;
                    }
                    Some((
                        path,
                        f_amount_out,
                        second_pool_amount_out,
                        second_pool_amount_out.sub(start_amount_in),
                    ))
                } else {
                    None
                }
            })
            .max_by(|(_, _, _, profit_a), (_, _, _, profit_b)| {
                profit_a.partial_cmp(profit_b).unwrap()
            })
            .map(
                |(path, first_amount_out, seconde_amount_out, profit)| PathQuoteResult {
                    path: path.clone(),
                    in_mint: amount_in_mint,
                    amount_in: start_amount_in,
                    first_amount_out,
                    amount_out: seconde_amount_out,
                    profit,
                },
            )
    }
}

struct PathQuoteResult {
    pub path: Path,
    pub in_mint: Pubkey,
    pub amount_in: u64,
    pub first_amount_out: u64,
    pub amount_out: u64,
    pub profit: u64,
}

#[derive(Debug, Clone, Default)]
pub struct DexQuoteResult {
    pub instruction_items: Vec<InstructionItem>,
    pub amount_in_mint: Pubkey,
    pub amount_in: u64,
    pub first_amount_out: u64,
    pub amount_out: u64,
    pub profit: u64,
    pub start_time: Option<Instant>,
    pub route_calculate_cost: Option<u128>,
    pub grop_cost: Option<u128>,
}

pub fn supported_protocols() -> Vec<DexType> {
    vec![
        DexType::RaydiumAMM,
        DexType::RaydiumCLmm,
        DexType::PumpFunAMM,
        DexType::MeteoraDLMM,
    ]
}

#[derive(Clone, Debug)]
pub struct PoolCacheHolder {
    pub pool_cache: PoolCache,
    pub path_cache: Cache<(Pubkey, Pubkey), Option<(Vec<Path>, Vec<Path>)>>,
}

impl PoolCacheHolder {
    pub async fn new(rpc_client: Arc<RpcClient>, dex_json_path: String) -> Self {
        let rpc_db = FileDB::new(rpc_client.clone(), dex_json_path);
        let pools = rpc_db.load_token_pools().await.unwrap();
        let pool_map = DashMap::with_capacity_and_shard_amount(1000, 32);
        let mut edges = HashMap::new();
        for pool in pools.into_iter() {
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
        let data = rpc_client
            .get_account_with_config(
                &Clock::id(),
                RpcAccountInfoConfig {
                    data_slice: Some(UiDataSliceConfig {
                        offset: 0,
                        length: 40,
                    }),
                    commitment: Some(CommitmentConfig::finalized()),
                    ..Default::default()
                },
            )
            .await
            .unwrap()
            .value
            .unwrap()
            .data;
        let src = array_ref![data, 0, 40];
        let (slot, epoch_start_timestamp, epoch, leader_schedule_epoch, unix_timestamp) =
            array_refs![src, 8, 8, 8, 8, 8];
        let clock = Clock {
            slot: u64::from_le_bytes(*slot),
            epoch_start_timestamp: i64::from_le_bytes(*epoch_start_timestamp),
            epoch: u64::from_le_bytes(*epoch),
            leader_schedule_epoch: u64::from_le_bytes(*leader_schedule_epoch),
            unix_timestamp: i64::from_le_bytes(*unix_timestamp),
        };
        Self {
            pool_cache: PoolCache::new(edges, pool_map, clock),
            path_cache: Cache::builder()
                .max_capacity(10000)
                .time_to_live(Duration::from_secs(60))
                .build(),
        }
    }

    pub fn build_graph(
        &self,
        amount_in_mint: &Pubkey,
        update_pool: &Pubkey,
    ) -> Option<(Vec<Path>, Vec<Path>)> {
        let start = Instant::now();
        let path_array = self
            .path_cache
            .entry((update_pool.clone(), amount_in_mint.clone()))
            .or_insert_with(|| {
                let pool_cache = self.pool_cache.clone();
                let edges = pool_cache.edges;
                let mut positive_paths = Vec::with_capacity(edges.len() / 2);
                let mut reverse_paths = Vec::with_capacity(edges.len() / 2);
                // 返回的path已经包含了update_pool作为输入和作为输出的path
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
                                if f_pool == update_pool {
                                    positive_paths.push(Path {
                                        path: vec![*f_pool, *s_pool],
                                    })
                                } else {
                                    reverse_paths.push(Path {
                                        path: vec![*f_pool, *s_pool],
                                    })
                                }
                            }
                        }
                    }
                }
                if positive_paths.is_empty() || reverse_paths.is_empty() {
                    None
                } else {
                    Some((positive_paths, reverse_paths))
                }
            })
            .value()
            .clone();
        path_array
    }

    pub fn update_cache(&self, grpc_message: GrpcMessage) -> Option<Pubkey> {
        if let GrpcMessage::Clock(clock) = grpc_message {
            self.pool_cache.clone().clock = clock;
            return None;
        }
        let pool_id = grpc_message.pool_id()?;
        self.pool_cache
            .pool_map
            .get_mut(&pool_id)
            .and_then(|mut pool| {
                pool.update_cache(grpc_message)
                    .map_or(None, |()| Some(pool_id))
            })
    }
}

#[derive(Debug, Clone)]
pub struct Path {
    pub path: Vec<Pubkey>,
}
