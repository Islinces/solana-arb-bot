use crate::arbitrage::types::swap::Swap;
use crate::cache::{Mint, Pool, PoolState};
use crate::dex::common::utils::{change_data_if_not_same, change_option_ignore_none_old};
use crate::dex::pump_fun::math::CheckedCeilDiv;
use crate::dex::pump_fun::pool_state::{PumpFunInstructionItem, PumpFunPoolState};
use crate::dex::pump_fun::state::GlobalConfig;
use crate::dex::pump_fun::state::Pool as PumpFunPool;
use crate::dex::raydium_clmm::sdk::tickarray_bitmap_extension::TickArrayBitmapExtension;
use crate::dex::{get_ata_program, get_mint_program, get_system_program};
use crate::file_db::DexJson;
use crate::interface::GrpcMessage::{PumpFunAMMData, RaydiumAMMData};
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
use solana_account_decoder_client_types::{UiAccountEncoding, UiDataSliceConfig};
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client_api::config::RpcAccountInfoConfig;
use solana_sdk::address_lookup_table::AddressLookupTableAccount;
use solana_sdk::clock::Clock;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::instruction::AccountMeta;
use solana_sdk::program_pack::Pack;
use solana_sdk::pubkey::Pubkey;
use spl_token::state::Account;
use std::ops::{Add, Div, Mul, Sub};
use std::str::FromStr;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{error, warn};
use yellowstone_grpc_proto::geyser::SubscribeRequest;

pub struct PumpFunDex;

impl Quoter for PumpFunDex {
    fn quote(
        &self,
        amount_in: u64,
        in_mint: Pubkey,
        _out_mint: Pubkey,
        pool: &Pool,
        _clock: Arc<Clock>,
    ) -> Option<u64> {
        if amount_in == 0 || (in_mint != pool.mint_0() && in_mint != pool.mint_1()) {
            return None;
        }
        if let PoolState::PumpFunAMM(pool_state) = &pool.state {
            let base_vault = u128::from(pool_state.mint_0_vault_amount);
            let quote_vault = u128::from(pool_state.mint_1_vault_amount);
            let amount_in = u128::from(amount_in);
            let lp_fee = amount_in
                .mul(u128::from(pool_state.lp_fee_basis_points))
                .checked_ceil_div(10_000)
                .unwrap()
                .0;
            let protocol_fee = amount_in
                .mul(u128::from(pool_state.protocol_fee_basis_points))
                .checked_ceil_div(10_000)
                .unwrap()
                .0;
            let total_fee = lp_fee.add(protocol_fee);
            let effective_amount = amount_in.sub(total_fee);
            let amount_out = if in_mint == pool.mint_0() {
                quote_vault
                    .mul(effective_amount)
                    .div(base_vault.add(effective_amount))
            } else {
                base_vault
                    .mul(effective_amount)
                    .div(quote_vault.add(effective_amount))
            };
            Some(amount_out.try_into().unwrap_or_else(|_| {
                // error!("amount_out is too large");
                u64::MIN
            }))
        } else {
            None
        }
    }
}

impl InstructionItemCreator for PumpFunDex {
    fn create_instruction_item(&self, pool: &Pool, in_mint: &Pubkey) -> Option<InstructionItem> {
        if let PoolState::PumpFunAMM(pool_state) = &pool.state {
            Some(InstructionItem::PumpFunAMM(PumpFunInstructionItem {
                pool_id: pool.pool_id,
                mint_0: pool.mint_0(),
                mint_1: pool.mint_1(),
                mint_0_vault: pool_state.mint_0_vault,
                mint_1_vault: pool_state.mint_1_vault,
                alt: pool.alt.clone(),
                zero_to_one: in_mint == &pool.mint_0(),
            }))
        } else {
            None
        }
    }
}

