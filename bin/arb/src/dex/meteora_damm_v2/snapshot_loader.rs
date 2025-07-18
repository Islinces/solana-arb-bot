use crate::dex::meteora_damm_v2::state::pool::Pool;
use crate::dex::raydium_amm::RAYDIUM_AMM_PROGRAM_ID;
use crate::dex::{
    get_account_data, AccountDataSlice, AccountType, AmmInfo, DexType, SnapshotInitializer,
};
use crate::dex_data::DexJson;
use ahash::{AHashMap, AHashSet};
use anyhow::anyhow;
use async_trait::async_trait;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use std::sync::Arc;
use tracing::info;

pub struct MeteoraDAMMV2SnapshotLoader;

#[async_trait]
impl SnapshotInitializer for MeteoraDAMMV2SnapshotLoader {
    async fn init_snapshot(
        &self,
        dex_json: &mut Vec<DexJson>,
        rpc_client: Arc<RpcClient>,
    ) -> Vec<AccountDataSlice> {
        let dex_data = dex_json
            .iter()
            .filter(|json| &json.owner == DexType::MeteoraDAMMV2.get_ref_program_id())
            .collect::<Vec<_>>();
        if dex_data.is_empty() {
            return vec![];
        }
        info!("【{}】开始初始化Snapshot...", DexType::MeteoraDAMMV2);
        let mut invalid_pool = AHashSet::with_capacity(dex_data.len());
        let pool_accounts = dex_data
            .into_iter()
            .map(|json| json.pool)
            .collect::<Vec<_>>();
        let mut all_pool_account_data = self
            .get_account_data_with_data_slice(
                pool_accounts,
                DexType::MeteoraDAMMV2,
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
        dex_json.retain(|json| !invalid_pool.contains(&json.pool));
        info!(
            "【{}】初始化Snapshot完毕, 初始化池子数量 : {}",
            DexType::MeteoraDAMMV2,
            all_pool_account_data.len()
        );
        if dex_json.is_empty() {
            vec![]
        } else {
            all_pool_account_data.into_iter().collect()
        }
    }

    fn print_snapshot(&self, dex_json: &[DexJson]) -> anyhow::Result<()> {
        if let Some(json) = dex_json
            .iter()
            .find(|json| &json.owner == DexType::MeteoraDAMMV2.get_ref_program_id())
        {
            let pool = get_account_data::<Pool>(&json.pool)
                .ok_or(anyhow!("{}找不到缓存数据", json.pool))?;
            info!(
                "【{}】【{:?}】, key : {:?}\ndata : {:#?}",
                DexType::MeteoraDAMMV2,
                AccountType::Pool,
                json.pool,
                pool
            );
        }
        Ok(())
    }
}
