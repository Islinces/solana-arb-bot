use crate::dex::meteora_dlmm::sdk::commons::pda::derive_bin_array_pda;
use crate::dex::meteora_dlmm::sdk::commons::token_2022::calculate_transfer_fee_excluded_amount;
use crate::dex::meteora_dlmm::sdk::commons::typedefs::SwapResult;
use crate::dex::meteora_dlmm::sdk::extensions::bin::BinExtension;
use crate::dex::meteora_dlmm::sdk::extensions::bin_array::BinArrayExtension;
use crate::dex::meteora_dlmm::sdk::extensions::bin_array_bitmap::BinArrayBitmapExtExtension;
use crate::dex::meteora_dlmm::sdk::interface::accounts::{
    BinArray, BinArrayBitmapExtension, LbPair,
};
use crate::dex::meteora_dlmm::sdk::interface::typedefs::{ActivationType, PairStatus, PairType};
use crate::dex::meteora_dlmm::sdk::lb_pair::LbPairExtension;
use anchor_spl::token_2022::spl_token_2022::extension::transfer_fee::TransferFeeConfig;
use anyhow::Result;
use anyhow::{ensure, Context};
use solana_program::pubkey::Pubkey;
use solana_sdk::clock::Clock;
use std::{collections::HashMap, ops::Deref};
use std::sync::Arc;

#[derive(Debug)]
pub struct SwapExactInQuote {
    pub amount_out: u64,
    pub fee: u64,
}

#[derive(Debug)]
pub struct SwapExactOutQuote {
    pub amount_in: u64,
    pub fee: u64,
}