impl AccountMetaConverter for PumpFunDex {
    fn converter(
        &self,
        wallet: Pubkey,
        instruction_item: InstructionItem,
    ) -> Option<(Vec<AccountMeta>, Vec<AddressLookupTableAccount>)> {
        match instruction_item {
            InstructionItem::PumpFunAMM(item) => {
                let mut accounts = Vec::with_capacity(17);
                // 1.pool
                accounts.push(AccountMeta::new_readonly(item.pool_id, false));
                // 2. wallet
                accounts.push(AccountMeta::new(wallet, true));
                // 3. global config
                accounts.push(AccountMeta::new_readonly(GlobalConfig::key(), false));
                // 4.base mint
                accounts.push(AccountMeta::new_readonly(item.mint_0, false));
                // 5.quote mint
                accounts.push(AccountMeta::new_readonly(item.mint_1, false));
                // 6.base mint ata
                let (base_ata, _) = Pubkey::find_program_address(
                    &[
                        &wallet.to_bytes(),
                        &get_mint_program().to_bytes(),
                        &item.mint_0.to_bytes(),
                    ],
                    &get_ata_program(),
                );
                accounts.push(AccountMeta::new(base_ata, false));
                // 7.quote mint ata
                let (quote_ata, _) = Pubkey::find_program_address(
                    &[
                        &wallet.to_bytes(),
                        &get_mint_program().to_bytes(),
                        &item.mint_1.to_bytes(),
                    ],
                    &get_ata_program(),
                );
                accounts.push(AccountMeta::new(quote_ata, false));
                // 8.base mint vault
                accounts.push(AccountMeta::new(item.mint_0_vault, false));
                // 9.quote mint vault
                accounts.push(AccountMeta::new(item.mint_1_vault, false));
                // 10.fee account
                accounts.push(AccountMeta::new_readonly(
                    crate::dex::pump_fun::get_fee_account_with_rand(),
                    false,
                ));
                // 11.pump fun sol ata 小费账户
                accounts.push(AccountMeta::new(
                    Pubkey::find_program_address(
                        &[
                            &wallet.to_bytes(),
                            &get_mint_program().to_bytes(),
                            &spl_token::native_mint::id().to_bytes(),
                        ],
                        &get_ata_program(),
                    )
                    .0,
                    false,
                ));
                // 12.base quote program
                accounts.push(AccountMeta::new_readonly(get_mint_program(), false));
                // 13.quote quote program
                accounts.push(AccountMeta::new_readonly(get_mint_program(), false));
                // 14.system program
                accounts.push(AccountMeta::new_readonly(get_system_program(), false));
                // 15.system program
                accounts.push(AccountMeta::new_readonly(get_ata_program(), false));
                // 16.event authority
                accounts.push(AccountMeta::new_readonly(
                    Pubkey::from_str("GS4CU59F31iL7aR2Q8zVS8DRrcRnXX1yjQ66TqNVQnaR").unwrap(),
                    false,
                ));
                // 17.pump fun program
                accounts.push(AccountMeta::new_readonly(
                    DexType::PumpFunAMM.get_program_id(),
                    false,
                ));
                Some((accounts, vec![item.alt]))
            }
            _ => None,
        }
    }
}

pub struct PumpFunGrpcSubscribeRequestGenerator;

impl GrpcSubscribeRequestGenerator for PumpFunGrpcSubscribeRequestGenerator {
    fn create_subscribe_requests(
        &self,
        pools: &[Pool],
    ) -> Option<Vec<(SubscribeKey, SubscribeRequest)>> {
        let vault_subscribe_request = self.mint_vault_subscribe_request(pools);
        if vault_subscribe_request.accounts.is_empty() {
            warn!("【{}】所有池子未找到金库账户", DexType::PumpFunAMM);
            None
        } else {
            Some(vec![(
                (DexType::PumpFunAMM, GrpcAccountUpdateType::MintVault),
                vault_subscribe_request,
            )])
        }
    }
}

pub struct PumpFunReadyGrpcMessageOperator;

