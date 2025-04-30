use crate::cache::Pool;
use crate::dex::meteora_dlmm::meteora_dlmm::{
    MeteoraDLMMCacheUpdater, MeteoraDLMMGrpcMessageOperator,
    MeteoraDLMMGrpcSubscribeRequestGenerator, MeteoraDLMMSnapshotFetcher, MeteoraDlmmDex,
};
use crate::dex::pump_fun::pump_fun::{
    PumpFunAccountSnapshotFetcher, PumpFunCacheUpdater, PumpFunDex,
    PumpFunGrpcSubscribeRequestGenerator, PumpFunReadyGrpcMessageOperator,
};
use crate::dex::raydium_amm::raydium_amm::{
    RaydiumAmmCacheUpdater, RaydiumAmmDex, RaydiumAmmGrpcMessageOperator,
    RaydiumAmmSnapshotFetcher, RaydiumAmmSubscribeRequestCreator,
};
use crate::dex::raydium_clmm::raydium_clmm::{
    RaydiumClmmCacheUpdater, RaydiumClmmDex, RaydiumClmmGrpcMessageOperator,
    RaydiumClmmSnapshotFetcher, RaydiumClmmSubscribeRequestCreator,
};
use crate::interface::SourceMessage::Account;
use anyhow::{anyhow, Result};
use serde::Deserialize;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::clock::Clock;
use solana_program::pubkey::Pubkey;
use std::fmt::{Debug, Display, Formatter};
use std::sync::Arc;
use yellowstone_grpc_proto::geyser::{SubscribeRequest, SubscribeUpdateAccount};

pub type SubscribeKey = (Protocol, GrpcAccountUpdateType);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub enum Protocol {
    RaydiumAMM,
    RaydiumCLmm,
    PumpFunAMM,
    MeteoraDLMM,
}

impl Display for Protocol {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Protocol::RaydiumAMM => "RaydiumAMM",
            Protocol::RaydiumCLmm => "RaydiumCLmm",
            Protocol::PumpFunAMM => "PumpFunAM",
            Protocol::MeteoraDLMM => "MeteoraDLMM",
        })
    }
}

impl From<Pubkey> for Protocol {
    fn from(value: Pubkey) -> Self {
        if value == crate::dex::raydium_amm::ID {
            Protocol::RaydiumAMM
        } else if value == crate::dex::raydium_clmm::ID {
            Protocol::RaydiumCLmm
        } else if value == crate::dex::pump_fun::ID {
            Protocol::PumpFunAMM
        } else if value == crate::dex::meteora_dlmm::ID {
            Protocol::MeteoraDLMM
        } else {
            unreachable!()
        }
    }
}

impl Protocol {
    pub fn get_owner(&self) -> Pubkey {
        match self {
            Protocol::RaydiumAMM => crate::dex::raydium_amm::ID,
            Protocol::RaydiumCLmm => crate::dex::raydium_clmm::ID,
            Protocol::PumpFunAMM => crate::dex::pump_fun::ID,
            Protocol::MeteoraDLMM => crate::dex::meteora_dlmm::ID,
        }
    }

    pub fn get_subscribe_request_generator(
        &self,
    ) -> Result<Box<dyn GrpcSubscribeRequestGenerator>> {
        match self {
            Protocol::RaydiumAMM => Ok(Box::new(RaydiumAmmSubscribeRequestCreator)),
            Protocol::RaydiumCLmm => Ok(Box::new(RaydiumClmmSubscribeRequestCreator)),
            Protocol::PumpFunAMM => Ok(Box::new(PumpFunGrpcSubscribeRequestGenerator)),
            Protocol::MeteoraDLMM => Ok(Box::new(MeteoraDLMMGrpcSubscribeRequestGenerator)),
        }
    }

    pub fn get_grpc_message_operator(
        &self,
        account_update: AccountUpdate,
    ) -> Result<Box<dyn ReadyGrpcMessageOperator>> {
        match self {
            Protocol::RaydiumAMM => {
                Ok(Box::new(RaydiumAmmGrpcMessageOperator::new(account_update)))
            }
            Protocol::RaydiumCLmm => Ok(Box::new(RaydiumClmmGrpcMessageOperator::new(
                account_update,
            ))),
            Protocol::PumpFunAMM => Ok(Box::new(PumpFunReadyGrpcMessageOperator::new(
                account_update,
            ))),
            Protocol::MeteoraDLMM => Ok(Box::new(MeteoraDLMMGrpcMessageOperator::new(
                account_update,
            ))),
        }
    }

    pub fn get_snapshot_fetcher(&self) -> Result<Box<dyn AccountSnapshotFetcher>> {
        match self {
            Protocol::RaydiumAMM => Ok(Box::new(RaydiumAmmSnapshotFetcher)),
            Protocol::RaydiumCLmm => Ok(Box::new(RaydiumClmmSnapshotFetcher)),
            Protocol::PumpFunAMM => Ok(Box::new(PumpFunAccountSnapshotFetcher)),
            Protocol::MeteoraDLMM => Ok(Box::new(MeteoraDLMMSnapshotFetcher)),
        }
    }

    pub fn get_cache_updater(&self, grpc_message: GrpcMessage) -> Result<Box<dyn CacheUpdater>> {
        match self {
            Protocol::RaydiumAMM => Ok(Box::new(RaydiumAmmCacheUpdater::new(grpc_message)?)),
            Protocol::RaydiumCLmm => Ok(Box::new(RaydiumClmmCacheUpdater::new(grpc_message)?)),
            Protocol::PumpFunAMM => Ok(Box::new(PumpFunCacheUpdater::new(grpc_message)?)),
            Protocol::MeteoraDLMM => Ok(Box::new(MeteoraDLMMCacheUpdater::new(grpc_message)?)),
        }
    }

