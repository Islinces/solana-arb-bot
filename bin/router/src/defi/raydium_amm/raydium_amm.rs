use crate::defi::common::mint_vault::MintVaultSubscribe;
use crate::defi::common::utils::change_option_ignore_none_old;
use crate::defi::dex::Dex;
use crate::defi::file_db::FILE_DB_DIR;
use crate::defi::raydium_amm::math::{CheckedCeilDiv, SwapDirection};
use crate::defi::raydium_amm::state::{AmmInfo, Loadable, PoolInfo};
use crate::defi::types::{AccountUpdate, GrpcAccountUpdateType, Mint, Pool, PoolExtra, Protocol};
use crate::strategy::grpc_message_processor::GrpcMessage;
use crate::strategy::grpc_message_processor::GrpcMessage::RaydiumAmmData;
use anyhow::anyhow;
use arrayref::{array_ref, array_refs};
use base58::ToBase58;
use moka::ops::compute::{CompResult, Op};
use moka::sync::Cache;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::program_pack::Pack;
use solana_program::pubkey::Pubkey;
use solana_sdk::commitment_config::CommitmentConfig;
use spl_token::state::Account;
use std::collections::HashMap;
use std::fs::File;
use std::ops::Add;
use std::sync::Arc;
use tracing::error;
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestAccountsDataSlice,
    SubscribeRequestFilterAccounts,
};

#[derive(Debug)]
pub struct RaydiumAmmDex {
    pool: Pool,
    swap_direction: SwapDirection,
}

impl RaydiumAmmDex {
    pub fn new(pool: Pool, amount_in_mint: Pubkey) -> Self {
        Self {
            swap_direction: if amount_in_mint == pool.mint_0() {
                SwapDirection::Coin2PC
            } else {
                SwapDirection::PC2Coin
            },
            pool,
        }
    }

