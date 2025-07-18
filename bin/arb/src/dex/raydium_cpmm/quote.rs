use crate::dex::raydium_cpmm::curve::CurveCalculator;
use crate::dex::raydium_cpmm::states::{AmmConfig, PoolState};
use crate::dex::{get_account_data, get_clock, get_transfer_fee, MintVault, QuoteResult, Quoter};
use anyhow::anyhow;
use solana_sdk::pubkey::Pubkey;
use tracing::error;

#[derive(Debug)]
pub struct RaydiumCPMMQuoter;

impl Quoter for RaydiumCPMMQuoter {
    fn quote(&self, amount_in: u64, swap_direction: bool, pool_id: &Pubkey) -> Option<QuoteResult> {
        match get_quote(amount_in, swap_direction, pool_id) {
            Ok(amount_out) => Some(QuoteResult { amount_out }),
            Err(e) => {
                error!("[RaydiumCPMM][{pool_id}] quote失败，原因：{}", e);
                None
            }
        }
    }
}

fn get_quote(amount_in: u64, swap_direction: bool, pool_id: &Pubkey) -> anyhow::Result<u64> {
    let pool_state =
        get_account_data::<PoolState>(pool_id).ok_or(anyhow!("缓存中无池子[{pool_id}]"))?;
    let trade_fee_rate = get_account_data::<AmmConfig>(&pool_state.amm_config)
        .ok_or(anyhow!("缓存中无AmmConfig[{}]", pool_state.amm_config))?
        .trade_fee_rate;

    let token_0_vault_amount = get_account_data::<MintVault>(&pool_state.token_0_vault)
        .ok_or(anyhow!("缓存中无金库[{}]", pool_state.token_0_vault))?
        .amount;
    let token_1_vault_amount = get_account_data::<MintVault>(&pool_state.token_1_vault)
        .ok_or(anyhow!("缓存中无金库[{}]", pool_state.token_1_vault))?
        .amount;
    let (total_token_0_amount, total_token_1_amount) =
        pool_state.vault_amount_without_fee(token_0_vault_amount, token_1_vault_amount);
    let epoch = get_clock().ok_or(anyhow!("缓存中无Clock"))?.epoch;
    let (total_input_token_amount, total_output_token_amount, transfer_fee) = if swap_direction {
        (
            total_token_0_amount,
            total_token_1_amount,
            get_transfer_fee(&pool_state.token_0_mint, epoch, amount_in),
        )
    } else {
        (
            total_token_1_amount,
            total_token_0_amount,
            get_transfer_fee(&pool_state.token_1_mint, epoch, amount_in),
        )
    };
    // Take transfer fees into account for actual amount transferred in
    let actual_amount_in = amount_in.saturating_sub(transfer_fee);
    let amount_out = u64::try_from(
        CurveCalculator::swap_base_input(
            u128::from(actual_amount_in),
            u128::from(total_input_token_amount),
            u128::from(total_output_token_amount),
            trade_fee_rate,
        )
        .ok_or(anyhow!("Quote返回None"))?,
    )?;
    let transfer_fee = if swap_direction {
        get_transfer_fee(&pool_state.token_1_mint, epoch, amount_out)
    } else {
        get_transfer_fee(&pool_state.token_0_mint, epoch, amount_out)
    };
    amount_out
        .checked_sub(transfer_fee)
        .ok_or(anyhow!("扣减transfer_fee失败"))
}
