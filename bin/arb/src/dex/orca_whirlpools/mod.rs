use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;

pub mod accounts;
mod cache_init;
mod data_slice;
pub mod error;
mod math;
mod swap;
mod quote;
mod instruction;

pub use cache_init::*;
pub use data_slice::*;
pub use math::*;
pub use swap::*;
pub use accounts::*;
pub use quote::*;
pub use instruction::*;

pub const WHIRLPOOL_ID: Pubkey = pubkey!("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc");
