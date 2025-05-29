use crate::dex_data::DexJson;
use crate::interface::{AccountType, DexType};
use ahash::AHashMap;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::address_lookup_table::AddressLookupTableAccount;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;

pub async fn init_cache(
    dex_data: &mut Vec<DexJson>,
    rpc_client: Arc<RpcClient>,
    pool_accounts: Vec<Pubkey>,
    vault_accounts: Vec<Pubkey>,
    alt_map: AHashMap<Pubkey, AddressLookupTableAccount>,
) -> Option<(
    AHashMap<Pubkey, Vec<u8>>,
    AHashMap<Pubkey, Vec<u8>>,
    AHashMap<Pubkey, Vec<AddressLookupTableAccount>>,
)> {
    let all_pool_account_data = crate::account_cache::get_account_data_with_data_slice(
        pool_accounts,
        DexType::RaydiumAMM,
        AccountType::Pool,
        rpc_client.clone(),
    )
    .await;
    let all_vault_account_data = crate::account_cache::get_account_data_with_data_slice(
        vault_accounts,
        DexType::RaydiumAMM,
        AccountType::MintVault,
        rpc_client,
    )
    .await;
    let mut dynamic_data = AHashMap::with_capacity(all_pool_account_data.len());
    let mut static_data = AHashMap::with_capacity(all_pool_account_data.len());
    let mut alt_data = AHashMap::with_capacity(all_pool_account_data.len());
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
                    dynamic_data.insert(json.pool.clone(), pool_dynamic_data);
                    dynamic_data.insert(
                        json.vault_a.clone(),
                        vault_data.first().unwrap().clone().0.unwrap(),
                    );
                    dynamic_data.insert(
                        json.vault_b.clone(),
                        vault_data.last().unwrap().clone().0.unwrap(),
                    );

                    static_data.insert(json.pool.clone(), pool_static_data);

                    alt_data.insert(
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
    if dex_data.is_empty() {
        None
    } else {
        Some((static_data, dynamic_data, alt_data))
    }
}
