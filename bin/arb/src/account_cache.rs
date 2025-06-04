use crate::data_slice::{slice_data, SliceType};
use crate::dex::byte_utils::read_from;
use crate::dex::FromCache;
use crate::dex_data::DexJson;
use crate::interface::{
    get_dex_type_with_program_id, AccountType, DexType, CLOCK_ID, MINT2022_PROGRAM_ID,
    MINT_PROGRAM_ID,
};
use ahash::{AHashMap, AHashSet, RandomState};
use anyhow::anyhow;
use dashmap::mapref::one::Ref;
use dashmap::DashMap;
use futures_util::future::ok;
use parking_lot::RwLock;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::account::accounts_equal;
use solana_sdk::address_lookup_table::state::AddressLookupTable;
use solana_sdk::address_lookup_table::AddressLookupTableAccount;
use solana_sdk::clock::Clock;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::SysvarId;
use spl_token_2022::extension::transfer_fee::TransferFeeConfig;
use spl_token_2022::extension::{BaseStateWithExtensions, StateWithExtensions};
use std::collections::hash_map::Entry;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tokio::task::JoinSet;
use tracing::{error, info, warn};

static DYNAMIC_ACCOUNT_CACHE: OnceCell<DynamicCache> = OnceCell::const_new();
static STATIC_ACCOUNT_CACHE: OnceCell<RwLock<StaticCache>> = OnceCell::const_new();
static ALT_CACHE: OnceCell<RwLock<AltCache>> = OnceCell::const_new();

#[derive(Debug)]
pub struct DynamicCache(DashMap<Pubkey, Vec<u8>, RandomState>);
#[derive(Debug)]
pub struct StaticCache(AHashMap<Pubkey, Vec<u8>>);
#[derive(Debug)]
pub struct AltCache(AHashMap<Pubkey, Vec<AddressLookupTableAccount>>);

impl DynamicCache {
    pub(crate) fn new(capacity: usize) -> Self {
        Self(DashMap::with_capacity_and_hasher_and_shard_amount(
            capacity,
            RandomState::default(),
            128,
        ))
    }

    pub fn get(&self, account_key: &Pubkey) -> Option<Ref<Pubkey, Vec<u8>>> {
        self.0.get(account_key).map_or(None, |v| Some(v))
    }

    pub fn insert(&self, account_key: Pubkey, data: Vec<u8>) -> Option<Vec<u8>> {
        self.0.insert(account_key, data)
    }
}

impl StaticCache {
    pub(crate) fn new() -> Self {
        Self(AHashMap::with_capacity(1_000))
    }

    pub fn get(&self, account_key: &Pubkey) -> Option<&[u8]> {
        self.0.get(account_key).map_or(None, |v| Some(v.as_slice()))
    }

    pub fn insert(&mut self, account_key: Pubkey, data: Vec<u8>) {
        self.0.insert(account_key, data);
    }
}

impl AltCache {
    pub(crate) fn new() -> Self {
        Self(AHashMap::with_capacity(1_000))
    }

    pub fn get(&self, pool_id: &Pubkey) -> Option<Vec<AddressLookupTableAccount>> {
        self.0.get(pool_id).map_or(None, |v| Some(v.clone()))
    }

    pub fn insert(&mut self, pool_id: Pubkey, alts: Vec<AddressLookupTableAccount>) {
        if !alts.is_empty() {
            self.0.insert(pool_id, alts);
        }
    }
}

