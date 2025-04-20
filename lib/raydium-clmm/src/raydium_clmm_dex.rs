use crate::clmm_pool::ClmmPool;
use crate::sdk::tick_array::TickArrayState;
use crate::sdk::tickarray_bitmap_extension::TickArrayBitmapExtension;
use crate::sdk::utils::{
    deserialize_anchor_account, load_cur_and_next_specify_count_tick_array_key,
};
use crate::sdk::{config, pool};
use crate::state::PoolSnapshotInfo;
use dex::interface::{DexInterface, DexPoolInterface, GrpcSubscriber};
use dex::state::FetchConfig;
use dex::trigger::TriggerEvent;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::pubkey::Pubkey;
use solana_sdk::commitment_config::CommitmentConfig;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

pub struct RaydiumClmmDex {
    pub clmm_pool: Vec<Arc<dyn DexPoolInterface>>,
}

#[async_trait::async_trait]
impl DexInterface for RaydiumClmmDex {
    fn name(&self) -> String {
        "raydium-clmm".to_string()
    }

    fn get_base_pools(&self) -> Vec<Arc<dyn DexPoolInterface>> {
        self.clmm_pool.clone()
    }

    async fn fetch_pool_base_info(
        rpc_client: &RpcClient,
        fetch_config: &FetchConfig,
    ) -> anyhow::Result<Arc<dyn DexInterface>>
    where
        Self: Sized,
    {
        let need_fetch_accounts =
            generate_subscribe_account_with_specify_mint(&fetch_config.subscribe_mints);
        let amm_config_keys = need_fetch_accounts.0;
        let amm_configs = rpc_client
            .get_multiple_accounts_with_commitment(&amm_config_keys, CommitmentConfig::finalized())
            .await?
            .value
            .iter()
            .zip(amm_config_keys)
            .map(|(account, key)| {
                (
                    key,
                    deserialize_anchor_account::<config::AmmConfig>(account.as_ref().unwrap())
                        .unwrap()
                        .trade_fee_rate,
                )
            })
            .collect::<HashMap<_, _>>();
        let mut clmm_pools: Vec<Arc<dyn DexPoolInterface>> =
            Vec::with_capacity(need_fetch_accounts.1.len());
        for pool_chunks in need_fetch_accounts.1.chunks(50) {
            let chunks_accounts = pool_chunks
                .iter()
                .flat_map(|pool_info| {
                    vec![pool_info.pool_id, pool_info.tick_array_bitmap_extension_key]
                })
                .collect::<Vec<_>>();
            let mut tick_array_state_keys = Vec::with_capacity(chunks_accounts.len() * 10);
            let mut snapshot_pool_map = rpc_client
                .get_multiple_accounts_with_commitment(
                    &chunks_accounts,
                    CommitmentConfig::finalized(),
                )
                .await?
                .value
                .into_iter()
                .zip(chunks_accounts)
                .collect::<Vec<_>>()
                .chunks(2)
                .into_iter()
                .map(|info| {
                    let (pool_account, pool_id) = &info[0];
                    let (bitmap_extension_account, bitmap_extension_key) = &info[1];
                    let pool_state = deserialize_anchor_account::<pool::PoolState>(
                        pool_account.as_ref().unwrap(),
                    )
                    .unwrap();
                    let tick_array_bitmap_extension =
                        deserialize_anchor_account::<TickArrayBitmapExtension>(
                            bitmap_extension_account.as_ref().unwrap(),
                        )
                        .unwrap();
                    let left_tick_array_state_keys = load_cur_and_next_specify_count_tick_array_key(
                        5,
                        pool_id,
                        &pool_state,
                        &tick_array_bitmap_extension,
                        true,
                    );
                    let right_tick_array_state_keys =
                        load_cur_and_next_specify_count_tick_array_key(
                            5,
                            pool_id,
                            &pool_state,
                            &tick_array_bitmap_extension,
                            false,
                        );
                    tick_array_state_keys.extend(left_tick_array_state_keys);
                    tick_array_state_keys.extend(right_tick_array_state_keys);
                    let trade_fee_rate = amm_configs.get(&pool_state.amm_config).unwrap().clone();
                    (
                        pool_id.clone(),
                        PoolSnapshotInfo {
                            pool_id: pool_id.clone(),
                            pool_state,
                            tick_array_bitmap_extension_key: bitmap_extension_key.clone(),
                            tick_array_bitmap_extension,
                            trade_fee_rate,
                            tick_array_states: Vec::with_capacity(10),
                        },
                    )
                })
                .collect::<HashMap<_, _>>();
            for tick_array in rpc_client
                .get_multiple_accounts_with_commitment(
                    &tick_array_state_keys,
                    CommitmentConfig::finalized(),
                )
                .await?
                .value
            {
                if let Some(tick_array) = tick_array {
                    let tick_array_state =
                        deserialize_anchor_account::<TickArrayState>(&tick_array)?;
                    if let Entry::Occupied(mut exists) =
                        snapshot_pool_map.entry(tick_array_state.pool_id.clone())
                    {
                        exists.get_mut().tick_array_states.push(tick_array_state);
                    }
                }
            }
            for (_, pool_snapshot_info) in snapshot_pool_map.into_iter() {
                clmm_pools.push(Arc::new(ClmmPool::from(pool_snapshot_info)));
            }
        }
        Ok(Arc::new(Self {
            clmm_pool: clmm_pools,
        }))
    }
}

