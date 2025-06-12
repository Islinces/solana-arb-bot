use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;

mod account_subscriber;
mod big_num;
mod data_slice;
mod full_math;
pub mod instruction;
mod liquidity_math;
pub mod quote;
mod relation;
mod snapshot_loader;
mod sqrt_price_math;
pub mod state;
mod swap_math;
mod tick_math;
mod unsafe_math;
pub mod utils;

pub use account_subscriber::*;
pub use data_slice::*;
pub(super) use relation::*;
pub use snapshot_loader::*;
pub use state::*;

pub(super) const RAYDIUM_CLMM_PROGRAM_ID: Pubkey =
    pubkey!("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK");

const Q64: u128 = (u64::MAX as u128) + 1; // 2^64
const RESOLUTION: u8 = 64;

#[macro_export]
macro_rules! require_gt {
    ($value1: expr, $value2: expr, $error_code: expr $(,)?) => {
        if $value1 <= $value2 {
            return Err(anyhow::anyhow!(
                "{}: value1={}, value2={}",
                $error_code,
                $value1,
                $value2
            ));
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
            return Err(anyhow::anyhow!(
                "{}: value1={}, value2={}",
                $error_code,
                $value1,
                $value2
            ));
        }
    };
    ($value1: expr, $value2: expr $(,)?) => {
        if $value1 < $value2 {
            return Err(
                anyhow::anyhow!(anchor_lang::error::ErrorCode::RequireGteViolated)
                    .with_values(($value1, $value2)),
            );
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
