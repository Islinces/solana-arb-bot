pub mod sdk;
pub mod dlmm_pool;
pub mod meteora_dlmm_dex;

#[cfg(not(feature = "staging"))]
solana_program::declare_id!("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo");

#[cfg(feature = "staging")]
solana_program::declare_id!("tLBro6JJuZNnpoad3p8pXKohE9f7f7tBZJpaeh6pXt1");