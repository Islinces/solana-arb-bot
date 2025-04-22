use crate::pump_fun_pool::PumpFunPool;
use crate::utils::deserialize_anchor_account;
use crate::GlobalConfig;
use anchor_spl::token::spl_token::state::Account;
use arrayref::array_refs;
use base58::ToBase58;
use chrono::Utc;
use dex::interface::{DexInterface, DexPoolInterface};
use dex::state::{FetchConfig, GrpcAccountUpdateType, SourceMessage};
use dex::subscribe_common::{GrpcClientCreator, MintVaultSubscribe, MintVaultUpdate};
use dex::trigger::{TriggerEvent, TriggerEventHolder};
use dex::util::tokio_spawn;
use futures_util::future::join_all;
use log::{error, info};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::program_pack::Pack;
use solana_program::pubkey::Pubkey;
use solana_sdk::commitment_config::CommitmentConfig;
use std::any::Any;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;
use tokio_stream::{StreamExt, StreamMap};
use yellowstone_grpc_client::GeyserGrpcClient;
use yellowstone_grpc_proto::geyser::subscribe_update::UpdateOneof;
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestFilterAccounts,
};
use yellowstone_grpc_proto::tonic::service::Interceptor;

pub struct PumpFunDex {
    pub pools: Vec<Arc<dyn DexPoolInterface>>,
}

impl PumpFunDex {
    fn global_config_pda() -> Pubkey {
        Pubkey::find_program_address(&[b"global_config"], &crate::ID).0
    }
}

impl PumpFunDex {}
#[async_trait::async_trait]
impl DexInterface for PumpFunDex {
    fn name(&self) -> String {
        "PumpFun".to_string()
    }

    fn get_base_pools(&self) -> Vec<Arc<dyn DexPoolInterface>> {
        self.pools.clone()
    }

    async fn fetch_pool_base_info(
        rpc_client: &RpcClient,
        fetch_config: &FetchConfig,
    ) -> anyhow::Result<Arc<dyn DexInterface>>
    where
        Self: Sized,
    {
        let pubkeys = vec![
            Pubkey::from_str("Gf7sXMoP8iRw4iiXmJ1nq4vxcRycbGXy5RL8a8LnTd3v").unwrap(),
            Pubkey::from_str("4rj6TSVxvRxzD5YiwZkLbpwbZAmLidQFnhiQbHCZVE6E").unwrap(),
            Pubkey::from_str("FBirun32CEFCQXeZCWjZVWzCs628Co3DqY3vV8u9Xhdz").unwrap(),
            Pubkey::from_str("DKMAXWcKCcwwxUNFeWmmKLG8KNuv9aLnfcFu8vFMNAP").unwrap(),
            Pubkey::from_str("3aSmj1W5gaRgnJdv7FUdKroVoUgUvM6beSF8wMacCTD8").unwrap(),
            Pubkey::from_str("8xpFcEgUxkTgGbs6pfJG4HM5MPu3gN31od7iBpt3TQWV").unwrap(),
            Pubkey::from_str("9YbiDakSvQjJFKktu5c9qZTS1vBGd9B4Vbi2cTnpBnW6").unwrap(),
            Pubkey::from_str("AJBKwFV9H4Ag7RpntdArfNNsqygxsMTcviSLiqE8BQkX").unwrap(),
            Pubkey::from_str("HKgz8ky2dGL8EepSLD9PbhCYHYc1fR193SW8HuhetBnq").unwrap(),
            Pubkey::from_str("Eyd2Jpm6mwjFXioxvhoKkg7mTvTz327hbNHtN6zmyG3T").unwrap(),
        ];
        let copy = pubkeys.clone();
        let accounts = pubkeys
            .into_iter()
            .zip(
                rpc_client
                    .get_multiple_accounts_with_commitment(&copy, CommitmentConfig::finalized())
                    .await?
                    .value,
            )
            .collect::<Vec<_>>();
        let mut base_pools: Vec<Arc<dyn DexPoolInterface>> = Vec::new();
        let all_base_pools = accounts
            .iter()
            .filter_map(|(key, account)| {
                if let Some(account) = account {
                    match deserialize_anchor_account::<crate::state::Pool>(account) {
                        Ok(pool) => Some((key, pool)),
                        Err(e) => None,
                    }
                } else {
                    None
                }
            })
            .filter(|(_, pool)| {
                fetch_config.subscribe_mints.contains(&pool.base_mint)
                    && fetch_config.subscribe_mints.contains(&pool.quote_mint)
            })
            .map(|(pool_id, pool)| {
                Arc::new(PumpFunPool {
                    pool_id: *pool_id,
                    mint_0_vault: pool.pool_base_token_account,
                    mint_1_vault: pool.pool_quote_token_account,
                    mint_0: pool.base_mint,
                    mint_1: pool.quote_mint,
                    ..Default::default()
                })
            })
            .collect::<Vec<_>>();
        for base_pool in all_base_pools {
            base_pools.push(base_pool);
        }
        Ok(Arc::new(PumpFunDex { pools: base_pools }))
    }
}

