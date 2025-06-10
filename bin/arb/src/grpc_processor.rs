use crate::global_cache::get_account_data;
use crate::data_slice;
use crate::data_slice::SliceType;
use crate::dex::byte_utils::read_u64;
use crate::dex::pump_fun::state::Pool;
use crate::dex::raydium_amm::state::AmmInfo;
use crate::dex::raydium_clmm::state::{PoolState, TickArrayBitmapExtension, TickArrayState};
use crate::dex::FromCache;
use crate::interface1::{AccountType, DexType};
use crate::state::{GrpcMessage, GrpcTransactionMsg};
use ahash::RandomState;
use anyhow::anyhow;
use borsh::BorshDeserialize;
use dashmap::DashMap;
use flume::{Receiver, RecvError, TrySendError};
use futures_util::future::err;
use solana_sdk::pubkey::Pubkey;
use spl_token::solana_program::program_pack::Pack;
use spl_token::state::Account;
use std::ptr;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::task::JoinSet;
use tokio::time::Instant;
use tracing::{error, info};

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
        cached_message_sender: flume::Sender<GrpcTransactionMsg>,
        cached_message_receiver: Receiver<GrpcTransactionMsg>,
    ) {
        for index in 0..self.process_size {
            let cached_message_sender = cached_message_sender.clone();
            let cached_msg_drop_receiver = cached_message_receiver.clone();
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
                                match cached_message_sender.try_send(transaction_msg) {
                                    Ok(_) => {}
                                    Err(TrySendError::Full(msg)) => {
                                        cached_msg_drop_receiver.try_recv().ok();
                                        info!("Processor_{index} Channel丢弃消息");
                                        let mut retry_count = 3;
                                        loop {
                                            if retry_count != 0 {
                                                match cached_message_sender.try_send(msg.clone()) {
                                                    Err(TrySendError::Full(_)) => {
                                                        cached_msg_drop_receiver.try_recv().ok();
                                                        retry_count -= 1;
                                                    }
                                                    _ => {
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Err(TrySendError::Disconnected(_)) => {
                                        error!("Processor_{index} 发送消息到Arb失败，原因：所有Arb关闭");
                                        break;
                                    }
                                }
                            }
                        },
                        Err(RecvError::Disconnected) => {
                            error!("Processor_{index} 接收消息失败，原因：Grpc订阅线程关闭");
                            break;
                        }
                    };
                }
            });
        }
    }

    fn update_cache(owner: Vec<u8>, account_key: Vec<u8>, data: Vec<u8>) -> anyhow::Result<()> {
        let account_key = Pubkey::try_from(account_key)
            .map_or(Err(anyhow!("转换account_key失败")), |a| Ok(a))?;
        let sliced_data = data_slice::slice_data_auto_get_dex_type(
            &account_key,
            &Pubkey::try_from(owner).map_or(Err(anyhow!("转换owner失败")), |a| Ok(a))?,
            data,
            SliceType::Subscribed,
        );
        match sliced_data {
            Ok(sliced_data) => crate::global_cache::update_cache(account_key, sliced_data),
            Err(e) => Err(anyhow!("账户数据切片失败，原因：{}", e)),
        }
    }
}
