use crate::interface::DexType;
use solana_program::address_lookup_table::AddressLookupTableAccount;
use solana_program::pubkey::Pubkey;
use std::fmt::{Debug, Display, Formatter};

#[derive(Debug, Clone)]
pub struct RaydiumAMMPoolState {
    pub mint_0_vault: Option<Pubkey>,
    pub mint_1_vault: Option<Pubkey>,
    pub mint_0_vault_amount: Option<u64>,
    pub mint_1_vault_amount: Option<u64>,
    pub mint_0_need_take_pnl: Option<u64>,
    pub mint_1_need_take_pnl: Option<u64>,
    pub swap_fee_numerator: u64,
    pub swap_fee_denominator: u64,
}

impl RaydiumAMMPoolState {
    pub fn new(
        mint_0_vault: Option<Pubkey>,
        mint_1_vault: Option<Pubkey>,
        mint_0_vault_amount: Option<u64>,
        mint_1_vault_amount: Option<u64>,
        mint_0_need_take_pnl: Option<u64>,
        mint_1_need_take_pnl: Option<u64>,
        swap_fee_numerator: u64,
        swap_fee_denominator: u64,
    ) -> Self {
        Self {
            mint_0_vault,
            mint_1_vault,
            mint_0_vault_amount,
            mint_1_vault_amount,
            mint_0_need_take_pnl,
            mint_1_need_take_pnl,
            swap_fee_numerator,
            swap_fee_denominator,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RaydiumAMMInstructionItem {
    pub pool_id: Pubkey,
    pub mint_0: Pubkey,
    pub mint_1: Pubkey,
    pub mint_0_vault: Pubkey,
    pub mint_1_vault: Pubkey,
    pub alt: AddressLookupTableAccount,
    pub zero_to_one: bool,
}

impl Display for RaydiumAMMInstructionItem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}: {},{:?}",
            DexType::RaydiumAMM,
            self.pool_id,
            self.zero_to_one
        )
    }
}
