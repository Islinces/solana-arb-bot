pub mod account_load;
pub mod big_num;
pub mod clmm_pool;
pub mod config;
pub mod error;
pub mod fixed_point_64;
pub mod full_math;
pub mod liquidity_math;
pub mod pool;
pub mod sqrt_price_math;
pub mod swap_math;
pub mod system;
pub mod tick_array;
pub mod tick_array_bit_map;
pub mod tick_math;
pub mod tickarray_bitmap_extension;
pub mod unsafe_math;
pub mod utils;
pub mod raydium_clmm_dex;

use anchor_lang::declare_id;

#[cfg(feature = "devnet")]
declare_id!("devi51mZmdwUJGU9hjN27vEz64Gps7uUefqxg27EAtH");
#[cfg(not(feature = "devnet"))]
declare_id!("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK");