use crate::dex_data::DexJson;
use crate::interface::DexType;
use crate::state::{GrpcAccountMsg, GrpcMessage, GrpcTransactionMsg};
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
    pub single_mode: bool,
    pub specify_pool: Option<String>,
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
                        match transaction.transaction {
                            None => {}
                            Some(tx) => {
                                match message_sender
                                    .send(GrpcMessage::Transaction(GrpcTransactionMsg::from(tx)))
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
                    error!("grpc推送消息失败，原因：{}", e)
                }
            }
        }
    }

    async fn single_subscribe_grpc(
        grpc_url: String,
        dex_data: Vec<DexJson>,
    ) -> anyhow::Result<impl Stream<Item = Result<SubscribeUpdate, Status>>> {
        let mut raydium_amm_account_keys = Vec::with_capacity(dex_data.len());
        let mut pump_fun_account_keys = Vec::with_capacity(dex_data.len());
        let mut raydium_clmm_account_keys = Vec::with_capacity(dex_data.len());
        let mut raydium_clmm_bitmap_extension_account_keys = Vec::with_capacity(dex_data.len());
        let mut meteora_dlmm_account_keys = Vec::with_capacity(dex_data.len());
        let mut meteora_dlmm_bitmap_extension_account_keys = Vec::with_capacity(dex_data.len());

        for json in dex_data.iter() {
            if &json.owner == DexType::RaydiumAMM.get_ref_program_id() {
                raydium_amm_account_keys.push(json.pool);
                raydium_amm_account_keys.push(json.vault_a);
                raydium_amm_account_keys.push(json.vault_b);
            }
            if &json.owner == DexType::PumpFunAMM.get_ref_program_id() {
                pump_fun_account_keys.push(json.vault_a);
                pump_fun_account_keys.push(json.vault_b);
            }
            if &json.owner == DexType::RaydiumCLMM.get_ref_program_id() {
                // TickArrayBitmapExtension
                raydium_clmm_bitmap_extension_account_keys.push(
                    Pubkey::find_program_address(
                        &[POOL_TICK_ARRAY_BITMAP_SEED.as_bytes(), json.pool.as_ref()],
                        DexType::RaydiumCLMM.get_ref_program_id(),
                    )
                    .0,
                );
                raydium_clmm_account_keys.push(json.pool);
            }
            if &json.owner == DexType::MeteoraDLMM.get_ref_program_id() {
                // BinArrayBitmapExtension
                meteora_dlmm_bitmap_extension_account_keys.push(
                    crate::dex::meteora_dlmm::commons::pda::derive_bin_array_bitmap_extension(
                        &json.pool,
                    ),
                );
                meteora_dlmm_account_keys.push(json.pool);
            }
        }
        let need_clock = !meteora_dlmm_account_keys.is_empty();
        let all_account_keys = raydium_amm_account_keys
            .iter()
            .chain(pump_fun_account_keys.iter())
            .chain(raydium_clmm_account_keys.iter())
            .chain(raydium_clmm_bitmap_extension_account_keys.iter())
            .chain(meteora_dlmm_account_keys.iter())
            .chain(meteora_dlmm_bitmap_extension_account_keys.iter())
            .map(|key| key.to_string())
            .collect::<Vec<_>>();
        let mut grpc_client = create_grpc_client(grpc_url).await;
        let mut accounts = HashMap::new();
        // 所有池子、金库订阅
        if !all_account_keys.is_empty() {
            accounts.insert(
                "accounts".to_string(),
                SubscribeRequestFilterAccounts {
                    account: {
                        let mut accounts = all_account_keys.clone();
                        if need_clock {
                            accounts.push(Clock::id().to_string());
                        }
                        accounts
                    },
                    ..Default::default()
                },
            );
        }
        // CLMM TickArrayState订阅
        if !raydium_clmm_account_keys.is_empty() {
            for pool_id in raydium_clmm_account_keys.iter() {
                accounts.insert(
                    pool_id.to_string(),
                    SubscribeRequestFilterAccounts {
                        owner: vec![DexType::RaydiumCLMM.get_ref_program_id().to_string()],
                        filters: vec![
                            // TickArrayState data大小为10240
                            SubscribeRequestFilterAccountsFilter {
                                filter: Some(Datasize(10240)),
                            },
                            // 订阅关注的池子的TickArrayState
                            SubscribeRequestFilterAccountsFilter {
                                filter: Some(
                                    Filter::Memcmp(SubscribeRequestFilterAccountsFilterMemcmp {
                                        offset: 8,
                                        data: Some(
                                            subscribe_request_filter_accounts_filter_memcmp::Data::Bytes(
                                                pool_id.to_bytes().to_vec(),
                                            ),
                                        ),
                                    }),
                                ),
                            },
                        ],
                        ..Default::default()
                    },
                );
            }
        }
        // dlmm bin array
        if !meteora_dlmm_account_keys.is_empty() {
            for pool_id in meteora_dlmm_account_keys.iter() {
                accounts.insert(
                    pool_id.to_string(),
                    SubscribeRequestFilterAccounts {
                        owner: vec![DexType::RaydiumCLMM.get_ref_program_id().to_string()],
                        filters: vec![
                            // BinArray data大小为10136
                            SubscribeRequestFilterAccountsFilter {
                                filter: Some(Datasize(10136)),
                            },
                            // 订阅关注的池子的BinArray
                            SubscribeRequestFilterAccountsFilter {
                                filter: Some(
                                    Filter::Memcmp(SubscribeRequestFilterAccountsFilterMemcmp {
                                        offset: 24,
                                        data: Some(
                                            subscribe_request_filter_accounts_filter_memcmp::Data::Bytes(
                                                pool_id.to_bytes().to_vec(),
                                            ),
                                        ),
                                    }),
                                ),
                            },
                        ],
                        ..Default::default()
                    },
                );
            }
        }
        // 交易订阅
        if !accounts.is_empty() {
            let mut transactions = HashMap::new();
            transactions.insert(
                "transactions".to_string(),
                SubscribeRequestFilterTransactions {
                    vote: Some(false),
                    failed: Some(false),
                    account_include: all_account_keys,
                    ..Default::default()
                },
            );
            let subscribe_request = SubscribeRequest {
                accounts,
                transactions,
                commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
                ..Default::default()
            };
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
            return Ok(stream);
        }
        Err(anyhow::anyhow!("没有找到需要订阅的账户数据"))
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
