use crate::defi::common::utils::{change_data_if_not_same, change_option_ignore_none_old};
use crate::defi::raydium_clmm::sdk::tick_array::TickArrayState;
use crate::defi::raydium_clmm::sdk::tickarray_bitmap_extension::TickArrayBitmapExtension;
use crate::interface::{GrpcMessage, Protocol};
use anyhow::anyhow;
use dashmap::DashMap;
use solana_program::pubkey::Pubkey;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct PoolCache {
    pub edges: Arc<HashMap<Pubkey, Vec<(Pubkey, Pubkey)>>>,
    pub pool_map: Arc<DashMap<Pubkey, Pool>>,
}

impl PoolCache {
    pub fn new(
        edges: HashMap<Pubkey, Vec<(Pubkey, Pubkey)>>,
        pool_map: DashMap<Pubkey, Pool>,
    ) -> Self {
        Self {
            edges: Arc::new(edges),
            pool_map: Arc::new(pool_map),
        }
    }
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
                    if change_option_ignore_none_old(
                        mint_0_vault_amount,
                        change_mint_0_vault_amount,
                    ) || change_option_ignore_none_old(
                        mint_1_vault_amount,
                        change_mint_1_vault_amount,
                    ) || change_option_ignore_none_old(
                        mint_0_need_take_pnl,
                        change_mint_0_need_take_pnl,
                    ) || change_option_ignore_none_old(
                        mint_1_need_take_pnl,
                        change_mint_1_need_take_pnl,
                    ) {
                        Ok(())
                    } else {
                        Err(anyhow!(""))
                    }
                } else {
                    Err(anyhow!(""))
                }
            }
            PoolExtra::PumpFun { .. } => Err(anyhow!("")),
            PoolExtra::RaydiumCLMM {
                liquidity,
                sqrt_price_x64,
                tick_current,
                tick_array_bitmap,
                ..
            } => {
                if let GrpcMessage::RaydiumClmmData {
                    pool_id: _pool_id,
                    tick_current: update_tick_current,
                    liquidity: update_liquidity,
                    sqrt_price_x64: update_sqrt_price_x64,
                    tick_array_bitmap: update_tick_array_bitmap,
                } = change
                {
                    if change_data_if_not_same(liquidity, update_liquidity)
                        || change_data_if_not_same(sqrt_price_x64, update_sqrt_price_x64)
                        || change_data_if_not_same(tick_current, update_tick_current)
                        || change_data_if_not_same(tick_array_bitmap, update_tick_array_bitmap)
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
