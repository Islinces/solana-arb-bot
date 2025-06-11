use crate::dex::{get_account_data, is_follow_vault, update_cache};
use crate::dex::{slice_data_auto_get_dex_type, SliceType};
use crate::dex::{DexType, FromCache};
use crate::grpc_subscribe::{GrpcMessage, GrpcTransactionMsg};
use ahash::RandomState;
use anyhow::anyhow;
use borsh::BorshDeserialize;
use dashmap::DashMap;
use flume::{Receiver, RecvError, TrySendError};
use futures_util::future::err;
use solana_sdk::pubkey::Pubkey;
use spl_token::solana_program::program_pack::Pack;
use spl_token::state::Account;
use std::fmt::{Debug, Formatter};
use std::ops::Sub;
use std::ptr;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::task::JoinSet;
use tokio::time::Instant;
use tracing::{error, info};
use yellowstone_grpc_proto::prelude::TokenBalance;

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
                                    _=>{}
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
        update_cache(
            account_key,
            slice_data_auto_get_dex_type(
                &account_key,
                &Pubkey::try_from(owner).map_or(Err(anyhow!("转换owner失败")), |a| Ok(a))?,
                data,
                SliceType::Subscribed,
            )?,
        )
    }
}

pub struct BalanceChangeInfo {
    pub dex_type: DexType,
    pub pool_id: Pubkey,
    pub account_index: usize,
    pub vault_account: Pubkey,
    pub change_value: f64,
}

impl BalanceChangeInfo {
    pub fn new(pre: &TokenBalance, post: &TokenBalance, account_keys: &[Pubkey]) -> Option<Self> {
        let account_index = pre.account_index as usize;
        match (pre.ui_token_amount.as_ref(), post.ui_token_amount.as_ref()) {
            (Some(pre_amount), Some(post_amount)) => {
                if pre_amount.ui_amount == post_amount.ui_amount {
                    None
                } else {
                    let vault_account = &account_keys[account_index];
                    is_follow_vault(vault_account).map_or(None, |(pool_id, dex_type)| {
                        Some(Self {
                            dex_type,
                            pool_id,
                            account_index,
                            vault_account: vault_account.clone(),
                            change_value: post_amount.ui_amount.sub(pre_amount.ui_amount),
                        })
                    })
                }
            }
            _ => None,
        }
    }
}

impl Debug for BalanceChangeInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut formatter = f.debug_struct("BalanceChangeInfo");
        formatter.field("dex_type", &self.dex_type);
        formatter.field("pool_id", &self.pool_id);
        formatter.field("vault_account", &self.vault_account.to_string());
        formatter.field("change_value", &self.change_value.to_string());
        formatter.finish()
    }
}
