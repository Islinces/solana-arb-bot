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
        let mint_0_vault = u128::from(self.mint_0_vault);
        let mint_1_vault = u128::from(self.mint_1_vault);
        let amount_in = u128::from(amount_in);
        if amount_in_mint == self.mint_0 {
            let quote_amount_out = mint_1_vault.mul(amount_in).div(mint_0_vault.add(amount_in));
            let lp_fee = quote_amount_out
                .mul(u128::from(self.lp_fee_basis_points))
                .checked_ceil_div(10_000)
                .unwrap()
                .0;
            let protocol_fee = quote_amount_out
                .mul(u128::from(self.protocol_fee_basis_points))
                .checked_ceil_div(10_000)
                .unwrap()
                .0;
            let mint_1_amount_out = quote_amount_out
                .sub(lp_fee)
                .sub(protocol_fee)
                .try_into()
                .unwrap_or_else(|_| {
                    eprintln!("amount_out is too large");
                    u64::MIN
                });
            Some(mint_1_amount_out)
        } else {
            let mint_0_vault = u128::from(self.mint_0_vault);
            let mint_1_vault = u128::from(self.mint_1_vault);
            let effective_quote = amount_in.mul(10_000).div(u128::from(
                10_000 + self.lp_fee_basis_points.add(self.protocol_fee_basis_points),
            ));
            let mint_0_amount_out = mint_0_vault
                .mul(effective_quote)
                .div(mint_1_vault.add(effective_quote))
                .try_into()
                .unwrap_or_else(|_| {
                    eprintln!("amount_out is too large");
                    u64::MIN
                });
            Some(mint_0_amount_out)
        }
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
