use solana_sdk::declare_id;

pub mod collector;
pub mod executor;
pub mod interface;
pub mod strategy;

#[cfg(feature = "devnet")]
declare_id!("devi51mZmdwUJGU9hjN27vEz64Gps7uUefqxg27EAtH");
#[cfg(not(feature = "devnet"))]
declare_id!("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK");
