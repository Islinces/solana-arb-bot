use crate::defi::common::utils::change_option_ignore_none_old;
use crate::defi::raydium_clmm::sdk::tick_array::TickArrayState;
use crate::defi::raydium_clmm::sdk::tickarray_bitmap_extension::TickArrayBitmapExtension;
use crate::defi::types::SourceMessage::{Account, NONE};
use crate::strategy::grpc_message_processor::GrpcMessage;
use anyhow::anyhow;
use dashmap::{DashMap, Entry};
use futures_util::future::err;
use serde::{Deserialize, Deserializer};
use solana_program::pubkey::Pubkey;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::{Display, Formatter, Write};
use std::str::FromStr;
use std::sync::Arc;
use tracing::info;
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestAccountsDataSlice,
    SubscribeRequestFilterAccounts, SubscribeUpdateAccount,
};

pub type TokenPools = DashMap<Pubkey, HashSet<Pubkey>>;
pub type TokenPairPools = DashMap<(Pubkey, Pubkey), HashSet<Pubkey>>;

#[derive(Debug, Clone, Default)]
pub struct PoolCache {
    // pub token_pools: Arc<TokenPools>,
    // pub token01_pools: Arc<TokenPairPools>,
    pub edges: Arc<HashMap<Pubkey, Vec<(Pubkey, Pubkey)>>>,
    pub pool_map: Arc<DashMap<Pubkey, Pool>>,
}

impl PoolCache {
    pub fn new(
        // token_pools: TokenPools,
        // token01_pools: TokenPairPools,
        edges: HashMap<Pubkey, Vec<(Pubkey, Pubkey)>>,
        pool_map: DashMap<Pubkey, Pool>,
    ) -> Self {
        Self {
            edges: Arc::new(edges),
            pool_map: Arc::new(pool_map),
        }
    }

    pub fn update_cache(&self, grpc_message: GrpcMessage) -> Option<Pool> {
        let pool_id = match grpc_message {
            GrpcMessage::RaydiumAmmData { pool_id, .. } => pool_id,
            GrpcMessage::RaydiumClmmData { pool_id, .. } => pool_id,
        };
        let arc = &self.pool_map;
        let successful = match arc.clone().entry(pool_id) {
            Entry::Occupied(ref mut exists) => {
                let pool = exists.get_mut();
                if pool.extra.try_change(grpc_message).is_ok() {
                    Ok(pool.clone())
                } else {
                    Err(anyhow!(""))
                }
            }
            Entry::Vacant(_) => Err(anyhow!("")),
        };
        if successful.is_ok() {
            info!("更新缓存成功：{:#?}", pool_id);
            Some(successful.unwrap())
        } else {
            None
        }
    }
}

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

    pub fn index(&self) -> usize {
        match self {
            Protocol::RaydiumAMM => 0,
            Protocol::RaydiumCLmm => 1,
            Protocol::PumpFunAMM => 2,
            Protocol::MeteoraDLMM => 3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Pool {
    pub protocol: Protocol,
    pub pool_id: Pubkey,
    pub tokens: Vec<Mint>,
    pub extra: PoolExtra,
}

impl Pool {
    pub fn mint_0(&self) -> Pubkey {
        self.tokens.first().unwrap().mint.clone()
    }

    pub fn mint_1(&self) -> Pubkey {
        self.tokens.last().unwrap().mint.clone()
    }

    pub fn token_pair(&self) -> (Pubkey, Pubkey) {
        (self.mint_0(), self.mint_1())
    }

    pub fn mint_vault_pair(&self) -> Option<(Pubkey, Pubkey)> {
        match self.extra {
            PoolExtra::None => None,
            PoolExtra::RaydiumAMM {
                mint_0_vault,
                mint_1_vault,
                ..
            } => Some((mint_0_vault.unwrap(), mint_1_vault.unwrap())),
            PoolExtra::RaydiumCLMM { .. } => None,
            PoolExtra::PumpFun { .. } => None,
        }
    }

    // pub fn get_mut_extra(&mut self) -> &mut PoolExtra {
    //     let extra = &mut self.extra;
    //     RefMut::
    // }
}

#[derive(Debug, Clone)]
pub struct Mint {
    pub mint: Pubkey,
    pub decimals: u8,
}

#[derive(Debug, Clone)]
pub enum PoolExtra {
    None,
    RaydiumAMM {
        mint_0_vault: Option<Pubkey>,
        mint_1_vault: Option<Pubkey>,
        mint_0_vault_amount: Option<u64>,
        mint_1_vault_amount: Option<u64>,
        mint_0_need_take_pnl: Option<u64>,
        mint_1_need_take_pnl: Option<u64>,
        swap_fee_numerator: u64,
        swap_fee_denominator: u64,
    },
    RaydiumCLMM {
        tick_spacing: u16,
        trade_fee_rate: u32,
        liquidity: u128,
        sqrt_price_x64: u128,
        tick_current: i32,
        tick_array_bitmap: [u64; 16],
        tick_array_bitmap_extension: TickArrayBitmapExtension,
        tick_array_states: VecDeque<TickArrayState>,
    },
    PumpFun {
        mint_0_vault: Pubkey,
        mint_1_vault: Pubkey,
        lp_fee_basis_points: u64,
        protocol_fee_basis_points: u64,
    },
}
impl PoolExtra {
    pub fn try_change(&mut self, change: GrpcMessage) -> anyhow::Result<()> {
        match self {
            PoolExtra::None => Err(anyhow!("")),
            PoolExtra::RaydiumAMM {
                mint_0_vault_amount,
                mint_1_vault_amount,
                mint_0_need_take_pnl,
                mint_1_need_take_pnl,
                ..
            } => {
                if let GrpcMessage::RaydiumAmmData {
                    mint_0_vault_amount: change_mint_0_vault_amount,
                    mint_1_vault_amount: change_mint_1_vault_amount,
                    mint_0_need_take_pnl: change_mint_0_need_take_pnl,
                    mint_1_need_take_pnl: change_mint_1_need_take_pnl,
                    ..
                } = change
                {
                    let mut has_change = change_option_ignore_none_old(
                        mint_0_vault_amount,
                        change_mint_0_vault_amount,
                    );
                    has_change |= change_option_ignore_none_old(
                        mint_1_vault_amount,
                        change_mint_1_vault_amount,
                    );
                    has_change |= change_option_ignore_none_old(
                        mint_0_need_take_pnl,
                        change_mint_0_need_take_pnl,
                    );
                    has_change |= change_option_ignore_none_old(
                        mint_1_need_take_pnl,
                        change_mint_1_need_take_pnl,
                    );
                    if has_change {
                        Ok(())
                    } else {
                        Err(anyhow!(""))
                    }
                } else {
                    Err(anyhow!(""))
                }
            }
            PoolExtra::PumpFun { .. } => Err(anyhow!("")),
            PoolExtra::RaydiumCLMM { .. } => Err(anyhow!("")),
        }
    }
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

impl GrpcAccountUpdateType {
    pub fn filter_name() -> String {
        "".to_string()
    }
}
