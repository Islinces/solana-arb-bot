use crate::cache::Pool;
use crate::defi::raydium_amm::raydium_amm::{
    RaydiumAmmDex, RaydiumAmmGrpcMessageOperator, RaydiumAmmSnapshotFetcher,
    RaydiumAmmSubscribeRequestCreator,
};
use crate::defi::raydium_clmm::raydium_clmm::{
    RaydiumClmmDex, RaydiumClmmGrpcMessageOperator, RaydiumClmmSnapshotFetcher,
    RaydiumClmmSubscribeRequestCreator,
};
use crate::interface::SourceMessage::{Account, NONE};
use anyhow::Result;
use serde::Deserialize;
use solana_client::nonblocking::rpc_client::RpcClient;
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

impl Protocol {
    pub fn name(&self) -> String {
        match self {
            Protocol::RaydiumAMM => "RaydiumAMM".to_string(),
            Protocol::RaydiumCLmm => "RaydiumCLmm".to_string(),
            Protocol::PumpFunAMM => "PumpFunAMM".to_string(),
            Protocol::MeteoraDLMM => "MeteoraDLMM".to_string(),
        }
    }

    pub fn get_subscribe_request_generator(
        &self,
    ) -> Option<Box<dyn GrpcSubscribeRequestGenerator>> {
        match self {
            Protocol::RaydiumAMM => Some(Box::new(RaydiumAmmSubscribeRequestCreator::default())),
            Protocol::RaydiumCLmm => Some(Box::new(RaydiumClmmSubscribeRequestCreator::default())),
            Protocol::PumpFunAMM => None,
            Protocol::MeteoraDLMM => None,
        }
    }

    pub fn get_grpc_message_operator(
        &self,
        account_update: AccountUpdate,
    ) -> Option<Box<dyn ReadyGrpcMessageOperator>> {
        match self {
            Protocol::RaydiumAMM => {
                Some(Box::new(RaydiumAmmGrpcMessageOperator::new(account_update)))
            }
            Protocol::RaydiumCLmm => Some(Box::new(RaydiumClmmGrpcMessageOperator::new(
                account_update,
            ))),
            Protocol::PumpFunAMM => None,
            Protocol::MeteoraDLMM => None,
        }
    }

    pub fn get_snapshot_fetcher(&self) -> Option<Box<dyn AccountSnapshotFetcher>> {
        match self {
            Protocol::RaydiumAMM => Some(Box::new(RaydiumAmmSnapshotFetcher::default())),
            Protocol::RaydiumCLmm => Some(Box::new(RaydiumClmmSnapshotFetcher::default())),
            Protocol::PumpFunAMM => None,
            Protocol::MeteoraDLMM => None,
        }
    }

    pub fn use_cache(&self) -> bool {
        match self {
            Protocol::RaydiumAMM => true,
            Protocol::RaydiumCLmm => false,
            Protocol::PumpFunAMM => false,
            Protocol::MeteoraDLMM => false,
        }
    }

    pub fn create_dex(&self, amount_in_mint: Pubkey, pool: Pool) -> Option<Box<dyn Dex>> {
        match self {
            Protocol::RaydiumAMM => {
                if let Some(dex) = RaydiumAmmDex::new(pool, amount_in_mint) {
                    Some(Box::new(dex))
                } else {
                    None
                }
            }
            Protocol::RaydiumCLmm => {
                if let Some(dex) = RaydiumClmmDex::new(pool, amount_in_mint) {
                    Some(Box::new(dex))
                } else {
                    None
                }
            }
            Protocol::PumpFunAMM => None,
            Protocol::MeteoraDLMM => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum GrpcMessage {
    RaydiumAmmData {
        pool_id: Pubkey,
        mint_0_vault_amount: Option<u64>,
        mint_1_vault_amount: Option<u64>,
        mint_0_need_take_pnl: Option<u64>,
        mint_1_need_take_pnl: Option<u64>,
    },
    RaydiumClmmData {
        pool_id: Pubkey,
        tick_current: i32,
        liquidity: u128,
        sqrt_price_x64: u128,
        tick_array_bitmap: [u64; 16],
    },
}

#[derive(Debug, Clone)]
pub enum SourceMessage {
    Account(AccountUpdate),
    NONE,
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
        match value.0 {
            Protocol::RaydiumAMM => Account(AccountUpdate {
                protocol: value.0,
                account_type: value.1,
                filters: value.2,
                account: value.3,
            }),
            Protocol::RaydiumCLmm => NONE,
            Protocol::PumpFunAMM => NONE,
            Protocol::MeteoraDLMM => NONE,
        }
    }
}

#[derive(Eq, PartialEq, Hash, Debug, Clone)]
pub enum GrpcAccountUpdateType {
    PoolState,
    MintVault,
    NONE,
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
    async fn fetch_snapshot(&self, rpc_client: Arc<RpcClient>) -> Option<Vec<Pool>>;
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
