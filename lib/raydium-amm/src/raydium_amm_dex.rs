use crate::amm_pool::AmmPool;
use crate::state::{AmmInfo, AmmStatus, Fees, Loadable, StateData};
use arrayref::{array_ref, array_refs};
use base58::ToBase58;
use chrono::Utc;
use dex::interface::{DexInterface, DexPoolInterface, GrpcSubscriber};
use dex::state::{FetchConfig, GrpcAccountUpdateType, SourceMessage};
use dex::trigger::{TriggerEvent, TriggerEventHolder};
use dex::util::tokio_spawn;
use log::{error, info};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_config::RpcProgramAccountsConfig;
use solana_program::program_pack::Pack;
use solana_program::pubkey::Pubkey;
use solana_sdk::commitment_config::CommitmentConfig;
use spl_token::state::Account;
use std::any::Any;
use std::cell::RefMut;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::ops::Sub;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use yellowstone_grpc_client::{GeyserGrpcClient, Interceptor};
use yellowstone_grpc_proto::geyser::subscribe_update::UpdateOneof;
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestAccountsDataSlice,
    SubscribeRequestFilterAccounts, SubscribeRequestPing, SubscribeUpdateAccountInfo,
};
use yellowstone_grpc_proto::tonic::codegen::tokio_stream::{StreamExt, StreamMap};

pub struct RaydiumAmmDex {
    pub base_pool_map: Vec<Arc<dyn DexPoolInterface>>,
}

#[async_trait::async_trait]
impl DexInterface for RaydiumAmmDex {
    fn name(&self) -> String {
        "raydium-amm".to_string()
    }

    fn get_base_pools(&self) -> Vec<Arc<dyn DexPoolInterface>> {
        self.base_pool_map.clone()
    }

    async fn fetch_pool_base_info(
        rpc_client: &RpcClient,
        fetch_config: &FetchConfig,
    ) -> anyhow::Result<Arc<dyn DexInterface>>
    where
        Self: Sized,
    {
        // let program_id = Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8").unwrap();
        // let rpc_program_accounts_config = RpcProgramAccountsConfig {
        //     with_context: Some(false),
        //     account_config: RpcAccountInfoConfig {
        //         encoding: Some(UiAccountEncoding::Base64),
        //         min_context_slot: None,
        //         commitment: Some(CommitmentConfig::finalized()),
        //         data_slice: None,
        //     },
        //     filters: Some(vec![RpcFilterType::DataSize(size_of::<AmmInfo>() as u64)]),
        // };
        // let result = rpc_client
        //     .get_program_accounts_with_config(&program_id, rpc_program_accounts_config)
        //     .await
        //     .map_err(|e| error!("raydium amm fetch faile,{}", e));

        let pubkeys = vec![
            Pubkey::from_str("5oAvct85WyF7Sj73VYHbyFJkdRJ28D8m4z4Sxjvzuc6n").unwrap(),
            Pubkey::from_str("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2").unwrap(),
            Pubkey::from_str("61acRgpURKTU8LKPJKs6WQa18KzD9ogavXzjxfD84KLu").unwrap(),
            Pubkey::from_str("FyqYBBJ8vhr5AtDZiyJue4Khx9Be6Xijx5nm6aL6wZZV").unwrap(),
            Pubkey::from_str("AbbG2aR8iNhy2prC32iDRW7pKJjzqhUtri8rV5HboHUY").unwrap(),
            Pubkey::from_str("CwF4aUPjMciM3u9DpxoZyLGVKVxgUgBmxHW32RbtmZNz").unwrap(),
            Pubkey::from_str("EUvnsWhMnhY3S5EnV3tLATNRvTM2xBMWZQQguzpFNwYT").unwrap(),
            
            Pubkey::from_str("9DzLnkFRg5zMy532q7sbJjZx7uEL1SeFUDyAnF5yizoH").unwrap(),
            
            
        ];
        let copy = pubkeys.clone();
        let accounts = pubkeys
            .into_iter()
            .zip(
                rpc_client
                    .get_multiple_accounts_with_commitment(&copy, CommitmentConfig::finalized())
                    .await
                    .unwrap()
                    .value
                    .iter()
                    .map(|account| account.as_ref().unwrap().clone())
                    .collect::<Vec<_>>(),
            )
            .collect::<Vec<_>>();

        let mut base_pools: Vec<Arc<dyn DexPoolInterface>> = Vec::new();
        // if let Ok(accounts) = result {
        let all_base_pools = accounts
            .iter()
            .filter_map(
                |(key, account)| match AmmInfo::load_from_bytes(account.data.as_slice()) {
                    Ok(amm) => Some((key, amm)),
                    Err(e) => None,
                },
            )
            .filter(|(_, amm)| {
                // 过滤掉不能交换的池子
                AmmStatus::from_u64(amm.status).swap_permission()
                    // 过滤掉开启订单薄的池子
                    && !AmmStatus::from_u64(amm.status).orderbook_permission()
            })
            .filter(|(_, amm)| amm.coin_vault_mint != amm.pc_vault_mint)
            .filter(|(_, amm)| {
                // 过滤掉还未开启交易功能的池子
                amm.status != AmmStatus::WaitingTrade as u64
                    // 过滤掉还未达到开放时间的池子
                    || amm.state_data.pool_open_time < (Utc::now().timestamp() as u64)
            })
            .filter(|(_, amm)| {
                fetch_config.subscribe_mints.contains(&amm.coin_vault_mint)
                    && fetch_config.subscribe_mints.contains(&amm.pc_vault_mint)
            })
            .map(|(pool_id, amm)| {
                Arc::new(AmmPool {
                    pool_id: *pool_id,
                    mint_0_vault: amm.coin_vault,
                    mint_1_vault: amm.pc_vault,
                    mint_0: amm.coin_vault_mint,
                    mint_1: amm.pc_vault_mint,
                    ..Default::default()
                })
            })
            .collect::<Vec<_>>();
        for base_pool in all_base_pools {
            base_pools.push(base_pool);
        }
        // }
        Ok(Arc::new(RaydiumAmmDex {
            base_pool_map: base_pools,
        }))
    }
}

