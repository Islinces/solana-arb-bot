use crate::cache::{Mint, Pool, PoolState};
use crate::dex::common::utils::change_option_ignore_none_old;
use crate::dex::raydium_amm::math::CheckedCeilDiv;
use crate::dex::raydium_amm::pool_state::{RaydiumAMMInstructionItem, RaydiumAMMPoolState};
use crate::dex::raydium_amm::state::{AmmInfo, AmmStatus};
use crate::dex::{get_ata_program, get_mint_program};
use crate::file_db::DexJson;
use crate::interface::GrpcMessage::RaydiumAMMData;
use crate::interface::{
    AccountMetaConverter, AccountSnapshotFetcher, AccountUpdate, Dex, DexType,
    GrpcAccountUpdateType, GrpcMessage, GrpcSubscribeRequestGenerator, InstructionItem,
    InstructionItemCreator, Quoter, ReadyGrpcMessageOperator, SubscribeKey,
};
use anyhow::anyhow;
use anyhow::Result;
use arrayref::{array_ref, array_refs};
use base58::ToBase58;
use borsh::BorshDeserialize;
use chrono::Utc;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::address_lookup_table::AddressLookupTableAccount;
use solana_sdk::clock::Clock;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::instruction::AccountMeta;
use solana_sdk::program_pack::Pack;
use solana_sdk::pubkey::Pubkey;
use spl_token::state::Account;
use std::collections::HashMap;
use std::ops::Add;
use std::str::FromStr;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{info, instrument, warn};
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestAccountsDataSlice,
    SubscribeRequestFilterAccounts,
};

pub struct RaydiumAmmDex;

impl Quoter for RaydiumAmmDex {
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
        if let PoolState::RaydiumAMM(pool_state) = &pool.state {
            let amount_in = u128::from(amount_in);
            let swap_fee = amount_in
                .checked_mul(u128::from(pool_state.swap_fee_numerator))
                .unwrap()
                .checked_ceil_div(u128::from(pool_state.swap_fee_denominator))
                .unwrap()
                .0;
            info!("fee : {:?}", swap_fee);
            let swap_in_after_deduct_fee = amount_in.checked_sub(swap_fee).unwrap();

            let mint_0_amount_without_pnl = u128::from(
                pool_state
                    .mint_0_vault_amount
                    .unwrap()
                    .checked_sub(pool_state.mint_0_need_take_pnl.unwrap())
                    .unwrap(),
            );
            let mint_1_amount_without_pnl = u128::from(
                pool_state
                    .mint_1_vault_amount
                    .unwrap()
                    .checked_sub(pool_state.mint_1_need_take_pnl.unwrap())
                    .unwrap(),
            );
            let amount_out = if in_mint == pool.mint_0() {
                mint_1_amount_without_pnl
                    .checked_mul(swap_in_after_deduct_fee)
                    .unwrap()
                    .checked_div(
                        mint_0_amount_without_pnl.add(swap_in_after_deduct_fee), // .unwrap(),
                    )
                    .unwrap()
            } else {
                mint_0_amount_without_pnl
                    .checked_mul(swap_in_after_deduct_fee)
                    .unwrap()
                    .checked_div(
                        mint_1_amount_without_pnl.add(swap_in_after_deduct_fee), // .unwrap(),
                    )
                    .unwrap()
            };
            Some(amount_out.try_into().unwrap_or_else(|_| u64::MIN))
        } else {
            None
        }
    }
}

impl InstructionItemCreator for RaydiumAmmDex {
    fn create_instruction_item(&self, pool: &Pool, in_mint: &Pubkey) -> Option<InstructionItem> {
        if let PoolState::RaydiumAMM(pool_state) = &pool.state {
            Some(InstructionItem::RaydiumAMM(RaydiumAMMInstructionItem {
                pool_id: pool.pool_id,
                mint_0: pool.mint_0(),
                mint_1: pool.mint_1(),
                mint_0_vault: pool_state.mint_0_vault.unwrap(),
                mint_1_vault: pool_state.mint_1_vault.unwrap(),
                alt: pool.alt.clone(),
                zero_to_one: in_mint == &pool.mint_0(),
            }))
        } else {
            None
        }
    }
}

