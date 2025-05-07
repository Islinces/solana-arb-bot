use crate::interface::DexType;
use solana_program::pubkey::Pubkey;
use std::fmt::{Debug, Display, Formatter};
use serde::{Deserialize, Serialize};
use solana_program::address_lookup_table::AddressLookupTableAccount;

#[derive(Debug, Clone)]
pub struct PumpFunPoolState {
    pub mint_0_vault: Pubkey,
    pub mint_1_vault: Pubkey,
    pub mint_0_vault_amount: u64,
    pub mint_1_vault_amount: u64,
    pub lp_fee_basis_points: u64,
    pub protocol_fee_basis_points: u64,
}

impl PumpFunPoolState {
    pub fn new(
        mint_0_vault: Pubkey,
        mint_1_vault: Pubkey,
        mint_0_vault_amount: u64,
        mint_1_vault_amount: u64,
        lp_fee_basis_points: u64,
        protocol_fee_basis_points: u64,
    ) -> Self {
        Self {
            mint_0_vault,
            mint_1_vault,
            mint_0_vault_amount,
            mint_1_vault_amount,
            lp_fee_basis_points,
            protocol_fee_basis_points,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PumpFunInstructionItem {
    pub pool_id: Pubkey,
    pub mint_0: Pubkey,
    pub mint_1: Pubkey,
    pub mint_0_vault: Pubkey,
    pub mint_1_vault: Pubkey,
    pub alt: AddressLookupTableAccount,
    pub zero_to_one: bool,
}

impl Display for PumpFunInstructionItem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}: {},{:?}",
            DexType::PumpFunAMM,
            self.pool_id,
            self.zero_to_one
        )
    }
}
