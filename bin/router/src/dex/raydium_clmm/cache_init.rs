use crate::account_cache::get_account_data_with_data_slice;
use crate::data_slice::SliceType;
use crate::dex::raydium_clmm::state::{PoolState, TickArrayBitmapExtension};
use crate::dex::raydium_clmm::utils::load_cur_and_next_specify_count_tick_array_key;
use crate::dex_data::DexJson;
use crate::interface::{AccountType, DexType};
use ahash::{AHashMap, AHashSet};
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::address_lookup_table::AddressLookupTableAccount;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;

pub async fn init_cache(
    dex_data: &mut Vec<DexJson>,
    rpc_client: Arc<RpcClient>,
    pool_accounts: Vec<Pubkey>,
    _vault_accounts: Vec<Pubkey>,
    alt_map: AHashMap<Pubkey, AddressLookupTableAccount>,
) -> Option<(
    AHashMap<Pubkey, Vec<u8>>,
    AHashMap<Pubkey, Vec<u8>>,
    AHashMap<Pubkey, Vec<AddressLookupTableAccount>>,
)> {
    let mut dynamic_data = AHashMap::with_capacity(dex_data.len());
    let mut static_data = AHashMap::with_capacity(dex_data.len());
    let mut alt_data = AHashMap::with_capacity(dex_data.len());
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
        dex_data.retain(|_| false);
        return None;
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
                static_data.insert(amm_config_key, amm_config.clone());
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
        let tick_array_bitmap_extension =
            match all_bitmap_extension_account_data.remove(&bitmap_extension_key) {
                None => {
                    invalid_pool_index.push(index);
                    continue;
                }
                Some(bitmap_extension) => {
                    let extension =
                        TickArrayBitmapExtension::from_slice_data(bitmap_extension.as_slice());
                    dynamic_data.insert(bitmap_extension_key, bitmap_extension);
                    Some(extension)
                }
            };
        let pool_state = PoolState::from_slice_data(
            pool_static_data.as_ref().unwrap(),
            pool_dynamic_data.as_ref().unwrap(),
        );
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
        dynamic_data.insert(key, data);
    }

    // 缓存pool
    for (index, (d_data, s_data)) in all_pool_account_data.into_iter().enumerate() {
        if invalid_pool_index.contains(&index) {
            dex_data.remove(index);
            continue;
        }
        let json = dex_data.get(index).unwrap();
        let pool = json.pool;
        let alt = json.address_lookup_table_address;
        dynamic_data.insert(pool, d_data.unwrap());
        static_data.insert(pool, s_data.unwrap());
        alt_data.insert(
            alt.unwrap(),
            vec![alt_map.get(alt.as_ref().unwrap()).unwrap().clone()],
        );
    }
    if dex_data.is_empty() {
        None
    } else {
        Some((static_data, dynamic_data, alt_data))
    }
}