impl AccountMetaConverter for RaydiumAmmDex {
    fn converter(
        &self,
        wallet: Pubkey,
        instruction_item: InstructionItem,
    ) -> Option<(Vec<AccountMeta>, Vec<AddressLookupTableAccount>)> {
        match instruction_item {
            InstructionItem::RaydiumAMM(item) => {
                let mut accounts = Vec::with_capacity(17);
                // 1.mint program
                accounts.push(AccountMeta::new_readonly(get_mint_program(), false));
                // 2.pool
                accounts.push(AccountMeta::new_readonly(item.pool_id, false));
                // 3.authority id
                accounts.push(AccountMeta::new_readonly(
                    crate::dex::raydium_amm::RAYDIUM_AUTHORITY_ID,
                    false,
                ));
                // 4.open order
                accounts.push(AccountMeta::new_readonly(item.pool_id, false));
                // 5.coin vault
                accounts.push(AccountMeta::new(item.mint_0_vault, false));
                // 6.pc vault
                accounts.push(AccountMeta::new(item.mint_1_vault, false));
                // 7.Serum Program Id
                accounts.push(AccountMeta::new_readonly(item.pool_id, false));
                // 8.Serum Market
                accounts.push(AccountMeta::new_readonly(item.pool_id, false));
                // 9.Serum Bids
                accounts.push(AccountMeta::new_readonly(item.pool_id, false));
                // 10.Serum Asks
                accounts.push(AccountMeta::new_readonly(item.pool_id, false));
                // 11.Serum Event Queue
                accounts.push(AccountMeta::new_readonly(item.pool_id, false));
                // 12.Serum Coin Vault Account
                accounts.push(AccountMeta::new_readonly(item.pool_id, false));
                // 13.Serum Pc Vault Account
                accounts.push(AccountMeta::new_readonly(item.pool_id, false));
                // 14.Serum Vault Signer
                accounts.push(AccountMeta::new_readonly(item.pool_id, false));
                // 15.coin mint ata
                let (coin_ata, _) = Pubkey::find_program_address(
                    &[
                        &wallet.to_bytes(),
                        &get_mint_program().to_bytes(),
                        &item.mint_0.to_bytes(),
                    ],
                    &get_ata_program(),
                );
                accounts.push(AccountMeta::new(coin_ata, false));
                // 16.pc mint ata
                let (pc_ata, _) = Pubkey::find_program_address(
                    &[
                        &wallet.to_bytes(),
                        &get_mint_program().to_bytes(),
                        &item.mint_0.to_bytes(),
                    ],
                    &get_ata_program(),
                );
                accounts.push(AccountMeta::new(pc_ata, false));
                // 17.wallet
                accounts.push(AccountMeta::new(wallet, true));
                Some((accounts, vec![item.alt]))
            }
            _ => None,
        }
    }
}

