use crate::dex::raydium_amm::RAYDIUM_AMM_PROGRAM_ID;
use crate::dex::{
    get_account_data, get_data_slice_size, AccountDataSlice, AccountType, AmmInfo, DexType,
    SliceType, SnapshotInitializer,
};
use crate::dex_data::DexJson;
use ahash::{AHashMap, AHashSet};
use anyhow::anyhow;
use async_trait::async_trait;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tracing::info;

pub struct RaydiumCPMMSnapshotLoader;

#[async_trait]
impl SnapshotInitializer for RaydiumCPMMSnapshotLoader {
    async fn init_snapshot(
        &self,
        dex_json: &mut Vec<DexJson>,
        rpc_client: Arc<RpcClient>,
    ) -> Vec<AccountDataSlice> {
        let dex_data = dex_json
            .iter()
            .filter(|json| &json.owner == DexType::RaydiumCPMM.get_ref_program_id())
            .collect::<Vec<_>>();
        if dex_data.is_empty() {
            return vec![];
        }
        info!("【RaydiumCPMM】开始初始化Snapshot...");
        let mut invalid_pool = AHashSet::with_capacity(dex_data.len());
        let mut pool_accounts = Vec::with_capacity(dex_data.len());
        let mut vault_to_pool = AHashMap::with_capacity(dex_data.len() * 2);
        for json in dex_data.iter() {
            pool_accounts.push(json.pool);
            vault_to_pool.insert(json.vault_a, json.pool);
            vault_to_pool.insert(json.vault_b, json.pool);
        }
        // pool
        let mut all_pool_account_data = self
            .get_account_data_with_data_slice(
                pool_accounts,
                DexType::RaydiumCPMM,
                AccountType::Pool,
                rpc_client.clone(),
            )
            .await;
        all_pool_account_data.retain(|account| {
            if account.static_slice_data.as_ref().is_none()
                || account.dynamic_slice_data.as_ref().is_none()
            {
                invalid_pool.insert(account.account_key);
                false
            } else {
                true
            }
        });
        // amm config
        let all_amm_config_accounts = self
            .get_amm_config_accounts(
                rpc_client.clone(),
                all_pool_account_data.as_slice(),
                &mut invalid_pool,
            )
            .await;
        all_pool_account_data.retain(|account| !invalid_pool.contains(&account.account_key));
        // mint vault
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
        let mut all_vault_account_data = self
            .get_account_data_with_data_slice(
                all_vault_accounts,
                DexType::RaydiumCPMM,
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
        dex_json.retain(|json| !invalid_pool.contains(&json.pool));
        info!(
            "【RaydiumCPMM】初始化Snapshot完毕, 初始化池子数量 : {}",
            all_pool_account_data.len()
        );
        if dex_json.is_empty() {
            vec![]
        } else {
            all_pool_account_data
                .into_iter()
                .chain(all_vault_account_data.into_iter())
                .chain(all_amm_config_accounts.into_iter())
                .collect()
        }
    }

    fn print_snapshot(&self, dex_json: &[DexJson]) -> anyhow::Result<()> {
        if let Some(json) = dex_json
            .iter()
            .find(|json| &json.owner == DexType::RaydiumCPMM.get_ref_program_id())
        {
            let pool = get_account_data::<crate::dex::raydium_cpmm::states::PoolState>(&json.pool)
                .ok_or(anyhow!("{}找不到缓存数据", json.pool))?;
            info!(
                "【{}】【{:?}】, key : {:?}\ndata : {:#?}",
                DexType::RaydiumCPMM,
                AccountType::Pool,
                json.pool,
                pool
            );
            let amm_config =
                get_account_data::<crate::dex::raydium_cpmm::states::AmmConfig>(&pool.amm_config)
                    .ok_or(anyhow!("{}找不到缓存数据", pool.amm_config))?;
            info!(
                "【{}】【{:?}】, key : {:?}\ndata : {:#?}",
                DexType::RaydiumCPMM,
                AccountType::AmmConfig,
                pool.amm_config,
                amm_config
            );
        }
        Ok(())
    }
}

impl RaydiumCPMMSnapshotLoader {
    async fn get_amm_config_accounts(
        &self,
        rpc_client: Arc<RpcClient>,
        all_pool_account_data: &[AccountDataSlice],
        invalid_pool: &mut AHashSet<Pubkey>,
    ) -> Vec<AccountDataSlice> {
        let mut all_amm_config_accounts = AHashMap::with_capacity(50);
        for account in all_pool_account_data {
            // amm_config
            let amm_config_key =
                Pubkey::try_from(&account.static_slice_data.as_ref().unwrap()[0..32]).unwrap();
            all_amm_config_accounts
                .entry(amm_config_key)
                .or_insert_with(Vec::new)
                .push(account.account_key);
        }
        // 查询amm config
        let mut all_amm_config_account_data = self
            .get_account_data_with_data_slice(
                all_amm_config_accounts
                    .iter()
                    .map(|(key, _)| key.clone())
                    .collect::<Vec<_>>(),
                DexType::RaydiumCPMM,
                AccountType::AmmConfig,
                rpc_client.clone(),
            )
            .await;
        all_amm_config_account_data.retain(|account| {
            if account.static_slice_data.as_ref().is_none_or(|data| {
                data.len()
                    != get_data_slice_size(
                        DexType::RaydiumCPMM,
                        AccountType::AmmConfig,
                        SliceType::Unsubscribed,
                    )
                    .unwrap()
                    .unwrap()
            }) {
                for pool_id in all_amm_config_accounts.get(&account.account_key).unwrap() {
                    invalid_pool.insert(pool_id.clone());
                }
                false
            } else {
                true
            }
        });
        all_amm_config_account_data
    }
}
