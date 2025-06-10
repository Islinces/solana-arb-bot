use crate::data_slice::SliceType;
use crate::dex::raydium_clmm::state::{PoolState, TickArrayBitmapExtension};
use crate::dex::raydium_clmm::utils::load_cur_and_next_specify_count_tick_array_key;
use crate::dex::raydium_clmm::RAYDIUM_CLMM_PROGRAM_ID;
use crate::dex_data::DexJson;
use crate::interface1::{AccountType, DexType};
use crate::{AccountDataSlice, SnapshotInitializer};
use ahash::{AHashMap, AHashSet};
use async_trait::async_trait;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tracing::info;

pub struct RaydiumCLMMSnapshotInitializer;

#[async_trait]
impl SnapshotInitializer for RaydiumCLMMSnapshotInitializer {
    async fn init_snapshot(
        &self,
        dex_json: &mut Vec<DexJson>,
        rpc_client: Arc<RpcClient>,
    ) -> Vec<AccountDataSlice> {
        let dex_data = dex_json
            .iter()
            .filter(|json| json.owner == RAYDIUM_CLMM_PROGRAM_ID)
            .collect::<Vec<_>>();
        if dex_data.is_empty() {
            return vec![];
        }
        info!("【RaydiumCLMM】开始初始化Snapshot...");
        let mut invalid_pool = AHashSet::with_capacity(dex_data.len());
        let mut pool_accounts = Vec::with_capacity(dex_data.len());
        for json in dex_data.iter() {
            pool_accounts.push(json.pool);
        }
        let dex_type = DexType::RaydiumCLMM;
        // 池子
        let mut all_pool_account_data = self
            .get_account_data_with_data_slice(
                pool_accounts,
                dex_type.clone(),
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
        // 查询amm config
        let all_amm_config_account_data = self
            .get_amm_config_accounts(
                rpc_client.clone(),
                &all_pool_account_data,
                &mut invalid_pool,
            )
            .await;
        all_pool_account_data.retain(|account| !invalid_pool.contains(&account.account_key));
        // bitmap extension
        let all_bitmap_extension_account_data = self
            .get_bitmap_extension_accounts(
                rpc_client.clone(),
                &all_pool_account_data,
                &mut invalid_pool,
            )
            .await;
        all_pool_account_data.retain(|account| !invalid_pool.contains(&account.account_key));
        // tick array
        let tick_array_account_data = self
            .get_tick_array_accounts(
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
            "【RaydiumCLMM】初始化Snapshot完毕, 初始化池子数量 : {}",
            all_pool_account_data.len()
        );
        if dex_json.is_empty() {
            vec![]
        } else {
            all_pool_account_data
                .into_iter()
                .chain(all_amm_config_account_data.into_iter())
                .chain(all_bitmap_extension_account_data.into_iter())
                .chain(tick_array_account_data.into_iter())
                .collect::<Vec<_>>()
        }
    }
}

impl RaydiumCLMMSnapshotInitializer {
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
                DexType::RaydiumCLMM,
                AccountType::AmmConfig,
                rpc_client.clone(),
            )
            .await;
        all_amm_config_account_data.retain(|account| {
            if account.static_slice_data.as_ref().is_none_or(|data| {
                data.len()
                    != crate::data_slice::get_slice_size(
                        DexType::RaydiumCLMM,
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

    async fn get_bitmap_extension_accounts(
        &self,
        rpc_client: Arc<RpcClient>,
        all_pool_account_data: &[AccountDataSlice],
        invalid_pool: &mut AHashSet<Pubkey>,
    ) -> Vec<AccountDataSlice> {
        // bitmap_extension
        let mut all_bitmap_extension_accounts = all_pool_account_data
            .iter()
            .map(|account| {
                (
                    crate::dex::raydium_clmm::state::pda_bit_map_extension_key(
                        &account.account_key,
                    ),
                    account.account_key,
                )
            })
            .collect::<AHashMap<_, _>>();
        // 查询bitmap extension
        let mut all_bitmap_extension_account_data = self
            .get_account_data_with_data_slice(
                all_bitmap_extension_accounts
                    .iter()
                    .map(|(key, _)| key.clone())
                    .collect::<Vec<_>>(),
                DexType::RaydiumCLMM,
                AccountType::TickArrayBitmap,
                rpc_client.clone(),
            )
            .await;
        all_bitmap_extension_account_data.retain(|account| {
            if account.dynamic_slice_data.as_ref().is_none_or(|data| {
                data.len()
                    != crate::data_slice::get_slice_size(
                        DexType::RaydiumCLMM,
                        AccountType::TickArrayBitmap,
                        SliceType::Subscribed,
                    )
                    .unwrap()
                    .unwrap()
            }) {
                invalid_pool.insert(
                    all_bitmap_extension_accounts
                        .get(&account.account_key)
                        .unwrap()
                        .clone(),
                );
                false
            } else {
                true
            }
        });
        all_bitmap_extension_account_data
    }

    async fn get_tick_array_accounts(
        &self,
        rpc_client: Arc<RpcClient>,
        all_pool_account_data: &[AccountDataSlice],
        all_bitmap_extension_account_data: &[AccountDataSlice],
        load_count: u8,
        invalid_pool: &mut AHashSet<Pubkey>,
    ) -> Vec<AccountDataSlice> {
        let tick_array_state_keys = all_pool_account_data
            .iter()
            .filter_map(|account| {
                let pool_id = account.account_key;
                // bitmap extension
                let bitmap_extension_key =
                    crate::dex::raydium_clmm::state::pda_bit_map_extension_key(&pool_id);
                let tick_array_bitmap_extension = all_bitmap_extension_account_data
                    .iter()
                    .find_map(|bitmap_extension| {
                        if bitmap_extension.account_key == bitmap_extension_key {
                            Some(TickArrayBitmapExtension::from_slice_data(
                                bitmap_extension.dynamic_slice_data.as_ref().unwrap(),
                            ))
                        } else {
                            None
                        }
                    });
                let pool_state = PoolState::from_slice_data(
                    account.static_slice_data.as_ref().unwrap(),
                    account.dynamic_slice_data.as_ref().unwrap(),
                );
                let mut tick_array_states = load_cur_and_next_specify_count_tick_array_key(
                    load_count,
                    &pool_id,
                    &pool_state,
                    &tick_array_bitmap_extension,
                    true,
                )
                .unwrap_or(vec![]);
                tick_array_states.extend(
                    load_cur_and_next_specify_count_tick_array_key(
                        load_count,
                        &pool_id,
                        &pool_state,
                        &tick_array_bitmap_extension,
                        false,
                    )
                    .unwrap_or(vec![]),
                );
                if tick_array_states.is_empty() {
                    None
                } else {
                    Some(
                        tick_array_states
                            .into_iter()
                            .map(|key| (key, pool_id))
                            .collect::<Vec<_>>(),
                    )
                }
            })
            .flatten()
            .collect::<AHashMap<_, _>>();

        // 查询tick array state
        let mut all_tick_array_state_account_data = self
            .get_account_data_with_data_slice(
                tick_array_state_keys
                    .iter()
                    .map(|(key, _)| key.clone())
                    .collect::<Vec<_>>(),
                DexType::RaydiumCLMM,
                AccountType::TickArray,
                rpc_client.clone(),
            )
            .await;
        all_tick_array_state_account_data.retain(|account| {
            if account.dynamic_slice_data.as_ref().is_none() {
                invalid_pool.insert(
                    tick_array_state_keys
                        .get(&account.account_key)
                        .unwrap()
                        .clone(),
                );
                false
            } else {
                true
            }
        });
        all_tick_array_state_account_data
    }
}
