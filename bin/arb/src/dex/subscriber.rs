use crate::dex::meteora_damm_v2::MeteoraDAMMV2AccountSubscriber;
use crate::dex::meteora_dlmm::MeteoraDLMMAccountSubscriber;
use crate::dex::orca_whirlpools::OrcaWhirlAccountSubscriber;
use crate::dex::pump_fun::PumpFunAMMAccountSubscriber;
use crate::dex::raydium_amm::RaydiumAMMAccountSubscriber;
use crate::dex::raydium_clmm::RaydiumCLMMAccountSubscriber;
use crate::dex::raydium_cpmm::RaydiumCPMMAccountSubscriber;
use crate::dex::{DexType, GlobalCache, CLOCK_ID};
use crate::dex_data::DexJson;
use ahash::AHashSet;
use anyhow::anyhow;
use enum_dispatch::enum_dispatch;
use futures_util::Stream;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::OnceCell;
use tracing::error;
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient, Interceptor};
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestFilterAccounts,
    SubscribeRequestFilterTransactions, SubscribeUpdate,
};
use yellowstone_grpc_proto::tonic::Status;

pub const GRPC_SUBSCRIBED_ACCOUNTS: OnceCell<Arc<AHashSet<Vec<u8>>>> = OnceCell::const_new();

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
    RaydiumCPMM(RaydiumCPMMAccountSubscriber),
}

impl Display for Subscriber {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Subscriber::MeteoraDLMM(_) => "MeteoraDLMM",
            Subscriber::MeteoraDAMMV2(_) => "MeteoraDAMMV2",
            Subscriber::PumpFunAMM(_) => "PumpFunAM",
            Subscriber::RaydiumAMM(_) => "RaydiumAMM",
            Subscriber::RaydiumCLMM(_) => "RaydiumCLMM",
            Subscriber::OrcaWhirl(_) => "OrcaWhirl",
            Subscriber::RaydiumCPMM(_) => "RaydiumCPMM",
        })
    }
}

pub async fn grpc_subscribe(
    grpc_url: String,
    dex_json: Vec<DexJson>,
) -> anyhow::Result<impl Stream<Item = Result<SubscribeUpdate, Status>>> {
    let mut account_subscribe_owners: AHashSet<Pubkey> =
        AHashSet::with_capacity(dex_json.len() * 3);
    let mut tx_include_owners = AHashSet::with_capacity(dex_json.len() * 3);
    let mut subscribe_accounts = AHashSet::with_capacity(10_000_000);
    let mut need_clock = false;
    for sub in vec![
        Subscriber::from(MeteoraDLMMAccountSubscriber),
        Subscriber::from(MeteoraDAMMV2AccountSubscriber),
        Subscriber::from(PumpFunAMMAccountSubscriber),
        Subscriber::from(RaydiumAMMAccountSubscriber),
        Subscriber::from(RaydiumCLMMAccountSubscriber),
        Subscriber::from(OrcaWhirlAccountSubscriber),
        Subscriber::from(RaydiumCPMMAccountSubscriber),
    ] {
        match sub.get_subscription_accounts(dex_json.as_slice()) {
            None => {}
            Some(accounts) => {
                if accounts.account_subscribe_owners.is_empty()
                    && accounts.tx_include_accounts.is_empty()
                    && accounts.subscribed_accounts.is_empty()
                {
                    continue;
                }
                account_subscribe_owners.extend(
                    accounts
                        .account_subscribe_owners
                        .into_iter()
                        .collect::<AHashSet<_>>(),
                );
                tx_include_owners.extend(accounts.tx_include_accounts);
                subscribe_accounts.extend(accounts.subscribed_accounts);
                need_clock |= accounts.need_clock;
            }
        }
    }
    if account_subscribe_owners.is_empty() || tx_include_owners.is_empty() {
        return Err(anyhow!("没有订阅账户"));
    }

    let mut accounts = HashMap::with_capacity(dex_json.len() * 3);
    if need_clock {
        accounts.insert(
            "clock".to_string(),
            SubscribeRequestFilterAccounts {
                account: vec![CLOCK_ID.to_string()],
                ..Default::default()
            },
        );
        subscribe_accounts.insert(CLOCK_ID);
    }
    accounts.insert(
        "account_owner".to_string(),
        SubscribeRequestFilterAccounts {
            owner: account_subscribe_owners
                .into_iter()
                .map(|t| t.to_string())
                .collect(),
            ..Default::default()
        },
    );
    let mut transactions = HashMap::new();
    transactions.insert(
        "transactions".to_string(),
        SubscribeRequestFilterTransactions {
            vote: Some(false),
            failed: Some(false),
            account_include: tx_include_owners
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
    GRPC_SUBSCRIBED_ACCOUNTS.set(Arc::new(
        subscribe_accounts
            .into_iter()
            .map(|t| t.to_bytes().to_vec())
            .collect::<AHashSet<_>>(),
    ))?;
    Ok(stream)
}

#[derive(Debug, Default)]
pub struct SubscriptionAccounts {
    pub tx_include_accounts: Vec<Pubkey>,
    pub account_subscribe_owners: Vec<Pubkey>,
    pub subscribed_accounts: Vec<Pubkey>,
    pub need_clock: bool,
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
