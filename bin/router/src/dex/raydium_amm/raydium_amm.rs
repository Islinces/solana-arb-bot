use crate::cache::{Mint, Pool, PoolState};
use crate::dex::common::mint_vault::MintVaultSubscribe;
use crate::dex::common::utils::change_option_ignore_none_old;
use crate::dex::raydium_amm::math::CheckedCeilDiv;
use crate::dex::raydium_amm::state::{AmmInfo, Loadable};
use crate::interface::GrpcMessage::RaydiumAMMData;
use crate::interface::{
    AccountSnapshotFetcher, AccountUpdate, CacheUpdater, Dex, GrpcAccountUpdateType, GrpcMessage,
    GrpcSubscribeRequestGenerator, Protocol, ReadyGrpcMessageOperator, SubscribeKey,
};
use anyhow::anyhow;
use anyhow::Result;
use arrayref::{array_ref, array_refs};
use base58::ToBase58;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::program_pack::Pack;
use solana_program::pubkey::Pubkey;
use solana_sdk::commitment_config::CommitmentConfig;
use spl_token::state::Account;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::ops::Add;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::warn;
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestAccountsDataSlice,
    SubscribeRequestFilterAccounts,
};

#[derive(Clone)]
pub struct RaydiumAmmDex {
    pool: Pool,
    amount_in_mint: Pubkey,
}

impl RaydiumAmmDex {
    pub fn new(pool: Pool, amount_in_mint: Pubkey) -> Option<Self> {
        Some(Self {
            amount_in_mint,
            pool,
        })
    }
}

impl Debug for RaydiumAmmDex {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RaydiumAmmDex: {},{}",
            self.pool.pool_id, self.amount_in_mint
        )
    }
}

#[async_trait::async_trait]
impl Dex for RaydiumAmmDex {
    async fn quote(&self, amount_in: u64) -> Option<u64> {
        if amount_in == u64::MIN {
            return None;
        }
        let amount_in = u128::from(amount_in);
        if let PoolState::RaydiumAMM {
            swap_fee_numerator,
            swap_fee_denominator,
            mint_0_vault_amount,
            mint_0_need_take_pnl,
            mint_1_vault_amount,
            mint_1_need_take_pnl,
            ..
        } = self.pool.state
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
            let amount_out = if self.pool.mint_0() == self.amount_in_mint {
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
            Some(amount_out.try_into().unwrap_or_else(|_| {
                eprintln!("amount_out is too large");
                u64::MIN
            }))
        } else {
            None
        }
    }

    fn clone_self(&self) -> Box<dyn Dex> {
        Box::new(self.clone())
    }
}

pub struct RaydiumAmmGrpcMessageOperator {
    update_account: AccountUpdate,
    txn: Option<String>,
    pool_id: Option<Pubkey>,
    grpc_message: Option<GrpcMessage>,
}

impl RaydiumAmmGrpcMessageOperator {
    pub fn new(update_account: AccountUpdate) -> Self {
        Self {
            update_account,
            txn: None,
            pool_id: None,
            grpc_message: None,
        }
    }
}
impl ReadyGrpcMessageOperator for RaydiumAmmGrpcMessageOperator {
    fn parse_message(&mut self) -> Result<()> {
        let account_type = &self.update_account.account_type;
        let filters = &self.update_account.filters;
        let account = &self.update_account.account;
        if let Some(update_account_info) = &account.account {
            let data = &update_account_info.data;
            let txn = &update_account_info
                .txn_signature
                .as_ref()
                .unwrap()
                .to_base58();
            let txn = txn.clone();
            match account_type {
                GrpcAccountUpdateType::PoolState => {
                    let src = array_ref![data, 0, 16];
                    let (need_take_pnl_coin, need_take_pnl_pc) = array_refs![src, 8, 8];
                    let pool_id = Pubkey::try_from(update_account_info.pubkey.as_slice())?;
                    self.pool_id = Some(pool_id);
                    self.txn = Some(txn);
                    self.grpc_message = Some(RaydiumAMMData {
                        pool_id,
                        mint_0_vault_amount: None,
                        mint_1_vault_amount: None,
                        mint_0_need_take_pnl: Some(u64::from_le_bytes(*need_take_pnl_coin)),
                        mint_1_need_take_pnl: Some(u64::from_le_bytes(*need_take_pnl_pc)),
                    });
                    Ok(())
                }
                GrpcAccountUpdateType::MintVault => {
                    let src = array_ref![data, 0, 41];
                    let (_mint, amount, _state) = array_refs![src, 32, 8, 1];
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
                    self.pool_id = Some(pool_id);
                    self.txn = Some(txn);
                    self.grpc_message = Some(RaydiumAMMData {
                        pool_id,
                        mint_0_vault_amount,
                        mint_1_vault_amount,
                        mint_0_need_take_pnl: None,
                        mint_1_need_take_pnl: None,
                    });
                    Ok(())
                }
                _ => Err(anyhow!("")),
            }
        } else {
            Err(anyhow!(""))
        }
    }

