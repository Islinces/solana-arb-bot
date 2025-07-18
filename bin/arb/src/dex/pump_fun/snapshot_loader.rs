use crate::dex::global_cache::get_account_data;
use crate::dex::orca_whirlpools::WHIRLPOOL_ID;
use crate::dex::pump_fun::state::Pool;
use crate::dex::pump_fun::PUMP_FUN_AMM_PROGRAM_ID;
use crate::dex::raydium_amm::state::AmmInfo;
use crate::dex::snapshot::{AccountDataSlice, SnapshotInitializer};
use crate::dex::utils::read_from;
use crate::dex::{AccountType, DexType, ATA_PROGRAM_ID, MINT_PROGRAM_ID, SYSTEM_PROGRAM_ID};
use crate::dex_data::DexJson;
use ahash::{AHashMap, AHashSet};
use anyhow::anyhow;
use async_trait::async_trait;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tracing::info;

pub struct PumpFunAMMSnapshotInitializer;

#[async_trait]
impl SnapshotInitializer for PumpFunAMMSnapshotInitializer {
    async fn init_snapshot(
        &self,
        dex_json: &mut Vec<DexJson>,
        rpc_client: Arc<RpcClient>,
    ) -> Vec<AccountDataSlice> {
        let dex_data = dex_json
            .iter()
            .filter(|json| json.owner == PUMP_FUN_AMM_PROGRAM_ID)
            .map(|json| json.clone())
            .collect::<Vec<_>>();
        if dex_data.is_empty() {
            return vec![];
        }
        info!("【PumpFunAMM】开始初始化Snapshot...");

        let mut pool_accounts = AHashSet::with_capacity(dex_data.len());
        let mut vault_accounts = AHashSet::with_capacity(dex_data.len() * 2);
        for json in dex_data.iter() {
            pool_accounts.insert(json.pool);
            vault_accounts.insert(json.vault_a);
            vault_accounts.insert(json.vault_b);
        }
        // global config
        let global_config_account = self.get_global_config_account(rpc_client.clone()).await;
        if global_config_account.as_ref().is_none() {
            dex_json.retain(|json| json.owner != PUMP_FUN_AMM_PROGRAM_ID);
            return vec![];
        }
        let all_pool_account_data = self
            .get_pool_snapshot(
                pool_accounts.into_iter().collect(),
                global_config_account.unwrap(),
                rpc_client.clone(),
            )
            .await;
        if all_pool_account_data.is_empty() {
            dex_json.retain(|json| json.owner != PUMP_FUN_AMM_PROGRAM_ID);
            return vec![];
        }
        let all_mint_vault_data = self
            .get_mint_vault_snapshot(vault_accounts.into_iter().collect(), rpc_client.clone())
            .await;
        info!(
            "【PumpFunAMM】初始化Snapshot完毕, 初始化池子数量 : {}",
            dex_data.len()
        );
        all_pool_account_data
            .into_iter()
            .chain(all_mint_vault_data.into_iter())
            .collect::<Vec<AccountDataSlice>>()
    }

    fn print_snapshot(&self, dex_json: &[DexJson]) -> anyhow::Result<()> {
        if let Some(json) = dex_json
            .iter()
            .find(|json| &json.owner == DexType::PumpFunAMM.get_ref_program_id())
        {
            let pool = get_account_data::<Pool>(&json.pool)
                .ok_or(anyhow!("{}找不到缓存数据", json.pool))?;
            info!(
                "【{}】【{:?}】, key : {:?}\ndata : {:#?}",
                DexType::PumpFunAMM,
                AccountType::Pool,
                json.pool,
                pool
            );
        }
        Ok(())
    }
}

impl PumpFunAMMSnapshotInitializer {
    async fn get_global_config_account(
        &self,
        rpc_client: Arc<RpcClient>,
    ) -> Option<AccountDataSlice> {
        let global_config_account = self
            .get_account_data_with_data_slice(
                vec![crate::dex::pump_fun::state::global_config_key()],
                DexType::PumpFunAMM,
                AccountType::PumpFunGlobalConfig,
                rpc_client.clone(),
            )
            .await;
        let g = global_config_account.first();
        if g.is_none_or(|v| v.static_slice_data.is_none()) {
            None
        } else {
            g.cloned()
        }
    }

