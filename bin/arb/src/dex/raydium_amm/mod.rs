use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;

mod account_subscriber;
mod data_slice;
pub mod instruction;
pub mod old_state;
pub mod quote;
mod relation;
mod snapshot_loader;
pub mod state;

pub use account_subscriber::*;
pub use data_slice::*;
pub(super) use relation::*;
pub use snapshot_loader::*;
pub use state::*;

pub(super) const RAYDIUM_AMM_PROGRAM_ID: Pubkey =
    pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");
const RAYDIUM_AMM_VAULT_OWNER: Pubkey = pubkey!("5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1");
const SERUM_PROGRAM_ID: Pubkey = pubkey!("opnb2LAfJYbRMAHHvqjCwQxanZn7ReEHp1k81EohpZb");

mod test {
    use crate::dex::{
        init_account_relations, init_data_slice_config, init_global_cache, init_snapshot,
        DataSliceInitializer, FromCache,
    };
    use crate::dex_data::DexJson;
    use serde::{Deserialize, Serialize};
    use solana_rpc_client::nonblocking::rpc_client::RpcClient;
    use solana_sdk::program_pack::Pack;
    use solana_sdk::pubkey::Pubkey;
    use std::io::Write;
    use std::str::FromStr;
    use std::sync::Arc;

    async fn setup(rpc_client: Arc<RpcClient>) -> anyhow::Result<Vec<DexJson>> {
        let mut dex_json = vec![DexJson {
            pool: Pubkey::from_str("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2")?,
            owner: Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8")?,
            mint_a: Pubkey::from_str("So11111111111111111111111111111111111111112")?,
            mint_b: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?,
            vault_a: Pubkey::from_str("DQyrAcCrDXQ7NeoqGgDCZwBvWDcYmFCjSb9JtteuvPpz")?,
            vault_b: Pubkey::from_str("HLmqeL62xR1QoZ1HKKbXRrdN1p3phKpxRMb2VVopvBBz")?,
            address_lookup_table_address: Some(Pubkey::from_str(
                "E59uBXGqn83xN17kMbBVfU1M7T4wHG91eiygHb88Aovb",
            )?),
        }];
        init_data_slice_config()?;
        let global_cache = init_snapshot(&mut dex_json, rpc_client.clone()).await?;
        init_global_cache(global_cache);
        init_account_relations(dex_json.as_slice())?;
        assert!(!dex_json.is_empty());
        Ok(dex_json)
    }

    pub(super) mod test_snapshot {
        use crate::dex::raydium_amm::old_state;
        use crate::dex::raydium_amm::test::setup;
        use crate::dex::{
            get_account_data, slice_data_auto_get_dex_type, update_cache, AmmInfo, MintVault,
            SliceType,
        };
        use solana_rpc_client::nonblocking::rpc_client::RpcClient;
        use solana_sdk::program_pack::Pack;
        use spl_token::state::Account;
        use std::sync::Arc;

        #[tokio::test]
        pub(super) async fn test_snapshot_load() -> anyhow::Result<()> {
            let rpc_client = Arc::new(RpcClient::new(
                "https://solana-rpc.publicnode.com".to_string(),
            ));
            let dex_json = setup(rpc_client.clone()).await?;
            let dex_json = dex_json.first().unwrap();
            let account_data = rpc_client.get_account_data(&dex_json.pool).await?;
            let mut test_amm_info =
                bytemuck::from_bytes::<old_state::pool::AmmInfo>(account_data.as_slice()).clone();
            let slice_amm_info = get_account_data::<AmmInfo>(&dex_json.pool).unwrap();
            // 池子校验
            let slice_data = slice_amm_info.swap_fee_numerator;
            let data = test_amm_info.fees.swap_fee_numerator;
            assert!(slice_data.eq(&data));
            let slice_data = slice_amm_info.swap_fee_denominator;
            let data = test_amm_info.fees.swap_fee_denominator;
            assert!(slice_data.eq(&data));
            let slice_data = slice_amm_info.coin_vault;
            let data = test_amm_info.coin_vault;
            assert!(slice_data.eq(&data));
            let slice_data = slice_amm_info.pc_vault;
            let data = test_amm_info.pc_vault;
            assert!(slice_data.eq(&data));
            let slice_data = slice_amm_info.coin_vault_mint;
            let data = test_amm_info.coin_vault_mint;
            assert!(slice_data.eq(&data));
            let slice_data = slice_amm_info.pc_vault_mint;
            let data = test_amm_info.pc_vault_mint;
            assert!(slice_data.eq(&data));
            let slice_data = slice_amm_info.need_take_pnl_coin;
            let data = test_amm_info.state_data.need_take_pnl_coin;
            assert!(slice_data.eq(&data));
            let slice_data = slice_amm_info.need_take_pnl_pc;
            let data = test_amm_info.state_data.need_take_pnl_pc;
            assert!(slice_data.eq(&data));

            // 修改池子数据，更新缓存，验证缓存更新正确
            {
                let state_data = &mut test_amm_info.state_data;
                state_data.need_take_pnl_coin = 100;
                state_data.need_take_pnl_pc = 200;
            }
            let update_pool_data = bytemuck::bytes_of(&test_amm_info);
            update_cache(
                dex_json.pool,
                slice_data_auto_get_dex_type(
                    &dex_json.pool,
                    &dex_json.owner,
                    update_pool_data.to_vec(),
                    SliceType::Subscribed,
                )?,
            )?;
            let update_amm_info = get_account_data::<AmmInfo>(&dex_json.pool).unwrap();
            let cache_data = update_amm_info.need_take_pnl_coin;
            let origin_data = test_amm_info.state_data.need_take_pnl_coin;
            assert_eq!(cache_data, origin_data);
            let cache_data = update_amm_info.need_take_pnl_pc;
            let origin_data = test_amm_info.state_data.need_take_pnl_pc;
            assert_eq!(cache_data, origin_data);

            // 金库校验
            let vault_amount = Account::unpack(
                rpc_client
                    .get_account_data(&dex_json.vault_a)
                    .await?
                    .as_slice(),
            )?
            .amount;
            let slice_vault_amount = get_account_data::<MintVault>(&dex_json.vault_a)
                .unwrap()
                .amount;
            assert!(slice_vault_amount.eq(&vault_amount));

            let mut vault_account = Account::unpack(
                rpc_client
                    .get_account_data(&dex_json.vault_b)
                    .await?
                    .as_slice(),
            )?;
            let vault_amount = vault_account.amount;
            let slice_vault_amount = get_account_data::<MintVault>(&dex_json.vault_b)
                .unwrap()
                .amount;
            assert!(slice_vault_amount.eq(&vault_amount));

            // 修改金库数据，更新缓存，验证缓存更新正确
            vault_account.amount = 1000;
            let mut account_data = [0_u8; 165];
            vault_account.pack_into_slice(&mut account_data);
            update_cache(
                dex_json.vault_b,
                slice_data_auto_get_dex_type(
                    &dex_json.vault_b,
                    &dex_json.owner,
                    account_data.to_vec(),
                    SliceType::Subscribed,
                )?,
            )?;
            let update_amount = get_account_data::<MintVault>(&dex_json.vault_b)
                .unwrap()
                .amount;
            assert!(update_amount.eq(&vault_account.amount));
            Ok(())
        }
    }

    mod test_quote {
        use crate::dex::raydium_amm::test::test_snapshot::test_snapshot_load;

        #[test]
        fn test_quote() {
            test_snapshot_load().unwrap();
        }
    }
}