pub struct RaydiumAmmGrpcSubscriber();

impl RaydiumAmmGrpcSubscriber {
    async fn get_snapshot(
        rpc_url: String,
        snapshot_pool_accounts: Vec<(Pubkey, Pubkey, Pubkey)>,
        sender: UnboundedSender<Box<dyn DexPoolInterface>>,
    ) {
        for (index, chunks) in snapshot_pool_accounts.chunks(100).enumerate() {
            let rpc_url_clone = rpc_url.clone();
            let pool_chunks = chunks
                .iter()
                .flat_map(|(pool_id, mint_vault_0, mint_vault_1)| {
                    vec![*pool_id, *mint_vault_0, *mint_vault_1]
                })
                .collect::<Vec<_>>();
            let clone_sender = sender.clone();
            tokio_spawn(format!("fetch_snapshot_{}", index).as_str(), async move {
                let rpc_client = RpcClient::new(rpc_url_clone);
                let account_fetch_result = rpc_client
                    .get_multiple_accounts_with_commitment(
                        pool_chunks.as_slice(),
                        CommitmentConfig::finalized(),
                    )
                    .await;
                match account_fetch_result {
                    Ok(account_response) => {
                        for (single_account_chunks, single_account_key_chunks) in
                            account_response.value.chunks(3).zip(pool_chunks.chunks(3))
                        {
                            if single_account_chunks.iter().any(|t| t.is_none()) {
                                continue;
                            }
                            let amm_info = single_account_chunks[0]
                                .as_ref()
                                .map(|t| AmmInfo::load_from_bytes(t.data.as_slice()).unwrap())
                                .unwrap();
                            let mint_vaults = single_account_chunks[1..=2]
                                .iter()
                                .map(|t| {
                                    t.as_ref()
                                        .map(|t| {
                                            Account::unpack_from_slice(t.data.as_slice())
                                                .unwrap()
                                                .amount
                                        })
                                        .unwrap()
                                })
                                .collect::<Vec<_>>();
                            clone_sender
                                .send(Box::new(AmmPool::from((
                                    amm_info.to_owned(),
                                    single_account_key_chunks[0],
                                    mint_vaults[0],
                                    mint_vaults[1],
                                ))))
                                .expect("raydium amm info send error");
                        }
                    }
                    Err(e) => error!("raydium amm dex fetch account snaphot error,{}", e),
                }
            });
        }
    }
}

