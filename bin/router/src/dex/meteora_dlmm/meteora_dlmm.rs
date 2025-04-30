use crate::cache::PoolState::MeteoraDLMM;
use crate::cache::{Mint, Pool};
use crate::dex::common::utils::{change_data_if_not_same, SwapDirection};
use crate::dex::meteora_dlmm::meteora_dlmm_pool_extra::MeteoraDLMMPoolExtra;
use crate::dex::meteora_dlmm::sdk::commons::pda::derive_bin_array_bitmap_extension;
use crate::dex::meteora_dlmm::sdk::commons::quote::{
    get_bin_array_pubkeys_for_swap, quote_exact_in,
};
use crate::dex::meteora_dlmm::sdk::interface::accounts::{
    BinArray, BinArrayAccount, BinArrayBitmapExtension, BinArrayBitmapExtensionAccount, LbPair,
    LbPairAccount,
};
use crate::interface::{
    AccountSnapshotFetcher, AccountUpdate, CacheUpdater, Dex, GrpcAccountUpdateType, GrpcMessage,
    GrpcSubscribeRequestGenerator, Protocol, ReadyGrpcMessageOperator, SubscribeKey,
};
use anyhow::Result;
use anyhow::{anyhow, Context};
use arrayref::{array_ref, array_refs};
use base58::ToBase58;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::clock::Clock;
use solana_program::pubkey::Pubkey;
use solana_program::sysvar::SysvarId;
use solana_sdk::commitment_config::CommitmentConfig;
use spl_token_2022::extension::transfer_fee::TransferFeeConfig;
use spl_token_2022::extension::{BaseStateWithExtensions, StateWithExtensions};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::error;
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestAccountsDataSlice,
    SubscribeRequestFilterAccounts,
};

#[derive(Debug, Clone)]
pub struct MeteoraDlmmDex {
    pool_info: MeteoraDLMMPoolExtra,
    pool_id: Pubkey,
    swap_direction: SwapDirection,
    clock: Clock,
}

impl MeteoraDlmmDex {
    pub fn new(pool: Pool, amount_in_mint: Pubkey, clock: Clock) -> Option<Self> {
        if let MeteoraDLMM(data) = &pool.state {
            let mint_0 = pool.mint_0();
            let mint_1 = pool.mint_1();
            if amount_in_mint != mint_0 && amount_in_mint != mint_1 {
                return None;
            }
            Some(Self {
                pool_info: data.clone(),
                pool_id: pool.pool_id,
                swap_direction: if mint_0 == amount_in_mint {
                    SwapDirection::Coin2PC
                } else {
                    SwapDirection::PC2Coin
                },
                clock,
            })
        } else {
            None
        }
    }
}

#[async_trait::async_trait]
impl Dex for MeteoraDlmmDex {
    async fn quote(&self, amount_in: u64) -> Option<u64> {
        if amount_in == u64::MIN {
            return None;
        }
        let swap_for_y = self.swap_direction == SwapDirection::Coin2PC;
        let pool_info = &self.pool_info;
        let lp_pair_state = pool_info.clone().into();
        let result = quote_exact_in(
            self.pool_id,
            lp_pair_state,
            amount_in,
            swap_for_y,
            if swap_for_y {
                pool_info.swap_for_y_bin_array_map.clone()
            } else {
                pool_info.swap_for_x_bin_array_map.clone()
            },
            pool_info.bin_array_bitmap_extension.clone(),
            self.clock.clone(),
            pool_info.mint_x_transfer_fee_config.clone(),
            pool_info.mint_y_transfer_fee_config.clone(),
        );
        match result {
            Ok(quote) => Some(quote.amount_out),
            Err(e) => {
                error!("dlmm swap error : {:?}", e);
                None
            }
        }
    }

    fn clone_self(&self) -> Box<dyn Dex> {
        Box::new(self.clone())
    }
}

pub struct MeteoraDLMMGrpcSubscribeRequestGenerator;

