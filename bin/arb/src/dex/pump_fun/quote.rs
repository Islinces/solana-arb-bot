use crate::dex::amm_math::CheckedCeilDiv;
use crate::dex::pump_fun::state::Pool;
use crate::global_cache::get_account_data;
use crate::{QuoteResult, Quoter};
use solana_sdk::pubkey::Pubkey;
use std::ops::{Add, Div, Mul, Sub};

#[derive(Debug)]
pub struct PumpFunAMMQuoter;

impl Quoter for PumpFunAMMQuoter {
    fn quote(&self, amount_in: u64, swap_direction: bool, pool_id: &Pubkey) -> Option<QuoteResult> {
        let pool = get_account_data::<Pool>(pool_id)?;
        let base_vault_amount = u128::from(pool.base_vault_amount);
        let quote_vault_amount = u128::from(pool.quote_vault_amount);
        let amount_in = u128::from(amount_in);
        let lp_fee = amount_in
            .mul(u128::from(pool.lp_fee_basis_points))
            .checked_ceil_div(10_000)?
            .0;
        let protocol_fee = amount_in
            .mul(u128::from(pool.protocol_fee_basis_points))
            .checked_ceil_div(10_000)?
            .0;
        let total_fee = lp_fee.add(protocol_fee);
        let effective_amount = amount_in.sub(total_fee);
        let amount_out = if swap_direction {
            quote_vault_amount
                .mul(effective_amount)
                .div(base_vault_amount.add(effective_amount))
        } else {
            base_vault_amount
                .mul(effective_amount)
                .div(quote_vault_amount.add(effective_amount))
        };
        Some(QuoteResult {
            amount_out: u64::try_from(amount_out).ok()?,
        })
    }
}
