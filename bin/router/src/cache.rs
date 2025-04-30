use crate::dex::meteora_dlmm::meteora_dlmm_pool_extra::MeteoraDLMMPoolExtra;
use crate::dex::raydium_clmm::sdk::tick_array::TickArrayState;
use crate::dex::raydium_clmm::sdk::tickarray_bitmap_extension::TickArrayBitmapExtension;
use crate::interface::Protocol;
use dashmap::DashMap;
use solana_program::clock::Clock;
use solana_program::pubkey::Pubkey;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct PoolCache {
    pub edges: Arc<HashMap<Pubkey, Vec<(Pubkey, Pubkey)>>>,
    pub pool_map: Arc<DashMap<Pubkey, Pool>>,
    pub clock: Clock,
}

impl PoolCache {
    pub fn new(
        edges: HashMap<Pubkey, Vec<(Pubkey, Pubkey)>>,
        pool_map: DashMap<Pubkey, Pool>,
        clock: Clock,
    ) -> Self {
        Self {
            edges: Arc::new(edges),
            pool_map: Arc::new(pool_map),
            clock,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PoolState {
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
        zero_to_one_tick_array_states: VecDeque<TickArrayState>,
        one_to_zero_tick_array_states: VecDeque<TickArrayState>,
    },
    PumpFunAMM {
        mint_0_vault: Pubkey,
        mint_1_vault: Pubkey,
        mint_0_vault_amount: u64,
        mint_1_vault_amount: u64,
        lp_fee_basis_points: u64,
        protocol_fee_basis_points: u64,
    },
    MeteoraDLMM(MeteoraDLMMPoolExtra),
}

#[derive(Debug, Clone)]
pub struct Pool {
    pub protocol: Protocol,
    pub pool_id: Pubkey,
    pub tokens: Vec<Mint>,
    pub state: PoolState,
}

impl Pool {
    pub fn mint_0(&self) -> Pubkey {
        self.tokens.first().unwrap().mint
    }

    pub fn mint_1(&self) -> Pubkey {
        self.tokens.last().unwrap().mint
    }

    pub fn token_pair(&self) -> (Pubkey, Pubkey) {
        (self.mint_0(), self.mint_1())
    }

    pub fn mint_vault_pair(&self) -> Option<(Pubkey, Pubkey)> {
        match self.state {
            PoolState::RaydiumAMM {
                mint_0_vault,
                mint_1_vault,
                ..
            } => Some((mint_0_vault.unwrap(), mint_1_vault.unwrap())),
            PoolState::RaydiumCLMM { .. } => None,
            PoolState::PumpFunAMM {
                mint_0_vault,
                mint_1_vault,
                ..
            } => Some((mint_0_vault, mint_1_vault)),
            PoolState::MeteoraDLMM(..) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Mint {
    pub mint: Pubkey,
    // pub decimals: u8,
}
