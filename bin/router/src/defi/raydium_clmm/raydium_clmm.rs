use crate::defi::common::mint_vault::MintVaultSubscribe;
use crate::defi::common::utils::{change_data_if_not_same, change_option_ignore_none_old};
use crate::defi::dex::Dex;
use crate::defi::file_db::FILE_DB_DIR;
use crate::defi::raydium_amm::state::PoolInfo;
use crate::defi::raydium_clmm::sdk::config::AmmConfig;
use crate::defi::raydium_clmm::sdk::pool::PoolState;
use crate::defi::raydium_clmm::sdk::tick_array::TickArrayState;
use crate::defi::raydium_clmm::sdk::tickarray_bitmap_extension::TickArrayBitmapExtension;
use crate::defi::raydium_clmm::sdk::utils::{
    deserialize_anchor_account, load_cur_and_next_specify_count_tick_array,
};
use crate::defi::raydium_clmm::sdk::{config, pool, utils};
use crate::defi::raydium_clmm::state::ClmmPoolInfo;
use crate::defi::types::PoolExtra::RaydiumCLMM;
use crate::defi::types::{AccountUpdate, GrpcAccountUpdateType, Mint, Pool, PoolExtra, Protocol};
use crate::strategy::grpc_message_processor::GrpcMessage;
use crate::strategy::grpc_message_processor::GrpcMessage::{RaydiumAmmData, RaydiumClmmData};
use anchor_lang::error::ComparedValues::Pubkeys;
use anyhow::anyhow;
use arrayref::{array_ref, array_refs};
use base58::ToBase58;
use futures_util::future::err;
use moka::ops::compute::{CompResult, Op};
use moka::sync::Cache;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::pubkey::Pubkey;
use solana_sdk::commitment_config::CommitmentConfig;
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::error;
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestAccountsDataSlice,
    SubscribeRequestFilterAccounts,
};

