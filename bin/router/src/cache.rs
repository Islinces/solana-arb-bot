use crate::data_slice::{
    slice_data_with_dex_type_and_account_type_for_dynamic,
    slice_data_with_dex_type_and_account_type_for_static,
};
use crate::dex_data::DexJson;
use crate::interface::{get_dex_type_with_program_id, AccountType, DexType, ATA_PROGRAM_ID};
use ahash::{AHashMap, RandomState};
use anyhow::anyhow;
use dashmap::DashMap;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::address_lookup_table::state::AddressLookupTable;
use solana_sdk::address_lookup_table::AddressLookupTableAccount;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use std::collections::hash_map::Entry;
use std::sync::Arc;
use tokio::sync::{OnceCell, RwLock};
use tokio::task::JoinSet;
use tracing::error;

static ACCOUNT_DYNAMIC_CACHE: OnceCell<DynamicCache> = OnceCell::const_new();
static ACCOUNT_STATIC_CACHE: OnceCell<RwLock<StaticCache>> = OnceCell::const_new();
static ALT_CACHE: OnceCell<RwLock<AltCache>> = OnceCell::const_new();

pub struct DynamicCache(DashMap<Pubkey, Vec<u8>, RandomState>);
pub struct StaticCache(AHashMap<Pubkey, Vec<u8>>);
pub struct AltCache(AHashMap<Pubkey, Vec<AddressLookupTableAccount>>);

pub async fn init_cache(
    dex_data: Vec<DexJson>,
    rpc_client: Arc<RpcClient>,
) -> anyhow::Result<Vec<DexJson>> {
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
    for (dex_type, dex_data) in dex_data_group {
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
                init_raydium_clmm_cache(dex_data, rpc_client.clone(), alt_map).await
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
        };
        effective_dex_data.extend(remaining_dex_data);
    }
    if effective_dex_data.is_empty() {
        Err(anyhow!("所有DexJson均加载失败"))
    } else {
        Ok(effective_dex_data)
    }
}

async fn init_raydium_clmm_cache(
    mut _dex_data: Vec<DexJson>,
    _rpc_client: Arc<RpcClient>,
    _alt_map: AHashMap<Pubkey, AddressLookupTableAccount>,
) -> Vec<DexJson> {
    todo!()
}

async fn init_pump_fun_cache(
    mut dex_data: Vec<DexJson>,
    rpc_client: Arc<RpcClient>,
    pool_accounts: Vec<Pubkey>,
    vault_accounts: Vec<Pubkey>,
    alt_map: AHashMap<Pubkey, AddressLookupTableAccount>,
) -> Vec<DexJson> {
    let global_config_account_data = get_account_data_with_data_slice(
        vec![
            Pubkey::find_program_address(
                &[b"global_config"],
                DexType::PumpFunAMM.get_ref_program_id(),
            )
            .0,
        ],
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
                    ACCOUNT_DYNAMIC_CACHE.get().unwrap().0.insert(
                        json.vault_a.clone(),
                        vault_data.first().unwrap().clone().0.unwrap(),
                    );
                    ACCOUNT_DYNAMIC_CACHE.get().unwrap().0.insert(
                        json.vault_b.clone(),
                        vault_data.last().unwrap().0.clone().unwrap(),
                    );

                    // ALT
                    ALT_CACHE.get().unwrap().write().await.0.insert(
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
                    let quote_mint = Pubkey::try_from(&pool_static_data[32..32 * 2]).unwrap();
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
                    combine_data.extend(coin_creator_vault_ata.to_bytes());
                    combine_data.extend(coin_creator_vault_authority.to_bytes());
                    ACCOUNT_STATIC_CACHE
                        .get()
                        .unwrap()
                        .write()
                        .await
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
                    ACCOUNT_DYNAMIC_CACHE
                        .get()
                        .unwrap()
                        .0
                        .insert(json.pool.clone(), pool_dynamic_data);
                    let x = vault_data.first().unwrap().clone();
                    ACCOUNT_DYNAMIC_CACHE
                        .get()
                        .unwrap()
                        .0
                        .insert(json.vault_a.clone(), x.0.unwrap());
                    ACCOUNT_DYNAMIC_CACHE.get().unwrap().0.insert(
                        json.vault_b.clone(),
                        vault_data.last().unwrap().0.clone().unwrap(),
                    );
                    // 为订阅的数据，不变的
                    ACCOUNT_STATIC_CACHE
                        .get()
                        .unwrap()
                        .write()
                        .await
                        .0
                        .insert(json.pool.clone(), pool_static_data);
                    // ALT
                    ALT_CACHE.get().unwrap().write().await.0.insert(
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
    let mut join_set = JoinSet::new();
    for vault_account_chunks in accounts.chunks(100) {
        let rpc_client = rpc_client.clone();
        let dex_type = dex_type.clone();
        let account_type = account_type.clone();
        let vault_account_chunks = vault_account_chunks.to_vec();
        join_set.spawn(async move {
            base_get_account_with_data_slice(
                rpc_client,
                vault_account_chunks.as_slice(),
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
                let dynamic_data = slice_data_with_dex_type_and_account_type_for_dynamic(
                    dex_type.clone(),
                    account_type.clone(),
                    acc.data.as_slice(),
                )
                .map_or(None, |v| Some(v));
                let static_data = slice_data_with_dex_type_and_account_type_for_static(
                    dex_type.clone(),
                    account_type.clone(),
                    acc.data.as_slice(),
                )
                .map_or(None, |v| Some(v));
                (dynamic_data, static_data)
            })
        })
        .collect::<Vec<_>>()
}
