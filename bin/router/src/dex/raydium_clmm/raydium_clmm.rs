use crate::cache::{Mint, Pool, PoolState};
use crate::dex::common::utils::change_data_if_not_same;
use crate::dex::raydium_clmm::pool_state::{RaydiumCLMMInstructionItem, RaydiumCLMMPoolState};
use crate::dex::raydium_clmm::sdk::config::AmmConfig;
use crate::dex::raydium_clmm::sdk::tickarray_bitmap_extension::TickArrayBitmapExtension;
use crate::dex::raydium_clmm::sdk::utils::{
    deserialize_anchor_account, load_cur_and_next_specify_count_tick_array,
};
use crate::dex::raydium_clmm::sdk::{config, utils};
use crate::dex::{get_ata_program, get_mint_program};
use crate::file_db::DexJson;
use crate::interface::GrpcMessage::RaydiumCLMMData;
use crate::interface::{
    AccountMetaConverter, AccountSnapshotFetcher, AccountUpdate, CacheUpdater, Dex, DexType,
    GrpcAccountUpdateType, GrpcMessage, GrpcSubscribeRequestGenerator, InstructionItem,
    InstructionItemCreator, Quoter, ReadyGrpcMessageOperator, SubscribeKey,
};
use anyhow::anyhow;
use anyhow::Result;
use arrayref::{array_ref, array_refs};
use base58::ToBase58;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::address_lookup_table::AddressLookupTableAccount;
use solana_program::clock::Clock;
use solana_program::instruction::AccountMeta;
use solana_program::pubkey::Pubkey;
use solana_sdk::commitment_config::CommitmentConfig;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::error;
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestAccountsDataSlice,
    SubscribeRequestFilterAccounts,
};

pub struct RaydiumCLMMDex;

impl Quoter for RaydiumCLMMDex {
    fn quote(
        &self,
        amount_in: u64,
        in_mint: Pubkey,
        _out_mint: Pubkey,
        pool: &Pool,
        _clock: Arc<Clock>,
    ) -> Option<u64> {
        if amount_in == u64::MIN || (in_mint != pool.mint_0() && in_mint != pool.mint_1()) {
            return None;
        }
        if let PoolState::RaydiumCLMM(pool_state) = &pool.state {
            //TODO：优化计算参数
            let mut amm_config = AmmConfig::default();
            amm_config.trade_fee_rate = pool_state.trade_fee_rate;
            let mut clmm_pool_state = crate::dex::raydium_clmm::sdk::pool::PoolState::default();
            clmm_pool_state.tick_current = pool_state.tick_current;
            clmm_pool_state.tick_spacing = pool_state.tick_spacing;
            clmm_pool_state.tick_array_bitmap = pool_state.tick_array_bitmap;
            clmm_pool_state.liquidity = pool_state.liquidity;
            clmm_pool_state.sqrt_price_x64 = pool_state.sqrt_price_x64;
            let (zero_for_one, mut tick_arrays) = if in_mint == pool.mint_0() {
                (
                    true,
                    pool_state.one_to_zero_tick_array_states.clone().unwrap(),
                )
            } else {
                (
                    false,
                    pool_state.zero_to_one_tick_array_states.clone().unwrap(),
                )
            };
            let result = utils::get_out_put_amount_and_remaining_accounts(
                amount_in,
                None,
                zero_for_one,
                true,
                &amm_config,
                &clmm_pool_state,
                &Some(pool_state.tick_array_bitmap_extension),
                &mut tick_arrays,
            );
            match result {
                Ok((amount_out, _, _)) => Some(amount_out),
                Err(e) => {
                    error!("get_out_put_amount_and_remaining_accounts error: {:?}", e);
                    None
                }
            }
        } else {
            None
        }
    }
}

impl InstructionItemCreator for RaydiumCLMMDex {
    fn create_instruction_item(&self, pool: &Pool, in_mint: &Pubkey) -> Option<InstructionItem> {
        if let PoolState::RaydiumCLMM(pool_state) = &pool.state {
            let zero_to_one = in_mint == &pool.mint_0();
            Some(InstructionItem::RaydiumCLMM(RaydiumCLMMInstructionItem {
                pool_id: pool.pool_id,
                amm_config: pool_state.amm_config,
                mint_0: pool.mint_0(),
                mint_1: pool.mint_1(),
                mint_0_vault: pool_state.mint_0_vault,
                mint_1_vault: pool_state.mint_1_vault,
                observation_key: pool_state.observation_key,
                tick_arrays: if zero_to_one {
                    pool_state.zero_to_one_tick_array_states.clone()
                } else {
                    pool_state.one_to_zero_tick_array_states.clone()
                }
                .unwrap()
                .iter()
                .take(3)
                .map(|a| {
                    Pubkey::find_program_address(
                        &[
                            crate::dex::raydium_clmm::sdk::tick_array::TICK_ARRAY_SEED.as_bytes(),
                            pool.pool_id.to_bytes().as_ref(),
                            &a.start_tick_index.to_le_bytes(),
                        ],
                        &DexType::RaydiumCLmm.get_program_id(),
                    )
                    .0
                })
                .collect::<Vec<_>>(),
                alt: pool.alt.clone(),
                zero_to_one,
            }))
        } else {
            None
        }
    }
}

