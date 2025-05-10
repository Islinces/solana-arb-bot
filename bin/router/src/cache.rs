use crate::dex::meteora_dlmm::pool_state::MeteoraDLMMPoolState;
use crate::dex::pump_fun::pool_state::PumpFunPoolState;
use crate::dex::raydium_amm::pool_state::RaydiumAMMPoolState;
use crate::dex::raydium_clmm::pool_state::RaydiumCLMMPoolState;
use crate::interface::{DexType, GrpcMessage, InstructionItem};
use dashmap::DashMap;
use solana_program::address_lookup_table::AddressLookupTableAccount;
use solana_program::clock::Clock;
use solana_program::pubkey::Pubkey;
use std::collections::HashMap;
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
pub struct Mint {
    pub mint: Pubkey,
    // pub decimals: u8,
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
    pub alt: AddressLookupTableAccount,
}

impl Pool {
    pub fn mint_0(&self) -> Pubkey {
        self.tokens.first().unwrap().mint
    }

    pub fn mint_1(&self) -> Pubkey {
        self.tokens.last().unwrap().mint
    }

    pub fn another_mint(&self, mint: &Pubkey) -> Pubkey {
        let mint_0 = self.mint_0();
        if &mint_0 == mint {
            self.mint_1()
        } else {
            mint_0
        }
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

    pub fn update_cache(&mut self, grpc_message: GrpcMessage) -> anyhow::Result<()> {
        match &mut self.state {
            PoolState::RaydiumAMM(cache_data) => cache_data.try_update(grpc_message),
            PoolState::RaydiumCLMM(cache_data) => cache_data.try_update(grpc_message),
            PoolState::PumpFunAMM(cache_data) => cache_data.try_update(grpc_message),
            PoolState::MeteoraDLMM(cache_data) => cache_data.try_update(grpc_message),
        }
    }

    pub fn quote(
        &self,
        amount_in: u64,
        in_mint: Pubkey,
        out_mint: Pubkey,
        clock: Arc<Clock>,
    ) -> Option<u64> {
        self.protocol
            .get_quoter()
            .quote(amount_in, in_mint, out_mint, self, clock)
    }

    pub fn to_instruction_item(&self, in_mint: &Pubkey) -> Option<InstructionItem> {
        self.protocol
            .get_instruction_item_creator()
            .create_instruction_item(self, in_mint)
    }
}
