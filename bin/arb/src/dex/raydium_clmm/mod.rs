use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;

mod account_subscriber;
mod big_num;
mod data_slice;
mod full_math;
pub mod instruction;
mod liquidity_math;
pub mod old_state;
pub mod quote;
mod relation;
mod snapshot_loader;
mod sqrt_price_math;
pub mod state;
mod swap_math;
mod tick_math;
mod unsafe_math;
pub mod utils;

pub use account_subscriber::*;
pub use data_slice::*;
pub(super) use relation::*;
pub use snapshot_loader::*;
pub use state::*;

pub(super) const RAYDIUM_CLMM_PROGRAM_ID: Pubkey =
    pubkey!("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK");

const Q64: u128 = (u64::MAX as u128) + 1; // 2^64
const RESOLUTION: u8 = 64;

#[macro_export]
macro_rules! require_gt {
    ($value1: expr, $value2: expr, $error_code: expr $(,)?) => {
        if $value1 <= $value2 {
            return Err(anyhow::anyhow!(
                "{}: value1={}, value2={}",
                $error_code,
                $value1,
                $value2
            ));
        }
    };
    ($value1: expr, $value2: expr $(,)?) => {
        if $value1 <= $value2 {
            return Err(anyhow::anyhow!("A require_gt expression was violated"));
        }
    };
}

#[macro_export]
macro_rules! require_gte {
    ($value1: expr, $value2: expr, $error_code: expr $(,)?) => {
        if $value1 < $value2 {
            return Err(anyhow::anyhow!(
                "{}: value1={}, value2={}",
                $error_code,
                $value1,
                $value2
            ));
        }
    };
    ($value1: expr, $value2: expr $(,)?) => {
        if $value1 < $value2 {
            return Err(
                anyhow::anyhow!(anchor_lang::error::ErrorCode::RequireGteViolated)
                    .with_values(($value1, $value2)),
            );
        }
    };
}

#[macro_export]
macro_rules! require {
    // ($invariant:expr, $error:tt $(,)?) => {
    //     if !($invariant) {
    //         return Err(anyhow::anyhow!($crate::ErrorCode::$error));
    //     }
    // };
    ($invariant:expr, $error:expr $(,)?) => {
        if !($invariant) {
            return Err(anyhow::anyhow!($error));
        }
    };
}

#[cfg(test)]
mod test {
    use crate::dex::raydium_clmm::{old_state, RaydiumCLMMSnapshotInitializer};
    use crate::dex::{get_account_data, init_account_relations, init_data_slice_config, init_global_cache, init_snapshot, PoolState, SnapshotInitializer};
    use crate::dex_data::DexJson;
    use solana_rpc_client::nonblocking::rpc_client::RpcClient;
    use solana_sdk::pubkey::Pubkey;
    use std::str::FromStr;
    use std::sync::Arc;

    async fn setup(rpc_client: Arc<RpcClient>) -> anyhow::Result<Vec<DexJson>> {
        let mut dex_json = vec![DexJson {
            pool: Pubkey::from_str("6MUjnGffYaqcHeqv4nNemUQVNMpJab3W2NV9bfPj576c")?,
            owner: Pubkey::from_str("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK")?,
            mint_a: Pubkey::from_str("So11111111111111111111111111111111111111112")?,
            mint_b: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?,
            vault_a: Pubkey::from_str("2P6xseti7bdTSkECNod9myYbY92WFAg4PUwYSkQUyUoD")?,
            vault_b: Pubkey::from_str("1i3HWdz8Yphfy9jWizbSK2f6cX1em8ANU6xhHLfuzsE")?,
            address_lookup_table_address: Some(Pubkey::from_str(
                "9RDJFu8AzKituXzkciF8u7MGWRdVYKEAbjZw8X2vRFYx",
            )?),
        }];
        init_data_slice_config()?;
        let snapshot=RaydiumCLMMSnapshotInitializer;
        let global_cache = init_snapshot(&mut dex_json, rpc_client.clone()).await?;
        init_global_cache(global_cache);
        init_account_relations(dex_json.as_slice())?;
        assert!(!dex_json.is_empty());
        Ok(dex_json)
    }

    #[tokio::test]
    async fn test_snapshot_loader() -> anyhow::Result<()> {
        let rpc_client = Arc::new(RpcClient::new(
            "https://solana-rpc.publicnode.com".to_string(),
        ));
        let dex_json = setup(rpc_client.clone()).await?.pop().unwrap();
        let pool_data = rpc_client.get_account_data(&dex_json.pool).await?;
        let pool_state =
            bytemuck::from_bytes::<old_state::pool::PoolState>(&pool_data.as_slice()[8..]);
        println!("{:#?}", pool_state);
        let cache_pool_state = get_account_data::<PoolState>(&dex_json.pool).unwrap();
        println!("Cache pool state: {:#?}", &cache_pool_state);

        Ok(())
    }
}
