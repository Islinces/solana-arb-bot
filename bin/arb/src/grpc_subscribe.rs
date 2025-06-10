use crate::dex_data::DexJson;
use crate::interface::AccountSubscriber;
use crate::interface1::DexType;
use crate::state::{GrpcAccountMsg, GrpcMessage, GrpcTransactionMsg};
use ahash::AHashSet;
use anyhow::anyhow;
use chrono::Local;
use flume::Sender;
use solana_sdk::clock::Clock;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::SysvarId;
use std::collections::HashMap;
use std::time::Duration;
use tokio_stream::{Stream, StreamExt};
use tracing::{error, info};
use yellowstone_grpc_client::GeyserGrpcClient;
use yellowstone_grpc_proto::geyser::subscribe_request_filter_accounts_filter::Filter;
use yellowstone_grpc_proto::geyser::subscribe_request_filter_accounts_filter::Filter::Datasize;
use yellowstone_grpc_proto::geyser::subscribe_update::UpdateOneof;
use yellowstone_grpc_proto::geyser::{
    subscribe_request_filter_accounts_filter_memcmp, CommitmentLevel, SubscribeRequest,
    SubscribeRequestFilterAccounts, SubscribeRequestFilterAccountsFilter,
    SubscribeRequestFilterAccountsFilterMemcmp, SubscribeRequestFilterTransactions,
    SubscribeUpdate,
};
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
