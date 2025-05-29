use crate::data_slice::{slice_data, SliceType};
use crate::dex::FromCache;
use crate::dex_data::DexJson;
use crate::interface::{get_dex_type_with_program_id, AccountType, DexType};
use ahash::{AHashMap, RandomState};
use anyhow::anyhow;
use dashmap::mapref::one::Ref;
use dashmap::DashMap;
use parking_lot::RwLock;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::address_lookup_table::state::AddressLookupTable;
use solana_sdk::address_lookup_table::AddressLookupTableAccount;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
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
}

impl StaticCache {
    pub(crate) fn new() -> Self {
        Self(AHashMap::with_capacity(1_000))
    }

    pub fn get(&self, account_key: &Pubkey) -> Option<&[u8]> {
        self.0.get(account_key).map_or(None, |v| Some(v.as_slice()))
    }
}

impl AltCache {
    pub(crate) fn new() -> Self {
        Self(AHashMap::with_capacity(1_000))
    }

    pub fn get(&self, pool_id: &Pubkey) -> Option<Vec<AddressLookupTableAccount>> {
        self.0.get(pool_id).map_or(None, |v| Some(v.clone()))
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
        // 有效alt
        let alt_map = load_lookup_table_accounts(
            rpc_client.clone(),
            dex_data
                .iter()
                .filter_map(|v| v.address_lookup_table_address)
                .collect::<Vec<_>>()
                .as_slice(),
        )
        .await?;
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
                crate::dex::raydium_amm::cache_init::init_cache(
                    &mut dex_data,
                    rpc_client.clone(),
                    pool_accounts,
                    vault_accounts,
                    alt_map,
                )
                .await
            }
            DexType::RaydiumCLMM => {
                crate::dex::raydium_clmm::cache_init::init_cache(
                    &mut dex_data,
                    rpc_client.clone(),
                    pool_accounts,
                    vault_accounts,
                    alt_map,
                )
                .await
            }
            DexType::PumpFunAMM => {
                crate::dex::pump_fun::cache_init::init_cache(
                    &mut dex_data,
                    rpc_client.clone(),
                    pool_accounts,
                    vault_accounts,
                    alt_map,
                )
                .await
            }
            DexType::MeteoraDLMM => {
                unimplemented!()
            }
        };
        match snapshot_data {
            None => {
                warn!("【{}】初始化Snapshot完毕, 无可使用的池子", dex_type,);
            }
            Some((static_data, dynamic_data, alts)) => {
                static_data.into_iter().for_each(|(k, v)| {
                    STATIC_ACCOUNT_CACHE.get().unwrap().write().0.insert(k, v);
                });
                dynamic_data.into_iter().for_each(|(k, v)| {
                    DYNAMIC_ACCOUNT_CACHE.get().unwrap().0.insert(k, v);
                });
                alts.into_iter().for_each(|(k, v)| {
                    ALT_CACHE.get().unwrap().write().0.insert(k, v);
                })
            }
        }
        info!(
            "【{}】初始化Snapshot完毕, 初始化池子数量 : {}",
            dex_type,
            dex_data.len()
        );
        effective_dex_data.extend(dex_data);
    }
    info!("初始化Snapshot结束");
    if effective_dex_data.is_empty() {
        Err(anyhow!("所有DexJson均加载失败"))
    } else {
        Ok(effective_dex_data)
    }
}

pub async fn get_account_data_with_data_slice(
    accounts: Vec<Pubkey>,
    dex_type: DexType,
    account_type: AccountType,
    rpc_client: Arc<RpcClient>,
) -> Vec<Vec<(Option<Vec<u8>>, Option<Vec<u8>>)>> {
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
            base_get_account_with_data_slice(
                rpc_client,
                account_chunks.as_slice(),
                dex_type,
                account_type,
            )
            .await
        });
    }
    join_set.join_all().await
}

async fn load_lookup_table_accounts(
    rpc_client: Arc<RpcClient>,
    all_alts: &[Pubkey],
) -> anyhow::Result<AHashMap<Pubkey, AddressLookupTableAccount>> {
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
                            Ok(lookup_table) => {
                                let lookup_table_account = AddressLookupTableAccount {
                                    key: pubkey,
                                    addresses: lookup_table.addresses.into_owned(),
                                };
                                Some((pubkey, lookup_table_account))
                            }
                            Err(e) => {
                                error!("   Failed to deserialize lookup table {}: {}", pubkey, e);
                                None
                            }
                        },
                    })
                    .collect::<AHashMap<_, _>>(),
                Err(_) => AHashMap::default(),
            }
        });
    }
    let mut alt_map = AHashMap::with_capacity(all_alts.len());
    while let Some(Ok(alt)) = join_set.join_next().await {
        alt_map.extend(alt);
    }
    if alt_map.is_empty() {
        Err(anyhow!("未找到任何【AddressLookupTableAccount】"))
    } else {
        Ok(alt_map)
    }
}

async fn base_get_account_with_data_slice(
    rpc_client: Arc<RpcClient>,
    accounts: &[Pubkey],
    dex_type: DexType,
    account_type: AccountType,
) -> Vec<(Option<Vec<u8>>, Option<Vec<u8>>)> {
    rpc_client
        .get_multiple_accounts_with_commitment(accounts, CommitmentConfig::finalized())
        .await
        .unwrap()
        .value
        .into_iter()
        .map(|account| {
            account.map_or((None, None), |acc| {
                let dynamic_data = slice_data(
                    dex_type.clone(),
                    account_type.clone(),
                    acc.data.as_slice(),
                    SliceType::Subscribed,
                )
                .map_or(None, |v| Some(v));
                let static_data = slice_data(
                    dex_type.clone(),
                    account_type.clone(),
                    acc.data.as_slice(),
                    SliceType::Unsubscribed,
                )
                .map_or(None, |v| Some(v));
                (dynamic_data, static_data)
            })
        })
        .collect::<Vec<_>>()
}

pub fn get_account_data<T: FromCache>(pool_id: &Pubkey) -> Option<T> {
    let static_data = STATIC_ACCOUNT_CACHE.get()?.read();
    let dynamic_data = DYNAMIC_ACCOUNT_CACHE.get()?;
    T::from_cache(pool_id, static_data, dynamic_data)
}

pub fn get_alt(pool_id: &Pubkey) -> Option<Vec<AddressLookupTableAccount>> {
    ALT_CACHE.get()?.read().get(pool_id)
}
