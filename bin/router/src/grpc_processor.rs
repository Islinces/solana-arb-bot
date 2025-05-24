use crate::interface::DexType;
use crate::state::{CacheValue, GrpcMessage, TxId};
use ahash::{AHashMap, AHasher, HashSet, RandomState};
use chrono::{DateTime, Local};
use dashmap::DashMap;
use flume::{Receiver, RecvError};
use moka::sync::{Cache, CacheBuilder};
use solana_sdk::pubkey::Pubkey;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tokio::sync::broadcast::error::SendError;
use tokio::sync::{broadcast, OnceCell};
use tokio::task::{JoinHandle, JoinSet};
use tokio::time::Instant;
use tracing::{error, info};

static ACCOUNT_CACHE: OnceCell<DashMap<Pubkey, (Vec<u8>, u64), RandomState>> =
    OnceCell::const_new();

pub struct MessageProcessor {
    pub process_size: usize,
}

impl MessageProcessor {
    pub fn new(process_size: usize) -> Self {
        Self { process_size }
    }

    pub async fn start(
        &mut self,
        mut join_set: &mut JoinSet<()>,
        mut grpc_message_receiver: &Receiver<GrpcMessage>,
        cached_message_sender: &broadcast::Sender<(
            TxId,
            Pubkey,
            Pubkey,
            DateTime<Local>,
            DateTime<Local>,
            bool,
            u128,
        )>,
    ) {
        // 初始化账户本地缓存
        self.init_account_cache().await;
        for _ in 0..self.process_size {
            let cached_message_sender = cached_message_sender.clone();
            let grpc_message_receiver = grpc_message_receiver.clone();
            join_set.spawn(async move {
                loop {
                    match grpc_message_receiver.recv_async().await {
                        Ok(grpc_message) => {
                            let update_cache_start_timestamp = Local::now();
                            let (changed, account_key, update_cache_cost) =
                                Self::update_cache(grpc_message.account_key, grpc_message.data);
                            match cached_message_sender.send((
                                TxId::from(grpc_message.tx),
                                account_key,
                                grpc_message.owner_key.try_into().unwrap(),
                                grpc_message.received_timestamp,
                                update_cache_start_timestamp,
                                changed,
                                update_cache_cost,
                            )) {
                                Ok(_) => {}
                                Err(e) => {
                                    error!("发送缓存更新完成消息失败，原因：{}", e);
                                }
                            }
                        }
                        Err(_) => {}
                    };
                }
            });
        }
    }

    pub fn update_cache(account_key: Vec<u8>, data: Vec<u8>) -> (bool, Pubkey, u128) {
        let now = Instant::now();
        let current_hash = {
            let mut hasher = AHasher::default();
            data.as_slice().hash(&mut hasher);
            hasher.finish()
        };
        let account_cache = ACCOUNT_CACHE.get().unwrap();
        let account_key: Pubkey = account_key.try_into().unwrap();
        match account_cache.insert(account_key, (data, current_hash)) {
            None => (true, account_key, now.elapsed().as_nanos()),
            Some((_, previous_hash)) => (
                current_hash == previous_hash,
                account_key,
                now.elapsed().as_nanos(),
            ),
        }
    }

    async fn init_account_cache(&self) {
        // TODO : 初始化缓存
        ACCOUNT_CACHE
            .get_or_init(|| async {
                DashMap::with_capacity_and_hasher_and_shard_amount(10_000, RandomState::new(), 128)
            })
            .await;
    }
}
