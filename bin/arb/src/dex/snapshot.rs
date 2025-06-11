use crate::dex::meteora_dlmm::MeteoraDLMMSnapshotInitializer;
use crate::dex::orca_whirlpools::OrcaWhirlpoolsSnapshotInitializer;
use crate::dex::pump_fun::PumpFunAMMSnapshotInitializer;
use crate::dex::raydium_amm::state::AmmInfo;
use crate::dex::raydium_amm::RaydiumAmmSnapshotInitializer;
use crate::dex::raydium_clmm::RaydiumCLMMSnapshotInitializer;
use crate::dex::{AccountType, DexType, CLOCK_ID};
use crate::dex_data::DexJson;
use crate::dex::global_cache::GlobalCache;
use ahash::AHashSet;
use anyhow::anyhow;
use async_trait::async_trait;
use enum_dispatch::enum_dispatch;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::address_lookup_table::state::AddressLookupTable;
use solana_sdk::address_lookup_table::AddressLookupTableAccount;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use spl_token_2022::extension::transfer_fee::TransferFeeConfig;
use spl_token_2022::extension::{BaseStateWithExtensions, StateWithExtensions};
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{error, info};
use crate::dex::data_slice::{try_slice_data, SliceType};

#[async_trait]
#[enum_dispatch(SnapshotType)]
pub trait SnapshotInitializer {
    async fn init_snapshot(
        &self,
        dex_json: &mut Vec<DexJson>,
        rpc_client: Arc<RpcClient>,
    ) -> Vec<AccountDataSlice>;

    fn print_snapshot(&self, dex_json: &[DexJson]) -> anyhow::Result<()>;

    async fn get_account_data_with_data_slice(
        &self,
        accounts: Vec<Pubkey>,
        dex_type: DexType,
        account_type: AccountType,
        rpc_client: Arc<RpcClient>,
    ) -> Vec<AccountDataSlice> {
        if accounts.is_empty() {
            return vec![];
        }
        let mut join_set = JoinSet::new();
        for account_chunks in accounts.chunks(100) {
            let rpc_client = rpc_client.clone();
            let dex_type = dex_type.clone();
            let account_type = account_type.clone();
            let account_chunks = account_chunks.to_vec();
            join_set.spawn(async move {
                rpc_client
                    .get_multiple_accounts_with_commitment(
                        account_chunks.as_slice(),
                        CommitmentConfig::finalized(),
                    )
                    .await
                    .unwrap()
                    .value
                    .into_iter()
                    .zip(account_chunks)
                    .map(|(account, account_key)| {
                        account.map_or(AccountDataSlice::new(account_key, None, None), |acc| {
                            let dynamic_data = try_slice_data(
                                dex_type.clone(),
                                account_type.clone(),
                                acc.data.clone(),
                                SliceType::Subscribed,
                            )
                            .map_or(None, |v| Some(v));
                            let static_data = try_slice_data(
                                dex_type.clone(),
                                account_type.clone(),
                                acc.data,
                                SliceType::Unsubscribed,
                            )
                            .map_or(None, |v| Some(v));
                            AccountDataSlice::new(account_key, static_data, dynamic_data)
                        })
                    })
                    .collect::<Vec<_>>()
            });
        }
        join_set.join_all().await.into_iter().flatten().collect()
    }
}

#[enum_dispatch]
pub enum SnapshotType {
    MeteoraDLMM(MeteoraDLMMSnapshotInitializer),
    OrcaWhirl(OrcaWhirlpoolsSnapshotInitializer),
    PumpFunAMM(PumpFunAMMSnapshotInitializer),
    RaydiumAmm(RaydiumAmmSnapshotInitializer),
    RaydiumCLMM(RaydiumCLMMSnapshotInitializer),
}

pub async fn init_snapshot(
    dex_data: &mut Vec<DexJson>,
    rpc_client: Arc<RpcClient>,
    cache: &'static GlobalCache,
) -> anyhow::Result<()> {
    info!("开始初始化Snapshot...");
    for snapshot in vec![
        SnapshotType::from(MeteoraDLMMSnapshotInitializer),
        SnapshotType::from(OrcaWhirlpoolsSnapshotInitializer),
        SnapshotType::from(PumpFunAMMSnapshotInitializer),
        SnapshotType::from(RaydiumAmmSnapshotInitializer),
        SnapshotType::from(RaydiumCLMMSnapshotInitializer),
    ] {
        let accounts: Vec<AccountDataSlice> =
            snapshot.init_snapshot(dex_data, rpc_client.clone()).await;
        // 缓存账户
        accounts.into_iter().for_each(|account| {
            account
                .static_slice_data
                .and_then(|data| cache.upsert_static(account.account_key, data));
            account
                .dynamic_slice_data
                .and_then(|data| cache.upsert_dynamic(account.account_key, data));
        })
    }
    // 加载alt
    cache_lookup_table_accounts(dex_data.as_slice(), rpc_client.clone(), &cache).await;
    // 加载token2022
    cache_token_2022(dex_data.as_slice(), rpc_client.clone(), &cache).await;
    // 加载clock
    cache_clock(dex_data.as_slice(), rpc_client.clone(), &cache).await;
    info!("初始化Snapshot结束, 数量 : {}", dex_data.len());
    #[cfg(feature = "print_slice_data")]
    print_slice_data(dex_data);
    if dex_data.is_empty() {
        Err(anyhow!("所有DexJson均加载失败"))
    } else {
        Ok(())
    }
}