pub struct RaydiumAmmGrpcMessageOperator;
impl ReadyGrpcMessageOperator for RaydiumAmmGrpcMessageOperator {
    fn parse_message(
        &self,
        update_account: AccountUpdate,
    ) -> Result<((String, Pubkey), GrpcMessage)> {
        let account_type = &update_account.account_type;
        let filters = &update_account.filters;
        let account = &update_account.account;
        if let Some(update_account_info) = &account.account {
            let data = &update_account_info.data;
            let txn = &update_account_info
                .txn_signature
                .as_ref()
                .unwrap()
                .to_base58();
            let tx = txn.clone();
            match account_type {
                GrpcAccountUpdateType::Pool => {
                    let src = array_ref![data, 0, 16];
                    let (need_take_pnl_coin, need_take_pnl_pc) = array_refs![src, 8, 8];
                    let pool_id = Pubkey::try_from(update_account_info.pubkey.as_slice())?;
                    Ok((
                        (txn.clone(), pool_id),
                        RaydiumAMMData {
                            pool_id,
                            mint_0_vault_amount: None,
                            mint_1_vault_amount: None,
                            mint_0_need_take_pnl: Some(u64::from_le_bytes(*need_take_pnl_coin)),
                            mint_1_need_take_pnl: Some(u64::from_le_bytes(*need_take_pnl_pc)),
                            instant: update_account.instant,
                            slot: account.slot,
                        },
                    ))
                }
                GrpcAccountUpdateType::MintVault => {
                    let src = array_ref![data, 0, 41];
                    let (mint, amount, _state) = array_refs![src, 32, 8, 1];
                    let mut mint_0_vault_amount = None;
                    let mut mint_1_vault_amount = None;
                    let items = filters.first().unwrap().split(":").collect::<Vec<&str>>();
                    let mint_flag = items.last().unwrap().to_string();
                    if mint_flag.eq("0") {
                        mint_0_vault_amount = Some(u64::from_le_bytes(*amount));
                    } else {
                        mint_1_vault_amount = Some(u64::from_le_bytes(*amount));
                    }
                    let pool_id = Pubkey::try_from(*items.first().unwrap())?;
                    Ok((
                        (txn.clone(), pool_id),
                        RaydiumAMMData {
                            pool_id,
                            mint_0_vault_amount,
                            mint_1_vault_amount,
                            mint_0_need_take_pnl: None,
                            mint_1_need_take_pnl: None,
                            instant: update_account.instant,
                            slot: account.slot,
                        },
                    ))
                }
                _ => Err(anyhow!("")),
            }
        } else {
            Err(anyhow!(""))
        }
    }

    fn change_data(&self, old: &mut GrpcMessage, new: GrpcMessage) {
        match old {
            RaydiumAMMData {
                mint_0_vault_amount,
                mint_1_vault_amount,
                mint_0_need_take_pnl,
                mint_1_need_take_pnl,
                ..
            } => {
                if let RaydiumAMMData {
                    mint_0_vault_amount: update_mint_0_vault_amount,
                    mint_1_vault_amount: update_mint_1_vault_amount,
                    mint_0_need_take_pnl: update_mint_0_need_take_pnl,
                    mint_1_need_take_pnl: update_mint_1_need_take_pnl,
                    ..
                } = new
                {
                    change_option_ignore_none_old(mint_0_vault_amount, update_mint_0_vault_amount);
                    change_option_ignore_none_old(mint_1_vault_amount, update_mint_1_vault_amount);
                    change_option_ignore_none_old(
                        mint_0_need_take_pnl,
                        update_mint_0_need_take_pnl,
                    );
                    change_option_ignore_none_old(
                        mint_1_need_take_pnl,
                        update_mint_1_need_take_pnl,
                    );
                }
            }
            _ => {}
        }
    }
}

pub struct RaydiumAmmSubscribeRequestCreator;

impl GrpcSubscribeRequestGenerator for RaydiumAmmSubscribeRequestCreator {
    fn create_subscribe_requests(
        &self,
        pools: &[Pool],
    ) -> Option<Vec<(SubscribeKey, SubscribeRequest)>> {
        let mut subscribe_pool_accounts = HashMap::new();
        subscribe_pool_accounts.insert(
            format!("{:?}", DexType::RaydiumAMM),
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
                // state_data.need_take_pnl_coin
                SubscribeRequestAccountsDataSlice {
                    offset: 192,
                    length: 8,
                },
                // state_data.need_take_pnl_pc
                SubscribeRequestAccountsDataSlice {
                    offset: 200,
                    length: 8,
                },
            ],
            ..Default::default()
        };
        let vault_subscribe_request = self.mint_vault_subscribe_request(pools);
        if vault_subscribe_request.accounts.is_empty() {
            warn!("【{}】所有池子未找到金库账户", DexType::RaydiumAMM);
            None
        } else {
            Some(vec![
                (
                    (DexType::RaydiumAMM, GrpcAccountUpdateType::Pool),
                    pool_request,
                ),
                (
                    (DexType::RaydiumAMM, GrpcAccountUpdateType::MintVault),
                    vault_subscribe_request,
                ),
            ])
        }
    }
}

