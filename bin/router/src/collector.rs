use crate::interface::{DexType, GrpcAccountUpdateType};
use anyhow::anyhow;
use async_trait::async_trait;
use burberry::{Collector, CollectorStream};
use chrono::{DateTime, Local};
use serde::{Deserialize, Deserializer};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::fs::File;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::Instant;
use tokio_stream::{Stream, StreamExt, StreamMap};
use tracing::{error, info, warn};
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient};
use yellowstone_grpc_proto::geyser::subscribe_update::UpdateOneof;
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestAccountsDataSlice,
    SubscribeRequestFilterAccounts, SubscribeRequestFilterTransactions, SubscribeUpdate,
};
use yellowstone_grpc_proto::tonic::service::Interceptor;
use yellowstone_grpc_proto::tonic::Status;

#[derive(Clone, Debug)]
pub enum CollectorType {
    Single(
        (
            Vec<u8>,
            Vec<u8>,
            Vec<u8>,
            Vec<String>,
            DateTime<Local>,
            Instant,
        ),
    ),
    Multiple(
        (
            Option<Vec<u8>>,
            Option<Vec<u8>>,
            Option<Vec<u8>>,
            Vec<String>,
            DateTime<Local>,
            Instant,
        ),
    ),
}

pub struct MultiSubscribeCollector(pub String, pub String);

impl MultiSubscribeCollector {
    pub async fn subscribe_grpc(
        &self,
    ) -> anyhow::Result<StreamMap<String, impl Stream<Item = Result<SubscribeUpdate, Status>>>>
    {
        let dex_data = get_dex_data(self.0.clone());
        let mut raydium_pool_accounts = Vec::with_capacity(dex_data.len());
        let mut raydium_vault_accounts = Vec::with_capacity(dex_data.len() * 2);
        let mut pump_fun_vault_accounts = Vec::with_capacity(dex_data.len() * 2);
        for json in dex_data {
            if &json.owner == &DexType::RaydiumAMM.get_program_id() {
                raydium_pool_accounts.push(json.pool);
                raydium_vault_accounts.push((json.pool, json.vault_a, json.vault_b));
            } else if &json.owner == &DexType::PumpFunAMM.get_program_id() {
                pump_fun_vault_accounts.push((json.pool, json.vault_a, json.vault_b));
            }
        }
        let mut raydium_pool_account_map = HashMap::new();
        raydium_pool_account_map.insert(
            DexType::RaydiumAMM.to_string(),
            SubscribeRequestFilterAccounts {
                account: raydium_pool_accounts
                    .iter()
                    .map(|key| key.to_string())
                    .collect::<Vec<_>>(),
                ..Default::default()
            },
        );
        let raydium_pool_subscribe_request = SubscribeRequest {
            accounts: raydium_pool_account_map,
            commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
            ..Default::default()
        };

        let mut raydium_vault_account_map = HashMap::new();
        for (pool_id, vault_a, vault_b) in raydium_vault_accounts {
            raydium_vault_account_map.insert(
                format!(
                    "{:?}:{:?}:{:?}",
                    pool_id,
                    0,
                    DexType::RaydiumAMM.get_program_id()
                ),
                SubscribeRequestFilterAccounts {
                    account: vec![vault_a.to_string()],
                    ..Default::default()
                },
            );
            raydium_vault_account_map.insert(
                format!(
                    "{:?}:{:?}:{:?}",
                    pool_id,
                    1,
                    DexType::RaydiumAMM.get_program_id()
                ),
                SubscribeRequestFilterAccounts {
                    account: vec![vault_b.to_string()],
                    ..Default::default()
                },
            );
        }
        let raydium_vault_subscribe_request = SubscribeRequest {
            accounts: raydium_vault_account_map,
            commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
            ..Default::default()
        };

        let mut pump_fun_vault_account_map = HashMap::new();
        for (pool_id, vault_a, vault_b) in pump_fun_vault_accounts {
            pump_fun_vault_account_map.insert(
                format!(
                    "{:?}:{:?}:{:?}",
                    pool_id,
                    0,
                    DexType::PumpFunAMM.get_program_id()
                ),
                SubscribeRequestFilterAccounts {
                    account: vec![vault_a.to_string()],
                    ..Default::default()
                },
            );
            pump_fun_vault_account_map.insert(
                format!(
                    "{:?}:{:?}:{:?}",
                    pool_id,
                    1,
                    DexType::PumpFunAMM.get_program_id()
                ),
                SubscribeRequestFilterAccounts {
                    account: vec![vault_b.to_string()],
                    ..Default::default()
                },
            );
        }
        let pump_fun_vault_subscribe_request = SubscribeRequest {
            accounts: pump_fun_vault_account_map,
            commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
            ..Default::default()
        };

        let mut subscrbeitions = StreamMap::new();
        let mut grpc_client = create_grpc_client(self.1.clone()).await;
        let (_, raydium_pool_stream) = grpc_client
            .subscribe_with_request(Some(raydium_pool_subscribe_request))
            .await?;
        subscrbeitions.insert(
            format!(
                "{:?}:{:?}",
                DexType::RaydiumAMM,
                GrpcAccountUpdateType::Pool
            ),
            raydium_pool_stream,
        );
        let (_, raydium_vault_stream) = grpc_client
            .subscribe_with_request(Some(raydium_vault_subscribe_request))
            .await?;
        subscrbeitions.insert(
            format!(
                "{:?}:{:?}",
                DexType::RaydiumAMM,
                GrpcAccountUpdateType::MintVault
            ),
            raydium_vault_stream,
        );
        let (_, pump_fun_vault_stream) = grpc_client
            .subscribe_with_request(Some(pump_fun_vault_subscribe_request))
            .await?;
        subscrbeitions.insert(
            format!(
                "{:?}:{:?}",
                DexType::PumpFunAMM,
                GrpcAccountUpdateType::MintVault
            ),
            pump_fun_vault_stream,
        );
        Ok(subscrbeitions)
    }
}