impl ReadyGrpcMessageOperator for PumpFunReadyGrpcMessageOperator {
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
            match account_type {
                GrpcAccountUpdateType::MintVault => {
                    let src = array_ref![data, 0, 41];
                    let (_mint, amount, _state) = array_refs![src, 32, 8, 1];
                    let mut mint_0_vault_amount = None;
                    let mut mint_1_vault_amount = None;
                    let items = filters.get(0).unwrap().split(":").collect::<Vec<&str>>();
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
            PumpFunAMMData {
                mint_0_vault_amount,
                mint_1_vault_amount,
                ..
            } => {
                if let PumpFunAMMData {
                    mint_0_vault_amount: update_mint_0_vault_amount,
                    mint_1_vault_amount: update_mint_1_vault_amount,
                    ..
                } = new
                {
                    change_option_ignore_none_old(mint_0_vault_amount, update_mint_0_vault_amount);
                    change_option_ignore_none_old(mint_1_vault_amount, update_mint_1_vault_amount);
                }
            }
            _ => {}
        }
    }
}

#[derive(Default)]
pub struct PumpFunAccountSnapshotFetcher;

#[async_trait::async_trait]
impl AccountSnapshotFetcher for PumpFunAccountSnapshotFetcher {
    async fn fetch_snapshot(
        &self,
        pool_json: Vec<DexJson>,
        rpc_client: Arc<RpcClient>,
    ) -> Option<Vec<Pool>> {
        let global_config = GlobalConfig::try_from_slice(
            rpc_client
                .get_account_with_config(
                    &GlobalConfig::key(),
                    RpcAccountInfoConfig {
                        encoding: Some(UiAccountEncoding::Base64),
                        data_slice: Some(UiDataSliceConfig {
                            offset: 8,
                            length: 305,
                        }),
                        ..Default::default()
                    },
                )
                .await
                .unwrap()
                .value?
                .data
                .as_slice(),
        )
        .unwrap();
        let mut join_set = JoinSet::new();
        for chunks_pool in pool_json.chunks(100) {
            let rpc_client = rpc_client.clone();
            let chunks_pool_json = Arc::new(chunks_pool.to_vec());
            let chunks_pool_id = chunks_pool_json
                .clone()
                .iter()
                .map(|json| json.pool)
                .collect::<Vec<_>>();
            let alt_map = self
                .load_lookup_table_accounts(rpc_client.clone(), chunks_pool_json.clone())
                .await
                .unwrap();
            join_set.spawn(async move {
                let mut pools = Vec::with_capacity(chunks_pool_id.len());
                for (index, (pool_id, pool_account)) in chunks_pool_id
                    .iter()
                    .zip(
                        rpc_client
                            .get_multiple_accounts_with_config(
                                &chunks_pool_id,
                                RpcAccountInfoConfig {
                                    commitment: Some(CommitmentConfig::finalized()),
                                    data_slice: Some(UiDataSliceConfig {
                                        offset: 8,
                                        length: 203,
                                    }),
                                    ..Default::default()
                                },
                            )
                            .await
                            .unwrap()
                            .value,
                    )
                    .enumerate()
                {
                    if let Some(account) = pool_account {
                        let pool_state =
                            crate::dex::pump_fun::state::Pool::try_from_slice(&account.data)
                                .unwrap();
                        let vault_accounts = rpc_client
                            .get_multiple_accounts_with_commitment(
                                &[
                                    pool_state.pool_base_token_account,
                                    pool_state.pool_quote_token_account,
                                ],
                                CommitmentConfig::finalized(),
                            )
                            .await
                            .unwrap()
                            .value
                            .iter()
                            .filter_map(|vault_account| {
                                if let Some(account) = vault_account {
                                    Some(
                                        Account::unpack_from_slice(account.data.as_slice())
                                            .unwrap(),
                                    )
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>();
                        if vault_accounts.len() != 2 {
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
                            protocol: DexType::PumpFunAMM,
                            pool_id: *pool_id,
                            tokens: vec![
                                Mint {
                                    mint: pool_state.base_mint,
                                },
                                Mint {
                                    mint: pool_state.quote_mint,
                                },
                            ],
                            state: PoolState::PumpFunAMM(PumpFunPoolState::new(
                                pool_state.pool_base_token_account,
                                pool_state.pool_quote_token_account,
                                vault_accounts.first().unwrap().amount,
                                vault_accounts.last().unwrap().amount,
                                global_config.lp_fee_basis_points,
                                global_config.protocol_fee_basis_points,
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
