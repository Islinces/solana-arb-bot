use crate::dex::global_cache::{get_account_data, get_token2022_data};
use crate::dex::meteora_dlmm::commons::{get_bin_array_pubkeys_for_swap, quote_exact_in};
use crate::dex::meteora_dlmm::lb_pair::LbPairExtension;
use crate::dex::quoter::{QuoteResult, Quoter};
use crate::dex::raydium_clmm::state::TickArrayState;
use crate::dex::{BinArray, BinArrayBitmapExtension, LbPair};
use solana_sdk::pubkey::Pubkey;
use spl_token_2022::extension::transfer_fee::TransferFeeConfig;
use std::array;
use std::collections::{HashMap, VecDeque};
use tracing::error;

#[derive(Debug)]
pub struct MeteoraDLMMQuoter;

impl Quoter for MeteoraDLMMQuoter {
    fn quote(&self, amount_in: u64, swap_direction: bool, pool_id: &Pubkey) -> Option<QuoteResult> {
        let pool = get_account_data::<LbPair>(pool_id)?;
        let bitmap_extension = get_bitmap_extension(pool_id);
        let token_transfer_configs = get_token_transfer_config(&pool);
        match get_bin_arrays(pool_id, &pool, bitmap_extension.as_ref(), swap_direction, 3) {
            None => None,
            Some(bin_arrays) => {
                match quote_exact_in(
                    pool,
                    amount_in,
                    swap_direction,
                    bin_arrays,
                    crate::dex::global_cache::get_clock()?,
                    token_transfer_configs[0],
                    token_transfer_configs[1],
                ) {
                    Ok(quote) => Some(QuoteResult {
                        amount_out: quote.amount_out,
                    }),
                    Err(_e) => {
                        // error!("【MeteoraDLMM】[{pool_id}]Quote失败，原因：{}", e);
                        None
                    }
                }
            }
        }
    }
}

fn get_bitmap_extension(pool_id: &Pubkey) -> Option<BinArrayBitmapExtension> {
    get_account_data::<BinArrayBitmapExtension>(
        &crate::dex::meteora_dlmm::commons::derive_bin_array_bitmap_extension(pool_id),
    )
}

fn get_token_transfer_config(pool: &LbPair) -> [Option<TransferFeeConfig>; 2] {
    [
        get_token2022_data(&pool.token_x_mint),
        get_token2022_data(&pool.token_y_mint),
    ]
}

fn get_bin_arrays(
    lb_pair_pubkey: &Pubkey,
    lb_pair: &LbPair,
    bitmap_extension: Option<&BinArrayBitmapExtension>,
    swap_for_y: bool,
    take_count: u8,
) -> Option<VecDeque<BinArray>> {
    match get_bin_array_pubkeys_for_swap(
        lb_pair_pubkey,
        lb_pair,
        bitmap_extension,
        swap_for_y,
        take_count,
    ) {
        Ok(keys) => {
            let expect_count = keys.len();
            let bin_array_map = keys
                .into_iter()
                .filter_map(|key| {
                    let bin_array = crate::dex::global_cache::get_account_data::<BinArray>(&key);
                    if let Some(bin_array) = bin_array {
                        Some(bin_array)
                    } else {
                        None
                    }
                })
                .collect::<VecDeque<_>>();
            if bin_array_map.len() != expect_count {
                error!("转换BinArray失败");
                None
            } else {
                Some(bin_array_map)
            }
        }
        Err(_) => None,
    }
}
