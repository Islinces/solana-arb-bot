use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;

pub mod account_cache;
mod account_relation;
pub mod arb;
pub mod arb_bot;
mod data_slice;
mod dex;
pub mod dex_data;
mod graph;
pub mod grpc_processor;
pub mod grpc_subscribe;
pub mod interface;
mod jupiter;
mod quoter;
pub mod state;
mod metadata;
mod keypair;
mod executor;

pub const ATA_PROGRAM: Pubkey = pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
pub const SYSTEM_PROGRAM: Pubkey = pubkey!("11111111111111111111111111111111");
pub const MINT_PROGRAM: Pubkey = spl_token::ID;
