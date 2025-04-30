use solana_program::pubkey::Pubkey;

pub mod arb_worker;
pub mod message_collector;
pub mod arb_executor;
pub mod message_processor;
pub mod arb_strategy;

#[derive(Debug, Clone)]
pub enum Action {
    SWAP(Pubkey),
}