impl GrpcSubscribeRequestGenerator for MeteoraDLMMGrpcSubscribeRequestGenerator {
    fn create_subscribe_requests(
        &self,
        pools: &[Pool],
    ) -> Option<Vec<(SubscribeKey, SubscribeRequest)>> {
        let mut subscribe_pool_accounts = HashMap::new();
        subscribe_pool_accounts.insert(
            format!("{:?}", Protocol::MeteoraDLMM),
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
                // v_parameters.volatility_accumulator
                SubscribeRequestAccountsDataSlice {
                    offset: 40,
                    length: 4,
                },
                // v_parameters.volatility_reference
                SubscribeRequestAccountsDataSlice {
                    offset: 44,
                    length: 4,
                },
                // v_parameters.index_reference
                SubscribeRequestAccountsDataSlice {
                    offset: 48,
                    length: 4,
                },
                // v_parameters.last_update_timestamp
                SubscribeRequestAccountsDataSlice {
                    offset: 56,
                    length: 8,
                },
                // pair_type
                SubscribeRequestAccountsDataSlice {
                    offset: 75,
                    length: 1,
                },
                // active_id
                SubscribeRequestAccountsDataSlice {
                    offset: 76,
                    length: 4,
                },
                // bin_step
                SubscribeRequestAccountsDataSlice {
                    offset: 80,
                    length: 2,
                },
                // status
                SubscribeRequestAccountsDataSlice {
                    offset: 82,
                    length: 1,
                },
                // activation_type
                SubscribeRequestAccountsDataSlice {
                    offset: 86,
                    length: 1,
                },
                // bin_array_bitmap
                SubscribeRequestAccountsDataSlice {
                    offset: 584,
                    length: 128,
                },
                // activation_point
                SubscribeRequestAccountsDataSlice {
                    offset: 816,
                    length: 8,
                },
            ],
            ..Default::default()
        };
        let mut clock_account = HashMap::new();
        clock_account.insert(
            "Clock".to_string(),
            SubscribeRequestFilterAccounts {
                account: vec![Clock::id().to_string()],
                ..Default::default()
            },
        );
        let clock_request = SubscribeRequest {
            accounts: clock_account,
            commitment: Some(CommitmentLevel::Finalized).map(|x| x as i32),
            ..Default::default()
        };
        Some(vec![
            (
                (Protocol::MeteoraDLMM, GrpcAccountUpdateType::PoolState),
                pool_request,
            ),
            (
                (Protocol::MeteoraDLMM, GrpcAccountUpdateType::Clock),
                clock_request,
            ),
        ])
    }
}

pub struct MeteoraDLMMSnapshotFetcher;

impl MeteoraDLMMSnapshotFetcher {
    async fn get_bin_arrays_by_swap_direction(
        lb_pair_pubkey: Pubkey,
        lb_pair_state: &LbPair,
        bitmap_extension: Option<&BinArrayBitmapExtension>,
        swap_for_y: bool,
        take: u8,
        rpc_client: Arc<RpcClient>,
    ) -> Result<HashMap<Pubkey, BinArray>> {
        let bin_arrays_for_swap = get_bin_array_pubkeys_for_swap(
            lb_pair_pubkey,
            lb_pair_state,
            bitmap_extension,
            swap_for_y,
            take,
        )?;
        rpc_client
            .get_multiple_accounts(&bin_arrays_for_swap)
            .await?
            .into_iter()
            .zip(bin_arrays_for_swap.iter())
            .map(|(account, &key)| {
                let account = account.unwrap();
                Some((
                    key,
                    BinArrayAccount::deserialize(account.data.as_ref()).ok()?.0,
                ))
            })
            .collect::<Option<HashMap<Pubkey, BinArray>>>()
            .context("Failed to fetch bin arrays")
    }