    pub fn get_subscribe_request(
        pools: &Vec<Pool>,
    ) -> Vec<(GrpcAccountUpdateType, SubscribeRequest)> {
        let mut subscribe_pool_accounts = HashMap::new();
        subscribe_pool_accounts.insert(
            Protocol::RaydiumAMM.name().to_string(),
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
                // coin_vault_mint
                SubscribeRequestAccountsDataSlice {
                    offset: 400,
                    length: 32,
                },
                // pc_vault_mint
                SubscribeRequestAccountsDataSlice {
                    offset: 432,
                    length: 32,
                },
            ],
            ..Default::default()
        };
        let vault_request = MintVaultSubscribe::mint_vault_subscribe_request(&pools);
        vec![
            (GrpcAccountUpdateType::PoolState, pool_request),
            (GrpcAccountUpdateType::MintVault, vault_request),
        ]
    }

    pub async fn fetch_snapshot(rpc_client: Arc<RpcClient>) -> Option<Vec<Pool>> {
        let pool_infos: Vec<PoolInfo> =
            match File::open(format!("{}/raydium_amm.json", FILE_DB_DIR)) {
                Ok(file) => serde_json::from_reader(file).expect("Could not parse JSON"),
                Err(e) => {
                    error!("{}", e);
                    vec![]
                }
            };
        let all_fetch_account_keys = pool_infos
            .into_iter()
            .flat_map(|t| vec![t.pool_id, t.mint_0_vault, t.mint_1_vault])
            .collect::<Vec<_>>();
        let mut all_pool = Vec::new();
        for keys in all_fetch_account_keys.chunks(50 * 3) {
            let chunks_accounts = rpc_client
                .get_multiple_accounts_with_commitment(&keys, CommitmentConfig::finalized())
                .await
                .unwrap()
                .value;
            let chunks_pool = keys
                .iter()
                .zip(chunks_accounts)
                .collect::<Vec<_>>()
                .chunks(3)
                .filter_map(|accounts| {
                    let pool_id = accounts[0].0.clone();
                    let pool_account = accounts[0].1.clone();
                    let vault_0_account = accounts[1].1.clone();
                    let vault_1_account = accounts[2].1.clone();
                    if pool_account.is_none()
                        || vault_0_account.is_none()
                        || vault_1_account.is_none()
                    {
                        None
                    } else {
                        let amm_info = AmmInfo::load_from_bytes(
                            pool_account.as_ref().unwrap().data.as_slice(),
                        );
                        let vault_0 = Account::unpack_from_slice(&vault_0_account.unwrap().data);
                        let vault_1 = Account::unpack_from_slice(&vault_1_account.unwrap().data);

                        if amm_info.is_err() || vault_0.is_err() || vault_1.is_err() {
                            None
                        } else {
                            let amm_info = amm_info.unwrap().clone();
                            let vault_0 = vault_0.unwrap().clone();
                            let vault_1 = vault_1.unwrap().clone();
                            Some(Pool {
                                protocol: Protocol::RaydiumAMM,
                                pool_id,
                                tokens: vec![
                                    Mint {
                                        mint: amm_info.coin_vault_mint,
                                        decimals: amm_info.coin_decimals as u8,
                                    },
                                    Mint {
                                        mint: amm_info.pc_vault_mint,
                                        decimals: amm_info.pc_decimals as u8,
                                    },
                                ],
                                extra: PoolExtra::RaydiumAMM {
                                    mint_0_vault: Some(amm_info.coin_vault),
                                    mint_1_vault: Some(amm_info.pc_vault),
                                    mint_0_vault_amount: Some(vault_0.amount),
                                    mint_1_vault_amount: Some(vault_1.amount),
                                    mint_0_need_take_pnl: Some(
                                        amm_info.state_data.need_take_pnl_coin,
                                    ),
                                    mint_1_need_take_pnl: Some(
                                        amm_info.state_data.need_take_pnl_pc,
                                    ),
                                    swap_fee_numerator: amm_info.fees.swap_fee_numerator,
                                    swap_fee_denominator: amm_info.fees.swap_fee_denominator,
                                },
                            })
                        }
                    }
                })
                .collect::<Vec<_>>();
            all_pool.extend(chunks_pool);
        }
        if all_pool.is_empty() {
            None
        } else {
            Some(all_pool)
        }
    }

    pub async fn try_get_ready_message(
        account_update: AccountUpdate,
        events: Arc<Cache<(String, Pubkey), GrpcMessage>>,
    ) -> Option<GrpcMessage> {
        let account_type = account_update.account_type;
        let filters = account_update.filters;
        let account = account_update.account;
        if let Some(update_account_info) = account.account {
            let txn = update_account_info.txn_signature.unwrap().to_base58();
            let data = update_account_info.data;
            let push_event = match account_type {
                GrpcAccountUpdateType::PoolState => {
                    let src = array_ref![data, 0, 80];
                    let (need_take_pnl_coin, need_take_pnl_pc, _coin_vault_mint, _pc_vault_mint) =
                        array_refs![src, 8, 8, 32, 32];
                    let pool_id = Pubkey::try_from(update_account_info.pubkey.as_slice()).unwrap();
                    Some((
                        pool_id,
                        RaydiumAmmData {
                            pool_id,
                            mint_0_vault_amount: None,
                            mint_1_vault_amount: None,
                            mint_0_need_take_pnl: Some(u64::from_le_bytes(*need_take_pnl_coin)),
                            mint_1_need_take_pnl: Some(u64::from_le_bytes(*need_take_pnl_pc)),
                        },
                    ))
                }
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
                    let pool_id = Pubkey::try_from(items.first().unwrap().clone()).unwrap();
                    Some((
                        pool_id,
                        RaydiumAmmData {
                            pool_id,
                            mint_0_vault_amount,
                            mint_1_vault_amount,
                            mint_0_need_take_pnl: None,
                            mint_1_need_take_pnl: None,
                        },
                    ))
                }
                GrpcAccountUpdateType::NONE => None,
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
            RaydiumAmmData {
                mint_0_vault_amount,
                mint_1_vault_amount,
                mint_0_need_take_pnl,
                mint_1_need_take_pnl,
                ..
            } => {
                if let RaydiumAmmData {
                    mint_0_vault_amount: update_mint_0_vault_amount,
                    mint_1_vault_amount: update_mint_1_vault_amount,
                    mint_0_need_take_pnl: update_mint_0_need_take_pnl,
                    mint_1_need_take_pnl: update_mint_1_need_take_pnl,
                    ..
                } = update_data
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
                    if mint_0_vault_amount.is_some()
                        && mint_1_vault_amount.is_some()
                        && mint_0_need_take_pnl.is_some()
                        && mint_1_need_take_pnl.is_some()
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

    pub async fn try_change_cache() {}
}

#[async_trait::async_trait]
impl Dex for RaydiumAmmDex {
    fn quote(&self, amount_in: u64) -> Option<u64> {
        let amount_in = u128::from(amount_in);
        if let PoolExtra::RaydiumAMM {
            swap_fee_numerator,
            swap_fee_denominator,
            mint_0_vault_amount,
            mint_0_need_take_pnl,
            mint_1_vault_amount,
            mint_1_need_take_pnl,
            ..
        } = self.pool.extra
        {
            let swap_fee = amount_in
                .checked_mul(u128::from(swap_fee_numerator))
                .unwrap()
                .checked_ceil_div(u128::from(swap_fee_denominator))
                .unwrap()
                .0;

            let swap_in_after_deduct_fee = amount_in.checked_sub(swap_fee).unwrap();

            let mint_0_amount_without_pnl = u128::from(
                mint_0_vault_amount
                    .unwrap()
                    .checked_sub(mint_0_need_take_pnl.unwrap())
                    .unwrap(),
            );
            let mint_1_amount_without_pnl = u128::from(
                mint_1_vault_amount
                    .unwrap()
                    .checked_sub(mint_1_need_take_pnl.unwrap())
                    .unwrap(),
            );
            let amount_out = if let SwapDirection::PC2Coin = self.swap_direction {
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
            Some(amount_out.try_into().unwrap())
        } else {
            None
        }
    }
}