fn print_slice_data(dex_json: &[DexJson]) {
    vec![
        SnapshotType::from(MeteoraDLMMSnapshotInitializer),
        SnapshotType::from(OrcaWhirlpoolsSnapshotInitializer),
        SnapshotType::from(PumpFunAMMSnapshotInitializer),
        SnapshotType::from(RaydiumAmmSnapshotInitializer),
        SnapshotType::from(RaydiumCLMMSnapshotInitializer),
    ]
    .into_iter()
    .for_each(|snapshot| {
        snapshot.print_snapshot(dex_json).unwrap();
    })
}

async fn cache_lookup_table_accounts(
    dex_data: &[DexJson],
    rpc_client: Arc<RpcClient>,
    cache: &GlobalCache,
) {
    let alts_keys = dex_data
        .iter()
        .filter_map(|json| json.address_lookup_table_address)
        .collect::<Vec<_>>();
    let mut join_set = JoinSet::new();
    for alts in alts_keys.chunks(100) {
        let alts = alts.to_vec();
        let rpc_client = rpc_client.clone();
        join_set.spawn(async move {
            match rpc_client.get_multiple_accounts(alts.as_slice()).await {
                Ok(alt_accounts) => alt_accounts
                    .into_iter()
                    .zip(alts)
                    .flat_map(|(account, pubkey)| match account {
                        None => None,
                        Some(account) => match AddressLookupTable::deserialize(&account.data) {
                            Ok(lookup_table) => Some(AddressLookupTableAccount {
                                key: pubkey,
                                addresses: lookup_table.addresses.into_owned(),
                            }),
                            Err(e) => {
                                error!("Failed to deserialize lookup table {}: {}", pubkey, e);
                                None
                            }
                        },
                    })
                    .collect::<Vec<_>>(),
                Err(_) => vec![],
            }
        });
    }
    let alt_accounts = join_set
        .join_all()
        .await
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    for json in dex_data {
        if let Some(address) = json.address_lookup_table_address {
            let pool_alts = alt_accounts
                .iter()
                .filter_map(|a| {
                    if a.key == address {
                        Some(a.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            if !pool_alts.is_empty() {
                cache.upsert_alt(json.pool, pool_alts);
            }
        }
    }
}

async fn cache_token_2022(dex_data: &[DexJson], rpc_client: Arc<RpcClient>, cache: &GlobalCache) {
    let all_tokens = dex_data
        .iter()
        .flat_map(|json| vec![json.mint_a, json.mint_b])
        .collect::<AHashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut join_set = JoinSet::new();
    for account_chunks in all_tokens.chunks(100) {
        let rpc_client = rpc_client.clone();
        let account_chunks = account_chunks.to_vec();
        join_set.spawn(async move {
            rpc_client
                .get_multiple_accounts_with_commitment(
                    account_chunks.as_slice(),
                    CommitmentConfig::finalized(),
                )
                .await
                .unwrap()
                .value
                .into_iter()
                .zip(account_chunks)
                .map(|(account, account_key)| {
                    account.map_or(AccountDataSlice::new(account_key, None, None), |acc| {
                        let token_2022 = if let Ok(mint_extensions) =
                            StateWithExtensions::<spl_token_2022::state::Mint>::unpack(
                                acc.data.as_ref(),
                            ) {
                            mint_extensions
                                .get_extension::<TransferFeeConfig>()
                                .map_or(None, |_| Some(()))
                        } else {
                            None
                        };
                        if token_2022.is_some() {
                            AccountDataSlice::new(account_key, Some(acc.data), None)
                        } else {
                            AccountDataSlice::new(account_key, None, None)
                        }
                    })
                })
                .collect::<Vec<_>>()
        });
    }
    let token_2022_accounts = join_set
        .join_all()
        .await
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .into_iter()
        .filter(|account| account.static_slice_data.is_some())
        .collect::<Vec<_>>();
    info!("Token2022加载完毕，数量 : {}", token_2022_accounts.len());
    token_2022_accounts.into_iter().for_each(|account| {
        cache.upsert_static(account.account_key, account.static_slice_data.unwrap());
    })
}

async fn cache_clock(dex_data: &[DexJson], rpc_client: Arc<RpcClient>, cache: &GlobalCache) {
    if dex_data.iter().any(|json| {
        &json.owner == DexType::MeteoraDLMM.get_ref_program_id()
            || &json.owner == DexType::OrcaWhirl.get_ref_program_id()
    }) {
        let clock_data = rpc_client
            .clone()
            .get_account_data(&CLOCK_ID)
            .await
            .unwrap();
        cache.upsert_dynamic(CLOCK_ID, clock_data);
    }
}

#[derive(Clone, Debug)]
pub struct AccountDataSlice {
    pub account_key: Pubkey,
    pub static_slice_data: Option<Vec<u8>>,
    pub dynamic_slice_data: Option<Vec<u8>>,
}

impl AccountDataSlice {
    pub fn new(
        account_key: Pubkey,
        static_slice_data: Option<Vec<u8>>,
        dynamic_slice_data: Option<Vec<u8>>,
    ) -> Self {
        Self {
            account_key,
            static_slice_data,
            dynamic_slice_data,
        }
    }
}