    async fn get_pool_snapshot(
        &self,
        pool_keys: Vec<Pubkey>,
        global_config_account_data: AccountDataSlice,
        rpc_client: Arc<RpcClient>,
    ) -> Vec<AccountDataSlice> {
        // lp_fee_basis_points 和 protocol_fee_basis_points
        let global_config_account_data = global_config_account_data.static_slice_data.unwrap();
        let mut all_pool_account_data = self
            .get_account_data_with_data_slice(
                pool_keys,
                DexType::PumpFunAMM,
                AccountType::Pool,
                rpc_client.clone(),
            )
            .await;
        all_pool_account_data.retain(|account| account.static_slice_data.is_some());
        for account in all_pool_account_data.iter_mut() {
            let pool_static_data = account.static_slice_data.as_ref().unwrap();
            // 池子未订阅，global config为订阅
            // 将两个合并在一起
            let mut combine_data = Vec::with_capacity(
                pool_static_data.len() + global_config_account_data.len() + 32 * 2,
            );
            // 先提前生成coin_creator_vault_authority和coin_creator_vault_ata
            let quote_mint = Pubkey::try_from(&pool_static_data[32..32 + 32]).unwrap();
            let coin_creator = Pubkey::try_from(&pool_static_data[32 * 4..32 * 5]).unwrap();
            let token_program = if quote_mint == spl_token::native_mint::ID {
                spl_token::ID
            } else {
                rpc_client
                    .clone()
                    .get_account(&quote_mint)
                    .await
                    .unwrap()
                    .owner
            };
            let (coin_creator_vault_authority, _) = Pubkey::find_program_address(
                &[b"creator_vault", coin_creator.to_bytes().as_ref()],
                DexType::PumpFunAMM.get_ref_program_id(),
            );
            let (coin_creator_vault_ata, _) = Pubkey::find_program_address(
                &[
                    coin_creator_vault_authority.to_bytes().as_ref(),
                    token_program.to_bytes().as_ref(),
                    quote_mint.to_bytes().as_ref(),
                ],
                &ATA_PROGRAM_ID,
            );
            combine_data.extend(pool_static_data);
            combine_data.extend(global_config_account_data.clone());
            combine_data.extend(coin_creator_vault_authority.to_bytes());
            combine_data.extend(coin_creator_vault_ata.to_bytes());
            account.static_slice_data.replace(combine_data);
        }
        all_pool_account_data
    }

    async fn get_mint_vault_snapshot(
        &self,
        mint_vault_keys: Vec<Pubkey>,
        rpc_client: Arc<RpcClient>,
    ) -> Vec<AccountDataSlice> {
        let mut all_vault_account_data = self
            .get_account_data_with_data_slice(
                mint_vault_keys,
                DexType::PumpFunAMM,
                AccountType::MintVault,
                rpc_client,
            )
            .await;
        all_vault_account_data.retain(|account| account.dynamic_slice_data.is_some());
        all_vault_account_data
    }
}

