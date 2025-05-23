use crate::dex_data::DexJson;
use crate::interface::DexType;
use crate::state::{CacheValue, GrpcMessage, TxId};
use ahash::{AHashMap, AHashSet, AHasher, HashSet, RandomState};
use base58::ToBase58;
use chrono::{DateTime, Local};
use dashmap::DashMap;
use flume::Receiver;
use futures_util::future;
use futures_util::future::Lazy;
use moka::sync::{Cache, CacheBuilder};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::ops::Deref;
use std::ptr::read;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::{OnceCell, RwLock};
use tokio::time::Instant;
use tracing::{info, warn};

static GLOBAL_CACHE: OnceCell<Arc<AHashMap<Pubkey, Arc<Cache<TxId, CacheValue>>>>> =
    OnceCell::const_new();
static ACCOUNT_CACHE: OnceCell<DashMap<Pubkey, Vec<u8>, RandomState>> = OnceCell::const_new();

pub struct MessageProcessor {
    pub vault_to_pool: Arc<AHashMap<Pubkey, (Pubkey, Pubkey)>>,
    pub specify_pool: Arc<Option<Pubkey>>,
    pub index: usize,
}

impl MessageProcessor {}

impl MessageProcessor {
    pub async fn start(&mut self, mut message_receiver: Receiver<GrpcMessage>) {
        // 初始化等待grpc消息本地缓存
        self.init_global_cache().await;
        // 初始化账户本地缓存
        self.init_account_cache().await;
        let specify_pool = self.specify_pool.clone();
        let vault_to_pool = self.vault_to_pool.clone();
        let index = self.index;
        tokio::spawn(async move {
            while let Ok(grpc_message) = message_receiver.recv() {
                // 获取可以更新缓存的数据
                let ready_data =
                    Self::process_data(grpc_message, specify_pool.clone(), vault_to_pool.clone())
                        .await;
                if let Some((tx, all_msg_cost, last_msg_cost, data)) = ready_data {
                    let now = Local::now();
                    let (should_trigger_swap, update_cache_cost) = Self::update_cache(data);
                    info!(
                        "processor_{}\n
                        tx : {:?}\n
                        等待所有数据耗时 : {}µs\n
                        最后一条数据耗时 : {}µs\n
                        更新缓存时间 : {:?}\n
                        更新缓存耗时 : {:?}ns\n
                        缓存是否发生变化 : {:?}",
                        index,
                        tx.0.as_slice().to_base58(),
                        all_msg_cost,
                        last_msg_cost,
                        now.format("%Y-%m-%d %H:%M:%S%.9f").to_string(),
                        update_cache_cost,
                        should_trigger_swap,
                    );
                }
            }
        });
    }

    pub fn update_cache(data: CacheValue) -> (bool, u128) {
        let now = Instant::now();
        let account_cache = ACCOUNT_CACHE.get().unwrap();
        let mut changed = false;
        for (account_key, (data, _timestamp)) in data.0 .0 {
            let un_same = if let Some(exists) = account_cache.get(&account_key) {
                let same = exists.value().eq(&data);
                // 释放读锁
                drop(exists);
                !same
            } else {
                true
            };
            account_cache.insert(account_key, data);
            changed |= un_same;
        }
        (changed, now.elapsed().as_nanos())
    }

    pub fn new(
        vault_to_pool: Arc<AHashMap<Pubkey, (Pubkey, Pubkey)>>,
        specify_pool: Arc<Option<Pubkey>>,
        index: usize,
    ) -> Self {
        Self {
            vault_to_pool,
            specify_pool,
            index,
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

    async fn init_account_cache(&self) {
        // TODO : 初始化缓存
        ACCOUNT_CACHE
            .get_or_init(|| async {
                DashMap::with_capacity_and_hasher_and_shard_amount(10_000, RandomState::new(), 128)
            })
            .await;
    }

    fn get_pool_cache(pool_id: &Pubkey) -> Arc<Cache<TxId, CacheValue>> {
        GLOBAL_CACHE.get().unwrap().get(pool_id).unwrap().clone()
    }

    async fn process_data(
        grpc_message: GrpcMessage,
        _specify_pool: Arc<Option<Pubkey>>,
        vault_to_pool: Arc<AHashMap<Pubkey, (Pubkey, Pubkey)>>,
    ) -> Option<(TxId, u128, u128, CacheValue)> {
        let now = Instant::now();
        let txn = TxId::from(grpc_message.tx);
        let account: Pubkey = grpc_message.account_key.try_into().unwrap();
        let owner: Pubkey = grpc_message.owner_key.try_into().unwrap();
        let data = grpc_message.data;
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
            return None;
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
                    cache_value.insert(account, data, grpc_message.received_timestamp);
                    cache_value
                } else {
                    CacheValue::new(account, data, grpc_message.received_timestamp, now)
                }
            });
        if entry.is_old_value_replaced() {
            if !entry
                .value()
                .is_ready(|size| size == (wait_account_len as usize))
            {
                return None;
            }
            let value = entry.into_value();
            Some((txn, value.0 .1.elapsed().as_micros(), now.elapsed().as_micros(), value))
        } else {
            None
        }
    }
}
