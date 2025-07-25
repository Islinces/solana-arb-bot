use crate::dex::global_cache::get_account_data;
use crate::dex::pump_fun::state::Pool;
use crate::dex::quoter::{QuoteResult, Quoter};
use crate::dex::utils::CheckedCeilDiv;
use crate::dex::MintVault;
use solana_sdk::pubkey::Pubkey;
use std::ops::{Add, Div, Mul, Sub};

#[derive(Debug)]
pub struct PumpFunAMMQuoter;

impl Quoter for PumpFunAMMQuoter {
    fn quote(&self, amount_in: u64, swap_direction: bool, pool_id: &Pubkey) -> Option<QuoteResult> {
        let pool = get_account_data::<Pool>(pool_id)?;
        let base_vault_amount =
            u128::from(get_account_data::<MintVault>(&pool.pool_base_token_account)?.amount);
        let quote_vault_amount =
            u128::from(get_account_data::<MintVault>(&pool.pool_quote_token_account)?.amount);
        let amount_in = u128::from(amount_in);
        let amount_out = if swap_direction {
            let quote_amount_out = quote_vault_amount
                .mul(amount_in)
                .div(base_vault_amount.add(amount_in));
            let lp_fee = quote_amount_out
                .mul(u128::from(pool.lp_fee_basis_points))
                .checked_ceil_div(10_000)?
                .0;
            let protocol_fee = quote_amount_out
                .mul(u128::from(pool.protocol_fee_basis_points))
                .checked_ceil_div(10_000)?
                .0;
            let coin_creator_fee = if pool.coin_creator == Pubkey::default() {
                0
            } else {
                quote_amount_out
                    .mul(u128::from(pool.coin_creator_fee_basis_points))
                    .checked_ceil_div(10_000)?
                    .0
            };
            quote_amount_out
                .sub(lp_fee)
                .sub(protocol_fee)
                .sub(coin_creator_fee)
        } else {
            let lp_fee = amount_in
                .mul(u128::from(pool.lp_fee_basis_points))
                .checked_ceil_div(10_000)?
                .0;
            let protocol_fee = amount_in
                .mul(u128::from(pool.protocol_fee_basis_points))
                .checked_ceil_div(10_000)?
                .0;
            let coin_creator_fee = if pool.coin_creator == Pubkey::default() {
                0
            } else {
                amount_in
                    .mul(u128::from(pool.coin_creator_fee_basis_points))
                    .checked_ceil_div(10_000)?
                    .0
            };
            let total_fee = lp_fee.add(protocol_fee).add(coin_creator_fee);
            let effective_amount = amount_in.sub(total_fee);
            base_vault_amount
                .mul(effective_amount)
                .div(quote_vault_amount.add(effective_amount))
        };
        Some(QuoteResult {
            amount_out: u64::try_from(amount_out).ok()?,
        })
    }
}