fn validate_swap_activation(
    lb_pair: &LbPair,
    current_timestamp: u64,
    current_slot: u64,
) -> Result<()> {
    ensure!(
        lb_pair.status()?.eq(&PairStatus::Enabled),
        "Pair is disabled"
    );

    let pair_type = lb_pair.pair_type()?;
    if pair_type.eq(&PairType::Permission) {
        let activation_type = lb_pair.activation_type()?;
        let current_point = match activation_type.deref() {
            ActivationType::Slot => current_slot,
            ActivationType::Timestamp => current_timestamp,
        };

        ensure!(
            current_point >= lb_pair.activation_point,
            "Pair is disabled"
        );
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn quote_exact_in(
    lb_pair_pubkey: Pubkey,
    lb_pair: LbPair,
    amount_in: u64,
    swap_for_y: bool,
    bin_arrays: HashMap<Pubkey, BinArray>,
    bitmap_extension: Option<BinArrayBitmapExtension>,
    clock: Arc<Clock>,
    mint_x_account: Option<TransferFeeConfig>,
    mint_y_account: Option<TransferFeeConfig>,
) -> Result<SwapExactInQuote> {
    let current_timestamp = clock.unix_timestamp as u64;
    let current_slot = clock.slot;
    let epoch = clock.epoch;
    // 池子状态判断
    validate_swap_activation(&lb_pair, current_timestamp, current_slot)?;

    let mut lb_pair = lb_pair;
    lb_pair.update_references(current_timestamp as i64)?;

    let mut total_amount_out: u64 = 0;
    let mut total_fee: u64 = 0;

    let (in_mint_transfer_fee_config, out_mint_transfer_fee_config) = if swap_for_y {
        (mint_x_account.as_ref(), mint_y_account.as_ref())
    } else {
        (mint_y_account.as_ref(), mint_x_account.as_ref())
    };

    let transfer_fee_excluded_amount_in =
        calculate_transfer_fee_excluded_amount(in_mint_transfer_fee_config, amount_in, epoch)?
            .amount;

    let mut amount_left = transfer_fee_excluded_amount_in;
    let mut loop_count = 0;
    // 循环swap，直到amount_in交换完
    while amount_left > 0 {
        loop_count += 1;
        // 查找第一个有流动性的bin_array
        let active_bin_array_pubkey = get_bin_array_pubkeys_for_swap(
            lb_pair_pubkey,
            &lb_pair,
            bitmap_extension.as_ref(),
            swap_for_y,
            1,
        )?
        .pop()
        .context("Pool out of liquidity")?;

        let mut active_bin_array = bin_arrays
            .get(&active_bin_array_pubkey)
            .cloned()
            .context("Active bin array not found")?;

        loop {
            // 判断当前active_id是否在bin_array中
            if !active_bin_array.is_bin_id_within_range(lb_pair.active_id)? || amount_left == 0 {
                break;
            }

            lb_pair.update_volatility_accumulator()?;
            // 当前active_id所在bin
            let active_bin = active_bin_array.get_bin_mut(lb_pair.active_id)?;
            // 计算当前active_id价格
            let price = active_bin.get_or_store_bin_price(lb_pair.active_id, lb_pair.bin_step)?;
            // bin中对应mint还有流动性
            if !active_bin.is_empty(!swap_for_y) {
                let SwapResult {
                    amount_in_with_fees,
                    amount_out,
                    fee,
                    ..
                } = active_bin.swap(amount_left, price, swap_for_y, &lb_pair, None)?;

                amount_left = amount_left
                    .checked_sub(amount_in_with_fees)
                    .context("MathOverflow")?;

                total_amount_out = total_amount_out
                    .checked_add(amount_out)
                    .context("MathOverflow")?;
                total_fee = total_fee.checked_add(fee).context("MathOverflow")?;
            }
            if amount_left > 0 {
                lb_pair.advance_active_bin(swap_for_y)?;
            }
        }
    }

    let transfer_fee_excluded_amount_out = calculate_transfer_fee_excluded_amount(
        out_mint_transfer_fee_config,
        total_amount_out,
        epoch,
    )?
    .amount;

    Ok(SwapExactInQuote {
        amount_out: transfer_fee_excluded_amount_out,
        fee: total_fee,
    })
}

pub fn get_bin_array_pubkeys_for_swap(
    lb_pair_pubkey: Pubkey,
    lb_pair: &LbPair,
    bitmap_extension: Option<&BinArrayBitmapExtension>,
    swap_for_y: bool,
    take_count: u8,
) -> Result<Vec<Pubkey>> {
    // 当前池子所在的bin_array_idx，active_id/70的商
    let mut start_bin_array_idx = BinArray::bin_id_to_bin_array_index(lb_pair.active_id)?;
    let mut bin_array_idx = vec![];
    let increment = if swap_for_y { -1 } else { 1 };

    loop {
        if bin_array_idx.len() == take_count as usize {
            break;
        }
        // 是否超过池子上bin_array_bitmap范围
        // [-512,511]
        if lb_pair.is_overflow_default_bin_array_bitmap(start_bin_array_idx) {
            let Some(bitmap_extension) = bitmap_extension else {
                break;
            };
            let Ok((next_bin_array_idx, has_liquidity)) = bitmap_extension
                .next_bin_array_index_with_liquidity(swap_for_y, start_bin_array_idx)
            else {
                // Out of search range. No liquidity.
                break;
            };
            if has_liquidity {
                bin_array_idx.push(next_bin_array_idx);
                start_bin_array_idx = next_bin_array_idx + increment;
            } else {
                // Switch to internal bitmap
                start_bin_array_idx = next_bin_array_idx;
            }
        } else {
            let Ok((next_bin_array_idx, has_liquidity)) = lb_pair
                .next_bin_array_index_with_liquidity_internal(swap_for_y, start_bin_array_idx)
            else {
                break;
            };
            if has_liquidity {
                bin_array_idx.push(next_bin_array_idx);
                start_bin_array_idx = next_bin_array_idx + increment;
            } else {
                // Switch to external bitmap
                start_bin_array_idx = next_bin_array_idx;
            }
        }
    }

    let bin_array_pubkeys = bin_array_idx
        .into_iter()
        .map(|idx| derive_bin_array_pda(lb_pair_pubkey, idx.into()).0)
        .collect();

    Ok(bin_array_pubkeys)
}
