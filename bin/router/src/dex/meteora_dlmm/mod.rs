use solana_sdk::declare_id;

pub mod sdk;
pub mod meteora_dlmm;
pub mod pool_state;

#[cfg(not(feature = "staging"))]
declare_id!("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo");

#[cfg(feature = "staging")]
declare_id!("tLBro6JJuZNnpoad3p8pXKohE9f7f7tBZJpaeh6pXt1");