mod account_subscriber;
mod curve;
mod data_slice;
mod instruction;
pub mod old_state;
mod quote;
mod relation;
mod snapshot_loader;
pub mod states;

pub use account_subscriber::*;
pub use data_slice::*;
pub use instruction::*;
pub use quote::*;
pub use relation::*;
pub use snapshot_loader::*;
use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;

pub(super) const RAYDIUM_CPMM_PROGRAM_ID: Pubkey =
    pubkey!("CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C");
const RAYDIUM_CPMM_AUTHORITY_ID: Pubkey = pubkey!("GpMZbSM2GgvTKHJirzeGfMFoaZ8UR2X7F4v8vHTvxFbL");

mod error {
    pub const EMPTY_SUPPLY: &'static str = "Input token account empty";
}

#[test]
fn test() {
    pub const AUTH_SEED: &str = "vault_and_lp_mint_auth_seed";
    let (authority, __bump) =
        Pubkey::find_program_address(&[AUTH_SEED.as_bytes()], &RAYDIUM_CPMM_PROGRAM_ID);
    println!("{:?}", authority);
}
