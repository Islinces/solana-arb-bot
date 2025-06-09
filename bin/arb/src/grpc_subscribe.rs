use crate::dex_data::DexJson;
use crate::interface::AccountSubscriber;
use crate::interface1::DexType;
use crate::state::{GrpcAccountMsg, GrpcMessage, GrpcTransactionMsg};
use crate::{Subscriber, SubscriptionAccounts};
use ahash::AHashSet;
use anyhow::anyhow;
use base58::ToBase58;
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

#[derive(Debug)]
pub struct GrpcSubscribe {
    pub grpc_url: String,
    pub standard_program: bool,
}

pub const POOL_TICK_ARRAY_BITMAP_SEED: &str = "pool_tick_array_bitmap_extension";

impl GrpcSubscribe {
    pub async fn subscribe(&self, dex_data: Vec<DexJson>, message_sender: Sender<GrpcMessage>) {
        let grpc_url = self.grpc_url.clone();
        let mut stream = Self::single_subscribe_grpc(grpc_url, dex_data)
            .await
            .unwrap();
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

    async fn single_subscribe_grpc(
        grpc_url: String,
        dex_data: Vec<DexJson>,
    ) -> anyhow::Result<impl Stream<Item = Result<SubscribeUpdate, Status>>> {
        let subscribers =
            crate::interface::get_subscribers().map_or(Err(anyhow!("")), |s| Ok(s))?;
        let mut unified_accounts: AHashSet<Pubkey> = AHashSet::with_capacity(dex_data.len() * 3);
        let mut account_with_owner_and_filter: HashMap<String, SubscribeRequestFilterAccounts> =
            HashMap::with_capacity(dex_data.len());
        let mut tx_include_accounts: Vec<Pubkey> = Vec::with_capacity(dex_data.len() * 3);
        for sub in subscribers {
            match sub.get_subscription_accounts(dex_data.as_slice()) {
                None => {}
                Some(accounts) => {
                    if accounts.unified_accounts.is_empty()
                        && accounts.tx_include_accounts.is_empty()
                        && accounts.account_with_owner_and_filter.as_ref().is_none()
                    {
                        continue;
                    }
                    unified_accounts.extend(accounts.unified_accounts);
                    tx_include_accounts.extend(accounts.tx_include_accounts);
                    accounts
                        .account_with_owner_and_filter
                        .unwrap_or(HashMap::new())
                        .into_iter()
                        .for_each(|(k, v)| {
                            account_with_owner_and_filter.insert(k, v);
                        });
                }
            }
        }
        if unified_accounts.is_empty() {
            return Err(anyhow!("没有订阅账户"));
        }
        let mut accounts = HashMap::with_capacity(dex_data.len() * 3);
        accounts.insert(
            "unified_accounts".to_string(),
            SubscribeRequestFilterAccounts {
                account: unified_accounts
                    .into_iter()
                    .map(|k| k.to_string())
                    .collect::<Vec<_>>(),
                ..Default::default()
            },
        );
        for (k, v) in account_with_owner_and_filter {
            accounts.insert(k, v);
        }
        if tx_include_accounts.is_empty() {
            return Err(anyhow!("未订阅tx"));
        }
        let mut transactions = HashMap::new();
        transactions.insert(
            "transactions".to_string(),
            SubscribeRequestFilterTransactions {
                vote: Some(false),
                failed: Some(false),
                account_include: tx_include_accounts
                    .into_iter()
                    .map(|k| k.to_string())
                    .collect(),
                ..Default::default()
            },
        );
        let subscribe_request = SubscribeRequest {
            accounts,
            transactions,
            commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
            ..Default::default()
        };
        let mut grpc_client = create_grpc_client(grpc_url).await;
        let (_, stream) = grpc_client
            .subscribe_with_request(Some(subscribe_request))
            .await?;
        tokio::spawn(async move {
            let mut ping = tokio::time::interval(Duration::from_secs(5));
            ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            ping.tick().await;
            loop {
                tokio::select! {
                    _ = ping.tick() => {
                        if let Err(e)=grpc_client.ping(1).await{
                            error!("GRPC PING 失败，{}",e);
                        }
                    },
                }
            }
        });
        Ok(stream)
        // Err(anyhow::anyhow!("没有找到需要订阅的账户数据"))
    }
}

async fn create_grpc_client(grpc_url: String) -> GeyserGrpcClient<impl Interceptor + Sized> {
    let use_tls = grpc_url.starts_with("https://");
    let mut builder = GeyserGrpcClient::build_from_shared(grpc_url).unwrap();
    if use_tls {
        builder = builder
            .tls_config(ClientTlsConfig::new().with_native_roots())
            .unwrap();
    }
    builder
        .max_decoding_message_size(100 * 1024 * 1024) // 100MB
        .connect_timeout(Duration::from_secs(10))
        .buffer_size(64 * 1024) // 64KB buffer
        .http2_adaptive_window(true)
        .http2_keep_alive_interval(Duration::from_secs(15))
        .initial_connection_window_size(2 * 1024 * 1024) // 2MB
        .initial_stream_window_size(2 * 1024 * 1024) // 2MB
        .keep_alive_timeout(Duration::from_secs(30))
        .keep_alive_while_idle(true)
        .tcp_keepalive(Some(Duration::from_secs(30)))
        .tcp_nodelay(true)
        .timeout(Duration::from_secs(10))
        .connect()
        .await
        .map_err(|e| {
            error!("GRPC订阅: 连接GRPC服务器失败，原因: {e}");
            anyhow::anyhow!(e)
        })
        .unwrap()
}
