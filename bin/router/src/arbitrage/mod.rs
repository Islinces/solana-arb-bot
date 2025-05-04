use solana_program::pubkey;
use solana_program::pubkey::Pubkey;
use crate::dex::DexQuoteResult;

pub mod jito_arb_executor;
pub mod arb_strategy;
pub mod arb_worker;
pub mod message_collector;
pub mod message_processor;
mod jupiter_route;
pub mod types;

/// `jupiter` program ID.
pub const JUPITER_ID: Pubkey = pubkey!("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4");
pub const JUPITER_EVENT_AUTHORITY: Pubkey = pubkey!("D8cy77BBepLMngZx6ZukaTff5hCt1HrWyKk3Hnd9oitf");

#[derive(Debug, Clone)]
pub enum Action {
    SWAP(DexQuoteResult),
}
