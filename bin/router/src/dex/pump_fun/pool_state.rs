use crate::dex::common::utils::change_data_if_not_same;
use crate::interface::{DexType, GrpcMessage};
use anyhow::anyhow;
use std::fmt::{Debug, Display, Formatter};
use solana_sdk::address_lookup_table::AddressLookupTableAccount;
use solana_sdk::pubkey::Pubkey;

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

    pub fn try_update(&mut self, grpc_message: GrpcMessage) -> anyhow::Result<()> {
        match grpc_message {
            GrpcMessage::PumpFunAMMData {
                mint_0_vault_amount,
                mint_1_vault_amount,
                ..
            } => {
                let mut changed = change_data_if_not_same(
                    &mut self.mint_0_vault_amount,
                    mint_0_vault_amount.unwrap(),
                );
                changed |= change_data_if_not_same(
                    &mut self.mint_1_vault_amount,
                    mint_1_vault_amount.unwrap(),
                );
                if changed {
                    Ok(())
                } else {
                    Err(anyhow!(""))
                }
            }
            _ => Err(anyhow!("")),
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
