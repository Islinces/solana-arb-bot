pub mod big_num;
pub mod config;
pub mod fixed_point_64;
pub mod full_math;
pub mod liquidity_math;
pub mod pool;
pub mod sqrt_price_math;
pub mod swap_math;
pub mod tick_array;
pub mod tick_array_bit_map;
pub mod tick_math;
pub mod tickarray_bitmap_extension;
pub mod unsafe_math;
pub mod utils;

#[macro_export]
macro_rules! require_gt {
    ($value1: expr, $value2: expr, $error_code: expr $(,)?) => {
        if $value1 <= $value2 {
            return Err(anyhow::anyhow!("{}: value1={}, value2={}", $error_code, $value1, $value2));
        }
    };
    ($value1: expr, $value2: expr $(,)?) => {
        if $value1 <= $value2 {
            return Err(anyhow::anyhow!("A require_gt expression was violated"));
        }
    };
}

#[macro_export]
macro_rules! require_gte {
    ($value1: expr, $value2: expr, $error_code: expr $(,)?) => {
        if $value1 < $value2 {
            return Err(anyhow::anyhow!("{}: value1={}, value2={}", $error_code, $value1, $value2));
        }
    };
    ($value1: expr, $value2: expr $(,)?) => {
        if $value1 < $value2 {
            return Err(anyhow::anyhow!(anchor_lang::error::ErrorCode::RequireGteViolated)
                .with_values(($value1, $value2)));
        }
    };
}

#[macro_export]
macro_rules! require {
    // ($invariant:expr, $error:tt $(,)?) => {
    //     if !($invariant) {
    //         return Err(anyhow::anyhow!($crate::ErrorCode::$error));
    //     }
    // };
    ($invariant:expr, $error:expr $(,)?) => {
        if !($invariant) {
            return Err(anyhow::anyhow!($error));
        }
    };
}
