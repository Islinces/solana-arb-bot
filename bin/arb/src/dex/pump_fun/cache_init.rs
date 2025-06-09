use crate::account_cache::{get_account_data_with_data_slice, AccountDataSlice};
use crate::dex_data::DexJson;
use crate::interface1::{AccountType, DexType, ATA_PROGRAM_ID, MINT_PROGRAM_ID};
use ahash::{AHashMap, AHashSet};
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;

pub async fn init_cache(
    dex_data: &mut Vec<DexJson>,
    rpc_client: Arc<RpcClient>,
) -> Vec<AccountDataSlice> {
    // global config
    let global_config_account = get_global_config_account(rpc_client.clone()).await;
    if global_config_account
        .as_ref()
        .is_none_or(|a| a.static_slice_data.is_none())
    {
        dex_data.retain(|_| false);
        return vec![];
    }
    // lp_fee_basis_points 和 protocol_fee_basis_points
    let global_config_account_data = global_config_account.unwrap().static_slice_data.unwrap();
    let mut invalid_pool = AHashSet::with_capacity(dex_data.len());
    let mut pool_accounts = Vec::with_capacity(dex_data.len());
    let mut vault_to_pool = AHashMap::with_capacity(dex_data.len() * 2);
    for json in dex_data.iter() {
        pool_accounts.push(json.pool);
        vault_to_pool.insert(json.vault_a, json.pool);
        vault_to_pool.insert(json.vault_b, json.pool);
    }
    let mut all_pool_account_data = get_account_data_with_data_slice(
        pool_accounts,
        DexType::PumpFunAMM,
        AccountType::Pool,
        rpc_client.clone(),
    )
    .await;
    all_pool_account_data.retain(|account| {
        if account.static_slice_data.as_ref().is_none() {
            invalid_pool.insert(account.account_key);
            false
        } else {
            true
        }
    });
    for account in all_pool_account_data.iter_mut() {
        let pool_static_data = account.static_slice_data.as_ref().unwrap();
        // 池子未订阅，global config为订阅
        // 将两个合并在一起
        let mut combine_data =
            Vec::with_capacity(pool_static_data.len() + global_config_account_data.len() + 32 * 2);
        // 先提前生成coin_creator_vault_authority和coin_creator_vault_ata
        let quote_mint = Pubkey::try_from(&pool_static_data[32..32 + 32]).unwrap();
        let coin_creator = Pubkey::try_from(&pool_static_data[32 * 4..32 * 5]).unwrap();
        let token_program = if quote_mint == MINT_PROGRAM_ID {
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
        combine_data.extend(global_config_account_data.clone());
        combine_data.extend(coin_creator_vault_authority.to_bytes());
        combine_data.extend(coin_creator_vault_ata.to_bytes());
        account.static_slice_data.replace(combine_data);
    }
    let all_vault_accounts = all_pool_account_data
        .iter()
        .map(|account| {
            let option = dex_data
                .iter()
                .find(|a| a.pool == account.account_key)
                .unwrap();
            vec![option.vault_a, option.vault_b]
        })
        .flatten()
        .collect::<Vec<_>>();
    let mut all_vault_account_data = get_account_data_with_data_slice(
        all_vault_accounts,
        DexType::PumpFunAMM,
        AccountType::MintVault,
        rpc_client,
    )
    .await;
    all_vault_account_data.retain(|account| {
        if account.dynamic_slice_data.as_ref().is_none() {
            invalid_pool.insert(vault_to_pool.get(&account.account_key).unwrap().clone());
            false
        } else {
            true
        }
    });
    dex_data.retain(|json| !invalid_pool.contains(&json.pool));
    if dex_data.is_empty() {
        vec![]
    } else {
        all_pool_account_data
            .into_iter()
            .chain(all_vault_account_data.into_iter())
            .collect()
    }
}

async fn get_global_config_account(rpc_client: Arc<RpcClient>) -> Option<AccountDataSlice> {
    let global_config_account = get_account_data_with_data_slice(
        vec![crate::dex::pump_fun::state::global_config_key()],
        DexType::PumpFunAMM,
        AccountType::PumpFunGlobalConfig,
        rpc_client.clone(),
    )
    .await;
    let g = global_config_account.first();
    if g.is_none_or(|v| v.static_slice_data.is_none()) {
        None
    } else {
        g.cloned()
    }
}
