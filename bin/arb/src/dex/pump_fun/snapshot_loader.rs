use crate::dex::orca_whirlpools::WHIRLPOOL_ID;
use crate::dex::pump_fun::state::Pool;
use crate::dex::pump_fun::PUMP_FUN_AMM_PROGRAM_ID;
use crate::dex::raydium_amm::state::AmmInfo;
use crate::dex::{AccountType, DexType, ATA_PROGRAM_ID, MINT_PROGRAM_ID, SYSTEM_PROGRAM_ID};
use crate::dex_data::DexJson;
use crate::global_cache::get_account_data;
use crate::{AccountDataSlice, SnapshotInitializer};
use ahash::{AHashMap, AHashSet};
use anyhow::anyhow;
use async_trait::async_trait;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tracing::info;

pub struct PumpFunAMMSnapshotInitializer;

#[async_trait]
impl SnapshotInitializer for PumpFunAMMSnapshotInitializer {
    async fn init_snapshot(
        &self,
        dex_json: &mut Vec<DexJson>,
        rpc_client: Arc<RpcClient>,
    ) -> Vec<AccountDataSlice> {
        let dex_data = dex_json
            .iter()
            .filter(|json| json.owner == PUMP_FUN_AMM_PROGRAM_ID)
            .map(|json| json.clone())
            .collect::<Vec<_>>();
        if dex_data.is_empty() {
            return vec![];
        }
        info!("【PumpFunAMM】开始初始化Snapshot...");
        // global config
        let global_config_account = self.get_global_config_account(rpc_client.clone()).await;
        let (invalid_pool, accounts) = if global_config_account
            .as_ref()
            .is_none_or(|a| a.static_slice_data.is_none())
        {
            (
                dex_data
                    .iter()
                    .map(|json| json.pool)
                    .collect::<AHashSet<_>>(),
                None,
            )
        } else {
            // lp_fee_basis_points 和 protocol_fee_basis_points
            let global_config_account_data =
                global_config_account.unwrap().static_slice_data.unwrap();
            let mut invalid_pool = AHashSet::with_capacity(dex_json.len());
            let mut pool_accounts = Vec::with_capacity(dex_json.len());
            let mut vault_to_pool = AHashMap::with_capacity(dex_json.len() * 2);
            for json in dex_json.iter() {
                pool_accounts.push(json.pool);
                vault_to_pool.insert(json.vault_a, json.pool);
                vault_to_pool.insert(json.vault_b, json.pool);
            }
            let mut all_pool_account_data = self
                .get_account_data_with_data_slice(
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
                let mut combine_data = Vec::with_capacity(
                    pool_static_data.len() + global_config_account_data.len() + 32 * 2,
                );
                // 先提前生成coin_creator_vault_authority和coin_creator_vault_ata
                let quote_mint = Pubkey::try_from(&pool_static_data[32..32 + 32]).unwrap();
                let coin_creator = Pubkey::try_from(&pool_static_data[32 * 4..32 * 5]).unwrap();
                let token_program = if quote_mint == SYSTEM_PROGRAM_ID {
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
                    let option = dex_json
                        .iter()
                        .find(|a| a.pool == account.account_key)
                        .unwrap();
                    vec![option.vault_a, option.vault_b]
                })
                .flatten()
                .collect::<Vec<_>>();
            let mut all_vault_account_data = self
                .get_account_data_with_data_slice(
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

            (
                invalid_pool,
                Some(
                    all_pool_account_data
                        .into_iter()
                        .chain(all_vault_account_data.into_iter())
                        .collect::<Vec<AccountDataSlice>>(),
                ),
            )
        };
        dex_json.retain(|json| !invalid_pool.contains(&json.pool));
        info!(
            "【PumpFunAMM】初始化Snapshot完毕, 初始化池子数量 : {}",
            dex_data.len()
        );
        if dex_json.is_empty() {
            vec![]
        } else {
            accounts.unwrap()
        }
    }

    fn print_snapshot(&self, dex_json: &[DexJson]) -> anyhow::Result<()> {
        if let Some(json) = dex_json
            .iter()
            .find(|json| &json.owner == DexType::PumpFunAMM.get_ref_program_id())
        {
            let pool = get_account_data::<Pool>(&json.pool)
                .ok_or(anyhow!("{}找不到缓存数据", json.pool))?;
            info!(
                "【{}】【{:?}】, key : {:?}\ndata : {:#?}",
                DexType::PumpFunAMM,
                AccountType::Pool,
                json.pool,
                pool
            );
        }
        Ok(())
    }
}

impl PumpFunAMMSnapshotInitializer {
    async fn get_global_config_account(
        &self,
        rpc_client: Arc<RpcClient>,
    ) -> Option<AccountDataSlice> {
        let global_config_account = self
            .get_account_data_with_data_slice(
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
}