#[derive(Debug)]
pub struct RaydiumClmmDex {
    amount_in_mint: Pubkey,
    pool_id: Pubkey,
    mint_0: Pubkey,
    mint_1: Pubkey,
    tick_spacing: u16,
    trade_fee_rate: u32,
    liquidity: u128,
    sqrt_price_x64: u128,
    tick_current: i32,
    tick_array_bitmap: [u64; 16],
    tick_array_bitmap_extension: TickArrayBitmapExtension,
    tick_array_states: VecDeque<TickArrayState>,
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
            tick_array_states,
        } = pool.extra.clone()
        {
            Some(Self {
                amount_in_mint,
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
                tick_array_states,
            })
        } else {
            None
        }
    }

    fn generate_all_config_keys() -> Vec<Pubkey> {
        let mut all_amm_config_keys = Vec::new();
        for index in 0..9 {
            let index = index as u16;
            let (amm_config_key, __bump) = Pubkey::find_program_address(
                &[config::AMM_CONFIG_SEED.as_bytes(), &index.to_be_bytes()],
                &crate::defi::raydium_clmm::ID,
            );
            all_amm_config_keys.push(amm_config_key);
        }
        all_amm_config_keys
    }

    pub fn get_subscribe_request(
        pools: &Vec<Pool>,
    ) -> Vec<(GrpcAccountUpdateType, SubscribeRequest)> {
        let mut subscribe_pool_accounts = HashMap::new();
        subscribe_pool_accounts.insert(
            Protocol::RaydiumCLmm.name().to_string(),
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
        vec![(GrpcAccountUpdateType::PoolState, pool_request)]
    }

    pub async fn fetch_snapshot(rpc_client: Arc<RpcClient>) -> Option<Vec<Pool>> {
        let pool_infos: Vec<ClmmPoolInfo> =
            match File::open(format!("{}/raydium_clmm.json", FILE_DB_DIR)) {
                Ok(file) => serde_json::from_reader(file).expect("Could not parse JSON"),
                Err(e) => {
                    error!("{}", e);
                    vec![]
                }
            };
        let all_config_keys = Self::generate_all_config_keys();
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
                    Some((config_key, amm_config_state))
                } else {
                    None
                }
            })
            .collect::<HashMap<Pubkey, AmmConfig>>();
        let filled_pad_pool_infos = pool_infos
            .into_iter()
            .filter_map(|pool_info| pool_info.calcul_pda())
            .flatten()
            .collect::<Vec<_>>();
        let mut all_pools = Vec::with_capacity(filled_pad_pool_infos.len());
        for chunks in filled_pad_pool_infos.chunks(50 * 3) {
            let rpc_client = rpc_client.clone();
            for one_pool_accounts in chunks
                .iter()
                .zip(
                    rpc_client
                        .get_multiple_accounts_with_commitment(
                            chunks,
                            CommitmentConfig::finalized(),
                        )
                        .await
                        .unwrap()
                        .value,
                )
                .collect::<Vec<_>>()
                .chunks(3)
            {
                let pool_account_pair = one_pool_accounts[0].clone();
                let bitmap_extension_pair = one_pool_accounts[1].clone();
                if pool_account_pair.1.is_none() || bitmap_extension_pair.1.is_none() {
                    continue;
                }
                let pool_id = pool_account_pair.0.clone();
                let pool_state = deserialize_anchor_account::<pool::PoolState>(
                    pool_account_pair.1.as_ref().unwrap(),
                )
                .unwrap();
                let trade_fee_rate = amm_config_map
                    .get(&pool_state.amm_config)
                    .unwrap()
                    .trade_fee_rate;
                let tickarray_bitmap_extension =
                    deserialize_anchor_account::<TickArrayBitmapExtension>(
                        bitmap_extension_pair.1.as_ref().unwrap(),
                    )
                    .unwrap();
                let mut all_tick_array_states = VecDeque::new();
                all_tick_array_states.extend(
                    load_cur_and_next_specify_count_tick_array(
                        rpc_client.clone(),
                        10,
                        &pool_id,
                        &Pubkey::default(),
                        &pool_state,
                        &tickarray_bitmap_extension,
                        true,
                    )
                    .await,
                );
                all_tick_array_states.extend(
                    load_cur_and_next_specify_count_tick_array(
                        rpc_client.clone(),
                        10,
                        &pool_id,
                        &Pubkey::default(),
                        &pool_state,
                        &tickarray_bitmap_extension,
                        false,
                    )
                    .await,
                );
                all_pools.push(Pool {
                    protocol: Protocol::RaydiumCLmm,
                    pool_id,
                    tokens: vec![
                        Mint {
                            mint: pool_state.token_mint_0.clone(),
                            decimals: pool_state.mint_decimals_0,
                        },
                        Mint {
                            mint: pool_state.token_mint_1.clone(),
                            decimals: pool_state.mint_decimals_1,
                        },
                    ],
                    extra: RaydiumCLMM {
                        tick_spacing: pool_state.tick_spacing,
                        trade_fee_rate,
                        liquidity: pool_state.liquidity,
                        sqrt_price_x64: pool_state.sqrt_price_x64,
                        tick_current: pool_state.tick_current,
                        tick_array_bitmap: pool_state.tick_array_bitmap,
                        tick_array_bitmap_extension: tickarray_bitmap_extension,
                        tick_array_states: all_tick_array_states,
                    },
                })
            }
        }
        if all_pools.is_empty() {
            None
        } else {
            Some(all_pools)
        }
    }

    pub async fn try_get_ready_message(
        account_update: AccountUpdate,
        events: Arc<Cache<(String, Pubkey), GrpcMessage>>,
    ) -> Option<GrpcMessage> {
        let account_type = account_update.account_type;
        let account = account_update.account;
        if let Some(update_account_info) = account.account {
            let txn = update_account_info.txn_signature.unwrap().to_base58();
            let data = update_account_info.data;
            let pool_id = Pubkey::try_from(update_account_info.pubkey).unwrap();
            let push_event = match account_type {
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
                    Some((
                        pool_id,
                        RaydiumClmmData {
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
                        },
                    ))
                }
                _ => None,
            };
            if let Some((pool_id, update_data)) = push_event {
                let entry = events
                    .entry((txn, pool_id))
                    .and_compute_with(|maybe_entry| {
                        if let Some(exists) = maybe_entry {
                            let mut message = exists.into_value();
                            if Self::fill_change_data(&mut message, update_data).is_ok() {
                                Op::Remove
                            } else {
                                Op::Put(message)
                            }
                        } else {
                            Op::Put(update_data)
                        }
                    });
                match entry {
                    CompResult::Removed(r) => Some(r.into_value()),
                    _ => None,
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    fn fill_change_data(old: &mut GrpcMessage, update_data: GrpcMessage) -> anyhow::Result<()> {
        match old {
            RaydiumClmmData {
                tick_current,
                liquidity,
                sqrt_price_x64,
                tick_array_bitmap,
                ..
            } => {
                if let RaydiumClmmData {
                    tick_current: update_tick_current,
                    liquidity: update_liquidity,
                    sqrt_price_x64: update_sqrt_price_x64,
                    tick_array_bitmap: update_tick_array_bitmap,
                    ..
                } = update_data
                {
                    if change_data_if_not_same(tick_current, update_tick_current)
                        || change_data_if_not_same(liquidity, update_liquidity)
                        || change_data_if_not_same(sqrt_price_x64, update_sqrt_price_x64)
                        || change_data_if_not_same(tick_array_bitmap, update_tick_array_bitmap)
                    {
                        Ok(())
                    } else {
                        Err(anyhow!(""))
                    }
                } else {
                    Err(anyhow!(""))
                }
            }
            _ => Err(anyhow!("")),
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

#[async_trait::async_trait]
impl Dex for RaydiumClmmDex {
    fn quote(&self, amount_in: u64) -> Option<u64> {
        if self.amount_in_mint != self.mint_0 && self.amount_in_mint != self.mint_1 {
            return None;
        }
        let zero_for_one = self.amount_in_mint == self.mint_0;
        let mut amm_config = AmmConfig::default();
        amm_config.trade_fee_rate = self.trade_fee_rate;
        let mut pool_state = PoolState::default();
        pool_state.tick_current = self.tick_current;
        pool_state.tick_spacing = self.tick_spacing;
        pool_state.tick_array_bitmap = self.tick_array_bitmap;
        pool_state.liquidity = self.liquidity;
        pool_state.sqrt_price_x64 = self.sqrt_price_x64;

        let mut tick_arrays = self.tick_array_states.clone();

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
}
