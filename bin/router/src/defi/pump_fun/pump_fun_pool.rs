use crate::math::CheckedCeilDiv;
use crate::pump_fun_dex::PumpFunTriggerEvent;
use crate::Pool;
use anyhow::anyhow;
use dex::interface::DexPoolInterface;
use dex::trigger::TriggerEvent;
use solana_program::pubkey::Pubkey;
use std::any::Any;
use std::ops::{Add, Div, Mul, Sub};

#[derive(Debug, Copy, Clone, Default)]
pub struct PumpFunPool {
    pub pool_id: Pubkey,
    pub mint_0: Pubkey,
    pub mint_1: Pubkey,
    pub mint_0_vault: Pubkey,
    pub mint_1_vault: Pubkey,
    pub mint_0_vault_amount: u64,
    pub mint_1_vault_amount: u64,
    pub lp_fee_basis_points: u64,
    pub protocol_fee_basis_points: u64,
}

impl PumpFunPool {
    pub fn new(
        pool_id: Pubkey,
        mint_0: Pubkey,
        mint_1: Pubkey,
        mint_0_vault_amount: u64,
        mint_1_vault_amount: u64,
        lp_fee_basis_points: u64,
        protocol_fee_basis_points: u64,
    ) -> Self {
        PumpFunPool {
            pool_id: pool_id,
            mint_0: mint_0,
            mint_1: mint_1,
            mint_0_vault: Pubkey::default(),
            mint_1_vault: Pubkey::default(),
            mint_0_vault_amount,
            mint_1_vault_amount,
            lp_fee_basis_points,
            protocol_fee_basis_points,
        }
    }
}

impl DexPoolInterface for PumpFunPool {
    fn get_pool_id(&self) -> Pubkey {
        self.pool_id
    }

    fn get_mint_0(&self) -> Pubkey {
        self.mint_0
    }

    fn get_mint_1(&self) -> Pubkey {
        self.mint_1
    }

    fn get_mint_0_vault(&self) -> Option<Pubkey> {
        Some(self.mint_0_vault)
    }

    fn get_mint_1_vault(&self) -> Option<Pubkey> {
        Some(self.mint_1_vault)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn quote(&self, amount_in: u64, amount_in_mint: Pubkey) -> Option<u64> {
        if amount_in_mint != self.mint_0 && amount_in_mint != self.mint_1 {
            return None;
        }
        let base_vault = u128::from(self.mint_0_vault_amount);
        let quote_vault = u128::from(self.mint_1_vault_amount);
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
        // println!("total_fee: {}", total_fee);
        Some(amount_out.try_into().unwrap_or_else(|_| {
            eprintln!("amount_out is too large");
            u64::MIN
        }))
    }

    fn update_data(&mut self, changed_pool: Box<dyn TriggerEvent>) -> anyhow::Result<Pubkey> {
        let pushed_event = changed_pool
            .as_any()
            .downcast_ref::<PumpFunTriggerEvent>()
            .unwrap();
        let mint_0_vault_amount = pushed_event.mint_0_vault_update.as_ref().unwrap().amount;
        let mint_1_vault_amount = pushed_event.mint_1_vault_update.as_ref().unwrap().amount;
        let mut changed = false;
        if self.mint_0_vault_amount != mint_0_vault_amount {
            self.mint_0_vault_amount = mint_0_vault_amount;
            changed |= true;
        }
        if self.mint_1_vault_amount != mint_1_vault_amount {
            self.mint_1_vault_amount = mint_1_vault_amount;
            changed |= true;
        }
        if changed {
            Ok(self.pool_id)
        } else {
            Err(anyhow!("[{}]池子数据未发生变化", self.pool_id))
        }
    }
}

impl From<(Pubkey, Pool, u64, u64, u64, u64)> for PumpFunPool {
    fn from(value: (Pubkey, Pool, u64, u64, u64, u64)) -> Self {
        Self {
            pool_id: value.0,
            mint_0_vault: value.1.pool_base_token_account,
            mint_1_vault: value.1.pool_quote_token_account,
            mint_0: value.1.base_mint,
            mint_1: value.1.quote_mint,
            mint_0_vault_amount: value.2,
            mint_1_vault_amount: value.3,
            lp_fee_basis_points: value.4,
            protocol_fee_basis_points: value.5,
        }
    }
}
