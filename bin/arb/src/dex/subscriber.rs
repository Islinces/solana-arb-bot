use crate::dex::meteora_damm_v2::MeteoraDAMMV2AccountSubscriber;
use crate::dex::meteora_dlmm::MeteoraDLMMAccountSubscriber;
use crate::dex::orca_whirlpools::OrcaWhirlAccountSubscriber;
use crate::dex::pump_fun::PumpFunAMMAccountSubscriber;
use crate::dex::raydium_amm::RaydiumAMMAccountSubscriber;
use crate::dex::raydium_clmm::RaydiumCLMMAccountSubscriber;
use crate::dex_data::DexJson;
use ahash::AHashSet;
use anyhow::anyhow;
use enum_dispatch::enum_dispatch;
use futures_util::Stream;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::time::Duration;
use tracing::error;
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient, Interceptor};
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestFilterAccounts,
    SubscribeRequestFilterTransactions, SubscribeUpdate,
};
use yellowstone_grpc_proto::tonic::Status;

#[enum_dispatch]
pub trait AccountSubscriber {
    fn get_subscription_accounts(&self, dex_json: &[DexJson]) -> Option<SubscriptionAccounts>;
}

#[enum_dispatch(AccountSubscriber)]
pub enum Subscriber {
    MeteoraDLMM(MeteoraDLMMAccountSubscriber),
    MeteoraDAMMV2(MeteoraDAMMV2AccountSubscriber),
    PumpFunAMM(PumpFunAMMAccountSubscriber),
    RaydiumAMM(RaydiumAMMAccountSubscriber),
    RaydiumCLMM(RaydiumCLMMAccountSubscriber),
    OrcaWhirl(OrcaWhirlAccountSubscriber),
}

pub async fn grpc_subscribe(
    grpc_url: String,
    dex_json: Vec<DexJson>,
) -> anyhow::Result<impl Stream<Item = Result<SubscribeUpdate, Status>>> {
    let mut unified_accounts: AHashSet<Pubkey> = AHashSet::with_capacity(dex_json.len() * 3);
    let mut account_with_owner_and_filter: HashMap<String, SubscribeRequestFilterAccounts> =
        HashMap::with_capacity(dex_json.len());
    let mut tx_include_accounts: Vec<Pubkey> = Vec::with_capacity(dex_json.len() * 3);
    for sub in vec![
        Subscriber::from(MeteoraDLMMAccountSubscriber),
        Subscriber::from(MeteoraDAMMV2AccountSubscriber),
        Subscriber::from(PumpFunAMMAccountSubscriber),
        Subscriber::from(RaydiumAMMAccountSubscriber),
        Subscriber::from(RaydiumCLMMAccountSubscriber),
        Subscriber::from(OrcaWhirlAccountSubscriber),
    ] {
        match sub.get_subscription_accounts(dex_json.as_slice()) {
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
    let mut accounts = HashMap::with_capacity(dex_json.len() * 3);
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
}

pub struct SubscriptionAccounts {
    // 放在一个SubscribeRequestFilterAccounts中
    pub unified_accounts: Vec<Pubkey>,
    // 每个value单独一个SubscribeRequestFilterAccounts，TickArray，BinArray等订阅
    pub account_with_owner_and_filter: Option<HashMap<String, SubscribeRequestFilterAccounts>>,
    // 订阅tx包含的账户
    pub tx_include_accounts: Vec<Pubkey>,
}

impl SubscriptionAccounts {
    pub fn new(
        unified_accounts: Vec<Pubkey>,
        account_with_owner_and_filter: Option<HashMap<String, SubscribeRequestFilterAccounts>>,
        tx_include_accounts: Vec<Pubkey>,
    ) -> Self {
        Self {
            unified_accounts,
            account_with_owner_and_filter,
            tx_include_accounts,
        }
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
