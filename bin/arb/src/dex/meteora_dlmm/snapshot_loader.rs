use crate::dex::global_cache::get_account_data;
use crate::dex::meteora_dlmm::commons::get_bin_array_pubkeys_for_swap;
use crate::dex::meteora_dlmm::METEORA_DLMM_PROGRAM_ID;
use crate::dex::raydium_clmm::state::pda_bit_map_extension_key;
use crate::dex::snapshot::{AccountDataSlice, SnapshotInitializer};
use crate::dex::{AccountType, BinArray, BinArrayBitmapExtension, DexType, LbPair};
use crate::dex_data::DexJson;
use ahash::{AHashMap, AHashSet};
use anyhow::anyhow;
use async_trait::async_trait;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tracing::info;

pub struct MeteoraDLMMSnapshotInitializer;

#[async_trait]
impl SnapshotInitializer for MeteoraDLMMSnapshotInitializer {
    async fn init_snapshot(
        &self,
        dex_json: &mut Vec<DexJson>,
        rpc_client: Arc<RpcClient>,
    ) -> Vec<AccountDataSlice> {
        let dex_data = dex_json
            .iter()
            .filter(|json| json.owner == METEORA_DLMM_PROGRAM_ID)
            .collect::<Vec<_>>();
        if dex_data.is_empty() {
            return vec![];
        }
        info!("【MeteoraDLMM】开始初始化Snapshot...");
        let mut invalid_pool = AHashSet::with_capacity(dex_data.len());
        let mut pool_accounts = Vec::with_capacity(dex_data.len());
        for json in dex_data.iter() {
            pool_accounts.push(json.pool);
        }
        // 池子
        let mut all_pool_account_data = self
            .get_account_data_with_data_slice(
                pool_accounts,
                DexType::MeteoraDLMM,
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
        // bitmap extension
        let all_bitmap_extension_account_data = self
            .get_bitmap_extension_accounts(rpc_client.clone(), &all_pool_account_data)
            .await;
        // bin array
        let all_bin_array_account_data = self
            .get_bin_array_accounts(
                rpc_client.clone(),
                &all_pool_account_data,
                &all_bitmap_extension_account_data,
                10,
                &mut invalid_pool,
            )
            .await;
        all_pool_account_data.retain(|account| !invalid_pool.contains(&account.account_key));
        dex_json.retain(|json| !invalid_pool.contains(&json.pool));
        info!(
            "【MeteoraDLMM】初始化Snapshot完毕, 初始化池子数量 : {}",
            all_pool_account_data.len()
        );
        if dex_json.is_empty() {
            vec![]
        } else {
            all_pool_account_data
                .into_iter()
                .chain(all_bitmap_extension_account_data.into_iter())
                .chain(all_bin_array_account_data.into_iter())
                .collect::<Vec<_>>()
        }
    }

    fn print_snapshot(&self, dex_json: &[DexJson]) -> anyhow::Result<()> {
        if let Some(json) = dex_json
            .iter()
            .find(|json| json.owner == METEORA_DLMM_PROGRAM_ID)
        {
            let lb_pair = get_account_data::<LbPair>(&json.pool)
                .ok_or(anyhow!("{}找不到缓存数据", json.pool))?;
            info!(
                "【{}】【{:?}】, key : {:?}\ndata : {:#?}",
                DexType::MeteoraDLMM,
                AccountType::Pool,
                json.pool,
                lb_pair
            );
            let bitmap_extension_key = pda_bit_map_extension_key(&json.pool);
            let bitmap_extension =
                get_account_data::<BinArrayBitmapExtension>(&bitmap_extension_key);
            info!(
                "【{}】【{:?}】, key : {:?}\ndata : {:#?}",
                DexType::MeteoraDLMM,
                AccountType::BinArrayBitmap,
                bitmap_extension_key,
                bitmap_extension
            );
            let _ = get_bin_array_pubkeys_for_swap(
                &json.pool,
                &lb_pair,
                bitmap_extension.as_ref(),
                true,
                1,
            )?
            .pop()
            .ok_or(anyhow!("池子{}未找到BinArray", json.pool))
            .and_then(|key| {
                let bin_array = get_account_data::<BinArray>(&key)
                    .ok_or(anyhow!("BinArray{}找不到数据", key))?;
                info!(
                    "【{}】【{:?}】, key : {:?}\ndata : {:#?}",
                    DexType::MeteoraDLMM,
                    AccountType::BinArray,
                    key,
                    bin_array
                );
                Ok(())
            });
        }
        Ok(())
    }
}

impl MeteoraDLMMSnapshotInitializer {
    async fn get_bitmap_extension_accounts(
        &self,
        rpc_client: Arc<RpcClient>,
        all_pool_account_data: &[AccountDataSlice],
    ) -> Vec<AccountDataSlice> {
        // bitmap_extension
        let mut all_bitmap_extension_accounts = all_pool_account_data
            .iter()
            .map(|account| {
                (
                    crate::dex::meteora_dlmm::derive_bin_array_bitmap_extension(
                        &account.account_key,
                    ),
                    account.account_key,
                )
            })
            .collect::<AHashMap<_, _>>();
        // 查询bitmap extension
        self.get_account_data_with_data_slice(
            all_bitmap_extension_accounts
                .iter()
                .map(|(key, _)| key.clone())
                .collect::<Vec<_>>(),
            DexType::MeteoraDLMM,
            AccountType::BinArrayBitmap,
            rpc_client.clone(),
        )
        .await
    }

