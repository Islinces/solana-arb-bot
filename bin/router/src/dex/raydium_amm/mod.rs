use solana_program::pubkey;
use solana_program::pubkey::Pubkey;
use solana_sdk::declare_id;

mod error;
mod math;
pub mod raydium_amm;
mod state;
pub mod pool_state;

declare_id!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");

pub(crate) const RAYDIUM_AUTHORITY_ID: Pubkey = pubkey!("5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1");
