use crate::dex::raydium_clmm::big_num::U256;
use crate::dex::raydium_clmm::full_math::MulDiv;
use crate::dex::raydium_clmm::unsafe_math::UnsafeMathTrait;
use crate::dex::raydium_clmm::{Q64, RESOLUTION};
use crate::{require_gt, require_gte};
use anyhow::{anyhow, Result};

pub fn add_delta(x: u128, y: i128) -> Result<u128> {
    let z: u128;
    if y < 0 {
        z = x - u128::try_from(-y)?;
        require_gt!(x, z, "Liquidity sub delta L must be smaller than before");
    } else {
        z = x + u128::try_from(y)?;
        require_gte!(
            z,
            x,
            "Liquidity add delta L must be greater, or equal to before"
        );
    }

    Ok(z)
}

/// Gets the delta amount_0 for given liquidity and price range
///
/// # Formula
///
/// * `Δx = L * (1 / √P_lower - 1 / √P_upper)`
/// * i.e. `L * (√P_upper - √P_lower) / (√P_upper * √P_lower)`
pub fn get_delta_amount_0_unsigned(
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    liquidity: u128,
    round_up: bool,
) -> Result<u64> {
    // sqrt_ratio_a_x64 should hold the smaller value
    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    };

    let numerator_1 = U256::from(liquidity) << RESOLUTION;
    let numerator_2 = U256::from(sqrt_ratio_b_x64 - sqrt_ratio_a_x64);

    assert!(sqrt_ratio_a_x64 > 0);
    // L *((√P_upper - √P_lower)) / √P_upper / √P_lower
    let result = if round_up {
        U256::div_rounding_up(
            numerator_1
                .mul_div_ceil(numerator_2, U256::from(sqrt_ratio_b_x64))
                .unwrap(),
            U256::from(sqrt_ratio_a_x64),
        )
    } else {
        numerator_1
            .mul_div_floor(numerator_2, U256::from(sqrt_ratio_b_x64))
            .unwrap()
            / U256::from(sqrt_ratio_a_x64)
    };
    if result > U256::from(u64::MAX) {
        return Err(anyhow!("Max token overflow"));
    }
    return Ok(result.as_u64());
}

/// Gets the delta amount_1 for given liquidity and price range
/// * `Δy = L (√P_upper - √P_lower)`
pub fn get_delta_amount_1_unsigned(
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    liquidity: u128,
    round_up: bool,
) -> Result<u64> {
    // sqrt_ratio_a_x64 should hold the smaller value
    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    };

    let result = if round_up {
        U256::from(liquidity).mul_div_ceil(
            U256::from(sqrt_ratio_b_x64 - sqrt_ratio_a_x64),
            U256::from(Q64),
        )
    } else {
        U256::from(liquidity).mul_div_floor(
            U256::from(sqrt_ratio_b_x64 - sqrt_ratio_a_x64),
            U256::from(Q64),
        )
    }
    .unwrap();
    if result > U256::from(u64::MAX) {
        return Err(anyhow!("Max token overflow"));
    }
    return Ok(result.as_u64());
}
