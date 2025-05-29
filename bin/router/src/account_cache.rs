use crate::data_slice::{slice_data, SliceType};
use crate::dex::raydium_clmm::state::{PoolState, TickArrayBitmapExtension};
use crate::dex::raydium_clmm::utils::load_cur_and_next_specify_count_tick_array_key;
use crate::dex::FromCache;
use crate::dex_data::DexJson;
use crate::interface::{get_dex_type_with_program_id, AccountType, DexType, ATA_PROGRAM_ID};
use ahash::{AHashMap, AHashSet, RandomState};
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
use tracing::{error, info};

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
    for (dex_type, dex_data) in dex_data_group {
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
        let remaining_dex_data = match dex_type {
            DexType::RaydiumAMM => {
                init_raydium_amm_cache(
                    dex_data,
                    rpc_client.clone(),
                    pool_accounts,
                    vault_accounts,
                    alt_map,
                )
                .await
            }
            DexType::RaydiumCLMM => {
                init_raydium_clmm_cache(
                    dex_data,
                    rpc_client.clone(),
                    pool_accounts,
                    vault_accounts,
                    alt_map,
                )
                .await
            }
            DexType::PumpFunAMM => {
                init_pump_fun_cache(
                    dex_data,
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
        info!(
            "【{}】初始化Snapshot完毕, 初始化池子数量 : {}",
            dex_type,
            remaining_dex_data.len()
        );
        effective_dex_data.extend(remaining_dex_data);
    }
    info!("初始化Snapshot结束");
    if effective_dex_data.is_empty() {
        Err(anyhow!("所有DexJson均加载失败"))
    } else {
        Ok(effective_dex_data)
    }
}

async fn init_raydium_clmm_cache(
    mut dex_data: Vec<DexJson>,
    rpc_client: Arc<RpcClient>,
    pool_accounts: Vec<Pubkey>,
    _vault_accounts: Vec<Pubkey>,
    alt_map: AHashMap<Pubkey, AddressLookupTableAccount>,
) -> Vec<DexJson> {
    let dex_type = DexType::RaydiumCLMM;
    // 池子
    let mut all_pool_account_data = get_account_data_with_data_slice(
        pool_accounts,
        dex_type.clone(),
        AccountType::Pool,
        rpc_client.clone(),
    )
    .await
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    // 初始化失败的池子index
    // 无alt的池子index
    let mut invalid_pool_index = all_pool_account_data
        .iter()
        .enumerate()
        .filter_map(|(index, (dynamic_data, static_data))| {
            // 初始化失败
            if dynamic_data.as_ref().is_none() || static_data.as_ref().is_none() {
                Some(index)
            }
            // 无alt
            else if !alt_map.contains_key(
                dex_data
                    .get(index)
                    .unwrap()
                    .address_lookup_table_address
                    .as_ref()
                    .unwrap(),
            ) {
                Some(index)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if invalid_pool_index.len() == all_pool_account_data.len() {
        return vec![];
    }
    // 循环有效的池子，获取amm_config，bitmap_extension，tick_array_ticks(初始化左右各10个)
    let mut all_amm_config_accounts = AHashSet::with_capacity(50);
    let mut all_bitmap_extension_accounts = Vec::with_capacity(all_pool_account_data.len());
    for (index, (_pool_dynamic_data, pool_static_data)) in all_pool_account_data.iter().enumerate()
    {
        // 跳过初始化失败的池子
        if invalid_pool_index.contains(&index) {
            continue;
        }
        let json = dex_data.get(index).unwrap();
        // amm_config
        let amm_config_key = Pubkey::try_from(&pool_static_data.as_ref().unwrap()[0..32]).unwrap();
        all_amm_config_accounts.insert(amm_config_key);
        // bitmap_extension
        all_bitmap_extension_accounts.push(
            crate::dex::raydium_clmm::state::pda_bit_map_extension_key(&json.pool),
        );
    }
    // 查询amm config
    let all_amm_config_accounts = all_amm_config_accounts.into_iter().collect::<Vec<_>>();
    let all_amm_config_account_data = get_account_data_with_data_slice(
        all_amm_config_accounts.clone(),
        dex_type.clone(),
        AccountType::AmmConfig,
        rpc_client.clone(),
    )
    .await
    .into_iter()
    .flatten()
    .map(|v| v.1)
    .zip(all_amm_config_accounts.into_iter())
    .filter_map(|(data, key)| {
        if data.as_ref().is_none_or(|v| {
            v.len()
                != crate::data_slice::get_slice_size(
                    dex_type.clone(),
                    AccountType::AmmConfig,
                    SliceType::Unsubscribed,
                )
                .unwrap()
                .unwrap()
        }) {
            None
        } else {
            Some((key, data.unwrap()))
        }
    })
    .collect::<AHashMap<Pubkey, Vec<u8>>>();

    // 查询bitmap extension
    let mut all_bitmap_extension_account_data = get_account_data_with_data_slice(
        all_bitmap_extension_accounts.clone(),
        dex_type.clone(),
        AccountType::TickArrayBitmapExtension,
        rpc_client.clone(),
    )
    .await
    .into_iter()
    .flatten()
    .map(|v| v.0)
    .zip(all_bitmap_extension_accounts.into_iter())
    .filter_map(|(data, key)| {
        if data.as_ref().is_none_or(|v| {
            v.len()
                != crate::data_slice::get_slice_size(
                    dex_type.clone(),
                    AccountType::TickArrayBitmapExtension,
                    SliceType::Subscribed,
                )
                .unwrap()
                .unwrap()
        }) {
            None
        } else {
            Some((key, data.unwrap()))
        }
    })
    .collect::<AHashMap<Pubkey, Vec<u8>>>();

    // 过滤出来amm config无效的池子
    // 缓存有效的amm config
    // 过滤出来bitmap extension无效的池子
    // 缓存有效的bitmap extension
    let mut all_tick_array_state_accounts =
        AHashSet::with_capacity(all_pool_account_data.len() * 20);
    for (index, (pool_dynamic_data, pool_static_data)) in
        all_pool_account_data.iter_mut().enumerate()
    {
        // amm config
        let amm_config_key = Pubkey::try_from(&pool_static_data.as_ref().unwrap()[0..32]).unwrap();
        match all_amm_config_account_data.get(&amm_config_key) {
            None => {
                invalid_pool_index.push(index);
                continue;
            }
            Some(amm_config) => {
                STATIC_ACCOUNT_CACHE
                    .get()
                    .unwrap()
                    .write()
                    .0
                    .insert(amm_config_key, amm_config.clone());
            }
        }
        let pool_id = dex_data.get(index).unwrap().pool;
        // bitmap extension
        let bitmap_extension_key = Pubkey::find_program_address(
            &[
                "pool_tick_array_bitmap_extension".as_bytes(),
                pool_id.as_ref(),
            ],
            dex_type.get_ref_program_id(),
        )
        .0;
        match all_bitmap_extension_account_data.remove(&bitmap_extension_key) {
            None => {
                invalid_pool_index.push(index);
                continue;
            }
            Some(bitmap_extension) => {
                DYNAMIC_ACCOUNT_CACHE
                    .get()
                    .unwrap()
                    .0
                    .insert(bitmap_extension_key, bitmap_extension);
            }
        }
        let pool_state = PoolState::from_slice_data(
            pool_static_data.as_ref().unwrap(),
            pool_dynamic_data.as_ref().unwrap(),
        );
        let tick_array_bitmap_extension = Some(TickArrayBitmapExtension::from_slice_data(
            DYNAMIC_ACCOUNT_CACHE
                .get()
                .unwrap()
                .0
                .get(&bitmap_extension_key)
                .unwrap()
                .value(),
        ));
        // 前后各10个tick array
        all_tick_array_state_accounts.extend(
            load_cur_and_next_specify_count_tick_array_key(
                10,
                &pool_id,
                &pool_state,
                &tick_array_bitmap_extension,
                true,
            )
            .unwrap(),
        );
        all_tick_array_state_accounts.extend(
            load_cur_and_next_specify_count_tick_array_key(
                10,
                &pool_id,
                &pool_state,
                &tick_array_bitmap_extension,
                false,
            )
            .unwrap(),
        );
    }
    // 查询tick array state
    let all_tick_array_state_accounts = all_tick_array_state_accounts
        .into_iter()
        .collect::<Vec<_>>();
    let all_tick_array_state_account_data = get_account_data_with_data_slice(
        all_tick_array_state_accounts.clone(),
        dex_type.clone(),
        AccountType::TickArrayState,
        rpc_client.clone(),
    )
    .await
    .into_iter()
    .flatten()
    .zip(all_tick_array_state_accounts.into_iter())
    .filter_map(|((dynamic_data, _), key)| {
        if dynamic_data.as_ref().is_none() {
            None
        } else {
            Some((key, dynamic_data.unwrap()))
        }
    })
    .collect::<Vec<_>>();
    // 缓存tick array state
    for (key, data) in all_tick_array_state_account_data.into_iter() {
        DYNAMIC_ACCOUNT_CACHE.get().unwrap().0.insert(key, data);
    }

    // 缓存pool
    for (index, (dynamic_data, static_data)) in all_pool_account_data.into_iter().enumerate() {
        if invalid_pool_index.contains(&index) {
            dex_data.remove(index);
            continue;
        }
        let json = dex_data.get(index).unwrap();
        let pool = json.pool;
        let alt = json.address_lookup_table_address;
        DYNAMIC_ACCOUNT_CACHE
            .get()
            .unwrap()
            .0
            .insert(pool, dynamic_data.unwrap());
        STATIC_ACCOUNT_CACHE
            .get()
            .unwrap()
            .write()
            .0
            .insert(pool, static_data.unwrap());
        let alt_account = alt_map.get(alt.as_ref().unwrap()).unwrap();
        ALT_CACHE
            .get()
            .unwrap()
            .write()
            .0
            .insert(alt.unwrap(), vec![alt_account.clone()]);
    }
    dex_data
}

async fn init_pump_fun_cache(
    mut dex_data: Vec<DexJson>,
    rpc_client: Arc<RpcClient>,
    pool_accounts: Vec<Pubkey>,
    vault_accounts: Vec<Pubkey>,
    alt_map: AHashMap<Pubkey, AddressLookupTableAccount>,
) -> Vec<DexJson> {
    let global_config_account_data = get_account_data_with_data_slice(
        vec![crate::dex::pump_fun::state::global_config_key()],
        DexType::PumpFunAMM,
        AccountType::PumpFunGlobalConfig,
        rpc_client.clone(),
    )
    .await;
    let global_config_account_data = global_config_account_data
        .first()
        .unwrap()
        .clone()
        .first()
        .unwrap()
        .clone()
        .1;
    // global config没有，删除所有
    if global_config_account_data.as_ref().is_none() {
        dex_data.retain(|_| false);
        return vec![];
    }
    // 池子
    let all_pool_account_data = get_account_data_with_data_slice(
        pool_accounts,
        DexType::PumpFunAMM,
        AccountType::Pool,
        rpc_client.clone(),
    )
    .await;
    // 金库
    let all_vault_account_data = get_account_data_with_data_slice(
        vault_accounts,
        DexType::PumpFunAMM,
        AccountType::MintVault,
        rpc_client.clone(),
    )
    .await;

    for (index, (pool_data, vault_data)) in all_pool_account_data
        .into_iter()
        .flatten()
        .zip(
            all_vault_account_data
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .chunks(2)
                .map(|v| v.to_vec())
                .collect::<Vec<_>>(),
        )
        .enumerate()
    {
        match (pool_data.0, pool_data.1) {
            (None, Some(pool_static_data)) => {
                let json = dex_data.get(index).unwrap();
                // 金库初始化失败
                if vault_data
                    .iter()
                    // 金库没有未订阅的数据，只需要amount
                    .any(|(vault_dynamic_data, _)| vault_dynamic_data.is_none())
                {
                    dex_data.remove(index);
                }
                // alt初始化失败
                else if !alt_map.contains_key(&json.address_lookup_table_address.unwrap()) {
                    dex_data.remove(index);
                } else {
                    // 订阅的数据，变化的
                    DYNAMIC_ACCOUNT_CACHE.get().unwrap().0.insert(
                        json.vault_a.clone(),
                        vault_data.first().unwrap().clone().0.unwrap(),
                    );
                    DYNAMIC_ACCOUNT_CACHE.get().unwrap().0.insert(
                        json.vault_b.clone(),
                        vault_data.last().unwrap().0.clone().unwrap(),
                    );

                    // ALT
                    ALT_CACHE.get().unwrap().write().0.insert(
                        json.pool.clone(),
                        vec![alt_map
                            .get(&json.address_lookup_table_address.unwrap())
                            .unwrap()
                            .clone()],
                    );

                    // lp_fee_basis_points 和 protocol_fee_basis_points
                    let global_config_account_data = global_config_account_data.clone().unwrap();
                    // 池子未订阅，global config为订阅
                    // 将两个合并在一起
                    let mut combine_data = Vec::with_capacity(
                        pool_static_data.len() + global_config_account_data.len() + 32 * 2,
                    );
                    // 先提前生成coin_creator_vault_authority和coin_creator_vault_ata
                    let quote_mint = Pubkey::try_from(&pool_static_data[32..32 + 32]).unwrap();
                    let coin_creator = Pubkey::try_from(&pool_static_data[32 * 4..32 * 5]).unwrap();
                    let token_program = if quote_mint == spl_token::native_mint::ID {
                        spl_token::ID
                    } else {
                        rpc_client
                            .clone()
                            .get_account(&quote_mint)
                            .await
                            .unwrap()
                            .owner
                    };
                    let (coin_creator_vault_authority, _) = Pubkey::find_program_address(
                        &[b"creator_vault", coin_creator.to_bytes().as_ref()],
                        DexType::PumpFunAMM.get_ref_program_id(),
                    );
                    let (coin_creator_vault_ata, _) = Pubkey::find_program_address(
                        &[
                            coin_creator_vault_authority.to_bytes().as_ref(),
                            token_program.to_bytes().as_ref(),
                            quote_mint.to_bytes().as_ref(),
                        ],
                        &ATA_PROGRAM_ID,
                    );
                    combine_data.extend(pool_static_data);
                    combine_data.extend(global_config_account_data);
                    combine_data.extend(coin_creator_vault_authority.to_bytes());
                    combine_data.extend(coin_creator_vault_ata.to_bytes());
                    STATIC_ACCOUNT_CACHE
                        .get()
                        .unwrap()
                        .write()
                        .0
                        .insert(json.pool.clone(), combine_data);
                }
            }
            _ => {
                dex_data.remove(index);
            }
        }
    }
    dex_data
}

async fn init_raydium_amm_cache(
    mut dex_data: Vec<DexJson>,
    rpc_client: Arc<RpcClient>,
    pool_accounts: Vec<Pubkey>,
    vault_accounts: Vec<Pubkey>,
    alt_map: AHashMap<Pubkey, AddressLookupTableAccount>,
) -> Vec<DexJson> {
    let all_pool_account_data = get_account_data_with_data_slice(
        pool_accounts,
        DexType::RaydiumAMM,
        AccountType::Pool,
        rpc_client.clone(),
    )
    .await;
    let all_vault_account_data = get_account_data_with_data_slice(
        vault_accounts,
        DexType::RaydiumAMM,
        AccountType::MintVault,
        rpc_client,
    )
    .await;
    for (index, (pool_data, vault_data)) in all_pool_account_data
        .into_iter()
        .flatten()
        .zip(
            all_vault_account_data
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .chunks(2)
                .map(|v| v.to_vec())
                .collect::<Vec<_>>(),
        )
        .enumerate()
    {
        match (pool_data.0, pool_data.1) {
            (Some(pool_dynamic_data), Some(pool_static_data)) => {
                let json = dex_data.get(index).unwrap();
                // 金库初始化失败
                if vault_data
                    .iter()
                    // 金库没有未订阅的数据，只需要amount
                    .any(|(vault_dynamic_data, _)| vault_dynamic_data.is_none())
                {
                    dex_data.remove(index);
                }
                // alt初始化失败
                else if !alt_map.contains_key(&json.address_lookup_table_address.unwrap()) {
                    dex_data.remove(index);
                } else {
                    // 订阅的数据，变化的
                    DYNAMIC_ACCOUNT_CACHE
                        .get()
                        .unwrap()
                        .0
                        .insert(json.pool.clone(), pool_dynamic_data);
                    DYNAMIC_ACCOUNT_CACHE.get().unwrap().0.insert(
                        json.vault_a.clone(),
                        vault_data.first().unwrap().clone().0.unwrap(),
                    );
                    DYNAMIC_ACCOUNT_CACHE.get().unwrap().0.insert(
                        json.vault_b.clone(),
                        vault_data.last().unwrap().0.clone().unwrap(),
                    );
                    // 为订阅的数据，不变的
                    STATIC_ACCOUNT_CACHE
                        .get()
                        .unwrap()
                        .write()
                        .0
                        .insert(json.pool.clone(), pool_static_data);
                    // ALT
                    ALT_CACHE.get().unwrap().write().0.insert(
                        json.pool.clone(),
                        vec![alt_map
                            .get(&json.address_lookup_table_address.unwrap())
                            .unwrap()
                            .clone()],
                    );
                }
            }
            _ => {
                dex_data.remove(index);
            }
        }
    }
    dex_data
}

async fn get_account_data_with_data_slice(
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
