use crate::dex::{get_subscribed_accounts, grpc_subscribe};
use crate::dex_data::DexJson;
use crate::grpc_subscribe;
use ahash::AHashSet;
use anyhow::anyhow;
use base58::ToBase58;
use chrono::{DateTime, Local, NaiveDateTime, Utc};
use flume::Sender;
use solana_sdk::clock::Clock;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::SysvarId;
use spl_token::solana_program::program_pack::Pack;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio_stream::{Stream, StreamExt};
use tracing::{error, info, warn};
use yellowstone_grpc_client::GeyserGrpcClient;
use yellowstone_grpc_proto::geyser::subscribe_request_filter_accounts_filter::Filter;
use yellowstone_grpc_proto::geyser::subscribe_request_filter_accounts_filter::Filter::Datasize;
use yellowstone_grpc_proto::geyser::subscribe_update::UpdateOneof;
use yellowstone_grpc_proto::geyser::{
    subscribe_request_filter_accounts_filter_memcmp, CommitmentLevel, SubscribeRequest,
    SubscribeRequestFilterAccounts, SubscribeRequestFilterAccountsFilter,
    SubscribeRequestFilterAccountsFilterMemcmp, SubscribeRequestFilterTransactions,
    SubscribeUpdate, SubscribeUpdateAccount, SubscribeUpdateAccountInfo,
    SubscribeUpdateTransactionInfo,
};
use yellowstone_grpc_proto::prelude::{Transaction, TransactionStatusMeta};
use yellowstone_grpc_proto::prost_types::Timestamp;
use yellowstone_grpc_proto::tonic::service::Interceptor;
use yellowstone_grpc_proto::tonic::transport::ClientTlsConfig;
use yellowstone_grpc_proto::tonic::Status;

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
        let subscribed_accounts = get_subscribed_accounts();
        info!("GRPC订阅成功, 等待GRPC推送数据");
        while let Some(message) = stream.next().await {
            match message {
                Ok(data) => {
                    let created_at = data.created_at;
                    if let Some(UpdateOneof::Account(account)) = data.update_oneof {
                        match account.account {
                            Some(acc) => {
                                if subscribed_accounts
                                    .contains(&Pubkey::try_from(acc.pubkey.as_slice()).unwrap())
                                {
                                    match message_sender
                                        .send_async(GrpcMessage::Account(GrpcAccountMsg::from(acc)))
                                        .await
                                    {
                                        Ok(_) => {}
                                        Err(e) => {
                                            error!("推送GRPC Account消息失败, 原因 : {}", e);
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    } else if let Some(UpdateOneof::Transaction(transaction)) = data.update_oneof {
                        let slot = transaction.slot;
                        match transaction.transaction {
                            None => {}
                            Some(tx) => {
                                match message_sender
                                    .send_async(GrpcMessage::Transaction(GrpcTransactionMsg::from(
                                        (tx, slot, created_at.unwrap()),
                                    )))
                                    .await
                                {
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

impl From<SubscribeUpdateAccountInfo> for GrpcAccountMsg {
    fn from(account: SubscribeUpdateAccountInfo) -> Self {
        let time = Local::now();
        let tx = account.txn_signature.unwrap_or([0; 64].try_into().unwrap());
        Self {
            tx,
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
    pub created_at: Timestamp,
}

impl From<(SubscribeUpdateTransactionInfo, u64, Timestamp)> for GrpcTransactionMsg {
    fn from(transaction: (SubscribeUpdateTransactionInfo, u64, Timestamp)) -> Self {
        let time = Local::now();
        Self {
            signature: transaction.0.signature,
            transaction: transaction.0.transaction,
            meta: transaction.0.meta,
            _index: transaction.0.index,
            received_timestamp: time,
            slot: transaction.1,
            instant: Instant::now(),
            created_at: transaction.2,
        }
    }
}