pub struct PumpFunGrpcSubscriber;

impl PumpFunGrpcSubscriber {
    async fn get_snapshot(
        rpc_url: String,
        dex: Arc<dyn DexInterface>,
        sender: UnboundedSender<Box<dyn DexPoolInterface>>,
    ) -> anyhow::Result<Vec<(Pubkey, Pubkey, Pubkey)>> {
        let dex_name = dex.name();
        let dex_pools = dex.get_base_pools();
        let mut snapshot_accounts = Vec::with_capacity(dex_pools.len());
        for base_pool in dex_pools {
            let mint_0_vault = base_pool.get_mint_0_vault().unwrap();
            let mint_1_vault = base_pool.get_mint_1_vault().unwrap();
            snapshot_accounts.push((base_pool.get_pool_id(), mint_0_vault, mint_1_vault));
        }
        drop(dex);
        info!("{} fetch snapshot start", dex_name);
        let mut snapshot_job = Vec::with_capacity(snapshot_accounts.len() / 100);
        let rpc_client = Arc::new(RpcClient::new_with_commitment(
            rpc_url,
            CommitmentConfig::finalized(),
        ));
        let global_config = match rpc_client
            .get_account(&PumpFunDex::global_config_pda())
            .await
        {
            Ok(account) => match deserialize_anchor_account::<GlobalConfig>(&account) {
                Ok(global_config) => global_config,
                Err(e) => {
                    return Err(anyhow::anyhow!(e));
                }
            },
            Err(e) => {
                return Err(anyhow::anyhow!(e));
            }
        };
        for (index, chunks) in snapshot_accounts.chunks(100).enumerate() {
            let pool_chunks = chunks
                .iter()
                .flat_map(|(pool_id, mint_vault_0, mint_vault_1)| {
                    vec![*pool_id, *mint_vault_0, *mint_vault_1]
                })
                .collect::<Vec<_>>();
            let clone_sender = sender.clone();
            let cloned_rpc_client = rpc_client.clone();
            let handle = tokio_spawn(format!("fetch_snapshot_{}", index).as_str(), async move {
                let mut snapshot_pool = Vec::with_capacity(100);
                let account_fetch_result = cloned_rpc_client
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
                            let pool_account = &single_account_chunks[0];
                            let pool = deserialize_anchor_account::<crate::state::Pool>(
                                pool_account.as_ref().unwrap(),
                            )
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
                            snapshot_pool.push((
                                single_account_key_chunks[0],
                                single_account_key_chunks[1],
                                single_account_key_chunks[2],
                            ));
                            clone_sender
                                .send(Box::new(PumpFunPool::from((
                                    single_account_key_chunks[0],
                                    pool.to_owned(),
                                    mint_vaults[0],
                                    mint_vaults[1],
                                    global_config.lp_fee_basis_points,
                                    global_config.protocol_fee_basis_points,
                                ))))
                                .expect("PumpFun发送snapshot失败");
                        }
                    }
                    Err(e) => {
                        error!("PumpFun Dex fetch account snaphot error,{}", e)
                    }
                };
                snapshot_pool
            });
            snapshot_job.push(handle);
        }
        let fetched_pools = join_all(snapshot_job)
            .await
            .into_iter()
            .flat_map(|t| t.unwrap())
            .collect::<Vec<_>>();
        info!("{} fetch snapshot finish", dex_name);
        Ok(fetched_pools)
    }

    pub async fn subscribe(
        dex: Arc<dyn DexInterface>,
        rpc_url: String,
        snapshot_sender: UnboundedSender<Box<dyn DexPoolInterface>>,
        trigger_event_sender: UnboundedSender<Box<dyn TriggerEvent>>,
    ) -> JoinHandle<()> {
        let pumpfun_need_subscribe_pools = PumpFunGrpcSubscriber::get_snapshot(
            rpc_url.to_string(),
            dex.clone(),
            snapshot_sender.clone(),
        )
        .await
        .unwrap();
        let dex_name = dex.name();
        tokio_spawn(
            format!("【PumpFun】{} grpc sub", dex_name).as_str(),
            async move {
                let mut grpc_client = GrpcClientCreator::create().await.unwrap();
                // 需要订阅的池子金库账户
                let (_, mint_vault_sub_stream) = grpc_client
                    .subscribe_with_request(Some(MintVaultSubscribe::create_subscribe_request(
                        pumpfun_need_subscribe_pools,
                        None,
                    )))
                    .await
                    .unwrap();
                let mut subscrbeitions = StreamMap::new();
                subscrbeitions.insert(
                    GrpcAccountUpdateType::MintVault as usize,
                    mint_vault_sub_stream,
                );
                // 事件holder和清理过期event的定时器
                let (mut trigger_event_holder, mut clear_timeout_update_cache) =
                    TriggerEventHolder::new_holder_with_expired_interval(
                        Some(1000 * 60 * 10),
                        None,
                        None,
                    );
                clear_timeout_update_cache.tick().await;
                // grpc ping周期
                let mut ping_interval = tokio::time::interval(Duration::from_secs(10));
                ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                let (account_update_sender, mut account_update_and_wait_receiver) =
                    tokio::sync::mpsc::unbounded_channel::<SourceMessage>();
                info!("【PumpFun】GRPC subscribe finished");
                loop {
                    tokio::select! {
                        Some((subscibe_type,Ok(data))) = subscrbeitions.next() => {
                            if let Some(UpdateOneof::Account(account)) = data.update_oneof {
                                let filter_name = &data.filters[0];
                                let account_info=account.account.unwrap();
                                let txn= account_info.txn_signature.as_ref().unwrap().to_base58();
                                let pubkey = &account_info.pubkey.to_base58();
                                info!("【PumpFun】GPRC推送, txn : {:?}, account : {:?}, type : {:?}",txn,pubkey,subscibe_type);
                                if let Err(e) = account_update_sender
                                .send(SourceMessage::GrpcAccountUpdate(GrpcAccountUpdateType::from(subscibe_type),account_info,Utc::now().timestamp(),filter_name.clone())) {
                                    error!("【PumpFun】GRPC推送, 推送账户变更失败，原因: {}",e);
                                }
                            }
                        },
                        data = account_update_and_wait_receiver.recv()=>{
                            if let Some(message_type) = data {
                                match message_type {
                                    SourceMessage::GrpcAccountUpdate(message_type, account_info,timestamp,filter_name) => {
                                        let pubkey = Pubkey::try_from(account_info.pubkey.as_slice()).unwrap();
                                        let txn = account_info.txn_signature.unwrap().to_base58();
                                        info!("【PumpFun】EventHolder接收数据, txn : {:?}, account : {:?}, type : {:?}",txn,pubkey,message_type);
                                        let trigger_event = match message_type {
                                            GrpcAccountUpdateType::MintVault => {
                                                if let (Some(pool_id),mint_0_vault_update,mint_1_vault_update) =
                                                    MintVaultUpdate::parse_mint_vault_by_filter_name(pubkey,account_info.data,filter_name){
                                                        Some(PumpFunTriggerEvent{
                                                            txn,
                                                            pool_id,
                                                            mint_0_vault_update,
                                                            mint_1_vault_update,
                                                            timestamp,
                                                        })
                                                    }else{
                                                        None
                                                    }
                                            },
                                            _=>{None}
                                        };
                                        if let Some(trigger_event)=trigger_event{
                                            if let Some(event)=trigger_event_holder.fetch_event(Box::new(trigger_event)){
                                                info!("【PumpFun】EventHolder触发ruote, txn : {:?} , account : {:?}, data : {:?}",event.get_txn(),event.get_pool_id(),event);
                                                if let Err(e) = trigger_event_sender.send(event){
                                                    error!("【PumpFun】EventHolder触发ruote失败, 原因: {}",e);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        _ = clear_timeout_update_cache.tick() => {
                            info!("【PumpFun】开始清理过期数据");
                            trigger_event_holder.clear_expired_event(1000);
                            info!("【PumpFun】开始清理过期数据");
                        },
                        _ = ping_interval.tick() => {
                            if let Err(e)=grpc_client.ping(1).await{
                                error!("【PumpFun】ping失败，{}",e);
                            }
                        }
                    }
                }
            },
        )
    }
}

#[derive(Debug)]
pub(crate) struct PumpFunTriggerEvent {
    pub txn: String,
    pub pool_id: Pubkey,
    pub mint_0_vault_update: Option<MintVaultUpdate>,
    pub mint_1_vault_update: Option<MintVaultUpdate>,
    pub timestamp: i64,
}

impl TriggerEvent for PumpFunTriggerEvent {
    fn update_and_return_ready_event(
        &mut self,
        push_event: Box<dyn TriggerEvent>,
    ) -> Option<Box<dyn TriggerEvent>> {
        let push_event = push_event
            .as_any()
            .downcast_ref::<PumpFunTriggerEvent>()
            .unwrap();
        if push_event.mint_0_vault_update.is_some() {
            self.mint_0_vault_update = push_event.mint_0_vault_update;
        }
        if push_event.mint_1_vault_update.is_some() {
            self.mint_1_vault_update = push_event.mint_1_vault_update;
        }
        if self.mint_0_vault_update.is_some() && self.mint_1_vault_update.is_some() {
            Some(Box::new(Self {
                txn: self.txn.clone(),
                pool_id: self.pool_id,
                mint_0_vault_update: self.mint_0_vault_update.take(),
                mint_1_vault_update: self.mint_1_vault_update.take(),
                timestamp: self.timestamp,
            }))
        } else {
            None
        }
    }

    fn get_dex(&self) -> &str {
        "PumpFun"
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

    fn as_any(&self) -> &dyn Any {
        self
    }
}