pub async fn init_snapshot(
    dex_data: Vec<DexJson>,
    rpc_client: Arc<RpcClient>,
) -> anyhow::Result<Vec<DexJson>> {
    info!("初始化Snapshot...");
    let mut dex_data_group: AHashMap<DexType, Vec<DexJson>> = AHashMap::with_capacity(4);
    for json in dex_data {
        if let Some(dex_type) = get_dex_type_with_program_id(&json.owner) {
            match dex_data_group.entry(dex_type) {
                Entry::Occupied(mut a) => a.get_mut().push(json),
                Entry::Vacant(a) => {
                    a.insert(vec![json]);
                }
            }
        }
    }
    if dex_data_group.is_empty() {
        return Err(anyhow::anyhow!("dex_json文件无匹配数据"));
    }
    let mut effective_dex_data = Vec::with_capacity(dex_data_group.len());
    DYNAMIC_ACCOUNT_CACHE.set(DynamicCache::new(10_000))?;
    STATIC_ACCOUNT_CACHE.set(RwLock::new(StaticCache::new()))?;
    ALT_CACHE.set(RwLock::new(AltCache::new()))?;
    for (dex_type, mut dex_data) in dex_data_group {
        info!("【{}】开始初始化Snapshot...", dex_type);
        let mut pool_accounts = Vec::with_capacity(dex_data.len());
        let mut vault_accounts = Vec::with_capacity(dex_data.len() * 2);
        for json in dex_data.iter() {
            pool_accounts.push(json.pool);
            vault_accounts.push(json.vault_a);
            vault_accounts.push(json.vault_b);
        }
        // 剩余有效的dex data，用于订阅使用
        let snapshot_data = match dex_type {
            DexType::RaydiumAMM => {
                crate::dex::raydium_amm::cache_init::init_cache(&mut dex_data, rpc_client.clone())
                    .await
            }
            DexType::RaydiumCLMM => {
                crate::dex::raydium_clmm::cache_init::init_cache(&mut dex_data, rpc_client.clone())
                    .await
            }
            DexType::PumpFunAMM => {
                crate::dex::pump_fun::cache_init::init_cache(&mut dex_data, rpc_client.clone())
                    .await
            }
            DexType::MeteoraDLMM => {
                crate::dex::meteora_dlmm::cache_init::init_cache(&mut dex_data, rpc_client.clone())
                    .await
            }
            _ => {
                vec![]
            }
        };
        for account in snapshot_data {
            let account_key = account.account_key;
            account.static_slice_data.and_then(|data| {
                STATIC_ACCOUNT_CACHE
                    .get()
                    .unwrap()
                    .write()
                    .0
                    .insert(account_key, data)
            });
            account.dynamic_slice_data.and_then(|data| {
                DYNAMIC_ACCOUNT_CACHE
                    .get()
                    .unwrap()
                    .insert(account_key, data)
            });
        }
        // 有效alt
        let alts = load_lookup_table_accounts(
            rpc_client.clone(),
            dex_data
                .iter()
                .filter_map(|v| v.address_lookup_table_address)
                .collect::<Vec<_>>()
                .as_slice(),
        )
        .await;
        dex_data.iter().for_each(|json| {
            if let Some(address) = json.address_lookup_table_address {
                ALT_CACHE.get().unwrap().write().insert(
                    json.pool,
                    alts.iter()
                        .filter_map(|alt| {
                            if alt.key == address {
                                Some(alt.clone())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>(),
                );
            }
        });
        info!(
            "【{}】初始化Snapshot完毕, 初始化池子数量 : {}",
            dex_type,
            dex_data.len()
        );
        effective_dex_data.extend(dex_data);
    }
    init_clock(effective_dex_data.as_slice(), rpc_client.clone()).await;
    init_token_2022(effective_dex_data.as_slice(), rpc_client.clone()).await;
    info!("初始化Snapshot结束, 数量 : {}", effective_dex_data.len());
    if effective_dex_data.is_empty() {
        Err(anyhow!("所有DexJson均加载失败"))
    } else {
        Ok(effective_dex_data)
    }
}

async fn init_clock(effective_dex_data: &[DexJson], rpc_client: Arc<RpcClient>) {
    if effective_dex_data
        .iter()
        .any(|json| &json.owner == DexType::MeteoraDLMM.get_ref_program_id())
    {
        let clock_data = rpc_client
            .clone()
            .get_account_data(&CLOCK_ID)
            .await
            .unwrap();
        DYNAMIC_ACCOUNT_CACHE
            .get()
            .unwrap()
            .insert(CLOCK_ID, clock_data);
    }
}

async fn init_token_2022(effective_dex_data: &[DexJson], rpc_client: Arc<RpcClient>) {
    let all_tokens = effective_dex_data
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
                            AccountDataSlice::new(account_key, None, Some(acc.data))
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
        .filter(|account| account.dynamic_slice_data.is_some())
        .collect::<Vec<_>>();
    info!("Token2022加载完毕，数量 : {}", token_2022_accounts.len());
    token_2022_accounts.into_iter().for_each(|account| {
        STATIC_ACCOUNT_CACHE
            .get()
            .unwrap()
            .write()
            .insert(account.account_key, account.dynamic_slice_data.unwrap());
    })
}

pub async fn get_account_data_with_data_slice(
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
            base_get_account_with_data_slice(rpc_client, account_chunks, dex_type, account_type)
                .await
        });
    }
    join_set.join_all().await.into_iter().flatten().collect()
}