#[async_trait]
impl
    Collector<(
        Option<Vec<u8>>,
        Option<Vec<u8>>,
        Option<Vec<u8>>,
        Vec<String>,
        DateTime<Local>,
        Instant,
    )> for MultiSubscribeCollector
{
    async fn get_event_stream(
        &self,
    ) -> eyre::Result<
        CollectorStream<
            '_,
            (
                Option<Vec<u8>>,
                Option<Vec<u8>>,
                Option<Vec<u8>>,
                Vec<String>,
                DateTime<Local>,
                Instant,
            ),
        >,
    > {
        let mut subscrbeitions = self.subscribe_grpc().await.unwrap();
        info!("GRPC 订阅成功");
        let stream = async_stream::stream! {
            loop {
                tokio::select! {
                    Some((_,Ok(data))) = subscrbeitions.next() => {
                        let time = Local::now();
                        let now=Instant::now();
                        let filters = data.filters;
                        if let Some(UpdateOneof::Account(account)) = data.update_oneof {
                            let account= account.account.unwrap();
                            yield (
                                account.txn_signature,
                                Some(account.pubkey),
                                Some(account.owner),
                                filters,
                                time,
                                now,
                            );
                        } else if let Some(UpdateOneof::Transaction(transaction)) = data.update_oneof {
                            yield (
                                Some(transaction.transaction.unwrap().signature),
                                None,
                                None,
                                filters,
                                time,
                                now,
                            );
                        }
                    }else => warn!("subscrbeitions closed"),
                }
            }
        };
        Ok(Box::pin(stream))
    }
}

#[derive(Debug)]
pub struct SingleSubscribeCollector(pub String, pub String);

impl SingleSubscribeCollector {
    pub async fn subscribe_grpc(
        &self,
    ) -> anyhow::Result<StreamMap<String, impl Stream<Item = Result<SubscribeUpdate, Status>>>>
    {
        let dex_data = get_dex_data(self.0.clone());
        let mut raydium_pool_keys = Vec::with_capacity(dex_data.len() * 3);
        let mut raydium_accounts = HashMap::new();
        let mut pump_fun_accounts = HashMap::new();

        for json in dex_data {
            if &json.owner == &DexType::RaydiumAMM.get_program_id() {
                raydium_pool_keys.push(json.pool);
                raydium_accounts.insert(
                    format!("{:?}:{:?}:{:?}", json.pool, 0, json.owner),
                    SubscribeRequestFilterAccounts {
                        account: vec![json.vault_a.to_string()],
                        ..Default::default()
                    },
                );
                raydium_accounts.insert(
                    format!("{:?}:{:?}:{:?}", json.pool, 1, json.owner),
                    SubscribeRequestFilterAccounts {
                        account: vec![json.vault_b.to_string()],
                        ..Default::default()
                    },
                );
            }
            if &json.owner == &DexType::PumpFunAMM.get_program_id() {
                pump_fun_accounts.insert(
                    format!("{:?}:{:?}:{:?}", json.pool, 0, json.owner),
                    SubscribeRequestFilterAccounts {
                        account: vec![json.vault_a.to_string()],
                        ..Default::default()
                    },
                );
                pump_fun_accounts.insert(
                    format!("{:?}:{:?}:{:?}", json.pool, 1, json.owner),
                    SubscribeRequestFilterAccounts {
                        account: vec![json.vault_b.to_string()],
                        ..Default::default()
                    },
                );
            }
        }
        if !raydium_pool_keys.is_empty() {
            raydium_accounts.insert(
                "accounts".to_string(),
                SubscribeRequestFilterAccounts {
                    account: raydium_pool_keys
                        .iter()
                        .map(|key| key.to_string())
                        .collect::<Vec<_>>(),
                    ..Default::default()
                },
            );
        }
        let subscribe_request = SubscribeRequest {
            accounts: raydium_accounts,
            commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
            ..Default::default()
        };
        let mut subscrbeitions = StreamMap::new();
        let mut grpc_client = create_grpc_client(self.1.clone()).await;
        let (_, stream) = grpc_client
            .subscribe_with_request(Some(subscribe_request))
            .await?;
        subscrbeitions.insert("SingleSubscribeCollector".to_string(), stream);
        Ok(subscrbeitions)
    }
}

