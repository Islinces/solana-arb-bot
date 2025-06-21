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

use rand::Rng;
use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

pub(super) const PUMP_FUN_AMM_PROGRAM_ID: Pubkey =
    pubkey!("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA");

/// pump fun fee 钱包列表，随机取一个
const PUMPSWAP_FEE_ACCOUNTS: [&str; 8] = [
    "AVmoTthdrX6tKt4nDjco2D775W2YK3sDhxPcMmzUAmTY",
    "7hTckgnGnLQR6sdH7YkqFTAA7VwTfYFaZ6EhEsU3saCX",
    "62qc2CNXwrYqQScmEdiZFFAnJR262PxWEuNQtxfafNgV",
    "G5UZAVbAf46s7cKWoyKu8kYTip9DGTpbLZ2qa9Aq69dP",
    "9rPYyANsfQZw3DnDmKE3YCQF5E8oD89UXoHn9JFEhJUz",
    "JCRGumoE9Qi5BBgULTgdgTLjSgkCMSbF62ZZfGs84JeU",
    "7VtfL8fvgNfhz17qKRMjzQEXgbdpnHHHQRh54R9jP2RJ",
    "FWsW1xNtWscwNmKv6wVsU1iTzRN6wmmk3MjxRP5tT7hz",
];

fn get_fee_account_with_rand() -> Pubkey {
    Pubkey::from_str(PUMPSWAP_FEE_ACCOUNTS[rand::rng().random_range(0..=7)]).unwrap()
}

mod test {
    use crate::dex::{init_account_relations, init_data_slice_config, init_global_cache, init_snapshot, DataSliceInitializer, FromCache};
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
            pool: Pubkey::from_str("Gf7sXMoP8iRw4iiXmJ1nq4vxcRycbGXy5RL8a8LnTd3v")?,
            owner: Pubkey::from_str("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA")?,
            mint_a: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?,
            mint_b: Pubkey::from_str("So11111111111111111111111111111111111111112")?,
            vault_a: Pubkey::from_str("nML7msD1MiJHxFvhv4po1u6C4KpWr64ugKqc75DMuD2")?,
            vault_b: Pubkey::from_str("EjHirXt2bQd2DDNveagHHCWYzUwtY1iwNbBrV5j84e6j")?,
            address_lookup_table_address: Some(Pubkey::from_str(
                "7KgsCbuJAXxELXhpzc9PwX7GenoF3UsuuW71qW1Gr3u9",
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
        use crate::dex::pump_fun::old_state;
        use crate::dex::pump_fun::old_state::global_config;
        use crate::dex::pump_fun::test::setup;
        use crate::dex::{get_account_data, global_config_key, slice_data_auto_get_dex_type, update_cache, MintVault, Pool, SliceType};
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
            let test_pool =
                bytemuck::from_bytes::<old_state::pool::Pool>(&account_data.as_slice()[8..243]);
            println!("{:#?}", test_pool);
            let slice_pool = get_account_data::<Pool>(&dex_json.pool).unwrap();
            assert_eq!(slice_pool.base_mint, test_pool.base_mint);
            assert_eq!(slice_pool.quote_mint, test_pool.quote_mint);
            assert_eq!(
                slice_pool.pool_base_token_account,
                test_pool.pool_base_token_account
            );
            assert_eq!(
                slice_pool.pool_quote_token_account,
                test_pool.pool_quote_token_account
            );
            assert_eq!(slice_pool.coin_creator, test_pool.coin_creator);
            let global_config_key = global_config_key();
            println!("{:#?}", global_config_key);
            let global_config_data = rpc_client.get_account_data(&global_config_key).await?;
            let global_config = bytemuck::from_bytes::<global_config::GlobalConfig>(
                &global_config_data.as_slice()[8..321],
            );
            println!("{:#?}", global_config);
            let origin_data = global_config.lp_fee_basis_points;
            let slice_data = slice_pool.lp_fee_basis_points;
            assert_eq!(slice_data, origin_data);
            let origin_data = global_config.protocol_fee_basis_points;
            let slice_data = slice_pool.protocol_fee_basis_points;
            assert_eq!(slice_data, origin_data);
            // 金库校验
            let mut vault_account = Account::unpack(
                rpc_client
                    .get_account_data(&dex_json.vault_a)
                    .await?
                    .as_slice(),
            )?;
            let vault_amount = vault_account.amount;
            let slice_vault_amount = get_account_data::<MintVault>(&dex_json.vault_a)
                .unwrap()
                .amount;
            assert!(slice_vault_amount.eq(&vault_amount));
            let vault_amount = Account::unpack(
                rpc_client
                    .get_account_data(&dex_json.vault_b)
                    .await?
                    .as_slice(),
            )?
            .amount;
            let slice_vault_amount = get_account_data::<MintVault>(&dex_json.vault_b)
                .unwrap()
                .amount;
            assert!(slice_vault_amount.eq(&vault_amount));
            // 修改数据，更新缓存，再读取缓存验证是否相等
            vault_account.amount = 10000000;
            let mut vault_account_slice = [0; 165];
            vault_account.pack_into_slice(vault_account_slice.as_mut_slice());
            update_cache(dex_json.vault_a, slice_data_auto_get_dex_type(&dex_json.vault_a, &dex_json.owner, vault_account_slice.to_vec(), SliceType::Subscribed)?)?;
            let slice_vault_amount = get_account_data::<MintVault>(&dex_json.vault_a)
                .unwrap()
                .amount;
            assert!(slice_vault_amount.eq(&vault_account.amount));
            Ok(())
        }
    }

    mod test_quote {
        #[test]
        fn test_quote() {}
    }
}