    pub fn use_cache(&self) -> bool {
        match self {
            Protocol::RaydiumAMM => true,
            Protocol::RaydiumCLmm => false,
            Protocol::PumpFunAMM => true,
            Protocol::MeteoraDLMM => false,
        }
    }

    pub async fn create_dex(
        &self,
        amount_in_mint: Pubkey,
        pool: Pool,
        clock: Clock,
    ) -> Option<Box<dyn Dex>> {
        match self {
            Protocol::RaydiumAMM => {
                Protocol::inner_create_dex(RaydiumAmmDex::new(pool, amount_in_mint))
            }
            Protocol::RaydiumCLmm => {
                Protocol::inner_create_dex(RaydiumClmmDex::new(pool, amount_in_mint))
            }
            Protocol::PumpFunAMM => {
                Protocol::inner_create_dex(PumpFunDex::new(pool, amount_in_mint))
            }
            Protocol::MeteoraDLMM => {
                Protocol::inner_create_dex(MeteoraDlmmDex::new(pool, amount_in_mint, clock))
            }
        }
    }

    fn inner_create_dex<T: Dex>(dex: Option<T>) -> Option<Box<dyn Dex>> {
        if let Some(dex) = dex {
            Some(dex.clone_self())
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub enum GrpcMessage {
    RaydiumAMMData {
        pool_id: Pubkey,
        mint_0_vault_amount: Option<u64>,
        mint_1_vault_amount: Option<u64>,
        mint_0_need_take_pnl: Option<u64>,
        mint_1_need_take_pnl: Option<u64>,
    },
    RaydiumCLMMData {
        pool_id: Pubkey,
        tick_current: i32,
        liquidity: u128,
        sqrt_price_x64: u128,
        tick_array_bitmap: [u64; 16],
    },
    PumpFunAMMData {
        pool_id: Pubkey,
        mint_0_vault_amount: Option<u64>,
        mint_1_vault_amount: Option<u64>,
    },
    MeteoraDLMMData {
        pool_id: Pubkey,
        active_id: i32,
        bin_array_bitmap: [u64; 16],
        volatility_accumulator: u32,
        volatility_reference: u32,
        index_reference: i32,
        last_update_timestamp: i64,
    },
    Clock(Clock),
}

impl GrpcMessage {
    pub fn pool_id(&self) -> Option<Pubkey> {
        match self {
            GrpcMessage::RaydiumAMMData { pool_id, .. } => Some(*pool_id),
            GrpcMessage::RaydiumCLMMData { pool_id, .. } => Some(*pool_id),
            GrpcMessage::PumpFunAMMData { pool_id, .. } => Some(*pool_id),
            GrpcMessage::MeteoraDLMMData { pool_id, .. } => Some(*pool_id),
            GrpcMessage::Clock(_) => unimplemented!(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum SourceMessage {
    Account(AccountUpdate),
    Clock(ClockUpdate),
}

#[derive(Debug, Clone)]
pub struct ClockUpdate {
    pub account: SubscribeUpdateAccount,
}

#[derive(Debug, Clone)]
pub struct AccountUpdate {
    pub protocol: Protocol,
    pub account_type: GrpcAccountUpdateType,
    pub filters: Vec<String>,
    pub account: SubscribeUpdateAccount,
}

impl
    From<(
        Protocol,
        GrpcAccountUpdateType,
        Vec<String>,
        SubscribeUpdateAccount,
    )> for SourceMessage
{
    fn from(
        value: (
            Protocol,
            GrpcAccountUpdateType,
            Vec<String>,
            SubscribeUpdateAccount,
        ),
    ) -> Self {
        Account(AccountUpdate {
            protocol: value.0,
            account_type: value.1,
            filters: value.2,
            account: value.3,
        })
    }
}

#[derive(Eq, PartialEq, Hash, Debug, Clone)]
pub enum GrpcAccountUpdateType {
    PoolState,
    MintVault,
    Clock,
}

pub trait ReadyGrpcMessageOperator {
    fn parse_message(&mut self) -> Result<()>;

    fn change_and_return_ready_data(&self, old: &mut GrpcMessage) -> Result<()>;

    fn get_cache_key(&self) -> (String, Pubkey);

    fn get_insert_data(&self) -> GrpcMessage;
}

pub trait GrpcSubscribeRequestGenerator {
    fn create_subscribe_requests(
        &self,
        pools: &[Pool],
    ) -> Option<Vec<(SubscribeKey, SubscribeRequest)>>;
}

#[async_trait::async_trait]
pub trait AccountSnapshotFetcher: Send + Sync {
    async fn fetch_snapshot(
        &self,
        pool_ids: Vec<Pubkey>,
        rpc_client: Arc<RpcClient>,
    ) -> Option<Vec<Pool>>;
}

pub trait CacheUpdater: Send + Sync {
    fn update_cache(&self, pool: &mut Pool) -> Result<()>;
}

#[async_trait::async_trait]
pub trait DB: Debug + Send + Sync {
    async fn load_token_pools(&self, protocols: &[Protocol]) -> anyhow::Result<Vec<Pool>>;
}

#[async_trait::async_trait]
pub trait Dex: Send + Sync + Debug {
    async fn quote(&self, amount_in: u64) -> Option<u64>;

    fn clone_self(&self) -> Box<dyn Dex>;
}
