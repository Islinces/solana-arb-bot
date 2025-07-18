use crate::dex::data_slice::{get_data_slice_size, SliceType};
use crate::dex::global_cache::get_account_data;
use crate::dex::raydium_clmm::state::pda_bit_map_extension_key;
use crate::dex::snapshot::{AccountDataSlice, SnapshotInitializer};
use crate::dex::{AccountType, DexType};
use crate::dex_data::DexJson;
use ahash::{AHashMap, AHashSet};
use anyhow::anyhow;
use async_trait::async_trait;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tracing::info;
use crate::dex::oracle::{get_oracle_address, Oracle};
use crate::dex::orca_whirlpools::math::get_tick_array_start_tick_index;
use crate::dex::orca_whirlpools::WHIRLPOOL_ID;
use crate::dex::tick_array::{get_tick_array_address, get_tick_array_keys, TickArray};
use crate::dex::whirlpool::Whirlpool;

pub struct OrcaWhirlpoolsSnapshotInitializer;

#[async_trait]
impl SnapshotInitializer for OrcaWhirlpoolsSnapshotInitializer {
    async fn init_snapshot(
        &self,
        dex_json: &mut Vec<DexJson>,
        rpc_client: Arc<RpcClient>,
    ) -> Vec<AccountDataSlice> {
        let dex_data = dex_json
            .iter()
            .filter(|json| json.owner == WHIRLPOOL_ID)
            .collect::<Vec<_>>();
        if dex_data.is_empty() {
            return vec![];
        }
        info!("【OrcaWhirls】开始初始化Snapshot...");
        let mut invalid_pool = AHashSet::with_capacity(dex_data.len());
        let mut pool_accounts = Vec::with_capacity(dex_data.len());
        for json in dex_data.iter() {
            pool_accounts.push(json.pool);
        }
        // pool
        let mut all_pool_account_data = self
            .get_account_data_with_data_slice(
                pool_accounts,
                DexType::OrcaWhirl,
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
        // oracle
        let all_oracle_account_data = self
            .get_oracle_accounts(rpc_client.clone(), &all_pool_account_data)
            .await;
        // tick array
        let tick_array_account_data = self
            .get_tick_array_accounts(rpc_client.clone(), &all_pool_account_data, 3)
            .await;
        dex_json.retain(|json| !invalid_pool.contains(&json.pool));
        info!(
            "【OrcaWhirls】初始化Snapshot完毕, 初始化池子数量 : {}",
            all_pool_account_data.len()
        );
        if dex_json.is_empty() {
            vec![]
        } else {
            all_pool_account_data
                .into_iter()
                .chain(all_oracle_account_data.into_iter())
                .chain(tick_array_account_data.into_iter())
                .collect::<Vec<_>>()
        }
    }

    // #[cfg(feature = "print_slice_data")]
    fn print_snapshot(&self, dex_json: &[DexJson]) -> anyhow::Result<()> {
        if let Some(json) = dex_json
            .iter()
            .find(|json| &json.owner == DexType::OrcaWhirl.get_ref_program_id())
        {
            let pool = get_account_data::<Whirlpool>(&json.pool).unwrap();
            info!(
                "【{}】【{:?}】, key : {:?}\ndata : {:#?}",
                DexType::OrcaWhirl,
                AccountType::Pool,
                json.pool,
                pool
            );

            let oracle_key = get_oracle_address(&json.pool)?;
            let oracle = get_account_data::<Oracle>(&oracle_key);
            info!(
                "【{}】【{:?}】, key : {:?}\ndata : {:#?}",
                DexType::OrcaWhirl,
                AccountType::Oracle,
                oracle_key,
                oracle
            );

            let tick_array_key = get_tick_array_address(
                &json.pool,
                get_tick_array_start_tick_index(pool.tick_current_index, pool.tick_spacing),
            )?
            .0;
            let tick_array = get_account_data::<TickArray>(&tick_array_key)
                .ok_or(anyhow!("TickArray{}找不到数据", tick_array_key))?;
            info!(
                "【{}】【{:?}】, key : {:?}\ndata : {:#?}",
                DexType::OrcaWhirl,
                AccountType::TickArray,
                tick_array_key,
                tick_array,
            );
        }
        Ok(())
    }
}

impl OrcaWhirlpoolsSnapshotInitializer {
    async fn get_oracle_accounts(
        &self,
        rpc_client: Arc<RpcClient>,
        all_pool_account_data: &[AccountDataSlice],
    ) -> Vec<AccountDataSlice> {
        let all_oracle_accounts = all_pool_account_data
            .iter()
            .map(|pool| get_oracle_address(&pool.account_key).unwrap())
            .collect::<Vec<_>>();

        // 查询oracle
        let mut all_oracle_account_data = self
            .get_account_data_with_data_slice(
                all_oracle_accounts,
                DexType::OrcaWhirl,
                AccountType::Oracle,
                rpc_client.clone(),
            )
            .await;
        // 并不是每个pool都有
        all_oracle_account_data.retain(|account| {
            account.dynamic_slice_data.as_ref().is_some()
                && account.static_slice_data.as_ref().is_some()
        });
        all_oracle_account_data
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
                    != get_data_slice_size(
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
        load_count: u8,
    ) -> Vec<AccountDataSlice> {
        let tick_array_state_keys = all_pool_account_data
            .iter()
            .filter_map(|account| {
                let pool = Whirlpool::from_slice_data(
                    account.static_slice_data.as_ref().unwrap().as_slice(),
                    account.dynamic_slice_data.as_ref().unwrap().as_slice(),
                )
                .unwrap();
                let pool_id = account.account_key;
                let mut tick_array_keys = get_tick_array_keys(pool_id, &pool, load_count, true)
                    .map_or(Vec::new(), |keys| keys);
                tick_array_keys.extend(
                    get_tick_array_keys(pool_id, &pool, load_count, false).map_or(vec![], |k| k),
                );
                if tick_array_keys.is_empty() {
                    None
                } else {
                    Some(tick_array_keys)
                }
            })
            .flatten()
            .collect::<AHashSet<_>>();

        // 查询tick array state
        let mut all_tick_array_state_account_data = self
            .get_account_data_with_data_slice(
                tick_array_state_keys.into_iter().collect::<Vec<_>>(),
                DexType::OrcaWhirl,
                AccountType::TickArray,
                rpc_client.clone(),
            )
            .await;
        all_tick_array_state_account_data
            .retain(|account| account.dynamic_slice_data.as_ref().is_some());
        all_tick_array_state_account_data
    }
}
