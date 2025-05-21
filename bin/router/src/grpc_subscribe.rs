use crate::dex_data::DexJson;
use crate::interface::{DexType, GrpcAccountUpdateType};
use base58::ToBase58;
use chrono::{DateTime, Local};
use serde::{Deserialize, Deserializer};
use serde_json::json;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::fs::File;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::Instant;
use tokio_stream::{Stream, StreamExt, StreamMap};
use tracing::{error, info, warn};
use yellowstone_grpc_client::GeyserGrpcClient;
use yellowstone_grpc_proto::geyser::subscribe_update::UpdateOneof;
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestFilterAccounts, SubscribeUpdate,
};
use yellowstone_grpc_proto::tonic::service::Interceptor;
use yellowstone_grpc_proto::tonic::transport::ClientTlsConfig;
use yellowstone_grpc_proto::tonic::Status;

#[derive(Debug)]
pub struct GrpcSubscribe {
    pub grpc_url: String,
    pub dex_json_path: String,
    pub message_sender: UnboundedSender<(Vec<u8>, Vec<u8>, Vec<u8>, DateTime<Local>)>,
    pub single_mode: bool,
    pub specify_pool: Option<String>,
    pub use_stream_map: bool,
    pub standard_program: bool,
}

impl GrpcSubscribe {
    async fn single_subscribe_grpc(
        &self,
        dex_data: Vec<DexJson>,
    ) -> anyhow::Result<impl Stream<Item = Result<SubscribeUpdate, Status>>> {
        let mut account_keys = Vec::with_capacity(dex_data.len());

        for json in dex_data.iter() {
            if &json.owner == DexType::RaydiumAMM.get_ref_program_id() {
                account_keys.push(json.pool);
                account_keys.push(json.vault_a);
                account_keys.push(json.vault_b);
            }
            if &json.owner == DexType::PumpFunAMM.get_ref_program_id() {
                account_keys.push(json.vault_a);
                account_keys.push(json.vault_b);
            }
        }
        let mut grpc_client = create_grpc_client(self.grpc_url.clone()).await;
        if !account_keys.is_empty() {
            let mut filter_accounts = HashMap::new();
            filter_accounts.insert(
                "accounts".to_string(),
                SubscribeRequestFilterAccounts {
                    account: account_keys
                        .iter()
                        .map(|key| key.to_string())
                        .collect::<Vec<_>>(),
                    ..Default::default()
                },
            );
            let subscribe_request = SubscribeRequest {
                accounts: filter_accounts,
                commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
                ..Default::default()
            };
            let (_, stream) = grpc_client
                .subscribe_with_request(Some(subscribe_request))
                .await?;
            return Ok(stream);
        }
        Err(anyhow::anyhow!("没有找到需要订阅的账户数据"))
    }

