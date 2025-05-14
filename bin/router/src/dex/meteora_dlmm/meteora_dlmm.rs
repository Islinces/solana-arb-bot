use crate::cache::PoolState::MeteoraDLMM;
use crate::cache::{Mint, Pool};
use crate::dex::meteora_dlmm::pool_state::{
    MeteoraDLMMInstructionItem, MeteoraDLMMPoolState, PoolMonitorData,
};
use crate::dex::meteora_dlmm::sdk::commons::pda::derive_bin_array_bitmap_extension;
use crate::dex::meteora_dlmm::sdk::commons::quote::{
    get_bin_array_pubkeys_for_swap, quote_exact_in,
};
use crate::dex::meteora_dlmm::sdk::interface::accounts::{
    BinArray, BinArrayAccount, BinArrayBitmapExtension, BinArrayBitmapExtensionAccount, LbPair,
    LbPairAccount,
};
use crate::dex::{get_ata_program, get_mint_program, POOL_CACHE_HOLDER};
use crate::file_db::DexJson;
use crate::interface::{
    AccountMetaConverter, AccountSnapshotFetcher, AccountUpdate, Dex, DexType,
    GrpcAccountUpdateType, GrpcMessage, GrpcSubscribeRequestGenerator, InstructionItem,
    InstructionItemCreator, Quoter, ReadyGrpcMessageOperator, SubscribeKey,
};
use anyhow::Result;
use anyhow::{anyhow, Context};
use base58::ToBase58;
use borsh::BorshDeserialize;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::address_lookup_table::AddressLookupTableAccount;
use solana_sdk::clock::Clock;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::instruction::AccountMeta;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::SysvarId;
use spl_token_2022::extension::transfer_fee::TransferFeeConfig;
use spl_token_2022::extension::{BaseStateWithExtensions, StateWithExtensions};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::task::JoinSet;
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestFilterAccounts,
};

pub struct MeteoraDLMMDex;

impl Quoter for MeteoraDLMMDex {
    fn quote(
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
            match pool_state.get_bin_array_map(swap_for_y, 3) {
                Ok(bin_array_map) => {
                    let result = quote_exact_in(
                        pool.pool_id,
                        lp_pair_state,
                        amount_in,
                        swap_for_y,
                        bin_array_map,
                        pool_state.bin_array_bitmap_extension,
                        clock.clone(),
                        pool_state.mint_x_transfer_fee_config,
                        pool_state.mint_y_transfer_fee_config,
                    );
                    match result {
                        Ok(quote) => Some(quote.amount_out),
                        Err(_) => None,
                    }
                }
                Err(_) => None,
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
                bin_arrays: pool_state.get_bin_array_keys(zero_to_one, 3),
                alt: pool.alt.clone(),
                oracle: pool_state.oracle,
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
                let mut accounts = Vec::with_capacity(20);
                // 1.lb pair
                accounts.push(AccountMeta::new(item.pool_id, false));
                // 2.bitmap extension
                // accounts.push(AccountMeta::new(item.bitmap_extension, false));
                accounts.push(AccountMeta::new_readonly(
                    DexType::MeteoraDLMM.get_program_id(),
                    false,
                ));
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
                accounts.push(AccountMeta::new(item.oracle, false));
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
                accounts.push(AccountMeta::new_readonly(
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
            PoolMonitorData::subscribe_request(pools),
            BinArray::subscribe_request(pools),
            (
                (DexType::MeteoraDLMM, GrpcAccountUpdateType::Clock),
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

pub struct MeteoraDLMMGrpcMessageOperator;

impl ReadyGrpcMessageOperator for MeteoraDLMMGrpcMessageOperator {
    fn parse_message(
        &self,
        update_account: AccountUpdate,
    ) -> Result<(Option<(String, Pubkey)>, GrpcMessage)> {
        let account_type = &update_account.account_type;
        let account = &update_account.account;
        if let Some(update_account_info) = &account.account {
            let data = &update_account_info.data;
            match account_type {
                GrpcAccountUpdateType::Pool => {
                    let txn = &update_account_info
                        .txn_signature
                        .as_ref()
                        .unwrap()
                        .to_base58();
                    let pool_id = Pubkey::try_from_slice(update_account_info.pubkey.as_slice())?;
                    let pool_monitor_data = PoolMonitorData::try_from_slice(data)?;
                    match POOL_CACHE_HOLDER.get() {
                        None => Err(anyhow!("")),
                        Some(cache_holder) => {
                            let pool_cache = cache_holder.pool_cache.clone();
                            let x = match pool_cache.pool_map.get(&pool_id) {
                                None => Err(anyhow!("")),
                                Some(pool) => {
                                    if let Some(distance_bin_array_indexs) = match pool.state {
                                        MeteoraDLMM(ref state) => {
                                            Some(state.calculate_distance_bin_array_indexs(
                                                pool_monitor_data.active_id,
                                            ))
                                        }
                                        _ => None,
                                    } {
                                        Ok((
                                            Some((txn.clone(), pool_id)),
                                            GrpcMessage::MeteoraDlmmMonitorData {
                                                pool_data: Some(pool_monitor_data),
                                                bin_arrays: None,
                                                expect_bin_array_index: Some(
                                                    distance_bin_array_indexs,
                                                ),
                                                pool_id,
                                                instant: update_account.instant,
                                                slot: update_account.account.slot,
                                            },
                                        ))
                                    } else {
                                        Err(anyhow!(""))
                                    }
                                }
                            };
                            x
                        }
                    }
                }
                GrpcAccountUpdateType::BinArray => {
                    let txn = &update_account_info
                        .txn_signature
                        .as_ref()
                        .unwrap()
                        .to_base58();
                    let bin_array = BinArrayAccount::deserialize(data)?.0;
                    Ok((
                        Some((txn.clone(), bin_array.lb_pair)),
                        GrpcMessage::MeteoraDlmmMonitorData {
                            pool_data: None,
                            bin_arrays: Some(vec![bin_array]),
                            expect_bin_array_index: None,
                            pool_id: bin_array.lb_pair,
                            instant: update_account.instant,
                            slot: update_account.account.slot,
                        },
                    ))
                }
                GrpcAccountUpdateType::Clock => {
                    let clock: Clock = serde_json::from_slice(data)?;
                    Ok((None, GrpcMessage::Clock(clock)))
                }
                _ => Err(anyhow!("")),
            }
        } else {
            Err(anyhow!(""))
        }
    }

    fn change_data(&self, old: &mut GrpcMessage, new: GrpcMessage) {
        match old {
            GrpcMessage::MeteoraDlmmMonitorData {
                pool_data,
                bin_arrays,
                expect_bin_array_index,
                ..
            } => match new {
                GrpcMessage::MeteoraDlmmMonitorData {
                    pool_data: update_pool_data,
                    bin_arrays: update_bin_arrays,
                    expect_bin_array_index: update_expect_bin_array_index,
                    ..
                } => {
                    if let Some(new) = update_expect_bin_array_index {
                        expect_bin_array_index.replace(new);
                    }
                    if let Some(new) = update_pool_data {
                        pool_data.replace(new);
                    }
                    if bin_arrays.as_ref().is_some() && update_bin_arrays.as_ref().is_some() {
                        bin_arrays
                            .as_mut()
                            .unwrap()
                            .extend(update_bin_arrays.unwrap());
                    } else if bin_arrays.as_ref().is_none() && update_bin_arrays.as_ref().is_some()
                    {
                        bin_arrays.replace(update_bin_arrays.unwrap());
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}
