use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;

mod account_subscriber;
mod data_slice;
pub mod instruction;
pub mod quote;
mod relation;
mod snapshot_loader;
pub mod state;

pub use account_subscriber::*;
pub use data_slice::*;
pub(super) use relation::*;
pub use snapshot_loader::*;
pub use state::*;

pub(super) const RAYDIUM_AMM_PROGRAM_ID: Pubkey =
    pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");
const RAYDIUM_AMM_VAULT_OWNER: Pubkey = pubkey!("5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1");
const SERUM_PROGRAM_ID: Pubkey = pubkey!("opnb2LAfJYbRMAHHvqjCwQxanZn7ReEHp1k81EohpZb");
