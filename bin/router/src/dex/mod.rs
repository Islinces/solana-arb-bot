use crate::cache::{Pool, PoolCache};
use crate::file_db::FileDB;
use crate::interface::{DexType, GrpcMessage, InstructionItem, DB};
use anyhow::anyhow;
use dashmap::{DashMap, Entry};
use num_traits::Pow;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::clock::Clock;
use solana_program::pubkey::Pubkey;
use solana_program::sysvar::SysvarId;
use std::collections::{HashMap, HashSet};
use std::ops::{Add, Mul, Sub};
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

static POOL_CACHE_HOLDER: OnceCell<Arc<PoolCacheHolder>> = OnceCell::const_new();

#[derive(Clone, Default)]
pub struct DexData {
    pool_cache_holder: Arc<PoolCacheHolder>,
    sol_ata_amount: Arc<u64>,
    max_amount_in_numerator: u8,
    max_amount_in_denominator: u8,
}

impl DexData {
    pub async fn new_only_cache_holder(rpc_client: Arc<RpcClient>) -> anyhow::Result<Self> {
        // 单例
        let pool_cache_holder = POOL_CACHE_HOLDER
            .get_or_init(|| async { Arc::new(PoolCacheHolder::new(rpc_client).await) })
            .await
            .clone();
        Ok(Self {
            pool_cache_holder,
            ..Self::default()
        })
    }