fn generate_subscribe_account_with_specify_mint(
    subscribe_mints: &Vec<Pubkey>,
) -> (Vec<Pubkey>, Vec<FetchPoolInfo>) {
    let mut amm_config_keys = Vec::with_capacity(9);
    for amm_config_index in 0..9 {
        let (amm_config_key, _) = Pubkey::find_program_address(
            &[
                config::AMM_CONFIG_SEED.as_bytes(),
                &(amm_config_index as u16).to_be_bytes(),
            ],
            &crate::ID,
        );
        amm_config_keys.push(amm_config_key);
    }
    let all_mint_combines = subscribe_mints
        .iter()
        .flat_map(|mint_0| {
            subscribe_mints.iter().map(move |mint_1| {
                let mut mint0 = mint_0;
                let mut mint1 = mint_1;
                if mint0 > mint1 {
                    let temp_mint = mint0;
                    mint0 = mint1;
                    mint1 = temp_mint;
                }
                (mint0, mint1)
            })
        })
        .collect::<HashSet<_>>();
    let pools = amm_config_keys
        .iter()
        .flat_map(|amm_config_index| {
            all_mint_combines
                .iter()
                .map(|(mint_0, mint_1)| {
                    let pool_id = Pubkey::find_program_address(
                        &[
                            crate::sdk::pool::POOL_SEED.as_bytes(),
                            amm_config_index.to_bytes().as_ref(),
                            mint_0.to_bytes().as_ref(),
                            mint_1.to_bytes().as_ref(),
                        ],
                        &crate::ID,
                    )
                    .0;
                    let tick_array_bitmap_extension_key = Pubkey::find_program_address(
                        &[
                            crate::sdk::pool::POOL_TICK_ARRAY_BITMAP_SEED.as_bytes(),
                            pool_id.to_bytes().as_ref(),
                        ],
                        &crate::ID,
                    )
                    .0;
                    FetchPoolInfo {
                        pool_id: pool_id.clone(),
                        tick_array_bitmap_extension_key: tick_array_bitmap_extension_key.clone(),
                        mint_0: *mint_0.clone(),
                        mint_1: *mint_1.clone(),
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    (amm_config_keys, pools)
}

struct FetchPoolInfo {
    pub pool_id: Pubkey,
    pub tick_array_bitmap_extension_key: Pubkey,
    pub mint_0: Pubkey,
    pub mint_1: Pubkey,
}

pub struct RaydiumClmmGrpcSubscriber();

#[async_trait::async_trait]
impl GrpcSubscriber for RaydiumClmmGrpcSubscriber {
    async fn subscribe(
        dex: Arc<dyn DexInterface>,
        fetch_config: Arc<FetchConfig>,
        account_write_sender: UnboundedSender<Box<dyn DexPoolInterface>>,
        trigger_event_sender: UnboundedSender<Box<dyn TriggerEvent>>,
    ) {
        todo!()
    }
}