impl AccountMetaConverter for RaydiumCLMMDex {
    fn converter(
        &self,
        wallet: Pubkey,
        instruction_item: InstructionItem,
    ) -> Option<(Vec<AccountMeta>, Vec<AddressLookupTableAccount>)> {
        match instruction_item {
            InstructionItem::RaydiumCLMM(item) => {
                let mut accounts = Vec::with_capacity(11);
                // 1. wallet
                accounts.push(AccountMeta::new(wallet, true));
                // 2.amm config
                accounts.push(AccountMeta::new_readonly(item.amm_config, false));
                // 3.pool state
                accounts.push(AccountMeta::new(item.pool_id, false));
                // 4.coin mint ata
                let (coin_ata, _) = Pubkey::find_program_address(
                    &[
                        &wallet.to_bytes(),
                        &get_mint_program().to_bytes(),
                        &item.mint_0.to_bytes(),
                    ],
                    &get_ata_program(),
                );
                accounts.push(AccountMeta::new(coin_ata, false));
                // 5.pc mint ata
                let (pc_ata, _) = Pubkey::find_program_address(
                    &[
                        &wallet.to_bytes(),
                        &get_mint_program().to_bytes(),
                        &item.mint_1.to_bytes(),
                    ],
                    &get_ata_program(),
                );
                accounts.push(AccountMeta::new(pc_ata, false));
                // 6.base mint vault
                accounts.push(AccountMeta::new(item.mint_0_vault, false));
                // 7.quote mint vault
                accounts.push(AccountMeta::new(item.mint_1_vault, false));
                // 8.Observation State
                accounts.push(AccountMeta::new(item.observation_key, false));
                // 9.mint program
                accounts.push(AccountMeta::new_readonly(get_mint_program(), false));
                // 10.current tick array
                let mut tick_arrays = item
                    .tick_arrays
                    .into_iter()
                    .map(|k| AccountMeta::new(k, false))
                    .collect::<Vec<_>>();
                accounts.push(tick_arrays.remove(0));
                // 11.bitmap_extension
                accounts.push(AccountMeta::new_readonly(
                    TickArrayBitmapExtension::key(item.pool_id),
                    false,
                ));
                accounts.extend(tick_arrays);
                Some((accounts, vec![item.alt]))
            }
            _ => None,
        }
    }
}

pub struct RaydiumCLMMGrpcMessageOperator {
    update_account: AccountUpdate,
    txn: Option<String>,
    pool_id: Option<Pubkey>,
    grpc_message: Option<GrpcMessage>,
}

impl RaydiumCLMMGrpcMessageOperator {
    pub fn new(update_account: AccountUpdate) -> Self {
        Self {
            update_account,
            txn: None,
            pool_id: None,
            grpc_message: None,
        }
    }
}
impl ReadyGrpcMessageOperator for RaydiumCLMMGrpcMessageOperator {
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
                GrpcAccountUpdateType::Pool => {
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

pub struct RaydiumCLMMSubscribeRequestCreator;

impl GrpcSubscribeRequestGenerator for RaydiumCLMMSubscribeRequestCreator {
    fn create_subscribe_requests(
        &self,
        pools: &[Pool],
    ) -> Option<Vec<(SubscribeKey, SubscribeRequest)>> {
        let mut subscribe_pool_accounts = HashMap::new();
        subscribe_pool_accounts.insert(
            format!("{:?}", DexType::RaydiumCLmm),
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
            (DexType::RaydiumCLmm, GrpcAccountUpdateType::Pool),
            pool_request,
        )])
    }
}

pub struct RaydiumCLMMSnapshotFetcher;

impl RaydiumCLMMSnapshotFetcher {
    async fn generate_all_config_keys(&self, rpc_client: Arc<RpcClient>) -> HashMap<Pubkey, u32> {
        let mut all_amm_config_keys = Vec::new();
        for index in 0..9 {
            let index = index as u16;
            let (amm_config_key, __bump) = Pubkey::find_program_address(
                &[config::AMM_CONFIG_SEED.as_bytes(), &index.to_be_bytes()],
                &crate::dex::raydium_clmm::ID,
            );
            all_amm_config_keys.push(amm_config_key);
        }
        rpc_client
            .get_multiple_accounts_with_commitment(
                all_amm_config_keys.as_slice(),
                CommitmentConfig::finalized(),
            )
            .await
            .unwrap()
            .value
            .into_iter()
            .zip(all_amm_config_keys)
            .filter_map(|(account, config_key)| {
                if let Ok(amm_config_state) =
                    deserialize_anchor_account::<config::AmmConfig>(account.as_ref().unwrap())
                {
                    Some((config_key, amm_config_state.trade_fee_rate))
                } else {
                    None
                }
            })
            .collect::<HashMap<_, _>>()
    }
}

