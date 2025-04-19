use crate::amm_pool::AmmPool;
use crate::state::{AmmInfo, AmmStatus, Fees, Loadable, StateData};
use arrayref::{array_ref, array_refs};
use base58::ToBase58;
use chrono::Utc;
use dex::interface::{DexInterface, DexPoolInterface, GrpcSubscriber};
use dex::state::{FetchConfig, GrpcAccountUpdateType, SourceMessage};
use dex::util::tokio_spawn;
use futures_util::future::ok;
use futures_util::stream::once;
use log::{error, info};
use solana_account_decoder::UiAccountEncoding;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use solana_client::rpc_filter::RpcFilterType;
use solana_program::program_pack::Pack;
use solana_program::pubkey::Pubkey;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey;
use spl_token::state::{Account, AccountState};
use std::any::Any;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use tonic::codec::CompressionEncoding;
use yellowstone_grpc_client::{GeyserGrpcClient, Interceptor};
use yellowstone_grpc_proto::geyser::geyser_client::GeyserClient;
use yellowstone_grpc_proto::geyser::subscribe_update::UpdateOneof;
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestAccountsDataSlice,
    SubscribeRequestFilterAccounts, SubscribeUpdateAccount, SubscribeUpdateAccountInfo,
};
use yellowstone_grpc_proto::tonic::codegen::tokio_stream::{StreamExt, StreamMap};
use yellowstone_grpc_proto::tonic::transport::Channel;
use yellowstone_grpc_proto::tonic::Request;

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
        ];
        let accounts = pubkeys
            .into_iter()
            .zip(
                rpc_client
                    .get_multiple_accounts_with_commitment(
                        &[
                            Pubkey::from_str("5oAvct85WyF7Sj73VYHbyFJkdRJ28D8m4z4Sxjvzuc6n")
                                .unwrap(),
                            Pubkey::from_str("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2")
                                .unwrap(),
                        ],
                        CommitmentConfig::finalized(),
                    )
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
        trigger_route_sender: UnboundedSender<Box<dyn DexPoolInterface>>,
    ) {
        let dex_pools = dex.get_base_pools();
        let mut snapshot_accounts = Vec::with_capacity(dex_pools.len());
        let mut subscribe_pools = Vec::with_capacity(dex_pools.len());
        let mut subscribe_vaults = Vec::with_capacity(dex_pools.len() * 2);
        let mut mint_vault_to_pool_map = HashMap::with_capacity(dex_pools.len() * 2);
        {
            for base_pool in dex_pools {
                let mint_0_vault = base_pool.get_mint_0_vault().unwrap();
                let mint_1_vault = base_pool.get_mint_1_vault().unwrap();
                subscribe_pools.push(base_pool.get_pool_id().to_string());
                subscribe_vaults.push(mint_0_vault.to_string());
                subscribe_vaults.push(mint_1_vault.to_string());
                snapshot_accounts.push((base_pool.get_pool_id(), mint_0_vault, mint_1_vault));
                mint_vault_to_pool_map
                    .insert(base_pool.get_mint_0_vault(), base_pool.get_pool_id());
                mint_vault_to_pool_map
                    .insert(base_pool.get_mint_1_vault(), base_pool.get_pool_id());
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
        let mut mint_vault_account_map: HashMap<String, SubscribeRequestFilterAccounts> =
            HashMap::new();
        mint_vault_account_map.insert(
            "raydium_amm_mint_vault_sub".to_string(),
            SubscribeRequestFilterAccounts {
                account: subscribe_vaults,
                ..Default::default()
            },
        );
        let mint_vault_subscribe_request = SubscribeRequest {
            accounts: mint_vault_account_map,
            commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
            accounts_data_slice: MintVaultUpdate::subscribe_request_data_slices(),
            ..Default::default()
        };
        // let mut pool_sub_stream = grpc_client
        //     .subscribe(once(async move { pool_subscribe_request }))
        //     .await
        //     .unwrap()
        //     .into_inner();
        // let mut mint_vault_sub_stream = grpc_client
        //     .subscribe(once(async move { mint_vault_subscribe_request }))
        //     .await
        //     .unwrap()
        //     .into_inner();
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

        let (account_update_sender, mut account_update_and_wait_receiver) =
            tokio::sync::mpsc::unbounded_channel::<SourceMessage>();
        let mut wait_triger_pool_cache: HashMap<String, PoolUpdate> = HashMap::new();
        let mut wait_triger_vault_cache: HashMap<
            String,
            (Option<MintVaultUpdate>, Option<MintVaultUpdate>),
        > = HashMap::new();
        loop {
            tokio::select! {
                Some((key,Ok(data))) = subscrbeitions.next() => {
                        if let Some(UpdateOneof::Account(account)) = data.update_oneof {
                            let account_info=account.account.unwrap();
                            match account_update_sender.send(SourceMessage::GrpcAccountUpdate(GrpcAccountUpdateType::from(key),account_info)) {
                                Ok(_)=>{},
                                Err(e)=>{
                                    error!("raydium amm dex send pool state update error:{}",e);
                                }
                            }
                        }
                },
                data = account_update_and_wait_receiver.recv()=>{
                    if let Some(account_type) = data {
                        match account_type {
                            SourceMessage::GrpcAccountUpdate(account_type, account_info) => {
                                let pubkey = Pubkey::try_from(account_info.pubkey.as_slice()).unwrap();
                                let txn = account_info.txn_signature.unwrap().to_base58();
                                match account_type {
                                    GrpcAccountUpdateType::PoolState => {
                                        let pool_update = PoolUpdate::from((pubkey,account_info.data));
                                        match wait_triger_pool_cache.entry(txn.clone()) {
                                            std::collections::hash_map::Entry::Occupied(entry) => {
                                                error!("raydium amm dex pool state update error:{}",txn);
                                            },
                                            std::collections::hash_map::Entry::Vacant(entry) => {
                                                if let Some((mint_vault_amount_0, mint_vault_amount_1))
                                                    = wait_triger_vault_cache.get(&txn) {
                                                    if mint_vault_amount_0.is_some() && mint_vault_amount_1.is_some() {
                                                        let (mint_0_vault_amount,mint_1_vault_amount)=
                                                        if mint_vault_amount_0.as_ref().unwrap().mint==pool_update.mint_0{
                                                            (mint_vault_amount_0.as_ref().unwrap().amount,mint_vault_amount_1.as_ref().unwrap().amount)
                                                        }else{
                                                            (mint_vault_amount_1.as_ref().unwrap().amount,mint_vault_amount_0.as_ref().unwrap().amount)
                                                        };
                                                        let mut amm_pool=AmmPool::from(pool_update);
                                                        amm_pool.mint_0_vault_amount=mint_0_vault_amount;
                                                        amm_pool.mint_1_vault_amount=mint_1_vault_amount;
                                                        info!("Raydium Amm触发swap[PoolState]，txn : {:?}, amm_pool : {:?}",txn,amm_pool);
                                                        if let Err(e) = trigger_route_sender.send(Box::new(amm_pool)) {
                                                           error!("[Raydium Amm Dex]触发swap失败，{}",e);
                                                        }
                                                    } else {
                                                        info!("Raydium Amm接收，txn : {:?}, PoolUpdate : {:?}",txn,pool_update);
                                                        entry.insert(pool_update);
                                                    }
                                                }else{
                                                    info!("Raydium Amm接收，txn : {:?}, PoolUpdate : {:?}",txn,pool_update);
                                                    entry.insert(pool_update);
                                                }
                                            }
                                        }
                                    },
                                    GrpcAccountUpdateType::MintVault=>{
                                        let mint_vault_update = MintVaultUpdate::from((pubkey,account_info.data));
                                        match wait_triger_vault_cache.entry(txn.clone()){
                                            std::collections::hash_map::Entry::Occupied( mint_vault_entry) => {
                                                if let std::collections::hash_map::Entry::Occupied(pool_entry) = wait_triger_pool_cache.entry(txn.clone()){
                                                    let  (_,mut pool_update) = pool_entry.remove_entry();
                                                    let (mint_0_vault_amount,mint_1_vault_amount)=if pool_update.mint_0==mint_vault_update.mint{
                                                        (mint_vault_update.amount,mint_vault_entry.get().0.as_ref().unwrap().amount)
                                                    }else{
                                                         (mint_vault_entry.get().0.as_ref().unwrap().amount,mint_vault_update.amount)
                                                    };
                                                    let mut amm_pool=AmmPool::from(pool_update);
                                                    amm_pool.mint_0_vault_amount=mint_0_vault_amount;
                                                    amm_pool.mint_1_vault_amount=mint_1_vault_amount;
                                                    info!("Raydium Amm触发swap[MintVault]，txn : {:?}, amm_pool : {:?}",txn,amm_pool);
                                                    if let Err(e) = trigger_route_sender.send(Box::new(amm_pool)) {
                                                        error!("[Raydium Amm Dex]触发swap失败，{}",e);
                                                    }
                                                }
                                            },
                                            std::collections::hash_map::Entry::Vacant(mint_vault_entry) => {
                                                info!("Raydium Amm接收，txn : {:?}, MintVaultUpdate : {:?}",txn,mint_vault_update);
                                                mint_vault_entry.insert((Some(mint_vault_update),None));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct PoolUpdate {
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

#[derive(Debug)]
pub(crate) struct MintVaultUpdate {
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
