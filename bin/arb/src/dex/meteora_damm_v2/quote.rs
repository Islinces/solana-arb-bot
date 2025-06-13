use crate::dex::meteora_damm_v2::state::fee::FeeMode;
use crate::dex::meteora_damm_v2::state::pool::Pool;
use crate::dex::meteora_damm_v2::{ActivationType, TradeDirection};
use crate::dex::{get_account_data, get_clock, get_transfer_fee, QuoteResult, Quoter};
use anyhow::{ensure, Context, Ok, Result};
use solana_sdk::pubkey::Pubkey;
use std::ops::Sub;
use tracing::info;

#[derive(Debug)]
pub struct MeteoraDAMMV2Quoter;

impl Quoter for MeteoraDAMMV2Quoter {
    fn quote(&self, amount_in: u64, swap_direction: bool, pool_id: &Pubkey) -> Option<QuoteResult> {
        let pool = get_account_data::<Pool>(pool_id)?;
        info!("pool: {:#?}", pool);
        Some(QuoteResult {
            amount_out: get_quote(pool, amount_in, swap_direction).ok()?,
        })
    }
}

fn get_quote(mut pool: Pool, amount_in: u64, swap_direction: bool) -> Result<u64> {
    ensure!(amount_in > 0, "amount is zero");
    let clock = get_clock().expect("无法获取Clock");
    let current_timestamp = clock.unix_timestamp as u64;
    let current_slot = clock.slot as u64;
    let epoch = clock.epoch as u64;
    let result = if pool.dynamic_fee.is_dynamic_fee_enable() {
        pool.update_pre_swap(current_timestamp)?;
        get_internal_quote(
            &pool,
            current_timestamp,
            current_slot,
            epoch,
            amount_in,
            swap_direction,
            true,
        )
    } else {
        get_internal_quote(
            &pool,
            current_timestamp,
            current_slot,
            epoch,
            amount_in,
            swap_direction,
            true,
        )
    };

    result
}

fn get_internal_quote(
    pool: &Pool,
    current_timestamp: u64,
    current_slot: u64,
    epoch: u64,
    amount_in: u64,
    a_to_b: bool,
    has_referral: bool,
) -> Result<u64> {
    let activation_type =
        ActivationType::try_from(pool.activation_type).context("invalid activation type")?;

    let current_point = match activation_type {
        ActivationType::Slot => current_slot,
        ActivationType::Timestamp => current_timestamp,
    };

    let (trade_direction, token_in) = if a_to_b {
        (TradeDirection::AtoB, pool.token_a_mint)
    } else {
        (TradeDirection::BtoA, pool.token_b_mint)
    };
    let actual_amount_in = amount_in.sub(get_transfer_fee(&token_in, epoch, amount_in));
    let fee_mode = &FeeMode::get_fee_mode(pool.collect_fee_mode, trade_direction, has_referral)?;
    Ok(pool
        .get_swap_result(actual_amount_in, fee_mode, trade_direction, current_point)?
        .output_amount)
}