#[cfg(test)]
mod test {
    use crate::dex::pump_fun::old_state::global_config::GlobalConfig;
    use crate::dex::pump_fun::{old_state, PumpFunAMMDataSlicer, PumpFunAMMSnapshotInitializer};
    use crate::dex::{
        init_data_slice_config, AccountType, AmmInfo, DataSliceInitializer, FromCache, MintVault,
        Pool, SliceType, SnapshotInitializer,
    };
    use crate::dex_data::DexJson;
    use serde_json::json;
    use solana_rpc_client::nonblocking::rpc_client::RpcClient;
    use solana_rpc_client::rpc_client::Mocks;
    use solana_rpc_client_api::request::RpcRequest;
    use solana_sdk::program_pack::Pack;
    use solana_sdk::pubkey::Pubkey;
    use spl_token::state::Account;
    use std::str::FromStr;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_pumpfun_snapshot_load() -> anyhow::Result<()> {
        init_data_slice_config()?;
        let dex_json = DexJson {
            pool: Pubkey::from_str("Gf7sXMoP8iRw4iiXmJ1nq4vxcRycbGXy5RL8a8LnTd3v")?,
            owner: Pubkey::from_str("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA")?,
            mint_a: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?,
            mint_b: Pubkey::from_str("So11111111111111111111111111111111111111112")?,
            vault_a: Pubkey::from_str("nML7msD1MiJHxFvhv4po1u6C4KpWr64ugKqc75DMuD2")?,
            vault_b: Pubkey::from_str("EjHirXt2bQd2DDNveagHHCWYzUwtY1iwNbBrV5j84e6j")?,
            address_lookup_table_address: Some(Pubkey::from_str(
                "7KgsCbuJAXxELXhpzc9PwX7GenoF3UsuuW71qW1Gr3u9",
            )?),
        };
        test_pool(&dex_json).await?;
        test_mint_vault(&dex_json).await?;
        Ok(())
    }

    async fn test_mint_vault(dex_json: &DexJson) -> anyhow::Result<()> {
        let mut mocks = Mocks::new();
        let mint_vault = Account {
            mint: dex_json.mint_a,
            owner: dex_json.pool,
            amount: 1234567890,
            ..Default::default()
        };
        let mut data = [0_u8; 165];
        mint_vault.pack_into_slice(&mut data);
        mocks.insert(
            RpcRequest::GetMultipleAccounts,
            json!({
                "context": { "slot": 1 },
                "value": [
                    {
                    "lamports": 0,
                    "data": [base64::encode(data), "base64"],
                    "owner": dex_json.pool.to_string(),
                    "executable": false,
                    "rentEpoch": 1
                }
                ]
            }),
        );
        let rpc_client = Arc::new(RpcClient::new_mock_with_mocks("success".to_string(), mocks));
        let mut snapshot_data = PumpFunAMMSnapshotInitializer
            .get_mint_vault_snapshot(vec![dex_json.vault_a], rpc_client.clone())
            .await;
        assert_eq!(snapshot_data.len(), 1);
        let data_slice = snapshot_data.pop().unwrap();
        assert_eq!(data_slice.account_key, dex_json.vault_a);
        assert!(data_slice.static_slice_data.is_none());
        assert!(data_slice.dynamic_slice_data.is_some());
        let amount =
            MintVault::from_cache(None, Some(Arc::new(data_slice.dynamic_slice_data.unwrap())))?
                .amount;
        assert_eq!(amount, mint_vault.amount);
        Ok(())
    }