    async fn multi_subscribe_grpc(
        &self,
        dex_data: Vec<DexJson>,
    ) -> anyhow::Result<StreamMap<String, impl Stream<Item = Result<SubscribeUpdate, Status>>>>
    {
        let mut raydium_pool_keys = Vec::with_capacity(dex_data.len());
        let mut raydium_vault_keys = Vec::with_capacity(dex_data.len() * 2);
        let mut pump_fun_vault_keys = Vec::with_capacity(dex_data.len() * 2);

        for json in dex_data {
            if &json.owner == DexType::RaydiumAMM.get_ref_program_id() {
                raydium_pool_keys.push(json.pool);
                raydium_vault_keys.push(json.vault_a);
                raydium_vault_keys.push(json.vault_b);
            } else if &json.owner == DexType::PumpFunAMM.get_ref_program_id() {
                pump_fun_vault_keys.push(json.vault_a);
                pump_fun_vault_keys.push(json.vault_b);
            }
        }
        let mut subscrbeitions = StreamMap::new();
        let mut grpc_client = create_grpc_client(self.grpc_url.clone()).await;

        if !raydium_pool_keys.is_empty() {
            let mut raydium_pool_account_map = HashMap::new();
            raydium_pool_account_map.insert(
                "account1".to_string(),
                SubscribeRequestFilterAccounts {
                    account: raydium_pool_keys
                        .into_iter()
                        .map(|key| key.to_string())
                        .collect(),
                    ..Default::default()
                },
            );
            let raydium_pool_subscribe_request = SubscribeRequest {
                accounts: raydium_pool_account_map,
                commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
                // accounts_data_slice: vec![
                //     // state_data.need_take_pnl_coin
                //     SubscribeRequestAccountsDataSlice {
                //         offset: 192,
                //         length: 8,
                //     },
                //     // state_data.need_take_pnl_pc
                //     SubscribeRequestAccountsDataSlice {
                //         offset: 200,
                //         length: 8,
                //     },
                // ],
                ..Default::default()
            };

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
        }
        // if !raydium_vault_keys.is_empty() {
        //     let mut raydium_vault_account_map = HashMap::new();
        //     for (pool_id, vault_a, vault_b) in raydium_vault_keys {
        //         raydium_vault_account_map.insert(
        //             format!(
        //                 "{}:{}",
        //                 DexType::RaydiumAMM.get_str_program_id(),
        //                 pool_id.to_string()
        //             ),
        //             SubscribeRequestFilterAccounts {
        //                 account: vec![vault_a.to_string(), vault_b.to_string()],
        //                 ..Default::default()
        //             },
        //         );
        //     }
        //     let raydium_vault_subscribe_request = SubscribeRequest {
        //         accounts: raydium_vault_account_map,
        //         commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
        //         // accounts_data_slice: vec![
        //         //     // mint
        //         //     SubscribeRequestAccountsDataSlice {
        //         //         offset: 0,
        //         //         length: 32,
        //         //     },
        //         //     // amount
        //         //     SubscribeRequestAccountsDataSlice {
        //         //         offset: 64,
        //         //         length: 8,
        //         //     },
        //         //     // state
        //         //     SubscribeRequestAccountsDataSlice {
        //         //         offset: 108,
        //         //         length: 1,
        //         //     },
        //         // ],
        //         ..Default::default()
        //     };
        //     let (_, raydium_vault_stream) = grpc_client
        //         .subscribe_with_request(Some(raydium_vault_subscribe_request))
        //         .await?;
        //     subscrbeitions.insert(
        //         format!(
        //             "{:?}:{:?}",
        //             DexType::RaydiumAMM,
        //             GrpcAccountUpdateType::MintVault
        //         ),
        //         raydium_vault_stream,
        //     );
        // }

        // if !pump_fun_vault_keys.is_empty() {
        //     let mut pump_fun_vault_account_map = HashMap::new();
        //     for (pool_id, vault_a, vault_b) in pump_fun_vault_keys {
        //         pump_fun_vault_account_map.insert(
        //             format!(
        //                 "{}:{}",
        //                 DexType::PumpFunAMM.get_str_program_id(),
        //                 pool_id.to_string()
        //             ),
        //             SubscribeRequestFilterAccounts {
        //                 account: vec![vault_a.to_string(), vault_b.to_string()],
        //                 ..Default::default()
        //             },
        //         );
        //     }
        //     let pump_fun_vault_subscribe_request = SubscribeRequest {
        //         accounts: pump_fun_vault_account_map,
        //         commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
        //         // accounts_data_slice: vec![
        //         //     // mint
        //         //     SubscribeRequestAccountsDataSlice {
        //         //         offset: 0,
        //         //         length: 32,
        //         //     },
        //         //     // amount
        //         //     SubscribeRequestAccountsDataSlice {
        //         //         offset: 64,
        //         //         length: 8,
        //         //     },
        //         //     // state
        //         //     SubscribeRequestAccountsDataSlice {
        //         //         offset: 108,
        //         //         length: 1,
        //         //     },
        //         // ],
        //         ..Default::default()
        //     };
        //
        //     let (_, pump_fun_vault_stream) = grpc_client
        //         .subscribe_with_request(Some(pump_fun_vault_subscribe_request))
        //         .await?;
        //     subscrbeitions.insert(
        //         format!(
        //             "{:?}:{:?}",
        //             DexType::PumpFunAMM,
        //             GrpcAccountUpdateType::MintVault
        //         ),
        //         pump_fun_vault_stream,
        //     );
        // }
        if subscrbeitions.is_empty() {
            Err(anyhow::anyhow!("没有找到需要订阅的账户数据"))
        } else {
            Ok(subscrbeitions)
        }
    }

    pub async fn subscribe(&self, dex_data: Vec<DexJson>) {
        let mut stream = self.single_subscribe_grpc(dex_data).await.unwrap();
        info!("GRPC 订阅成功");
        if self.standard_program {
            while let Some(message) = stream.next().await {
                match message {
                    Ok(data) => {
                        let time = Local::now();
                        if let Some(UpdateOneof::Account(account)) = data.update_oneof {
                            let account = account.account.unwrap();
                            let tx = account.txn_signature.as_ref().unwrap().to_base58();
                            let account_key = Pubkey::try_from(account.pubkey).unwrap();
                            if self.specify_pool.as_ref().is_none()
                                || self.specify_pool.as_ref().unwrap() == &account_key.to_string()
                            {
                                info!(
                                    "tx : {:?}, account : {:?}, timestamp : {:?}",
                                    tx,
                                    account_key,
                                    time.format("%Y-%m-%d %H:%M:%S%.9f").to_string()
                                );
                            }
                        }
                    }
                    Err(e) => {
                        error!("grpc推送消息失败，原因：{}", e)
                    }
                }
            }
        } else {
            while let Some(message) = stream.next().await {
                match message {
                    Ok(data) => {
                        let time = Local::now();
                        if let Some(UpdateOneof::Account(account)) = data.update_oneof {
                            let account = account.account.unwrap();
                            let _ = self.message_sender.send((
                                account.txn_signature.unwrap(),
                                account.pubkey,
                                account.owner,
                                time,
                            ));
                        }
                    }
                    Err(e) => {
                        error!("grpc推送消息失败，原因：{}", e)
                    }
                }
            }
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
        .connect_timeout(Duration::from_secs(2))
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
