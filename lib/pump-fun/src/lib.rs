use anchor_lang::declare_id;

pub mod pump_fun_pool;
pub mod pump_fun_dex;
pub mod state;
pub mod utils;
mod math;

pub use state::*;

#[cfg(not(feature = "devnet"))]
declare_id!("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA");