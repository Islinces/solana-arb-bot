use crate::cache::{Mint, Pool, PoolState};
use crate::dex::common::mint_vault::MintVaultSubscribe;
use crate::dex::common::utils::{
    change_data_if_not_same, change_option_ignore_none_old, deserialize_anchor_account,
    deserialize_anchor_bytes, SwapDirection,
};
use crate::dex::pump_fun::math::CheckedCeilDiv;
use crate::dex::pump_fun::state::GlobalConfig;
use crate::dex::pump_fun::state::Pool as PumpFunPoolState;
use crate::interface::GrpcMessage::{PumpFunAMMData, RaydiumAMMData};
use crate::interface::{
    AccountSnapshotFetcher, AccountUpdate, CacheUpdater, Dex, DexType, GrpcAccountUpdateType,
    GrpcMessage, GrpcSubscribeRequestGenerator, ReadyGrpcMessageOperator, SubscribeKey,
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
use std::fmt::{Debug, Formatter};
use std::ops::{Add, Div, Mul, Sub};
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{error, warn};
use yellowstone_grpc_proto::geyser::SubscribeRequest;

#[derive(Clone)]
pub struct PumpFunDex {
    pool_id: Pubkey,
    mint_0: Pubkey,
    mint_1: Pubkey,
    swap_direction: SwapDirection,
    mint_0_vault_amount: u64,
    mint_1_vault_amount: u64,
    lp_fee_basis_points: u64,
    protocol_fee_basis_points: u64,
}

impl PumpFunDex {
    pub fn new(pool: Pool, amount_in_mint: Pubkey) -> Option<Self> {
        if let PoolState::PumpFunAMM(ref pool_state) = pool.state {
            let mint_0 = pool.mint_0();
            let mint_1 = pool.mint_1();
            let pool_id = pool.pool_id;
            if mint_0 != amount_in_mint && mint_1 != amount_in_mint {
                return None;
            }
            Some(Self {
                pool_id,
                mint_0,
                mint_1,
                swap_direction: if amount_in_mint == mint_0 {
                    SwapDirection::Coin2PC
                } else {
                    SwapDirection::PC2Coin
                },
                mint_0_vault_amount: pool_state.mint_0_vault_amount,
                mint_1_vault_amount: pool_state.mint_1_vault_amount,
                lp_fee_basis_points: pool_state.lp_fee_basis_points,
                protocol_fee_basis_points: pool_state.protocol_fee_basis_points,
            })
        } else {
            None
        }
    }
}

impl Debug for PumpFunDex {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "PumpFunDex: {},{:?}", self.pool_id, self.swap_direction)
    }
}

#[async_trait::async_trait]
impl Dex for PumpFunDex {
    async fn quote(&self, amount_in: u64) -> Option<u64> {
        if amount_in == u64::MIN {
            return None;
        }
        let base_vault = u128::from(self.mint_0_vault_amount);
        let quote_vault = u128::from(self.mint_1_vault_amount);
        let amount_in = u128::from(amount_in);
        let lp_fee = amount_in
            .mul(u128::from(self.lp_fee_basis_points))
            .checked_ceil_div(10_000)
            .unwrap()
            .0;
        let protocol_fee = amount_in
            .mul(u128::from(self.protocol_fee_basis_points))
            .checked_ceil_div(10_000)
            .unwrap()
            .0;
        let total_fee = lp_fee.add(protocol_fee);
        let effective_amount = amount_in.sub(total_fee);
        let amount_out = match self.swap_direction {
            SwapDirection::Coin2PC => quote_vault
                .mul(effective_amount)
                .div(base_vault.add(effective_amount)),
            SwapDirection::PC2Coin => base_vault
                .mul(effective_amount)
                .div(quote_vault.add(effective_amount)),
        };
        Some(amount_out.try_into().unwrap_or_else(|_| {
            error!("amount_out is too large");
            u64::MIN
        }))
    }

    fn clone_self(&self) -> Box<dyn Dex> {
        Box::new(self.clone())
    }
}

pub struct PumpFunGrpcSubscribeRequestGenerator;

