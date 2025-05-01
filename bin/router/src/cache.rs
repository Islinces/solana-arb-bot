use crate::dex::meteora_dlmm::pool_state::MeteoraDLMMPoolState;
use crate::dex::pump_fun::pool_state::PumpFunPoolState;
use crate::dex::raydium_amm::pool_state::RaydiumAMMPoolState;
use crate::dex::raydium_clmm::pool_state::RaydiumCLMMPoolState;
use crate::dex::raydium_clmm::sdk::tick_array::TickArrayState;
use crate::dex::raydium_clmm::sdk::tickarray_bitmap_extension::TickArrayBitmapExtension;
use crate::interface::DexType;
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
    RaydiumAMM(RaydiumAMMPoolState),
    RaydiumCLMM(RaydiumCLMMPoolState),
    PumpFunAMM(PumpFunPoolState),
    MeteoraDLMM(MeteoraDLMMPoolState),
}

#[derive(Debug, Clone)]
pub struct Pool {
    pub protocol: DexType,
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
            PoolState::RaydiumAMM(ref pool_state) => Some((
                pool_state.mint_0_vault.unwrap(),
                pool_state.mint_1_vault.unwrap(),
            )),
            PoolState::RaydiumCLMM { .. } => None,
            PoolState::PumpFunAMM(ref pool_state) => {
                Some((pool_state.mint_0_vault, pool_state.mint_1_vault))
            }
            PoolState::MeteoraDLMM(..) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Mint {
    pub mint: Pubkey,
    // pub decimals: u8,
}
