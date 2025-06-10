use crate::dex_data::DexJson;
use crate::interface::AccountSubscriber;
use crate::interface1::DexType;
use ahash::AHashSet;
use anyhow::anyhow;
use chrono::{DateTime, Local};
use flume::Sender;
use solana_sdk::clock::Clock;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::SysvarId;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio_stream::{Stream, StreamExt};
use tracing::{error, info};
use yellowstone_grpc_client::GeyserGrpcClient;
use yellowstone_grpc_proto::geyser::subscribe_request_filter_accounts_filter::Filter;
use yellowstone_grpc_proto::geyser::subscribe_request_filter_accounts_filter::Filter::Datasize;
use yellowstone_grpc_proto::geyser::subscribe_update::UpdateOneof;
use yellowstone_grpc_proto::geyser::{subscribe_request_filter_accounts_filter_memcmp, CommitmentLevel, SubscribeRequest, SubscribeRequestFilterAccounts, SubscribeRequestFilterAccountsFilter, SubscribeRequestFilterAccountsFilterMemcmp, SubscribeRequestFilterTransactions, SubscribeUpdate, SubscribeUpdateAccount, SubscribeUpdateTransactionInfo};
use yellowstone_grpc_proto::prelude::{Transaction, TransactionStatusMeta};
use yellowstone_grpc_proto::tonic::service::Interceptor;
use yellowstone_grpc_proto::tonic::transport::ClientTlsConfig;
use yellowstone_grpc_proto::tonic::Status;
use crate::grpc_subscribe;

pub struct GrpcSubscribe;

pub const POOL_TICK_ARRAY_BITMAP_SEED: &str = "pool_tick_array_bitmap_extension";

impl GrpcSubscribe {
    pub async fn subscribe(
        &self,
        grpc_url: String,
        dex_data: Vec<DexJson>,
        message_sender: Sender<GrpcMessage>,
    ) {
        let mut stream = grpc_subscribe(grpc_url, dex_data).await.unwrap();
        info!("GRPC订阅成功, 等待GRPC推送数据");
        while let Some(message) = stream.next().await {
            match message {
                Ok(data) => {
                    if let Some(UpdateOneof::Account(account)) = data.update_oneof {
                        match message_sender
                            .send(GrpcMessage::Account(GrpcAccountMsg::from(account)))
                        {
                            Ok(_) => {}
                            Err(e) => {
                                error!("推送GRPC Account消息失败, 原因 : {}", e);
                            }
                        }
                    } else if let Some(UpdateOneof::Transaction(transaction)) = data.update_oneof {
                        let slot = transaction.slot;
                        match transaction.transaction {
                            None => {}
                            Some(tx) => {
                                match message_sender.send(GrpcMessage::Transaction(
                                    GrpcTransactionMsg::from((tx, slot)),
                                )) {
                                    Ok(_) => {}
                                    Err(e) => {
                                        error!("推送GRPC Transaction消息失败, 原因 : {}", e);
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("接收grpc推送消息失败，原因：{}", e);
                    break;
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum GrpcMessage {
    Account(GrpcAccountMsg),
    Transaction(GrpcTransactionMsg),
}

#[derive(Debug, Clone)]
pub struct GrpcAccountMsg {
    pub tx: Vec<u8>,
    pub account_key: Vec<u8>,
    pub owner_key: Vec<u8>,
    pub data: Vec<u8>,
    pub write_version: u64,
    pub received_timestamp: DateTime<Local>,
}

impl From<SubscribeUpdateAccount> for GrpcAccountMsg {
    fn from(subscribe_update_account: SubscribeUpdateAccount) -> Self {
        let time = Local::now();
        let account = subscribe_update_account.account.unwrap();
        Self {
            tx: account.txn_signature.unwrap_or([0; 64].try_into().unwrap()),
            account_key: account.pubkey,
            owner_key: account.owner,
            data: account.data,
            write_version: account.write_version,
            received_timestamp: time,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GrpcTransactionMsg {
    pub signature: Vec<u8>,
    pub transaction: Option<Transaction>,
    pub meta: Option<TransactionStatusMeta>,
    pub _index: u64,
    pub received_timestamp: DateTime<Local>,
    pub slot: u64,
    pub instant: Instant,
}

impl From<(SubscribeUpdateTransactionInfo, u64)> for GrpcTransactionMsg {
    fn from(transaction: (SubscribeUpdateTransactionInfo, u64)) -> Self {
        let time = Local::now();
        Self {
            signature: transaction.0.signature,
            transaction: transaction.0.transaction,
            meta: transaction.0.meta,
            _index: transaction.0.index,
            received_timestamp: time,
            slot: transaction.1,
            instant: Instant::now(),
        }
    }
}