    fn change_and_return_ready_data(&self, old: &mut GrpcMessage) -> Result<()> {
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
                } = self.grpc_message.as_ref().unwrap().clone()
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

    fn get_cache_key(&self) -> (String, Pubkey) {
        let txn = self.txn.as_ref().unwrap();
        (txn.clone(), self.pool_id.unwrap())
    }

    fn get_insert_data(&self) -> GrpcMessage {
        self.grpc_message.as_ref().unwrap().clone()
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
            format!("{:?}", Protocol::RaydiumAMM),
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
        let vault_subscribe_request = MintVaultSubscribe::mint_vault_subscribe_request(pools);
        if vault_subscribe_request.accounts.is_empty() {
            warn!("【{}】所有池子未找到金库账户", Protocol::RaydiumAMM);
            None
        } else {
            Some(vec![
                (
                    (Protocol::RaydiumAMM, GrpcAccountUpdateType::PoolState),
                    pool_request,
                ),
                (
                    (Protocol::RaydiumAMM, GrpcAccountUpdateType::MintVault),
                    vault_subscribe_request,
                ),
            ])
        }
    }
}

#[derive(Default)]
pub struct RaydiumAmmSnapshotFetcher;

#[async_trait::async_trait]
impl AccountSnapshotFetcher for RaydiumAmmSnapshotFetcher {
    async fn fetch_snapshot(
        &self,
        pool_ids: Vec<Pubkey>,
        rpc_client: Arc<RpcClient>,
    ) -> Option<Vec<Pool>> {
        let mut join_set = JoinSet::new();
        for chunks_pool_ids in pool_ids.chunks(100) {
            let chunks_pool_ids = chunks_pool_ids.to_vec();
            let rpc_client = rpc_client.clone();
            join_set.spawn(async move {
                let mut pools = Vec::with_capacity(chunks_pool_ids.len());
                for (pool_id, pool_account) in chunks_pool_ids.iter().zip(
                    rpc_client
                        .get_multiple_accounts_with_commitment(
                            &chunks_pool_ids,
                            CommitmentConfig::finalized(),
                        )
                        .await
                        .unwrap()
                        .value,
                ) {
                    if let Some(pool_account) = pool_account {
                        let amm_info =
                            AmmInfo::load_from_bytes(pool_account.data.as_slice()).unwrap();
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
                        pools.push(Pool {
                            protocol: Protocol::RaydiumAMM,
                            pool_id: *pool_id,
                            tokens: vec![
                                Mint {
                                    mint: amm_info.coin_vault_mint,
                                },
                                Mint {
                                    mint: amm_info.pc_vault_mint,
                                },
                            ],
                            state: PoolState::RaydiumAMM {
                                mint_0_vault: Some(amm_info.coin_vault),
                                mint_1_vault: Some(amm_info.pc_vault),
                                mint_0_vault_amount: Some(
                                    mint_vault_amount.get(0).unwrap().clone(),
                                ),
                                mint_1_vault_amount: Some(
                                    mint_vault_amount.get(1).unwrap().clone(),
                                ),
                                mint_0_need_take_pnl: Some(amm_info.state_data.need_take_pnl_coin),
                                mint_1_need_take_pnl: Some(amm_info.state_data.need_take_pnl_pc),
                                swap_fee_numerator: amm_info.fees.swap_fee_numerator,
                                swap_fee_denominator: amm_info.fees.swap_fee_denominator,
                            },
                        })
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

pub struct RaydiumAmmCacheUpdater {
    mint_0_vault_amount: Option<u64>,
    mint_1_vault_amount: Option<u64>,
    mint_0_need_take_pnl: Option<u64>,
    mint_1_need_take_pnl: Option<u64>,
}

impl RaydiumAmmCacheUpdater {
    pub fn new(grpc_message: GrpcMessage) -> Result<Self> {
        if let RaydiumAMMData {
            mint_0_vault_amount,
            mint_1_vault_amount,
            mint_0_need_take_pnl,
            mint_1_need_take_pnl,
            ..
        } = grpc_message
        {
            Ok(Self {
                mint_0_vault_amount,
                mint_1_vault_amount,
                mint_0_need_take_pnl,
                mint_1_need_take_pnl,
            })
        } else {
            Err(anyhow!("生成CachePoolUpdater失败：传入的参数类型不支持"))
        }
    }
}

impl CacheUpdater for RaydiumAmmCacheUpdater {
    fn update_cache(&self, pool: &mut Pool) -> Result<()> {
        if let PoolState::RaydiumAMM {
            ref mut mint_0_vault_amount,
            ref mut mint_1_vault_amount,
            ref mut mint_0_need_take_pnl,
            ref mut mint_1_need_take_pnl,
            ..
        } = pool.state
        {
            if change_option_ignore_none_old(mint_0_vault_amount, self.mint_0_vault_amount)
                || change_option_ignore_none_old(mint_1_vault_amount, self.mint_1_vault_amount)
                || change_option_ignore_none_old(mint_0_need_take_pnl, self.mint_0_need_take_pnl)
                || change_option_ignore_none_old(mint_1_need_take_pnl, self.mint_1_need_take_pnl)
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