impl GrpcSubscribeRequestGenerator for PumpFunGrpcSubscribeRequestGenerator {
    fn create_subscribe_requests(
        &self,
        pools: &[Pool],
    ) -> Option<Vec<(SubscribeKey, SubscribeRequest)>> {
        let vault_subscribe_request = MintVaultSubscribe::mint_vault_subscribe_request(pools);
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

pub struct PumpFunReadyGrpcMessageOperator {
    update_account: AccountUpdate,
    txn: Option<String>,
    pool_id: Option<Pubkey>,
    grpc_message: Option<GrpcMessage>,
}

impl PumpFunReadyGrpcMessageOperator {
    pub fn new(update_account: AccountUpdate) -> Self {
        Self {
            update_account,
            txn: None,
            pool_id: None,
            grpc_message: None,
        }
    }
}

impl ReadyGrpcMessageOperator for PumpFunReadyGrpcMessageOperator {
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
            PumpFunAMMData {
                mint_0_vault_amount,
                mint_1_vault_amount,
                ..
            } => {
                if let PumpFunAMMData {
                    mint_0_vault_amount: update_mint_0_vault_amount,
                    mint_1_vault_amount: update_mint_1_vault_amount,
                    ..
                } = self.grpc_message.as_ref().unwrap().clone()
                {
                    change_option_ignore_none_old(mint_0_vault_amount, update_mint_0_vault_amount);
                    change_option_ignore_none_old(mint_1_vault_amount, update_mint_1_vault_amount);
                    if mint_0_vault_amount.is_some() && mint_1_vault_amount.is_some() {
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
        (self.txn.clone().unwrap(), self.pool_id.unwrap())
    }

    fn get_insert_data(&self) -> GrpcMessage {
        self.grpc_message.as_ref().unwrap().clone()
    }
}

#[derive(Default)]
pub struct PumpFunAccountSnapshotFetcher;

#[async_trait::async_trait]
impl AccountSnapshotFetcher for PumpFunAccountSnapshotFetcher {
    async fn fetch_snapshot(
        &self,
        pool_ids: Vec<Pubkey>,
        rpc_client: Arc<RpcClient>,
    ) -> Option<Vec<Pool>> {
        let global_config = deserialize_anchor_bytes::<GlobalConfig>(
            rpc_client
                .get_account_data(&GlobalConfig::key())
                .await
                .unwrap()
                .as_slice(),
        )
        .unwrap();
        let mut join_set = JoinSet::new();
        for chunks_pool_id in pool_ids.chunks(100) {
            let chunks_pool_id = chunks_pool_id.to_vec();
            let rpc_client = rpc_client.clone();
            join_set.spawn(async move {
                let mut pools = Vec::with_capacity(chunks_pool_id.len());
                for (pool_id, pool_account) in chunks_pool_id.iter().zip(
                    rpc_client
                        .get_multiple_accounts_with_commitment(
                            &chunks_pool_id,
                            CommitmentConfig::finalized(),
                        )
                        .await
                        .unwrap()
                        .value,
                ) {
                    if let Some(account) = pool_account {
                        let pool_state =
                            deserialize_anchor_account::<PumpFunPoolState>(&account).unwrap();
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
                            state: PoolState::PumpFunAMM(
                                crate::dex::pump_fun::pool_state::PumpFunPoolState::new(
                                    pool_state.pool_base_token_account,
                                    pool_state.pool_quote_token_account,
                                    vault_accounts.first().unwrap().amount,
                                    vault_accounts.last().unwrap().amount,
                                    global_config.lp_fee_basis_points,
                                    global_config.protocol_fee_basis_points,
                                ),
                            ),
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

pub struct PumpFunCacheUpdater {
    mint_0_vault_amount: Option<u64>,
    mint_1_vault_amount: Option<u64>,
}

impl PumpFunCacheUpdater {
    pub fn new(grpc_message: GrpcMessage) -> Result<Self> {
        if let PumpFunAMMData {
            mint_0_vault_amount,
            mint_1_vault_amount,
            ..
        } = grpc_message
        {
            Ok(Self {
                mint_0_vault_amount,
                mint_1_vault_amount,
            })
        } else {
            Err(anyhow!("生成CachePoolUpdater失败：传入的参数类型不支持"))
        }
    }
}

impl CacheUpdater for PumpFunCacheUpdater {
    fn update_cache(&self, pool: &mut Pool) -> Result<()> {
        if let PoolState::PumpFunAMM(ref mut pool_state) = pool.state {
            if change_data_if_not_same(
                &mut pool_state.mint_0_vault_amount,
                self.mint_0_vault_amount.unwrap(),
            ) || change_data_if_not_same(
                &mut pool_state.mint_1_vault_amount,
                self.mint_1_vault_amount.unwrap(),
            ) {
                Ok(())
            } else {
                Err(anyhow!(""))
            }
        } else {
            Err(anyhow!(""))
        }
    }
}