#[async_trait::async_trait]
impl GrpcSubscriber for RaydiumAmmGrpcSubscriber {
    async fn subscribe(
        dex: Arc<dyn DexInterface>,
        fetch_config: Arc<FetchConfig>,
        snapshot_write_sender: UnboundedSender<Box<dyn DexPoolInterface>>,
        trigger_route_sender: UnboundedSender<Box<dyn TriggerEvent>>,
    ) {
        let mut subscribe_pools = Vec::new();
        let mut mint_vault_account_map: HashMap<String, SubscribeRequestFilterAccounts> =
            HashMap::new();
        {
            let mut snapshot_accounts = Vec::new();
            let dex_pools = dex.get_base_pools();
            for base_pool in dex_pools {
                let mint_0_vault = base_pool.get_mint_0_vault().unwrap();
                let mint_1_vault = base_pool.get_mint_1_vault().unwrap();
                subscribe_pools.push(base_pool.get_pool_id().to_string());
                mint_vault_account_map.insert(
                    format!("{}:{}", base_pool.get_pool_id(), 0),
                    SubscribeRequestFilterAccounts {
                        account: vec![mint_0_vault.to_string()],
                        ..Default::default()
                    },
                );
                mint_vault_account_map.insert(
                    format!("{}:{}", base_pool.get_pool_id(), 1),
                    SubscribeRequestFilterAccounts {
                        account: vec![mint_1_vault.to_string()],
                        ..Default::default()
                    },
                );
                snapshot_accounts.push((base_pool.get_pool_id(), mint_0_vault, mint_1_vault));
            }
            // snapshot
            RaydiumAmmGrpcSubscriber::get_snapshot(
                fetch_config.rpc_url.clone(),
                snapshot_accounts,
                snapshot_write_sender,
            )
            .await;
        }
        let mut grpc_client = grpc_client().await.unwrap();
        let mut sub_accounts: HashMap<String, SubscribeRequestFilterAccounts> = HashMap::new();
        sub_accounts.insert(
            "raydium_amm_pool_sub".to_string(),
            SubscribeRequestFilterAccounts {
                account: subscribe_pools,
                ..Default::default()
            },
        );
        let pool_subscribe_request = SubscribeRequest {
            accounts: sub_accounts,
            commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
            accounts_data_slice: PoolUpdate::subscribe_request_data_slices(),
            ..Default::default()
        };

        let mint_vault_subscribe_request = SubscribeRequest {
            accounts: mint_vault_account_map,
            commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
            accounts_data_slice: MintVaultUpdate::subscribe_request_data_slices(),
            ..Default::default()
        };
        let (_, pool_sub_stream) = grpc_client
            .subscribe_with_request(Some(pool_subscribe_request))
            .await
            .unwrap();
        let (_, mint_vault_sub_stream) = grpc_client
            .subscribe_with_request(Some(mint_vault_subscribe_request))
            .await
            .unwrap();
        let mut subscrbeitions = StreamMap::new();
        subscrbeitions.insert(GrpcAccountUpdateType::PoolState as usize, pool_sub_stream);
        subscrbeitions.insert(
            GrpcAccountUpdateType::MintVault as usize,
            mint_vault_sub_stream,
        );
        let ping_request = SubscribeRequest {
            ping: Some(SubscribeRequestPing { id: 1 }),
            ..Default::default()
        };

        let mut trigger_event_holder = TriggerEventHolder::default();
        let mut clear_timeout_update_cache =
            tokio::time::interval(Duration::from_millis(1000 * 60 * 10));
        // 只保留最后一次
        clear_timeout_update_cache.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        clear_timeout_update_cache.tick().await;
        // grpc ping周期
        let mut ping_timeout = tokio::time::interval(Duration::from_secs(10));
        ping_timeout.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        ping_timeout.tick().await;
        let (account_update_sender, mut account_update_and_wait_receiver) =
            tokio::sync::mpsc::unbounded_channel::<SourceMessage>();
        loop {
            tokio::select! {
                Some((subscibe_type,Ok(data))) = subscrbeitions.next() => {
                    if let Some(UpdateOneof::Account(account)) = data.update_oneof {
                        let filter_name = &data.filters[0];
                        let account_info=account.account.unwrap();
                        let txn= account_info.txn_signature.as_ref().unwrap().to_base58();
                        let pubkey = &account_info.pubkey.to_base58();
                        info!("GPRC推送, txn : {:?}, account : {:?}, type : {:?}",txn,pubkey,subscibe_type);
                        if let Err(e) = account_update_sender
                        .send(SourceMessage::GrpcAccountUpdate(GrpcAccountUpdateType::from(subscibe_type),account_info,Utc::now().timestamp(),filter_name.clone())) {
                            error!("GRPC推送, 推送账户变更失败，原因: {}",e);
                        }
                    }
                },
                data = account_update_and_wait_receiver.recv()=>{
                    if let Some(message_type) = data {
                        match message_type {
                            SourceMessage::GrpcAccountUpdate(message_type, account_info,timestamp,filter_name) => {
                                let pubkey = Pubkey::try_from(account_info.pubkey.as_slice()).unwrap();
                                let txn = account_info.txn_signature.unwrap().to_base58();
                                info!("EventHolder接收数据, txn : {:?}, account : {:?}, type : {:?}",txn,pubkey,message_type);
                                let amm_trigger_event = match message_type {
                                    GrpcAccountUpdateType::PoolState => {
                                        AmmTriggerEvent{
                                            txn,
                                            pool_id:pubkey,
                                            pool_update: Some(PoolUpdate::from((pubkey,account_info.data))),
                                            timestamp,
                                            ..Default::default()
                                        }
                                    },
                                    GrpcAccountUpdateType::MintVault => {
                                        let filter_split = filter_name.split(":").take(2).collect::<Vec<_>>();
                                        let (mint_0_vault_update,mint_1_vault_update) = if filter_split[1].eq("0"){
                                            (Some(MintVaultUpdate::from((pubkey,account_info.data))),None)
                                        }else{
                                            (None,Some(MintVaultUpdate::from((pubkey,account_info.data))))
                                        };
                                        AmmTriggerEvent{
                                            txn,
                                            pool_id:Pubkey::from_str(filter_split[0]).unwrap(),
                                            mint_0_vault_update,
                                            mint_1_vault_update,
                                            timestamp,
                                            ..Default::default()
                                        }
                                    }
                                };
                                if let Some(event)=trigger_event_holder.fetch_event(Box::new(amm_trigger_event)){
                                    info!("EventHolder触发ruote, txn : {:?} , account : {:?}, data : {:?}",event.get_txn(),event.get_pool_id(),event);
                                    if let Err(e) = trigger_route_sender.send(event){
                                        error!("EventHolder触发ruote失败, 原因: {}",e);
                                    }
                                }
                            }
                        }
                    }
                },
                _ = clear_timeout_update_cache.tick() => {
                    info!("开始清理过期数据");
                    trigger_event_holder.clear_timeout_event(1000);
                    info!("开始清理过期数据");
                },
                _ = ping_timeout.tick() => {
                    if let Err(e)=grpc_client.ping(1).await{
                        error!("[Raydium Amm Dex]ping失败，{}",e);
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PoolUpdate {
    pub pool_id: Pubkey,
    pub mint_0: Pubkey,
    pub mint_1: Pubkey,
    /// 交易费率
    pub swap_fee_numerator: u64,
    pub swap_fee_denominator: u64,
    /// pnl
    pub mint_0_need_take_pnl: u64,
    pub mint_1_need_take_pnl: u64,
}

impl PoolUpdate {
    fn subscribe_request_data_slices() -> Vec<SubscribeRequestAccountsDataSlice> {
        vec![
            // fees.swap_fee_numerator
            SubscribeRequestAccountsDataSlice {
                offset: 176,
                length: 8,
            },
            // fees.swap_fee_denominator
            SubscribeRequestAccountsDataSlice {
                offset: 184,
                length: 8,
            },
            // state_data.need_take_pnl_coin
            SubscribeRequestAccountsDataSlice {
                offset: 192,
                length: 8,
            },
            // state_data.need_take_pnl_pc
            SubscribeRequestAccountsDataSlice {
                offset: 200,
                length: 8,
            },
            // coin_vault_mint
            SubscribeRequestAccountsDataSlice {
                offset: 400,
                length: 32,
            },
            // pc_vault_mint
            SubscribeRequestAccountsDataSlice {
                offset: 432,
                length: 32,
            },
        ]
    }
}

impl From<(Pubkey, Vec<u8>)> for PoolUpdate {
    fn from(value: (Pubkey, Vec<u8>)) -> Self {
        let src = array_ref![value.1.as_slice(), 0, 96];
        let (
            swap_fee_numerator,
            swap_fee_denominator,
            need_take_pnl_coin,
            need_take_pnl_pc,
            coin_vault_mint,
            pc_vault_mint,
        ) = array_refs![src, 8, 8, 8, 8, 32, 32];
        let swap_fee_numerator = u64::from_le_bytes(*swap_fee_numerator);
        let swap_fee_denominator = u64::from_le_bytes(*swap_fee_denominator);
        let mint_0_need_take_pnl = u64::from_le_bytes(*need_take_pnl_coin);
        let mint_1_need_take_pnl = u64::from_le_bytes(*need_take_pnl_pc);
        let mint_0 = Pubkey::from(*coin_vault_mint);
        let mint_1 = Pubkey::from(*pc_vault_mint);
        Self {
            pool_id: value.0,
            mint_0,
            mint_1,
            swap_fee_numerator,
            swap_fee_denominator,
            mint_0_need_take_pnl,
            mint_1_need_take_pnl,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct MintVaultUpdate {
    pub pubkey: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
}

impl MintVaultUpdate {
    fn subscribe_request_data_slices() -> Vec<SubscribeRequestAccountsDataSlice> {
        vec![
            // mint
            SubscribeRequestAccountsDataSlice {
                offset: 0,
                length: 32,
            },
            // amount
            SubscribeRequestAccountsDataSlice {
                offset: 64,
                length: 8,
            },
            // state
            SubscribeRequestAccountsDataSlice {
                offset: 108,
                length: 1,
            },
        ]
    }
}

impl From<(Pubkey, Vec<u8>)> for MintVaultUpdate {
    fn from(value: (Pubkey, Vec<u8>)) -> Self {
        let src = array_ref![value.1.as_slice(), 0, 41];
        let (mint, amount, _state) = array_refs![src, 32, 8, 1];
        // let _state = AccountState::try_from(state[0]).unwrap();
        Self {
            pubkey: value.0,
            mint: Pubkey::from(mint.to_owned()),
            amount: u64::from_le_bytes(*amount),
        }
    }
}

async fn grpc_client() -> anyhow::Result<GeyserGrpcClient<impl Interceptor>> {
    let mut builder =
        GeyserGrpcClient::build_from_static("https://solana-yellowstone-grpc.publicnode.com");
    builder = builder
        .tcp_nodelay(true)
        .http2_adaptive_window(true)
        .buffer_size(65536)
        .initial_connection_window_size(5242880)
        .initial_stream_window_size(4194304)
        .connect_timeout(Duration::from_millis(10 * 1000));
    builder.connect().await.map_err(|e| {
        error!("failed to connect: {e}");
        anyhow::anyhow!(e)
    })
}

#[derive(Default, Debug)]
pub(crate) struct AmmTriggerEvent {
    pub txn: String,
    pub pool_id: Pubkey,
    pub pool_update: Option<PoolUpdate>,
    pub mint_0_vault_update: Option<MintVaultUpdate>,
    pub mint_1_vault_update: Option<MintVaultUpdate>,
    pub timestamp: i64,
}

impl TriggerEvent for AmmTriggerEvent {
    fn update_and_return_ready_event(
        &mut self,
        push_event: Box<dyn TriggerEvent>,
    ) -> Option<Box<dyn TriggerEvent>> {
        let push_event = push_event.any().downcast_ref::<AmmTriggerEvent>().unwrap();
        if push_event.pool_update.is_some() {
            self.pool_update = push_event.pool_update;
        }
        if push_event.mint_0_vault_update.is_some() {
            self.mint_0_vault_update = push_event.mint_0_vault_update;
        }
        if push_event.mint_1_vault_update.is_some() {
            self.mint_1_vault_update = push_event.mint_1_vault_update;
        }
        if self.pool_update.is_some()
            && self.mint_0_vault_update.is_some()
            && self.mint_1_vault_update.is_some()
        {
            Some(Box::new(Self {
                txn: self.txn.clone(),
                pool_id: self.pool_id,
                pool_update: self.pool_update.clone(),
                mint_0_vault_update: self.mint_0_vault_update.clone(),
                mint_1_vault_update: self.mint_1_vault_update.clone(),
                timestamp: self.timestamp,
            }))
        } else {
            None
        }
    }

    fn get_txn(&self) -> String {
        self.txn.clone()
    }

    fn get_pool_id(&self) -> Pubkey {
        self.pool_id
    }

    fn get_create_timestamp(&self) -> i64 {
        self.timestamp
    }

    fn any(&self) -> &dyn Any {
        self
    }
}
