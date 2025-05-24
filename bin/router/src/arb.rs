use crate::interface::DexType;
use crate::state::{CacheValue, GrpcMessage, TxId};
use ahash::{AHashMap, AHasher, HashSet};
use base58::ToBase58;
use chrono::{DateTime, Local};
use moka::sync::{Cache, CacheBuilder};
use solana_sdk::pubkey::Pubkey;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::OnceCell;
use tokio::task::JoinSet;
use tokio::time::Instant;
use tracing::{error, info, warn};

static GLOBAL_CACHE: OnceCell<Arc<AHashMap<Pubkey, Arc<Cache<TxId, CacheValue>>>>> =
    OnceCell::const_new();

pub struct Arb {
    pub arb_size: usize,
    pub vault_to_pool: Arc<AHashMap<Pubkey, (Pubkey, Pubkey)>>,
    pub specify_pool: Arc<Option<Pubkey>>,
}

impl Arb {
    pub fn new(
        arb_size: usize,
        vault_to_pool: AHashMap<Pubkey, (Pubkey, Pubkey)>,
        specify_pool: Option<Pubkey>,
    ) -> Self {
        Self {
            arb_size,
            vault_to_pool: Arc::new(vault_to_pool),
            specify_pool: Arc::new(specify_pool),
        }
    }

    pub async fn start(
        &self,
        mut join_set: &mut JoinSet<()>,
        message_cached_sender: &Sender<(
            TxId,
            Pubkey,
            Pubkey,
            DateTime<Local>,
            DateTime<Local>,
            bool,
            u128,
        )>,
    ) {
        self.init_global_cache().await;
        let arb_size = self.arb_size as u64;
        for index in 0..arb_size {
            let vault_to_pool = self.vault_to_pool.clone();
            let specify_pool = self.specify_pool.clone();
            let mut receiver = message_cached_sender.subscribe();
            join_set.spawn(async move {
                while let data = receiver.recv().await {
                    match data {
                        Ok((
                            tx,
                            account_key,
                            owner,
                            grpc_received_timestamp,
                            update_cache_start_timestamp,
                            cache_changed,
                            update_cache_cost,
                        )) => {
                            // info!(
                            //     "arb_{index} ==> txn : {:?}, account : {:?}",
                            //     tx.0.as_slice().to_base58(),
                            //     account_key.to_string()
                            // );
                            let received_cached_message_timestamp = Local::now();
                            let (process_data_cost, txn, ready_message) =
                                Self::process_data(tx, account_key, owner, vault_to_pool.clone());
                            let grpc_to_processor_channel_cost =
                                (update_cache_start_timestamp - grpc_received_timestamp)
                                    .num_microseconds()
                                    .unwrap() as u128;
                            let update_cache_cost = update_cache_cost.div_ceil(1000);
                            let processor_to_arb_channel_cost =
                                (received_cached_message_timestamp - update_cache_start_timestamp)
                                    .num_microseconds()
                                    .unwrap() as u128;
                            if let Some((ready_cost, data)) = ready_message {
                                if specify_pool.clone().is_none_or(|v| data.0 .0.contains(&v)) {
                                    info!(
                                        "arb_{index} ==> \n日志类型: 数据Ready\n\
                                        交易 : {:?}, \
                                        总耗时 : {:?}μs\n\
                                        当前账户 : {:?}, \
                                        GRPC推送时间 : {:?}, \
                                        到达更新缓存Receiver : {:?}, \
                                        缓存是否发生变化 : {:?}\n\
                                        GRPC到更新缓存通道耗时 : {:?}μs, \
                                        更新缓存耗时 : {:?}μs, \
                                        更新缓存到Arb通道耗时 : {:?}μs, \
                                        获取Ready数据耗时 : {:?}ns, \
                                        Ready数据涉及账户列表 : {:?}",
                                        txn.0.as_slice().to_base58(),
                                        grpc_to_processor_channel_cost
                                            + update_cache_cost
                                            + processor_to_arb_channel_cost
                                            + (ready_cost.div_ceil(1000)),
                                        account_key.to_string(),
                                        grpc_received_timestamp
                                            .format("%Y-%m-%d %H:%M:%S%.9f")
                                            .to_string(),
                                        update_cache_start_timestamp
                                            .format("%Y-%m-%d %H:%M:%S%.9f")
                                            .to_string(),
                                        cache_changed,
                                        grpc_to_processor_channel_cost,
                                        update_cache_cost,
                                        processor_to_arb_channel_cost,
                                        ready_cost,
                                        data.0
                                             .0
                                            .into_iter()
                                            .map(|v| v.to_string())
                                            .collect::<Vec<_>>(),
                                    )
                                }
                            } else {
                                if specify_pool.clone().is_none_or(|v| account_key == v) {
                                    info!(
                                        "arb_{index} ==> \n日志类型 : 数据未Ready\n\
                                        交易 : {:?}, \
                                        总耗时 : {:?}μs\n\
                                        当前账户 : {:?}, \
                                        GRPC推送时间 : {:?}, \
                                        到达更新缓存Receiver : {:?}, \
                                        缓存是否发生变化 : {:?}\n\
                                        GRPC到更新缓存通道耗时 : {:?}μs, \
                                        更新缓存耗时 : {:?}μs, \
                                        更新缓存到Arb通道耗时 : {:?}μs\
                                        处理Ready数据耗时(单条) : {:?}ns",
                                        txn.0.as_slice().to_base58(),
                                        grpc_to_processor_channel_cost
                                            + update_cache_cost
                                            + processor_to_arb_channel_cost
                                            + (process_data_cost.div_ceil(1000)),
                                        account_key.to_string(),
                                        grpc_received_timestamp
                                            .format("%Y-%m-%d %H:%M:%S%.9f")
                                            .to_string(),
                                        update_cache_start_timestamp
                                            .format("%Y-%m-%d %H:%M:%S%.9f")
                                            .to_string(),
                                        cache_changed,
                                        grpc_to_processor_channel_cost,
                                        update_cache_cost,
                                        processor_to_arb_channel_cost,
                                        process_data_cost,
                                    )
                                }
                            }
                        }
                        Err(RecvError::Closed) => {
                            error!("action channel closed!");
                            break;
                        }
                        Err(RecvError::Lagged(num)) => {
                            warn!("action channel lagged by {num}")
                        }
                    }
                }
            });
        }
    }