    async fn test_pool(dex_json: &DexJson) -> anyhow::Result<()> {
        let mut mocks = Mocks::new();
        let global_config = GlobalConfig {
            lp_fee_basis_points: 25,
            protocol_fee_basis_points: 10_000,
            ..Default::default()
        };
        let global_config_data = bytemuck::bytes_of(&global_config);
        mocks.insert(
            RpcRequest::GetMultipleAccounts,
            json!({
                "context": { "slot": 1 },
                "value": [
                    {
                    "lamports": 0,
                    "data": [base64::encode(global_config_data), "base64"],
                    "owner": dex_json.owner.to_string(),
                    "executable": false,
                    "rentEpoch": 1
                }
                ]
            }),
        );
        let global_config_data = PumpFunAMMSnapshotInitializer
            .get_global_config_account(Arc::new(RpcClient::new_mock_with_mocks(
                "success".to_string(),
                mocks,
            )))
            .await;
        assert!(global_config_data.is_some());
        assert!(global_config_data
            .as_ref()
            .unwrap()
            .static_slice_data
            .is_some());
        assert_eq!(
            global_config_data
                .as_ref()
                .unwrap()
                .static_slice_data
                .clone()
                .unwrap()
                .len(),
            PumpFunAMMDataSlicer
                .try_get_data_slice_size(AccountType::PumpFunGlobalConfig, SliceType::Unsubscribed)?
                .unwrap()
        );

        let mut mocks = Mocks::new();
        let pool = old_state::pool::Pool {
            base_mint: dex_json.mint_a,
            quote_mint: dex_json.mint_b,
            pool_base_token_account: dex_json.vault_a,
            pool_quote_token_account: dex_json.vault_b,
            coin_creator: Pubkey::from_str("11111111111111111111111111111111")?,
            ..Default::default()
        };
        let pool_data = bytemuck::bytes_of(&pool);
        mocks.insert(
            RpcRequest::GetMultipleAccounts,
            json!({
                "context": { "slot": 1 },
                "value": [
                    {
                    "lamports": 0,
                    "data": [base64::encode(pool_data), "base64"],
                    "owner": dex_json.owner.to_string(),
                    "executable": false,
                    "rentEpoch": 1
                }
                ]
            }),
        );
        let mut pool_snapshot_data = PumpFunAMMSnapshotInitializer
            .get_pool_snapshot(
                vec![dex_json.pool],
                global_config_data.unwrap(),
                Arc::new(RpcClient::new_mock_with_mocks("success".to_string(), mocks)),
            )
            .await;
        assert_eq!(pool_snapshot_data.len(), 1);
        let data_slice = pool_snapshot_data.pop().unwrap();
        assert_eq!(data_slice.account_key, dex_json.pool);
        assert!(data_slice.static_slice_data.is_some());
        assert!(data_slice.dynamic_slice_data.is_none());
        let slice_pool =
            Pool::from_cache(Some(Arc::new(data_slice.static_slice_data.unwrap())), None)?;
        let slice_data = slice_pool.base_mint;
        let origin_data = pool.base_mint;
        assert_eq!(slice_data, origin_data);
        let slice_data = slice_pool.quote_mint;
        let origin_data = pool.quote_mint;
        assert_eq!(slice_data, origin_data);
        let slice_data = slice_pool.pool_base_token_account;
        let origin_data = pool.pool_base_token_account;
        assert_eq!(slice_data, origin_data);
        let slice_data = slice_pool.pool_quote_token_account;
        let origin_data = pool.pool_quote_token_account;
        assert_eq!(slice_data, origin_data);
        let slice_data = slice_pool.coin_creator;
        let origin_data = pool.coin_creator;
        assert_eq!(slice_data, origin_data);

        let slice_data = slice_pool.lp_fee_basis_points;
        let origin_data = global_config.lp_fee_basis_points;
        assert_eq!(slice_data, origin_data);

        let slice_data = slice_pool.protocol_fee_basis_points;
        let origin_data = global_config.protocol_fee_basis_points;
        assert_eq!(slice_data, origin_data);
        Ok(())
    }

    #[tokio::test]
    async fn test_pumpfun_snapshot_load_error_owner() -> anyhow::Result<()> {
        let mut dex_json = vec![DexJson {
            pool: Pubkey::from_str("Gf7sXMoP8iRw4iiXmJ1nq4vxcRycbGXy5RL8a8LnTd3v")?,
            owner: Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8")?,
            mint_a: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?,
            mint_b: Pubkey::from_str("So11111111111111111111111111111111111111112")?,
            vault_a: Pubkey::from_str("nML7msD1MiJHxFvhv4po1u6C4KpWr64ugKqc75DMuD2")?,
            vault_b: Pubkey::from_str("EjHirXt2bQd2DDNveagHHCWYzUwtY1iwNbBrV5j84e6j")?,
            address_lookup_table_address: Some(Pubkey::from_str(
                "7KgsCbuJAXxELXhpzc9PwX7GenoF3UsuuW71qW1Gr3u9",
            )?),
        }];
        let data = PumpFunAMMSnapshotInitializer
            .init_snapshot(
                &mut dex_json,
                Arc::new(RpcClient::new_mock("success".to_string())),
            )
            .await;
        assert_eq!(data.len(), 0);
        Ok(())
    }
}
