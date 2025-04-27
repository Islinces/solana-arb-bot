use solana_program::pubkey::Pubkey;

pub mod arb;
pub mod grpc_message_collector;
pub mod arb_executor;
pub mod grpc_message_processor;
pub mod grpc_subscribe_strategy;

#[derive(Debug, Clone)]
pub enum Action {
    SWAP((Pubkey)),
}