#[async_trait::async_trait]
impl AccountSnapshotFetcher for RaydiumCLMMSnapshotFetcher {
    async fn fetch_snapshot(
        &self,
        pool_json: Vec<DexJson>,
        rpc_client: Arc<RpcClient>,
    ) -> Option<Vec<Pool>> {
        let amm_config_map = self.generate_all_config_keys(rpc_client.clone()).await;
        let amm_config_map = Arc::new(amm_config_map);
        let mut join_set = JoinSet::new();
        let pool_ids = pool_json.iter().map(|json| json.pool).collect::<Vec<_>>();
        for chunks_pool in pool_json.chunks(100) {
            let chunks_pool_json = Arc::new(chunks_pool.to_vec());
            let chunks_one_pool_keys = chunks_pool_json
                .clone()
                .iter()
                .flat_map(|pool| vec![pool.pool, TickArrayBitmapExtension::key(pool.pool)])
                .collect::<Vec<_>>();
            let rpc_client = rpc_client.clone();
            let amm_config_map = amm_config_map.clone();
            let alt_map = self
                .load_lookup_table_accounts(rpc_client.clone(), chunks_pool_json.clone())
                .await
                .unwrap();
            join_set.spawn(async move {
                let mut pools = Vec::with_capacity(chunks_one_pool_keys.len());
                // 一次性查询100个pool和对应的bitmap_extension
                let pool_and_bitmap_extension_account_pair = chunks_one_pool_keys
                    .iter()
                    .zip(
                        rpc_client
                            .get_multiple_accounts_with_commitment(
                                &chunks_one_pool_keys,
                                CommitmentConfig::finalized(),
                            )
                            .await
                            .unwrap()
                            .value,
                    )
                    .collect::<Vec<_>>();
                for (index, one_pool_pair) in
                    pool_and_bitmap_extension_account_pair.chunks(2).enumerate()
                {
                    let pool_pair = &one_pool_pair[0];
                    let bitmap_extension_pair = &one_pool_pair[1];
                    if let (pool_id, Some(pool_account)) = pool_pair {
                        let pool_id = **pool_id;
                        if let (_bitmap_extension_id, Some(bitmap_extension_account)) =
                            bitmap_extension_pair
                        {
                            let pool_state = deserialize_anchor_account::<
                                crate::dex::raydium_clmm::sdk::pool::PoolState,
                            >(pool_account)
                            .unwrap();
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
                            let alt = match chunks_pool_json.get(index) {
                                None => None,
                                Some(accounts) => Some(
                                    alt_map
                                        .get(
                                            accounts.address_lookup_table_address.as_ref().unwrap(),
                                        )
                                        .unwrap()
                                        .clone(),
                                ),
                            };
                            if alt.is_none() {
                                continue;
                            }
                            pools.push(Pool {
                                protocol: DexType::RaydiumCLmm,
                                pool_id,
                                tokens: vec![
                                    Mint {
                                        mint: pool_state.token_mint_0.clone(),
                                    },
                                    Mint {
                                        mint: pool_state.token_mint_1.clone(),
                                    },
                                ],
                                state: PoolState::RaydiumCLMM(RaydiumCLMMPoolState {
                                    amm_config: pool_state.amm_config,
                                    mint_0_vault: pool_state.token_vault_0,
                                    mint_1_vault: pool_state.token_vault_1,
                                    observation_key: pool_state.observation_key,
                                    tick_spacing: pool_state.tick_spacing,
                                    trade_fee_rate,
                                    liquidity: pool_state.liquidity,
                                    sqrt_price_x64: pool_state.sqrt_price_x64,
                                    tick_current: pool_state.tick_current,
                                    tick_array_bitmap: pool_state.tick_array_bitmap,
                                    tick_array_bitmap_extension,
                                    zero_to_one_tick_array_states,
                                    one_to_zero_tick_array_states,
                                }),
                                alt: alt.unwrap(),
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

pub struct RaydiumCLMMCacheUpdater {
    tick_current: i32,
    liquidity: u128,
    sqrt_price_x64: u128,
    tick_array_bitmap: [u64; 16],
}

impl RaydiumCLMMCacheUpdater {
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

impl CacheUpdater for RaydiumCLMMCacheUpdater {
    fn update_cache(&self, pool: &mut Pool) -> anyhow::Result<()> {
        if let PoolState::RaydiumCLMM(ref mut pool_state) = pool.state {
            if change_data_if_not_same(&mut pool_state.liquidity, self.liquidity)
                || change_data_if_not_same(&mut pool_state.tick_current, self.tick_current)
                || change_data_if_not_same(&mut pool_state.sqrt_price_x64, self.sqrt_price_x64)
                || change_data_if_not_same(
                    &mut pool_state.tick_array_bitmap,
                    self.tick_array_bitmap,
                )
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
