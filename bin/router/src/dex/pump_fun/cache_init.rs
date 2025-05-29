use std::sync::Arc;
use ahash::AHashMap;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::address_lookup_table::AddressLookupTableAccount;
use solana_sdk::pubkey::Pubkey;
use crate::account_cache::get_account_data_with_data_slice;
use crate::dex_data::DexJson;
use crate::interface::{AccountType, DexType, ATA_PROGRAM_ID};

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
    let mut dynamic_data = AHashMap::with_capacity(pool_accounts.len());
    let mut static_data = AHashMap::with_capacity(pool_accounts.len());
    let mut alt_data = AHashMap::with_capacity(pool_accounts.len());
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
        return None;
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
                    dynamic_data.insert(
                        json.vault_a.clone(),
                        vault_data.first().unwrap().clone().0.unwrap(),
                    );
                    dynamic_data.insert(
                        json.vault_b.clone(),
                        vault_data.last().unwrap().clone().0.unwrap(),
                    );
                    alt_data.insert(
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
                    static_data.insert(json.pool.clone(), combine_data);
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
