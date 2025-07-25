use crate::dex::meteora_dlmm::commons::BASIS_POINT_MAX;
use crate::dex::meteora_dlmm::math::u64x64_math::{pow, ONE, SCALE_OFFSET};
use anyhow::{Context, Result};

pub fn get_price_from_id(active_id: i32, bin_step: u16) -> Result<u128> {
    let bps = u128::from(bin_step)
        .checked_shl(SCALE_OFFSET.into())
        .context("overflow")?
        .checked_div(BASIS_POINT_MAX as u128)
        .context("overflow")?;

    let base = ONE.checked_add(bps).context("overflow")?;

    pow(base, active_id).context("overflow")
}
