use crate::math::CheckedCeilDiv;
use dex::account_write::AccountWrite;
use dex::interface::Pool;
use solana_program::pubkey::Pubkey;
use std::ops::{Add, Div, Mul, Sub};

#[derive(Debug, Copy, Clone)]
pub struct PumpFunPool {
    pub pool_id: Pubkey,
    pub mint_0: Pubkey,
    pub mint_1: Pubkey,
    pub mint_0_vault: u64,
    pub mint_1_vault: u64,
    pub lp_fee_basis_points: u64,
    pub protocol_fee_basis_points: u64,
}

impl PumpFunPool {
    pub fn new(
        pool_id: Pubkey,
        mint_0: Pubkey,
        mint_1: Pubkey,
        mint_0_vault: u64,
        mint_1_vault: u64,
        lp_fee_basis_points: u64,
        protocol_fee_basis_points: u64,
    ) -> Self {
        PumpFunPool {
            pool_id: pool_id,
            mint_0: mint_0,
            mint_1: mint_1,
            mint_0_vault: mint_0_vault,
            mint_1_vault: mint_1_vault,
            lp_fee_basis_points: lp_fee_basis_points,
            protocol_fee_basis_points,
        }
    }
}

impl Pool for PumpFunPool {
    fn get_pool_id(&self) -> Pubkey {
        self.pool_id
    }

    fn get_mint_0(&self) -> Pubkey {
        self.mint_0
    }

    fn get_mint_1(&self) -> Pubkey {
        self.mint_1
    }

    fn quote(&self, amount_in: u64, amount_in_mint: Pubkey) -> Option<u64> {
        if amount_in_mint != self.mint_0 && amount_in_mint != self.mint_1 {
            return None;
        }
        let base_vault = u128::from(self.mint_0_vault);
        let quote_vault = u128::from(self.mint_1_vault);
        let amount_in = u128::from(amount_in);
        let lp_fee = amount_in
            .mul(u128::from(self.lp_fee_basis_points))
            .checked_ceil_div(10_000)
            .unwrap()
            .0;
        let protocol_fee = amount_in
            .mul(u128::from(self.protocol_fee_basis_points))
            .checked_ceil_div(10_000)
            .unwrap()
            .0;
        let total_fee = lp_fee.add(protocol_fee);
        let effective_amount = amount_in.sub(total_fee);
        let amount_out = if amount_in_mint == self.mint_0 {
            quote_vault
                .mul(effective_amount)
                .div(base_vault.add(effective_amount))
        } else {
            base_vault
                .mul(effective_amount)
                .div(quote_vault.add(effective_amount))
        };
        println!("total_fee: {}", total_fee);
        Some(amount_out.try_into().unwrap_or_else(|_| {
            eprintln!("amount_out is too large");
            u64::MIN
        }))
    }

    fn clone_box(&self) -> Box<dyn Pool> {
        Box::new(*self)
    }

    fn update_data(&self, account_write: AccountWrite) {
        todo!()
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum SwapDirection {
    /// Input token pc, output token coin
    PC2Coin = 1u64,
    /// Input token coin, output token pc
    Coin2PC = 2u64,
}
