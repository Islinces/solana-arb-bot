use crate::arbitrage::types::swap::Swap;
use crate::cache::PoolState::MeteoraDLMM;
use crate::cache::{Mint, Pool};
use crate::dex::common::utils::change_data_if_not_same;
use crate::dex::meteora_dlmm::pool_state::{MeteoraDLMMInstructionItem, MeteoraDLMMPoolState};
use crate::dex::meteora_dlmm::sdk::commons::pda::derive_bin_array_bitmap_extension;
use crate::dex::meteora_dlmm::sdk::commons::quote::{
    get_bin_array_pubkeys_for_swap, quote_exact_in,
};
use crate::dex::meteora_dlmm::sdk::interface::accounts::{
    BinArray, BinArrayAccount, BinArrayBitmapExtension, BinArrayBitmapExtensionAccount, LbPair,
    LbPairAccount,
};
use crate::dex::{get_ata_program, get_mint_program};
use crate::file_db::DexJson;
use crate::interface::{
    AccountMetaConverter, AccountSnapshotFetcher, AccountUpdate, CacheUpdater, Dex, DexType,
    GrpcAccountUpdateType, GrpcMessage, GrpcSubscribeRequestGenerator, InstructionItem,
    InstructionItemCreator, Quoter, ReadyGrpcMessageOperator, SubscribeKey,
};
use anyhow::Result;
use anyhow::{anyhow, Context};
use arrayref::{array_ref, array_refs};
use base58::ToBase58;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::address_lookup_table::AddressLookupTableAccount;
use solana_program::clock::Clock;
use solana_program::instruction::AccountMeta;
use solana_program::pubkey::Pubkey;
use solana_program::sysvar::SysvarId;
use solana_sdk::commitment_config::CommitmentConfig;
use spl_token_2022::extension::transfer_fee::TransferFeeConfig;
use spl_token_2022::extension::{BaseStateWithExtensions, StateWithExtensions};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::task::JoinSet;
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestAccountsDataSlice,
    SubscribeRequestFilterAccounts,
};

pub struct MeteoraDLMMDex;

#[async_trait::async_trait]
impl Quoter for MeteoraDLMMDex {
    async fn quote(
        &self,
        amount_in: u64,
        in_mint: Pubkey,
        _out_mint: Pubkey,
        pool: &Pool,
        clock: Arc<Clock>,
    ) -> Option<u64> {
        let mint_0 = pool.mint_0();
        let mint_1 = pool.mint_1();
        if amount_in == u64::MIN || (in_mint != mint_0 && in_mint != mint_1) {
            return None;
        }
        if let MeteoraDLMM(pool_state) = &pool.state {
            let swap_for_y = in_mint == mint_0;
            let lp_pair_state: LbPair = pool_state.clone().into();
            let result = quote_exact_in(
                pool.pool_id,
                lp_pair_state,
                amount_in,
                swap_for_y,
                if swap_for_y {
                    pool_state.swap_for_y_bin_array_map.clone()
                } else {
                    pool_state.swap_for_x_bin_array_map.clone()
                },
                pool_state.bin_array_bitmap_extension,
                clock.clone(),
                pool_state.mint_x_transfer_fee_config,
                pool_state.mint_y_transfer_fee_config,
            );
            match result {
                Ok(quote) => Some(quote.amount_out),
                Err(e) => {
                    // error!(
                    //     "dlmm swap error : {:?}, pool_id : {:?}",
                    //     e,
                    //     self.pool_id.to_string()
                    // );
                    None
                }
            }
        } else {
            None
        }
    }
}