    pub async fn new(
        rpc_client: Arc<RpcClient>,
        sol_ata_amount: Arc<u64>,
        max_amount_in_numerator: u8,
        max_amount_in_denominator: u8,
    ) -> anyhow::Result<Self> {
        // 单例
        let pool_cache_holder = POOL_CACHE_HOLDER
            .get_or_init(|| async { Arc::new(PoolCacheHolder::new(rpc_client).await) })
            .await
            .clone();
        Ok(Self {
            pool_cache_holder,
            sol_ata_amount,
            max_amount_in_numerator,
            max_amount_in_denominator,
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
    ) -> Option<DexQuoteResult> {
        let indexer = self.pool_cache_holder.clone();
        //TODO：不一定是以修改的池子作为开始
        if let Some(update_pool) = indexer.update_cache(grpc_message) {
            //TODO:支持配置
            let amount_in_mint =
                Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
            if let Some(graph) = indexer.build_graph(&amount_in_mint, &update_pool).await {
                self.find_best_route(amount_in_mint, Arc::new(graph), profit_threshold)
            } else {
                None
            }
        } else {
            None
        }
    }

    fn find_best_route(
        &self,
        amount_in_mint: Pubkey,
        paths: Arc<Vec<Path>>,
        profit_threshold: u64,
    ) -> Option<DexQuoteResult> {
        let pool_cache = self.pool_cache_holder.pool_cache.clone();
        let pool_map = pool_cache.pool_map;
        let clock = Arc::new(pool_cache.clock);
        let mut start_amount_in = 10_000_000_u64;
        let mut global_best: Option<PathQuoteResult> = None;
        // 每次循环递增 0.5 WSOL
        let increase_step = 0.5_f64.mul(10.0.pow(9)) as u64;
        for _ in 1..=240 {
            start_amount_in = start_amount_in
                .add(increase_step)
                .checked_mul(self.max_amount_in_numerator as u64)
                .unwrap()
                .checked_div(self.max_amount_in_denominator as u64)?;
            // 不能大于WSOL的90%
            if start_amount_in > *self.sol_ata_amount {
                break;
            }
            let paths = paths.clone();
            let pool_map = pool_map.clone();
            let clock = clock.clone();
            let local_best = DexData::find_best_path_for_single_direction_path(
                paths,
                pool_map,
                start_amount_in,
                amount_in_mint,
                clock,
            );
            if let Some(local_best) = local_best {
                if global_best.is_none() || global_best.as_ref().unwrap().profit < local_best.profit
                {
                    global_best = Some(local_best);
                }
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
                amount_out: best_quote_result.amount_out,
                profit: best_quote_result.profit,
            })
        } else {
            None
        }
    }

    fn find_best_path_for_single_direction_path(
        paths: Arc<Vec<Path>>,
        pool_map: Arc<DashMap<Pubkey, Pool>>,
        start_amount_in: u64,
        amount_in_mint: Pubkey,
        clock: Arc<Clock>,
    ) -> Option<PathQuoteResult> {
        let mut local_best: Option<(&Path, &Pubkey, u64, u64, u64)> = None;
        for path in paths.iter() {
            let clock = clock.clone();
            let first_pool = pool_map.get(path.path.first().unwrap())?;
            let first_pool_amount_out = first_pool.quote(
                start_amount_in,
                amount_in_mint,
                first_pool.another_mint(&amount_in_mint),
                clock.clone(),
            );
            if let Some(f_amount_out) = first_pool_amount_out {
                let second_pool = pool_map.get(path.path.last().unwrap())?;
                let second_pool_amount_out = second_pool.quote(
                    f_amount_out,
                    first_pool.another_mint(&amount_in_mint),
                    amount_in_mint,
                    clock,
                );
                if second_pool_amount_out.is_none() {
                    continue;
                }
                let second_pool_amount_out = second_pool_amount_out.unwrap();
                if second_pool_amount_out < start_amount_in {
                    continue;
                }
                if local_best.is_none() || second_pool_amount_out > local_best.as_ref().unwrap().3 {
                    local_best = Some((
                        path,
                        &amount_in_mint,
                        start_amount_in,
                        second_pool_amount_out,
                        second_pool_amount_out.sub(start_amount_in),
                    ))
                }
            }
        }
        if let Some((path, in_mint, amount_in, amount_out, profit)) = local_best {
            Some(PathQuoteResult {
                path: path.clone(),
                in_mint: *in_mint,
                amount_in,
                amount_out,
                profit,
            })
        } else {
            None
        }
    }
}

struct PathQuoteResult {
    pub path: Path,
    pub in_mint: Pubkey,
    pub amount_in: u64,
    pub amount_out: u64,
    pub profit: u64,
}

#[derive(Debug, Clone)]
pub struct DexQuoteResult {
    pub instruction_items: Vec<InstructionItem>,
    pub amount_in_mint: Pubkey,
    pub amount_in: u64,
    pub amount_out: u64,
    pub profit: u64,
}

pub fn supported_protocols() -> Vec<DexType> {
    vec![
        DexType::RaydiumAMM,
        DexType::RaydiumCLmm,
        DexType::PumpFunAMM,
        DexType::MeteoraDLMM,
    ]
}

#[derive(Clone, Default)]
pub struct PoolCacheHolder {
    pool_cache: PoolCache,
}

impl PoolCacheHolder {
    pub async fn new(rpc_client: Arc<RpcClient>) -> Self {
        let rpc_db = FileDB::new(rpc_client.clone());
        let pools = rpc_db.load_token_pools().await.unwrap();
        let pool_map = DashMap::new();
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
        let clock: Clock = bincode::deserialize(
            rpc_client
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

    pub async fn build_graph(
        &self,
        amount_in_mint: &Pubkey,
        update_pool: &Pubkey,
    ) -> Option<Vec<Path>> {
        let pool_cache = self.pool_cache.clone();
        let edges = pool_cache.edges;
        let mut paths = Vec::new();
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
                        paths.push(Path {
                            path: vec![*f_pool, *s_pool],
                        })
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
            // info!("更新缓存: 成功 {:?}", pool_id);
            Some(successful.unwrap())
        } else {
            info!("更新缓存: 未发生变化{:?}", pool_id);
            None
        }
    }
}

#[derive(Clone)]
pub struct Path {
    path: Vec<Pubkey>,
}

// impl Clone for Path {
//     fn clone(&self) -> Self {
//         Self {
//             path: self.path.iter().map(|path| path.clone_self()).collect(),
//         }
//     }
// }
