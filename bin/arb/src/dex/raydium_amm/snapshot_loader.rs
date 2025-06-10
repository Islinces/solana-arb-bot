use crate::dex::pump_fun::PUMP_FUN_AMM_PROGRAM_ID;
use crate::dex::raydium_amm::state::AmmInfo;
use crate::dex::raydium_amm::RAYDIUM_AMM_PROGRAM_ID;
use crate::dex::{AccountType, DexType};
use crate::dex_data::DexJson;
use crate::global_cache::get_account_data;
use crate::{AccountDataSlice, SnapshotInitializer};
use ahash::{AHashMap, AHashSet};
use anyhow::anyhow;
use async_trait::async_trait;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use std::sync::Arc;
use tracing::info;

pub struct RaydiumAmmSnapshotInitializer;

#[async_trait]
impl SnapshotInitializer for RaydiumAmmSnapshotInitializer {
    async fn init_snapshot(
        &self,
        dex_json: &mut Vec<DexJson>,
        rpc_client: Arc<RpcClient>,
    ) -> Vec<AccountDataSlice> {
        let dex_data = dex_json
            .iter()
            .filter(|json| json.owner == RAYDIUM_AMM_PROGRAM_ID)
            .collect::<Vec<_>>();
        if dex_data.is_empty() {
            return vec![];
        }
        info!("【RaydiumAMM】开始初始化Snapshot...");
        let mut invalid_pool = AHashSet::with_capacity(dex_data.len());
        let mut pool_accounts = Vec::with_capacity(dex_data.len());
        let mut vault_to_pool = AHashMap::with_capacity(dex_data.len() * 2);
        for json in dex_data.iter() {
            pool_accounts.push(json.pool);
            vault_to_pool.insert(json.vault_a, json.pool);
            vault_to_pool.insert(json.vault_b, json.pool);
        }
        let mut all_pool_account_data = self
            .get_account_data_with_data_slice(
                pool_accounts,
                DexType::RaydiumAMM,
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
                DexType::RaydiumAMM,
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
            "【RaydiumAmm】初始化Snapshot完毕, 初始化池子数量 : {}",
            all_pool_account_data.len()
        );
        if dex_json.is_empty() {
            vec![]
        } else {
            all_pool_account_data
                .into_iter()
                .chain(all_vault_account_data.into_iter())
                .collect()
        }
    }

    fn print_snapshot(&self, dex_json: &[DexJson]) -> anyhow::Result<()> {
        if let Some(json) = dex_json
            .iter()
            .find(|json| &json.owner == DexType::RaydiumAMM.get_ref_program_id())
        {
            let pool = get_account_data::<AmmInfo>(&json.pool)
                .ok_or(anyhow!("{}找不到缓存数据", json.pool))?;
            info!(
                "【{}】【{:?}】, key : {:?}\ndata : {:#?}",
                DexType::RaydiumAMM,
                AccountType::Pool,
                json.pool,
                pool
            );
        }
        Ok(())
    }
}
