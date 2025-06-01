use crate::dex::meteora_dlmm::commons::constants::BASIS_POINT_MAX;
use crate::dex::meteora_dlmm::commons::typedefs::SwapResult;
use crate::dex::meteora_dlmm::interface::accounts::LbPair;
use crate::dex::meteora_dlmm::interface::typedefs::{Bin, Rounding};
use crate::dex::meteora_dlmm::lb_pair::LbPairExtension;
use crate::dex::meteora_dlmm::math::price_math::get_price_from_id;
use crate::dex::meteora_dlmm::math::u64x64_math::SCALE_OFFSET;
use crate::dex::meteora_dlmm::math::utils::{
    safe_mul_shr_cast, safe_shl_div_cast,
};
use anyhow::{Context, Result};

pub trait BinExtension {
    fn get_or_store_bin_price(&mut self, id: i32, bin_step: u16) -> Result<u128>;
    fn is_empty(&self, is_x: bool) -> bool;
    fn get_max_amount_out(&self, swap_for_y: bool) -> u64;
    fn get_max_amount_in(&self, price: u128, swap_for_y: bool) -> Result<u64>;

    fn swap(
        &mut self,
        amount_in: u64,
        price: u128,
        swap_for_y: bool,
        lb_pair: &LbPair,
        host_fee_bps: Option<u16>,
    ) -> Result<SwapResult>;

    fn get_amount_out(amount_in: u64, price: u128, swap_for_y: bool) -> Result<u64>;
    fn get_amount_in(amount_out: u64, price: u128, swap_for_y: bool) -> Result<u64>;
}

impl BinExtension for Bin {
    fn get_or_store_bin_price(&mut self, id: i32, bin_step: u16) -> Result<u128> {
        if self.price == 0 {
            self.price = get_price_from_id(id, bin_step)?;
        }

        Ok(self.price)
    }

    fn is_empty(&self, is_x: bool) -> bool {
        if is_x {
            self.amount_x == 0
        } else {
            self.amount_y == 0
        }
    }

    fn get_max_amount_out(&self, swap_for_y: bool) -> u64 {
        if swap_for_y {
            self.amount_y
        } else {
            self.amount_x
        }
    }

    fn get_max_amount_in(&self, price: u128, swap_for_y: bool) -> Result<u64> {
        if swap_for_y {
            // x=y/P
            safe_shl_div_cast(self.amount_y.into(), price, SCALE_OFFSET, Rounding::Up)
        } else {
            safe_mul_shr_cast(self.amount_x.into(), price, SCALE_OFFSET, Rounding::Up)
        }
    }

    fn get_amount_in(amount_out: u64, price: u128, swap_for_y: bool) -> Result<u64> {
        if swap_for_y {
            safe_shl_div_cast(amount_out.into(), price, SCALE_OFFSET, Rounding::Up)
        } else {
            safe_mul_shr_cast(amount_out.into(), price, SCALE_OFFSET, Rounding::Up)
        }
    }

    fn get_amount_out(amount_in: u64, price: u128, swap_for_y: bool) -> Result<u64> {
        if swap_for_y {
            // y=x*P
            safe_mul_shr_cast(price, amount_in.into(), SCALE_OFFSET, Rounding::Down)
        } else {
            safe_shl_div_cast(amount_in.into(), price, SCALE_OFFSET, Rounding::Down)
        }
    }

    fn swap(
        &mut self,
        amount_in: u64,
        price: u128,
        swap_for_y: bool,
        lb_pair: &LbPair,
        host_fee_bps: Option<u16>,
    ) -> Result<SwapResult> {
        // 当前bin全部的amount_out
        let max_amount_out = self.get_max_amount_out(swap_for_y);
        // 当前价格当前bin最大可支持的amount_in
        let mut max_amount_in = self.get_max_amount_in(price, swap_for_y)?;

        let max_fee = lb_pair.compute_fee(max_amount_in)?;
        max_amount_in = max_amount_in.checked_add(max_fee).context("overflow")?;

        let (amount_in_with_fees, amount_out, fee, protocol_fee) = if amount_in > max_amount_in {
            (
                max_amount_in,
                max_amount_out,
                max_fee,
                lb_pair.compute_protocol_fee(max_fee)?,
            )
        } else {
            // 不够使用amount_in重新计算fee
            let fee = lb_pair.compute_fee_from_amount(amount_in)?;
            // 减去fee之后可使用的amount_in
            let amount_in_after_fee = amount_in.checked_sub(fee).context("overflow")?;
            // 计算amount_out
            let amount_out = Bin::get_amount_out(amount_in_after_fee, price, swap_for_y)?;
            (
                amount_in,
                std::cmp::min(amount_out, max_amount_out),
                fee,
                lb_pair.compute_protocol_fee(fee)?,
            )
        };

        let host_fee = match host_fee_bps {
            Some(bps) => protocol_fee
                .checked_mul(bps.into())
                .context("overflow")?
                .checked_div(BASIS_POINT_MAX as u64)
                .context("overflow")?,
            None => 0,
        };

        let protocol_fee_after_host_fee = protocol_fee.checked_sub(host_fee).context("overflow")?;

        let amount_into_bin = amount_in_with_fees.checked_sub(fee).context("overflow")?;
        if swap_for_y {
            // 将amount_in加到bin中
            self.amount_x = self
                .amount_x
                .checked_add(amount_into_bin)
                .context("overflow")?;
            // bin 减去amount_out
            self.amount_y = self.amount_y.checked_sub(amount_out).context("overflow")?;
        } else {
            self.amount_y = self
                .amount_y
                .checked_add(amount_into_bin)
                .context("overflow")?;
            self.amount_x = self.amount_x.checked_sub(amount_out).context("overflow")?;
        }

        Ok(SwapResult {
            amount_in_with_fees,
            amount_out,
            fee,
            protocol_fee_after_host_fee,
            host_fee,
            is_exact_out_amount: false,
        })
    }
}
