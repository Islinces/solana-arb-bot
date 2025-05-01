use solana_sdk::declare_id;

mod math;
mod state;
pub mod pump_fun;
pub mod pool_state;

#[cfg(not(feature = "devnet"))]
declare_id!("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA");