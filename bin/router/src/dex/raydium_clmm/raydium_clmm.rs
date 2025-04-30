use crate::cache::PoolState::RaydiumCLMM;
use crate::cache::{Mint, Pool};
use crate::dex::common::utils::{change_data_if_not_same, SwapDirection};
use crate::dex::raydium_clmm::sdk::config::AmmConfig;
use crate::dex::raydium_clmm::sdk::pool::PoolState;
use crate::dex::raydium_clmm::sdk::tick_array::TickArrayState;
use crate::dex::raydium_clmm::sdk::tickarray_bitmap_extension::TickArrayBitmapExtension;
use crate::dex::raydium_clmm::sdk::utils::{
    deserialize_anchor_account, load_cur_and_next_specify_count_tick_array,
};
use crate::dex::raydium_clmm::sdk::{config, utils};
use crate::interface::GrpcMessage::RaydiumCLMMData;
use crate::interface::{
    AccountSnapshotFetcher, AccountUpdate, CacheUpdater, Dex, GrpcAccountUpdateType, GrpcMessage,
    GrpcSubscribeRequestGenerator, Protocol, ReadyGrpcMessageOperator, SubscribeKey,
};
use anyhow::anyhow;
use anyhow::Result;
use arrayref::{array_ref, array_refs};
use base58::ToBase58;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::pubkey::Pubkey;
use solana_sdk::commitment_config::CommitmentConfig;
use std::collections::{HashMap, VecDeque};
use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{debug, error};
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestAccountsDataSlice,
    SubscribeRequestFilterAccounts,
};

#[derive(Clone)]
pub struct RaydiumClmmDex {
    pool_id: Pubkey,
    swap_direction: SwapDirection,
    mint_0: Pubkey,
    mint_1: Pubkey,
    tick_spacing: u16,
    trade_fee_rate: u32,
    liquidity: u128,
    sqrt_price_x64: u128,
    tick_current: i32,
    tick_array_bitmap: [u64; 16],
    tick_array_bitmap_extension: TickArrayBitmapExtension,
    zero_to_one_tick_array_states: VecDeque<TickArrayState>,
    one_to_zero_tick_array_states: VecDeque<TickArrayState>,
}

impl RaydiumClmmDex {
    pub fn new(pool: Pool, amount_in_mint: Pubkey) -> Option<Self> {
        if let RaydiumCLMM {
            tick_spacing,
            trade_fee_rate,
            liquidity,
            sqrt_price_x64,
            tick_current,
            tick_array_bitmap,
            tick_array_bitmap_extension,
            zero_to_one_tick_array_states,
            one_to_zero_tick_array_states,
        } = pool.state.clone()
        {
            Some(Self {
                swap_direction: if amount_in_mint == pool.mint_0() {
                    SwapDirection::Coin2PC
                } else {
                    SwapDirection::PC2Coin
                },
                pool_id: pool.pool_id,
                mint_0: pool.mint_0(),
                mint_1: pool.mint_1(),
                tick_current,
                tick_spacing,
                trade_fee_rate,
                liquidity,
                sqrt_price_x64,
                tick_array_bitmap,
                tick_array_bitmap_extension,
                zero_to_one_tick_array_states,
                one_to_zero_tick_array_states,
            })
        } else {
            None
        }
    }

    fn get_out_put_amount_and_remaining_accounts(
        input_amount: u64,
        sqrt_price_limit_x64: Option<u128>,
        zero_for_one: bool,
        is_base_input: bool,
        pool_config: &AmmConfig,
        pool_state: &PoolState,
        tickarray_bitmap_extension: &Option<TickArrayBitmapExtension>,
        tick_arrays: &mut VecDeque<TickArrayState>,
    ) -> Result<(u64, u64, VecDeque<i32>), &'static str> {
        utils::get_out_put_amount_and_remaining_accounts(
            input_amount,
            sqrt_price_limit_x64,
            zero_for_one,
            is_base_input,
            pool_config,
            pool_state,
            tickarray_bitmap_extension,
            tick_arrays,
        )
    }
}

impl Debug for RaydiumClmmDex {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RaydiumClmmDex: {},{:?}",
            self.pool_id, self.swap_direction
        )
    }
}

#[async_trait::async_trait]
impl Dex for RaydiumClmmDex {
    async fn quote(&self, amount_in: u64) -> Option<u64> {
        if amount_in == u64::MIN {
            return None;
        }
        //TODO：优化计算参数
        let mut amm_config = AmmConfig::default();
        amm_config.trade_fee_rate = self.trade_fee_rate;
        let mut pool_state = PoolState::default();
        pool_state.tick_current = self.tick_current;
        pool_state.tick_spacing = self.tick_spacing;
        pool_state.tick_array_bitmap = self.tick_array_bitmap;
        pool_state.liquidity = self.liquidity;
        pool_state.sqrt_price_x64 = self.sqrt_price_x64;

        let (zero_for_one, mut tick_arrays) = match self.swap_direction {
            SwapDirection::PC2Coin => (false, self.one_to_zero_tick_array_states.clone()),
            SwapDirection::Coin2PC => (true, self.zero_to_one_tick_array_states.clone()),
        };
        let result = Self::get_out_put_amount_and_remaining_accounts(
            amount_in,
            None,
            zero_for_one,
            true,
            &amm_config,
            &pool_state,
            &Some(self.tick_array_bitmap_extension),
            &mut tick_arrays,
        );
        match result {
            Ok((amount_out, _, _)) => Some(amount_out),
            Err(e) => {
                error!("get_out_put_amount_and_remaining_accounts error: {:?}", e);
                None
            }
        }
    }

    fn clone_self(&self) -> Box<dyn Dex> {
        Box::new(self.clone())
    }
}

pub struct RaydiumClmmGrpcMessageOperator {
    update_account: AccountUpdate,
    txn: Option<String>,
    pool_id: Option<Pubkey>,
    grpc_message: Option<GrpcMessage>,
}

impl RaydiumClmmGrpcMessageOperator {
    pub fn new(update_account: AccountUpdate) -> Self {
        Self {
            update_account,
            txn: None,
            pool_id: None,
            grpc_message: None,
        }
    }
}
impl ReadyGrpcMessageOperator for RaydiumClmmGrpcMessageOperator {
    fn parse_message(&mut self) -> Result<()> {
        let account_type = &self.update_account.account_type;
        let account = &self.update_account.account;
        if let Some(update_account_info) = &account.account {
            let data = &update_account_info.data;
            let pool_id = Pubkey::try_from(update_account_info.pubkey.clone()).unwrap();
            let txn = &update_account_info
                .txn_signature
                .as_ref()
                .unwrap()
                .to_base58();
            let txn = txn.clone();
            match account_type {
                GrpcAccountUpdateType::PoolState => {
                    let src = array_ref![data, 0, 180];
                    let (
                        liquidity,
                        price,
                        tick_current,
                        bitmap,
                        _total_fees_token_0,
                        _total_fees_token_1,
                    ) = array_refs![src, 16, 16, 4, 128, 8, 8];
                    self.pool_id = Some(pool_id);
                    self.txn = Some(txn);
                    self.grpc_message = Some(RaydiumCLMMData {
                        pool_id,
                        liquidity: u128::from_le_bytes(*liquidity),
                        sqrt_price_x64: u128::from_le_bytes(*price),
                        tick_current: i32::from_le_bytes(*tick_current),
                        tick_array_bitmap: bitmap
                            .chunks_exact(8)
                            .map(|chunk| u64::from_le_bytes(chunk.try_into().unwrap()))
                            .collect::<Vec<_>>()
                            .try_into()
                            .unwrap(),
                    });
                    Ok(())
                }
                _ => Err(anyhow!("")),
            }
        } else {
            Err(anyhow!(""))
        }
    }

    fn change_and_return_ready_data(&self, _old: &mut GrpcMessage) -> anyhow::Result<()> {
        unimplemented!()
    }

    fn get_cache_key(&self) -> (String, Pubkey) {
        unimplemented!()
    }

    fn get_insert_data(&self) -> GrpcMessage {
        self.grpc_message.as_ref().unwrap().clone()
    }
}

pub struct RaydiumClmmSubscribeRequestCreator;

