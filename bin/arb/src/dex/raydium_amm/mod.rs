use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;

pub mod cache_init;
pub mod data_slice;
pub mod instruction;
pub mod quote;
pub mod state;
mod account_subscriber;

pub use account_subscriber::*;

pub const RAYDIUM_AMM_PROGRAM_ID: Pubkey = pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");
const RAYDIUM_AMM_VAULT_OWNER: Pubkey = pubkey!("5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1");
const SERUM_PROGRAM_ID: Pubkey = pubkey!("opnb2LAfJYbRMAHHvqjCwQxanZn7ReEHp1k81EohpZb");