#[async_trait]
impl
    Collector<(
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<String>,
        DateTime<Local>,
        Instant,
    )> for SingleSubscribeCollector
{
    async fn get_event_stream(
        &self,
    ) -> eyre::Result<
        CollectorStream<
            '_,
            (
                Vec<u8>,
                Vec<u8>,
                Vec<u8>,
                Vec<String>,
                DateTime<Local>,
                Instant,
            ),
        >,
    > {
        let mut subscrbeitions = self.subscribe_grpc().await.unwrap();
        info!("GRPC 订阅成功");
        let stream = async_stream::stream! {
            loop {
                tokio::select! {
                    Some((_,Ok(data))) = subscrbeitions.next() => {
                        let time = Local::now();
                        let now=Instant::now();
                        let filters = data.filters;
                        if let Some(UpdateOneof::Account(account)) = data.update_oneof {
                            let account= account.account.unwrap();
                            yield (
                                account.txn_signature.unwrap(),
                                account.pubkey,
                                account.owner,
                                filters,
                                time,
                                now,
                            );
                        }
                    }else => warn!("subscrbeitions closed"),
                }
            }
        };
        Ok(Box::pin(stream))
    }
}

pub fn get_dex_data(dex_json_path: String) -> Vec<DexJson> {
    let dex_jsons: Vec<DexJson> = match File::open(dex_json_path.as_str()) {
        Ok(file) => serde_json::from_reader(file).expect("解析【dex_data.json】失败"),
        Err(e) => {
            error!("{}", e);
            vec![]
        }
    };
    dex_jsons
}

#[derive(Debug, Clone, Deserialize)]
pub struct DexJson {
    #[serde(deserialize_with = "deserialize_pubkey")]
    pub pool: Pubkey,
    #[serde(deserialize_with = "deserialize_pubkey")]
    pub owner: Pubkey,
    #[serde(deserialize_with = "deserialize_pubkey", rename = "vaultA")]
    pub vault_a: Pubkey,
    #[serde(deserialize_with = "deserialize_pubkey", rename = "vaultB")]
    pub vault_b: Pubkey,
    #[serde(
        deserialize_with = "deserialize_option_pubkey",
        rename = "addressLookupTableAddress"
    )]
    pub address_lookup_table_address: Option<Pubkey>,
}

fn deserialize_pubkey<'de, D>(deserializer: D) -> anyhow::Result<Pubkey, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    Ok(Pubkey::from_str(s.as_str()).unwrap())
}

fn deserialize_option_pubkey<'de, D>(deserializer: D) -> anyhow::Result<Option<Pubkey>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: anyhow::Result<String, _> = Deserialize::deserialize(deserializer);
    if s.is_err() {
        return Ok(None);
    }
    Ok(Some(Pubkey::from_str(s?.as_str()).unwrap()))
}

async fn create_grpc_client(grpc_url: String) -> GeyserGrpcClient<impl Interceptor + Sized> {
    GeyserGrpcClient::build_from_shared(grpc_url)
        .unwrap()
        .tcp_nodelay(true)
        .http2_adaptive_window(true)
        .buffer_size(65536)
        .initial_connection_window_size(5242880)
        .initial_stream_window_size(4194304)
        .connect_timeout(Duration::from_millis(30 * 1000))
        .tls_config(ClientTlsConfig::new().with_native_roots())
        .unwrap()
        .connect()
        .await
        .map_err(|e| {
            error!("GRPC订阅: 连接GRPC服务器失败，原因: {e}");
            anyhow::anyhow!(e)
        })
        .unwrap()
}