    async fn get_mint_transfer_fee_config(
        mints: Vec<Pubkey>,
        rpc_client: Arc<RpcClient>,
    ) -> HashMap<Pubkey, TransferFeeConfig> {
        mints
            .iter()
            .zip(
                rpc_client
                    .get_multiple_accounts_with_commitment(
                        mints.as_slice(),
                        CommitmentConfig::finalized(),
                    )
                    .await
                    .unwrap()
                    .value,
            )
            .into_iter()
            .filter_map(|(mint_key, mint_account)| {
                if let Some(account) = mint_account {
                    if let Ok(mint_extensions) =
                        StateWithExtensions::<spl_token_2022::state::Mint>::unpack(
                            account.data.as_ref(),
                        )
                    {
                        if let Ok(transfer_fee_config) =
                            mint_extensions.get_extension::<TransferFeeConfig>()
                        {
                            Some((*mint_key, *transfer_fee_config))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect::<HashMap<Pubkey, TransferFeeConfig>>()
    }
}

#[async_trait::async_trait]
impl AccountSnapshotFetcher for MeteoraDLMMSnapshotFetcher {
    async fn fetch_snapshot(
        &self,
        pool_ids: Vec<Pubkey>,
        rpc_client: Arc<RpcClient>,
    ) -> Option<Vec<Pool>> {
        let mut join_set = JoinSet::new();
        for chunks_pool_id in pool_ids.chunks(100) {
            let rpc_client = rpc_client.clone();
            let chunks_pool_id = chunks_pool_id.iter().map(|id| *id).collect::<Vec<Pubkey>>();
            join_set.spawn(async move {
                let all_pool_accounts = chunks_pool_id
                    .iter()
                    .zip(
                        rpc_client
                            .get_multiple_accounts_with_commitment(
                                &chunks_pool_id,
                                CommitmentConfig::finalized(),
                            )
                            .await
                            .unwrap()
                            .value,
                    )
                    .collect::<Vec<_>>();
                let mut chunks_pools = Vec::with_capacity(chunks_pool_id.len());
                for (lb_pair_id, lb_pair_account) in all_pool_accounts {
                    let lb_pair_id = *lb_pair_id;
                    if let Some(account) = lb_pair_account {
                        let lb_pair = LbPairAccount::deserialize(&account.data).unwrap().0;
                        let bitmap_extension_id = derive_bin_array_bitmap_extension(lb_pair_id).0;
                        let bitmap_extension = if let Ok(bitmap_extension_account) =
                            rpc_client.get_account_data(&bitmap_extension_id).await
                        {
                            Some(
                                BinArrayBitmapExtensionAccount::deserialize(
                                    &bitmap_extension_account,
                                )
                                .unwrap()
                                .0,
                            )
                        } else {
                            None
                        };
                        let mut mint_transfer_fee_config =
                            MeteoraDLMMSnapshotFetcher::get_mint_transfer_fee_config(
                                vec![lb_pair.token_x_mint, lb_pair.token_y_mint],
                                rpc_client.clone(),
                            )
                            .await;
                        let swap_for_y_bin_array_map =
                            MeteoraDLMMSnapshotFetcher::get_bin_arrays_by_swap_direction(
                                lb_pair_id,
                                &lb_pair,
                                bitmap_extension.as_ref(),
                                true,
                                10,
                                rpc_client.clone(),
                            )
                            .await;
                        let swap_for_x_bin_array_map =
                            MeteoraDLMMSnapshotFetcher::get_bin_arrays_by_swap_direction(
                                lb_pair_id,
                                &lb_pair,
                                bitmap_extension.as_ref(),
                                false,
                                10,
                                rpc_client.clone(),
                            )
                            .await;
                        chunks_pools.push(Pool {
                            protocol: Protocol::MeteoraDLMM,
                            pool_id: lb_pair_id,
                            tokens: vec![
                                Mint {
                                    mint: lb_pair.token_x_mint,
                                },
                                Mint {
                                    mint: lb_pair.token_y_mint,
                                },
                            ],
                            state: MeteoraDLMM(MeteoraDLMMPoolExtra::new(
                                lb_pair,
                                bitmap_extension,
                                swap_for_x_bin_array_map.unwrap(),
                                swap_for_y_bin_array_map.unwrap(),
                                mint_transfer_fee_config.remove(&lb_pair.token_x_mint),
                                mint_transfer_fee_config.remove(&lb_pair.token_y_mint),
                            )),
                        })
                    }
                }
                chunks_pools
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

pub struct MeteoraDLMMGrpcMessageOperator {
    update_account: AccountUpdate,
    txn: Option<String>,
    pool_id: Option<Pubkey>,
    grpc_message: Option<GrpcMessage>,
}

impl MeteoraDLMMGrpcMessageOperator {
    pub fn new(update_account: AccountUpdate) -> Self {
        Self {
            update_account,
            txn: None,
            pool_id: None,
            grpc_message: None,
        }
    }
}
impl ReadyGrpcMessageOperator for MeteoraDLMMGrpcMessageOperator {
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
                    let src = array_ref![data, 0, 165];
                    let (
                        volatility_accumulator,
                        volatility_reference,
                        index_reference,
                        last_update_timestamp,
                        _pair_type,
                        active_id,
                        _bin_step,
                        _status,
                        _activation_type,
                        bin_array_bitmap,
                        _activation_point,
                    ) = array_refs![src, 4, 4, 4, 8, 1, 4, 2, 1, 1, 128, 8];
                    self.txn = Some(txn);
                    self.pool_id = Some(pool_id);
                    self.grpc_message = Some(GrpcMessage::MeteoraDLMMData {
                        pool_id,
                        active_id: i32::from_le_bytes(*active_id),
                        bin_array_bitmap: bin_array_bitmap
                            .chunks_exact(8)
                            .map(|chunk| u64::from_le_bytes(chunk.try_into().unwrap()))
                            .collect::<Vec<_>>()
                            .try_into()
                            .unwrap(),
                        volatility_accumulator: u32::from_le_bytes(*volatility_accumulator),
                        volatility_reference: u32::from_le_bytes(*volatility_reference),
                        index_reference: i32::from_le_bytes(*index_reference),
                        last_update_timestamp: i64::from_le_bytes(*last_update_timestamp),
                    });
                    Ok(())
                }
                GrpcAccountUpdateType::Clock => {
                    let clock: Clock = bincode::deserialize(data)?;
                    self.grpc_message = Some(GrpcMessage::Clock(clock));
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
        // let txn = self.txn.as_ref().unwrap();
        // (txn.clone(), self.pool_id.unwrap())
        unimplemented!()
    }

    fn get_insert_data(&self) -> GrpcMessage {
        self.grpc_message.as_ref().unwrap().clone()
    }
}

pub struct MeteoraDLMMCacheUpdater {
    active_id: i32,
    bin_array_bitmap: [u64; 16],
    volatility_accumulator: u32,
    volatility_reference: u32,
    index_reference: i32,
    last_update_timestamp: i64,
}

impl MeteoraDLMMCacheUpdater {
    pub fn new(grpc_message: GrpcMessage) -> Result<Self> {
        if let GrpcMessage::MeteoraDLMMData {
            active_id,
            bin_array_bitmap,
            volatility_accumulator,
            volatility_reference,
            index_reference,
            last_update_timestamp,
            ..
        } = grpc_message
        {
            Ok(Self {
                active_id,
                bin_array_bitmap,
                volatility_accumulator,
                volatility_reference,
                index_reference,
                last_update_timestamp,
            })
        } else {
            Err(anyhow!("生成CachePoolUpdater失败：传入的参数类型不支持"))
        }
    }
}

impl CacheUpdater for MeteoraDLMMCacheUpdater {
    fn update_cache(&self, pool: &mut Pool) -> anyhow::Result<()> {
        if let MeteoraDLMM(ref mut cache_data) = pool.state {
            let active_id = &mut cache_data.active_id;
            let bin_array_bit_map = &mut cache_data.bin_array_bitmap;
            let volatility_accumulator = &mut cache_data.volatility_accumulator;
            let volatility_reference = &mut cache_data.volatility_reference;
            let index_reference = &mut cache_data.index_reference;
            let last_update_timestamp = &mut cache_data.last_update_timestamp;
            if change_data_if_not_same(active_id, self.active_id)
                || change_data_if_not_same(bin_array_bit_map, self.bin_array_bitmap)
                || change_data_if_not_same(volatility_accumulator, self.volatility_accumulator)
                || change_data_if_not_same(volatility_reference, self.volatility_reference)
                || change_data_if_not_same(index_reference, self.index_reference)
                || change_data_if_not_same(last_update_timestamp, self.last_update_timestamp)
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
