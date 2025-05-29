use crate::dex::amm_math::CheckedCeilDiv;
use crate::dex::raydium_amm::state::AmmInfo;
use std::ops::{Add, Div, Mul, Sub};

pub fn quote(amount_in: u64, swap_direction: bool, amm_info: &AmmInfo) -> Option<u64> {
    let amount_in = u128::from(amount_in);
    let swap_fee_numerator = u128::from(amm_info.swap_fee_numerator);
    let swap_fee_denominator = u128::from(amm_info.swap_fee_denominator);
    let swap_fee = amount_in
        .mul(swap_fee_numerator)
        .checked_ceil_div(swap_fee_denominator)
        .unwrap()
        .0;
    let swap_in_after_deduct_fee = amount_in.sub(swap_fee);

    let mint_0_amount_without_pnl =
        u128::from(amm_info.coin_vault_amount.sub(amm_info.need_take_pnl_coin));
    let mint_1_amount_without_pnl =
        u128::from(amm_info.pc_vault_amount.sub(amm_info.need_take_pnl_pc));
    let amount_out = if swap_direction {
        mint_1_amount_without_pnl
            .mul(swap_in_after_deduct_fee)
            .div(mint_0_amount_without_pnl.add(swap_in_after_deduct_fee))
    } else {
        mint_0_amount_without_pnl
            .mul(swap_in_after_deduct_fee)
            .div(mint_1_amount_without_pnl.add(swap_in_after_deduct_fee))
    };
    u64::try_from(amount_out).ok()
}
