use super::u128x128_math::Rounding;
use crate::dex::meteora_damm_v2::error::TypeCastFailed;
use crate::dex::meteora_damm_v2::math::safe_math::SafeMath;
use anyhow::{anyhow, Result};
use num_traits::cast::FromPrimitive;

#[inline]
pub fn safe_mul_div_cast_u64<T: FromPrimitive>(
    x: u64,
    y: u64,
    denominator: u64,
    rounding: Rounding,
) -> Result<T> {
    let prod = u128::from(x).safe_mul(y.into())?;
    let denominator: u128 = denominator.into();

    let result = match rounding {
        Rounding::Up => prod
            .safe_add(denominator)?
            .safe_sub(1u128)?
            .safe_div(denominator)?,
        Rounding::Down => prod.safe_div(denominator)?,
    };

    T::from_u128(result).ok_or_else(|| anyhow!(TypeCastFailed))
}