    async fn get_bin_array_accounts(
        &self,
        rpc_client: Arc<RpcClient>,
        all_pool_account_data: &[AccountDataSlice],
        all_bitmap_extension_account_data: &[AccountDataSlice],
        load_count: u8,
        invalid_pool: &mut AHashSet<Pubkey>,
    ) -> Vec<AccountDataSlice> {
        let bin_array_keys = all_pool_account_data
            .iter()
            .filter_map(|account| {
                let pool_id = &account.account_key;
                // bitmap extension
                let bitmap_extension_key =
                    crate::dex::meteora_dlmm::derive_bin_array_bitmap_extension(pool_id);
                let bitmap_extension =
                    all_bitmap_extension_account_data
                        .iter()
                        .find_map(|bitmap_extension| {
                            if bitmap_extension.account_key == bitmap_extension_key
                                && bitmap_extension.dynamic_slice_data.as_ref().is_some()
                            {
                                Some(BinArrayBitmapExtension::from_slice_data(
                                    bitmap_extension.dynamic_slice_data.as_ref().unwrap(),
                                ))
                            } else {
                                None
                            }
                        });
                let lb_pair = LbPair::from_slice_data(
                    account.static_slice_data.as_ref().unwrap(),
                    account.dynamic_slice_data.as_ref().unwrap(),
                );
                let mut bin_array_keys = get_bin_array_pubkeys_for_swap(
                    pool_id,
                    &lb_pair,
                    bitmap_extension.as_ref(),
                    true,
                    load_count,
                )
                .unwrap_or(vec![]);
                bin_array_keys.extend(
                    get_bin_array_pubkeys_for_swap(
                        pool_id,
                        &lb_pair,
                        bitmap_extension.as_ref(),
                        false,
                        load_count,
                    )
                    .unwrap_or(vec![]),
                );
                if bin_array_keys.is_empty() {
                    None
                } else {
                    Some(
                        bin_array_keys
                            .into_iter()
                            .map(|key| (key, pool_id.clone()))
                            .collect::<Vec<_>>(),
                    )
                }
            })
            .flatten()
            .collect::<AHashMap<_, _>>();

        // 查询tick array state
        let mut all_bin_array_account_data = self
            .get_account_data_with_data_slice(
                bin_array_keys
                    .iter()
                    .map(|(key, _)| key.clone())
                    .collect::<Vec<_>>(),
                DexType::MeteoraDLMM,
                AccountType::BinArray,
                rpc_client.clone(),
            )
            .await;
        all_bin_array_account_data.retain(|account| {
            if account.dynamic_slice_data.as_ref().is_none() {
                invalid_pool.insert(bin_array_keys.get(&account.account_key).unwrap().clone());
                false
            } else {
                true
            }
        });
        all_bin_array_account_data
    }
}