impl InstructionItemCreator for MeteoraDLMMDex {
    fn create_instruction_item(&self, pool: &Pool, in_mint: &Pubkey) -> Option<InstructionItem> {
        if let MeteoraDLMM(pool_state) = &pool.state {
            let zero_to_one = in_mint == &pool.mint_0();
            Some(InstructionItem::MeteoraDLMM(MeteoraDLMMInstructionItem {
                pool_id: pool.pool_id,
                mint_0: pool.mint_0(),
                mint_1: pool.mint_1(),
                mint_0_vault: pool_state.mint_0_vault,
                mint_1_vault: pool_state.mint_1_vault,
                bitmap_extension: derive_bin_array_bitmap_extension(pool.pool_id).0,
                bin_arrays: if zero_to_one {
                    pool_state.swap_for_y_bin_array_map.clone()
                } else {
                    pool_state.swap_for_x_bin_array_map.clone()
                }
                .keys()
                .take(3)
                .map(|k| k.clone())
                .collect::<Vec<_>>(),
                alt: pool.alt.clone(),
                zero_to_one,
            }))
        } else {
            None
        }
    }
}

impl AccountMetaConverter for MeteoraDLMMDex {
    fn converter(
        &self,
        wallet: Pubkey,
        instruction_item: InstructionItem,
    ) -> Option<(Vec<AccountMeta>, Vec<AddressLookupTableAccount>)> {
        match instruction_item {
            InstructionItem::MeteoraDLMM(item) => {
                let mut accounts = Vec::with_capacity(13);
                // 1.lb pair
                accounts.push(AccountMeta::new(item.pool_id, false));
                // 2.bitmap extension
                accounts.push(AccountMeta::new(item.bitmap_extension, false));
                // 3.mint_0 vault
                accounts.push(AccountMeta::new(item.mint_0_vault, false));
                // 4.mint_1 vault
                accounts.push(AccountMeta::new(item.mint_1_vault, false));
                // 5.mint_0 ata
                let (mint_0_ata, _) = Pubkey::find_program_address(
                    &[
                        &wallet.to_bytes(),
                        &get_mint_program().to_bytes(),
                        &item.mint_0.to_bytes(),
                    ],
                    &get_ata_program(),
                );
                accounts.push(AccountMeta::new(mint_0_ata, false));
                // 6.mint_1 ata
                let (mint_1_ata, _) = Pubkey::find_program_address(
                    &[
                        &wallet.to_bytes(),
                        &get_mint_program().to_bytes(),
                        &item.mint_1.to_bytes(),
                    ],
                    &get_ata_program(),
                );
                accounts.push(AccountMeta::new(mint_1_ata, false));
                // 7.mint_0
                accounts.push(AccountMeta::new_readonly(item.mint_0, false));
                // 8.mint_1
                accounts.push(AccountMeta::new_readonly(item.mint_1, false));
                // 9.oracle
                accounts.push(AccountMeta::new(
                    Pubkey::from_str("39vUBP8XmUqKTb5oJWRoiEJQ7ZsKMQYdDMPohhpTEAwJ").unwrap(),
                    false,
                ));
                // 10.fee account
                accounts.push(AccountMeta::new_readonly(
                    DexType::MeteoraDLMM.get_program_id(),
                    false,
                ));
                // 11.wallet
                accounts.push(AccountMeta::new(wallet, true));
                // 12.mint_0 program
                accounts.push(AccountMeta::new_readonly(get_mint_program(), false));
                // 13.mint_1 program
                accounts.push(AccountMeta::new_readonly(get_mint_program(), false));
                // 14.Event Authority
                accounts.push(AccountMeta::new(
                    Pubkey::from_str("D1ZN9Wj1fRSUQfCjhvnu1hqDMT7hzjzBBpi12nVniYD6").unwrap(),
                    false,
                ));
                // 15.program
                accounts.push(AccountMeta::new_readonly(
                    DexType::MeteoraDLMM.get_program_id(),
                    false,
                ));
                // 16~~.current bin array
                let bin_arrays = item
                    .bin_arrays
                    .into_iter()
                    .map(|k| AccountMeta::new(k, false))
                    .collect::<Vec<_>>();
                accounts.extend(bin_arrays);
                Some((accounts, vec![item.alt]))
            }
            _ => None,
        }
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
            format!("{:?}", DexType::MeteoraDLMM),
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
        // let mut clock_account = HashMap::new();
        // clock_account.insert(
        //     "Clock".to_string(),
        //     SubscribeRequestFilterAccounts {
        //         account: vec![Clock::id().to_string()],
        //         ..Default::default()
        //     },
        // );
        // let clock_request = SubscribeRequest {
        //     accounts: clock_account,
        //     commitment: Some(CommitmentLevel::Finalized).map(|x| x as i32),
        //     ..Default::default()
        // };
        Some(vec![
            (
                (DexType::MeteoraDLMM, GrpcAccountUpdateType::Pool),
                pool_request,
            ),
            // (
            //     (DexType::MeteoraDLMM, GrpcAccountUpdateType::Clock),
            //     clock_request,
            // ),
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
            .filter_map(|(account, &key)| {
                if let Some(account) = account {
                    Some((
                        key,
                        BinArrayAccount::deserialize(account.data.as_ref()).ok()?.0,
                    ))
                } else {
                    None
                }
            })
            .map(Some)
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
        pool_json: Vec<DexJson>,
        rpc_client: Arc<RpcClient>,
    ) -> Option<Vec<Pool>> {
        let mut join_set = JoinSet::new();
        for chunks_dex_json in pool_json.chunks(100) {
            let rpc_client = rpc_client.clone();
            let chunks_pool_json = Arc::new(chunks_dex_json.to_vec());
            let chunks_pool_id = chunks_pool_json
                .clone()
                .iter()
                .map(|id| id.pool)
                .collect::<Vec<_>>();
            // 查询alt
            let alt_map = self
                .load_lookup_table_accounts(rpc_client.clone(), chunks_pool_json.clone())
                .await
                .unwrap();
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
                let mut chunks_pools = Vec::with_capacity(all_pool_accounts.len());
                for (index, (lb_pair_id, lb_pair_account)) in all_pool_accounts.iter().enumerate() {
                    let lb_pair_id = **lb_pair_id;
                    if let Some(account) = lb_pair_account {
                        let lb_pair = LbPairAccount::deserialize(&account.data).unwrap().0;
                        // if lb_pair.token_x_mint!=Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap()
                        //     &&lb_pair.token_y_mint!=Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(){
                        //     continue;
                        // }
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
                        let alt = match chunks_pool_json.get(index) {
                            None => None,
                            Some(accounts) => Some(
                                alt_map
                                    .get(accounts.address_lookup_table_address.as_ref().unwrap())
                                    .unwrap()
                                    .clone(),
                            ),
                        };
                        if alt.is_none() {
                            continue;
                        }
                        chunks_pools.push(Pool {
                            protocol: DexType::MeteoraDLMM,
                            pool_id: lb_pair_id,
                            tokens: vec![
                                Mint {
                                    mint: lb_pair.token_x_mint,
                                },
                                Mint {
                                    mint: lb_pair.token_y_mint,
                                },
                            ],
                            state: MeteoraDLMM(MeteoraDLMMPoolState::new(
                                lb_pair,
                                bitmap_extension,
                                swap_for_x_bin_array_map.unwrap(),
                                swap_for_y_bin_array_map.unwrap(),
                                mint_transfer_fee_config.remove(&lb_pair.token_x_mint),
                                mint_transfer_fee_config.remove(&lb_pair.token_y_mint),
                            )),
                            alt: alt.unwrap(),
                        })
                    }
                }
                chunks_pools
            });
        }
        let mut all_pools = Vec::new();
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
            match account_type {
                GrpcAccountUpdateType::Pool => {
                    let pool_id = Pubkey::try_from(update_account_info.pubkey.clone()).unwrap();
                    let txn = &update_account_info
                        .txn_signature
                        .as_ref()
                        .unwrap()
                        .to_base58();
                    let txn = txn.clone();
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