async fn load_lookup_table_accounts(
    rpc_client: Arc<RpcClient>,
    all_alts: &[Pubkey],
) -> Vec<AddressLookupTableAccount> {
    let mut join_set = JoinSet::new();
    for alts in all_alts.chunks(100) {
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
                                error!("   Failed to deserialize lookup table {}: {}", pubkey, e);
                                None
                            }
                        },
                    })
                    .collect::<Vec<_>>(),
                Err(_) => vec![],
            }
        });
    }
    join_set
        .join_all()
        .await
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
}

async fn base_get_account_with_data_slice(
    rpc_client: Arc<RpcClient>,
    accounts: Vec<Pubkey>,
    dex_type: DexType,
    account_type: AccountType,
) -> Vec<AccountDataSlice> {
    rpc_client
        .get_multiple_accounts_with_commitment(accounts.as_slice(), CommitmentConfig::finalized())
        .await
        .unwrap()
        .value
        .into_iter()
        .zip(accounts)
        .map(|(account, account_key)| {
            account.map_or(AccountDataSlice::new(account_key, None, None), |acc| {
                let dynamic_data = slice_data(
                    dex_type.clone(),
                    account_type.clone(),
                    acc.data.clone(),
                    SliceType::Subscribed,
                )
                .map_or(None, |v| Some(v));
                let static_data = slice_data(
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
}

pub fn get_account_data<T: FromCache>(account_key: &Pubkey) -> Option<T> {
    let static_data = STATIC_ACCOUNT_CACHE.get()?.read();
    let dynamic_data = DYNAMIC_ACCOUNT_CACHE.get()?;
    T::from_cache(account_key, static_data, dynamic_data)
}

pub fn get_token_program(mint: &Pubkey) -> Pubkey {
    if STATIC_ACCOUNT_CACHE
        .get()
        .unwrap()
        .read()
        .0
        .contains_key(mint)
    {
        MINT2022_PROGRAM_ID
    } else {
        MINT_PROGRAM_ID
    }
}

pub fn get_token2022_data(mint_key: &Pubkey) -> Option<TransferFeeConfig> {
    let static_data = STATIC_ACCOUNT_CACHE.get()?.read();
    static_data.0.get(mint_key).map_or(None, |data| {
        StateWithExtensions::<spl_token_2022::state::Mint>::unpack(data.as_slice()).map_or(
            None,
            |mint_extensions| {
                mint_extensions
                    .get_extension::<TransferFeeConfig>()
                    .map_or(None, |result| Some(result.clone()))
            },
        )
    })
}

pub fn get_clock() -> Option<Clock> {
    DYNAMIC_ACCOUNT_CACHE
        .get()?
        .get(&CLOCK_ID)
        .map_or(None, |result| {
            let clock_data = result.value().as_slice();
            Some(unsafe { read_from::<Clock>(clock_data) })
        })
}

pub fn get_alt(pool_id: &Pubkey) -> Option<Vec<AddressLookupTableAccount>> {
    ALT_CACHE.get()?.read().get(pool_id)
}

pub fn update_cache(account_key: Pubkey, data: Vec<u8>) -> anyhow::Result<()> {
    DYNAMIC_ACCOUNT_CACHE
        .get()
        .map_or(Err(anyhow!("")), |cache| {
            cache.insert(account_key, data);
            Ok(())
        })
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