#[derive(Default, Debug)]
pub struct RaydiumAmmSnapshotFetcher;

#[async_trait::async_trait]
impl AccountSnapshotFetcher for RaydiumAmmSnapshotFetcher {
    async fn fetch_snapshot(
        &self,
        pool_json: Vec<DexJson>,
        rpc_client: Arc<RpcClient>,
    ) -> Option<Vec<Pool>> {
        let mut join_set = JoinSet::new();
        for chunks_pools in pool_json.chunks(100) {
            let rpc_client = rpc_client.clone();
            let chunks_pool_json = Arc::new(chunks_pools.to_vec());
            let chunks_pool_ids = chunks_pool_json
                .clone()
                .iter()
                .map(|id| id.pool)
                .collect::<Vec<_>>();
            let alt_map = self
                .load_lookup_table_accounts(rpc_client.clone(), chunks_pool_json.clone())
                .await
                .unwrap();
            join_set.spawn(async move {
                let mut pools = Vec::with_capacity(chunks_pool_ids.len());
                for (index, (pool_id, pool_account)) in chunks_pool_ids
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
                    .enumerate()
                {
                    if let Some(pool_account) = pool_account {
                        let amm_info =
                            AmmInfo::try_from_slice(pool_account.data.as_slice()).unwrap();
                        if !AmmStatus::from_u64(amm_info.status).swap_permission()
                            || AmmStatus::from_u64(amm_info.status).orderbook_permission()
                            || amm_info.status == AmmStatus::WaitingTrade as u64
                            || amm_info.state_data.pool_open_time >= (Utc::now().timestamp() as u64)
                        {
                            continue;
                        }
                        // if amm_info.coin_vault_mint
                        //     != Pubkey::from_str("So11111111111111111111111111111111111111112")
                        //         .unwrap()
                        //     && amm_info.pc_vault_mint
                        //         != Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")
                        //             .unwrap()
                        // {
                        //     continue;
                        // }
                        let mint_vault_amount = rpc_client
                            .get_multiple_accounts_with_commitment(
                                &vec![amm_info.coin_vault, amm_info.pc_vault],
                                CommitmentConfig::finalized(),
                            )
                            .await
                            .unwrap()
                            .value
                            .iter()
                            .filter_map(|account| {
                                let vault_0 = Account::unpack_from_slice(
                                    account.as_ref().unwrap().data.as_slice(),
                                );
                                if let Ok(vault_0) = vault_0 {
                                    Some(vault_0.amount)
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>();
                        if mint_vault_amount.len() != 2 {
                            continue;
                        }
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
                        pools.push(Pool {
                            protocol: DexType::RaydiumAMM,
                            pool_id: *pool_id,
                            tokens: vec![
                                Mint {
                                    mint: amm_info.coin_vault_mint,
                                },
                                Mint {
                                    mint: amm_info.pc_vault_mint,
                                },
                            ],
                            state: PoolState::RaydiumAMM(RaydiumAMMPoolState::new(
                                Some(amm_info.coin_vault),
                                Some(amm_info.pc_vault),
                                Some(mint_vault_amount.get(0).unwrap().clone()),
                                Some(mint_vault_amount.get(1).unwrap().clone()),
                                Some(amm_info.state_data.need_take_pnl_coin),
                                Some(amm_info.state_data.need_take_pnl_pc),
                                amm_info.fees.swap_fee_numerator,
                                amm_info.fees.swap_fee_denominator,
                            )),
                            alt: alt.unwrap(),
                        })
                    }
                }
                pools
            });
        }
        let mut all_pools = Vec::with_capacity(pool_json.len());
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
