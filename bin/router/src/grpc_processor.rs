use crate::interface::DexType;
use crate::state::{CacheValue, GrpcMessage, GrpcTransactionMsg, TxId};
use ahash::{AHashMap, AHasher, HashSet, RandomState};
use base58::ToBase58;
use chrono::{DateTime, Local};
use dashmap::DashMap;
use flume::{Receiver, RecvError};
use solana_sdk::pubkey::Pubkey;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tokio::sync::broadcast::error::SendError;
use tokio::sync::{broadcast, OnceCell};
use tokio::task::{JoinHandle, JoinSet};
use tokio::time::Instant;
use tracing::{error, info};

static ACCOUNT_CACHE: OnceCell<DashMap<Pubkey, Vec<u8>, RandomState>> = OnceCell::const_new();

pub struct MessageProcessor {
    pub process_size: usize,
}

impl MessageProcessor {
    pub fn new(process_size: usize) -> Self {
        Self { process_size }
    }

    pub async fn start(
        &mut self,
        join_set: &mut JoinSet<()>,
        grpc_message_receiver: &Receiver<GrpcMessage>,
        cached_message_sender: &broadcast::Sender<(GrpcTransactionMsg, DateTime<Local>)>,
    ) {
        // 初始化账户本地缓存
        self.init_account_cache().await;
        for _index in 0..self.process_size {
            let cached_message_sender = cached_message_sender.clone();
            let grpc_message_receiver = grpc_message_receiver.clone();
            join_set.spawn(async move {
                loop {
                    match grpc_message_receiver.recv_async().await {
                        Ok(grpc_message) => {
                            let incoming_processor_timestamp = Local::now();
                            match grpc_message {
                                GrpcMessage::Account(account_msg) => {
                                    let _ = Self::update_cache(
                                        account_msg.account_key,
                                        account_msg.data,
                                    );
                                    // let grpc_to_processor_channel_cost =
                                    //     (incoming_processor_timestamp
                                    //         - account_msg.received_timestamp)
                                    //         .num_microseconds()
                                    //         .unwrap()
                                    //         as u128;
                                    // info!(
                                    //     "Processor_{index} ==> \n日志类型: Account, 总耗时 : {:?}μs, 交易 : {:?}\n\
                                    //     当前账户 : {:?}, \
                                    //     GRPC推送时间 : {:?}, \
                                    //     缓存是否发生变化 : {:?}\n\
                                    //     GRPC到更新缓存通道耗时 : {:?}μs, \
                                    //     更新缓存耗时 : {:?}ns",
                                    //     grpc_to_processor_channel_cost
                                    //         + (update_cache_cost.div_ceil(1000)),
                                    //     account_msg.tx.as_slice().to_base58(),
                                    //     account_key.to_string(),
                                    //     account_msg
                                    //         .received_timestamp
                                    //         .format("%Y-%m-%d %H:%M:%S%.9f")
                                    //         .to_string(),
                                    //     changed,
                                    //     grpc_to_processor_channel_cost,
                                    //     update_cache_cost,
                                    // );
                                }
                                GrpcMessage::Transaction(transaction_msg) => {
                                    match cached_message_sender
                                        .send((transaction_msg, incoming_processor_timestamp))
                                    {
                                        Ok(_) => {}
                                        Err(e) => {
                                            error!("发送Transaction到Arb失败，原因：{}", e);
                                        }
                                    }
                                }
                            }
                        }
                        Err(_) => {}
                    };
                }
            });
        }
    }

    pub fn update_cache(account_key: Vec<u8>, data: Vec<u8>) -> u128 {
        let now = Instant::now();
        let account_cache = ACCOUNT_CACHE.get().unwrap();
        let account_key: Pubkey = account_key.try_into().unwrap();
        account_cache.insert(account_key, data);
        now.elapsed().as_nanos()
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