    async fn init_global_cache(&self) {
        let vault_to_pool = self.vault_to_pool.clone();
        GLOBAL_CACHE
            .get_or_init(|| async {
                let pool_ids = vault_to_pool
                    .iter()
                    .map(|(_, (pool_id, _))| pool_id.clone())
                    .collect::<HashSet<_>>();
                let mut hash_map = AHashMap::with_capacity(1000);
                for pool_id in pool_ids {
                    hash_map.insert(pool_id, Arc::new(CacheBuilder::new(10_000).build()));
                }
                Arc::new(hash_map)
            })
            .await;
    }

    fn get_pool_cache(pool_id: &Pubkey) -> Arc<Cache<TxId, CacheValue>> {
        GLOBAL_CACHE.get().unwrap().get(pool_id).unwrap().clone()
    }

    fn process_data(
        txn: TxId,
        account: Pubkey,
        owner: Pubkey,
        vault_to_pool: Arc<AHashMap<Pubkey, (Pubkey, Pubkey)>>,
    ) -> (u128, TxId, Option<(u128, CacheValue)>) {
        let now = Instant::now();
        let raydium_program_id = DexType::RaydiumAMM.get_ref_program_id();
        let pumpfun_program_id = DexType::PumpFunAMM.get_ref_program_id();
        let (pool_id, _program_id, wait_account_len) = if &owner == raydium_program_id {
            (&account, raydium_program_id, 3)
        } else if &owner == pumpfun_program_id {
            (&account, pumpfun_program_id, 2)
        } else if let Some((pid, prog_id)) = vault_to_pool.get(&account) {
            if raydium_program_id == prog_id {
                (pid, prog_id, 3)
            } else if pumpfun_program_id == prog_id {
                (pid, prog_id, 2)
            } else {
                (pid, prog_id, 0)
            }
        } else {
            return (now.elapsed().as_nanos(), txn, None);
        };
        // info!(
        //     "tx : {:?}, account : {:?}, pool_id : {:?}, program_id : {:?}, timestamp : {:?}",
        //     txn.0.as_slice().to_base58(),
        //     account,
        //     pool_id,
        //     program_id,
        //     grpc_message.received_timestamp
        // );
        let pool_cache = Self::get_pool_cache(&pool_id);
        let entry = pool_cache
            .entry_by_ref(&txn)
            .and_upsert_with(|maybe_entry| {
                if let Some(entry) = maybe_entry {
                    let mut cache_value = entry.into_value();
                    cache_value.insert(account);
                    cache_value
                } else {
                    CacheValue::new(account, now)
                }
            });
        if entry.is_old_value_replaced() {
            if !entry
                .value()
                .is_ready(|size| size == (wait_account_len as usize))
            {
                return (now.elapsed().as_nanos(), txn, None);
            }
            let value = entry.into_value();
            pool_cache.invalidate(&txn);
            (
                now.elapsed().as_micros(),
                txn,
                Some((value.0 .1.elapsed().as_micros(), value)),
            )
        } else {
            (now.elapsed().as_nanos(), txn, None)
        }
    }
}
