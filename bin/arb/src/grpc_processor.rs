use crate::data_slice;
use crate::data_slice::SliceType;
use crate::state::{GrpcMessage, GrpcTransactionMsg};
use ahash::RandomState;
use borsh::BorshDeserialize;
use dashmap::DashMap;
use flume::Receiver;
use solana_sdk::pubkey::Pubkey;
use std::time::Duration;
use tokio::sync::{broadcast, OnceCell};
use tokio::task::JoinSet;
use tokio::time::Instant;
use tracing::error;

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
        cached_message_sender: &broadcast::Sender<(GrpcTransactionMsg, Duration, Instant)>,
    ) {
        // 初始化账户本地缓存
        self.init_account_cache().await;
        for _index in 0..self.process_size {
            let cached_message_sender = cached_message_sender.clone();
            let grpc_message_receiver = grpc_message_receiver.clone();
            join_set.spawn(async move {
                loop {
                    match grpc_message_receiver.recv_async().await {
                        Ok(grpc_message) => match grpc_message {
                            GrpcMessage::Account(account_msg) => {
                                let _ = Self::update_cache(
                                    account_msg.owner_key,
                                    account_msg.account_key,
                                    account_msg.data,
                                );
                            }
                            GrpcMessage::Transaction(transaction_msg) => {
                                let grpc_to_processor_cost =
                                    transaction_msg.instant.elapsed();
                                match cached_message_sender.send((
                                    transaction_msg,
                                    grpc_to_processor_cost,
                                    Instant::now(),
                                )) {
                                    Ok(_) => {}
                                    Err(e) => {
                                        error!("发送Transaction到Arb失败，原因：{}", e);
                                    }
                                }
                            }
                        },
                        Err(_) => {}
                    };
                }
            });
        }
    }

    fn update_cache(owner: Vec<u8>, account_key: Vec<u8>, mut data: Vec<u8>) -> (u128, u128) {
        let slice_data_instant = Instant::now();
        let sliced_data = data_slice::slice_data_auto_get_dex_type(
            &Pubkey::try_from_slice(account_key.as_slice()).unwrap(),
            &Pubkey::try_from_slice(owner.as_slice()).unwrap(),
            &mut data,
            SliceType::Subscribed,
        );
        let slice_data_cost = slice_data_instant.elapsed().as_nanos();
        let now = Instant::now();
        match sliced_data {
            Ok(sliced_data) => {
                let account_cache = ACCOUNT_CACHE.get().unwrap();
                let account_key: Pubkey = account_key.try_into().unwrap();
                account_cache.insert(account_key, sliced_data);
                (slice_data_cost, now.elapsed().as_nanos())
            }
            Err(e) => {
                error!("{}", e);
                (slice_data_cost, now.elapsed().as_nanos())
            }
        }
    }

    async fn init_account_cache(&self) {
        ACCOUNT_CACHE
            .get_or_init(|| async {
                DashMap::with_capacity_and_hasher_and_shard_amount(10_000, RandomState::new(), 128)
            })
            .await;
    }
}