impl GrpcSubscribeRequestGenerator for RaydiumClmmSubscribeRequestCreator {
    fn create_subscribe_requests(
        &self,
        pools: &[Pool],
    ) -> Option<Vec<(SubscribeKey, SubscribeRequest)>> {
        let mut subscribe_pool_accounts = HashMap::new();
        subscribe_pool_accounts.insert(
            format!("{:?}", Protocol::RaydiumCLmm),
            SubscribeRequestFilterAccounts {
                account: pools
                    .iter()
                    .map(|pool| pool.pool_id.to_string())
                    .collect::<Vec<_>>(),
                ..Default::default()
            },
        );
        let pool_request = SubscribeRequest {
            accounts: subscribe_pool_accounts,
            commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
            accounts_data_slice: vec![
                // liquidity
                SubscribeRequestAccountsDataSlice {
                    offset: 237,
                    length: 16,
                },
                // sqrt_price_x64
                SubscribeRequestAccountsDataSlice {
                    offset: 253,
                    length: 16,
                },
                // tick_current
                SubscribeRequestAccountsDataSlice {
                    offset: 269,
                    length: 4,
                },
                // tick_array_bitmap
                SubscribeRequestAccountsDataSlice {
                    offset: 904,
                    length: 128,
                },
                // total_fees_token_0
                SubscribeRequestAccountsDataSlice {
                    offset: 1032,
                    length: 8,
                },
                // total_fees_token_1
                SubscribeRequestAccountsDataSlice {
                    offset: 1048,
                    length: 8,
                },
            ],
            ..Default::default()
        };
        Some(vec![(
            (Protocol::RaydiumCLmm, GrpcAccountUpdateType::PoolState),
            pool_request,
        )])
    }
}

pub struct RaydiumClmmSnapshotFetcher;

impl RaydiumClmmSnapshotFetcher {
    fn generate_all_config_keys() -> Vec<Pubkey> {
        let mut all_amm_config_keys = Vec::new();
        for index in 0..9 {
            let index = index as u16;
            let (amm_config_key, __bump) = Pubkey::find_program_address(
                &[config::AMM_CONFIG_SEED.as_bytes(), &index.to_be_bytes()],
                &crate::dex::raydium_clmm::ID,
            );
            all_amm_config_keys.push(amm_config_key);
        }
        all_amm_config_keys
    }
}

#[async_trait::async_trait]
impl AccountSnapshotFetcher for RaydiumClmmSnapshotFetcher {
    async fn fetch_snapshot(
        &self,
        pool_ids: Vec<Pubkey>,
        rpc_client: Arc<RpcClient>,
    ) -> Option<Vec<Pool>> {
        let all_config_keys = RaydiumClmmSnapshotFetcher::generate_all_config_keys();
        let amm_config_map = rpc_client
            .get_multiple_accounts_with_commitment(
                all_config_keys.as_slice(),
                CommitmentConfig::finalized(),
            )
            .await
            .unwrap()
            .value
            .into_iter()
            .zip(all_config_keys)
            .filter_map(|(account, config_key)| {
                if let Ok(amm_config_state) =
                    deserialize_anchor_account::<config::AmmConfig>(account.as_ref().unwrap())
                {
                    Some((config_key, amm_config_state.trade_fee_rate))
                } else {
                    None
                }
            })
            .collect::<HashMap<_, _>>();
        let amm_config_map = Arc::new(amm_config_map);
        let mut join_set = JoinSet::new();
        // pool_id 和 bitmap_extension_id 每个pool按照顺序排列
        let pool_id_and_bitmap_extension_id_pairs = pool_ids
            .iter()
            .zip(
                pool_ids
                    .iter()
                    .map(|pool_id| TickArrayBitmapExtension::key(*pool_id))
                    .collect::<Vec<_>>(),
            )
            .flat_map(|(pool_id, bitmap_extension_id)| vec![*pool_id, bitmap_extension_id])
            .collect::<Vec<_>>();
        // 一次执行100个pool
        for chunks_one_pools in pool_id_and_bitmap_extension_id_pairs.chunks(100 * 2) {
            // chunks_one_pools[0]: pool_id
            // chunks_one_pools[1]: bitmap_extension_id
            let chunks_pool_ids = chunks_one_pools.to_vec();
            let rpc_client = rpc_client.clone();
            let amm_config_map = amm_config_map.clone();
            join_set.spawn(async move {
                let mut pools = Vec::with_capacity(chunks_pool_ids.len());
                // 一次性查询100个pool和对应的bitmap_extension
                let pool_and_bitmap_extension_account_pair = chunks_pool_ids
                    .iter()
                    .zip(
                        rpc_client
                            .get_multiple_accounts_with_commitment(
                                &chunks_pool_ids,
                                CommitmentConfig::finalized(),
                            )
                            .await
                            .unwrap()
                            .value,
                    )
                    .collect::<Vec<_>>();
                for one_pool_pair in pool_and_bitmap_extension_account_pair.chunks(2) {
                    let pool_pair = &one_pool_pair[0];
                    let bitmap_extension_pair = &one_pool_pair[1];
                    if let (pool_id, Some(pool_account)) = pool_pair {
                        let pool_id = **pool_id;
                        if let (_bitmap_extension_id, Some(bitmap_extension_account)) =
                            bitmap_extension_pair
                        {
                            let pool_state =
                                deserialize_anchor_account::<PoolState>(pool_account).unwrap();
                            let tick_array_bitmap_extension =
                                deserialize_anchor_account::<TickArrayBitmapExtension>(
                                    bitmap_extension_account,
                                )
                                .unwrap();
                            let zero_to_one_tick_array_states =
                                load_cur_and_next_specify_count_tick_array(
                                    rpc_client.clone(),
                                    10,
                                    &pool_id,
                                    &Pubkey::default(),
                                    &pool_state,
                                    &tick_array_bitmap_extension,
                                    true,
                                )
                                .await;
                            let one_to_zero_tick_array_states =
                                load_cur_and_next_specify_count_tick_array(
                                    rpc_client.clone(),
                                    10,
                                    &pool_id,
                                    &Pubkey::default(),
                                    &pool_state,
                                    &tick_array_bitmap_extension,
                                    false,
                                )
                                .await;
                            let trade_fee_rate =
                                amm_config_map.get(&pool_state.amm_config).unwrap().clone();
                            pools.push(Pool {
                                protocol: Protocol::RaydiumCLmm,
                                pool_id,
                                tokens: vec![
                                    Mint {
                                        mint: pool_state.token_mint_0.clone(),
                                    },
                                    Mint {
                                        mint: pool_state.token_mint_1.clone(),
                                    },
                                ],
                                state: RaydiumCLMM {
                                    tick_spacing: pool_state.tick_spacing,
                                    trade_fee_rate,
                                    liquidity: pool_state.liquidity,
                                    sqrt_price_x64: pool_state.sqrt_price_x64,
                                    tick_current: pool_state.tick_current,
                                    tick_array_bitmap: pool_state.tick_array_bitmap,
                                    tick_array_bitmap_extension,
                                    zero_to_one_tick_array_states,
                                    one_to_zero_tick_array_states,
                                },
                            })
                        }
                    }
                }
                pools
            });
        }
        let mut all_pools = Vec::with_capacity(pool_ids.len());
        while let Some(Ok(pools)) = join_set.join_next().await {
            all_pools.extend(pools);
        }
        if all_pools.is_empty() {
            None
        } else {
            Some(all_pools)
        }
    }
}

pub struct RaydiumClmmCacheUpdater {
    tick_current: i32,
    liquidity: u128,
    sqrt_price_x64: u128,
    tick_array_bitmap: [u64; 16],
}

impl RaydiumClmmCacheUpdater {
    pub fn new(grpc_message: GrpcMessage) -> Result<Self> {
        if let RaydiumCLMMData {
            tick_current,
            liquidity,
            sqrt_price_x64,
            tick_array_bitmap,
            ..
        } = grpc_message
        {
            Ok(Self {
                tick_current,
                liquidity,
                sqrt_price_x64,
                tick_array_bitmap,
            })
        } else {
            Err(anyhow!("生成CachePoolUpdater失败：传入的参数类型不支持"))
        }
    }
}

impl CacheUpdater for RaydiumClmmCacheUpdater {
    fn update_cache(&self, pool: &mut Pool) -> anyhow::Result<()> {
        if let RaydiumCLMM {
            ref mut liquidity,
            ref mut sqrt_price_x64,
            ref mut tick_current,
            ref mut tick_array_bitmap,
            ..
        } = pool.state
        {
            if change_data_if_not_same(liquidity, self.liquidity)
                || change_data_if_not_same(tick_current, self.tick_current)
                || change_data_if_not_same(sqrt_price_x64, self.sqrt_price_x64)
                || change_data_if_not_same(tick_array_bitmap, self.tick_array_bitmap)
            {
                Ok(())
            } else {
                Err(anyhow!(""))
            }
        } else {
            Err(anyhow!(""))
        }
    }
}
