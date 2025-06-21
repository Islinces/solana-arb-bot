use crate::dex::global_cache::get_account_data;
use crate::dex::pump_fun::PUMP_FUN_AMM_PROGRAM_ID;
use crate::dex::raydium_amm::state::AmmInfo;
use crate::dex::raydium_amm::RAYDIUM_AMM_PROGRAM_ID;
use crate::dex::snapshot::{AccountDataSlice, SnapshotInitializer};
use crate::dex::{AccountType, DexType, MintVault};
use crate::dex_data::DexJson;
use ahash::{AHashMap, AHashSet};
use anyhow::anyhow;
use async_trait::async_trait;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tracing::info;

pub struct RaydiumAmmSnapshotInitializer;

impl RaydiumAmmSnapshotInitializer {
    async fn get_pool_snapshot_data(
        &self,
        pool_accounts: Vec<Pubkey>,
        rpc_client: Arc<RpcClient>,
    ) -> Vec<AccountDataSlice> {
        let mut all_pool_account_data = self
            .get_account_data_with_data_slice(
                pool_accounts,
                DexType::RaydiumAMM,
                AccountType::Pool,
                rpc_client.clone(),
            )
            .await;
        all_pool_account_data.retain(|account| {
            account.static_slice_data.as_ref().is_some()
                && account.dynamic_slice_data.as_ref().is_some()
        });
        all_pool_account_data
    }

    async fn get_mint_vault_snapshot_data(
        &self,
        vault_accounts: Vec<Pubkey>,
        rpc_client: Arc<RpcClient>,
    ) -> Vec<AccountDataSlice> {
        let mut all_vault_account_data = self
            .get_account_data_with_data_slice(
                vault_accounts,
                DexType::RaydiumAMM,
                AccountType::MintVault,
                rpc_client,
            )
            .await;
        all_vault_account_data.retain(|account| account.dynamic_slice_data.as_ref().is_some());
        all_vault_account_data
    }
}

#[async_trait]
impl SnapshotInitializer for RaydiumAmmSnapshotInitializer {
    async fn init_snapshot(
        &self,
        dex_json: &mut Vec<DexJson>,
        rpc_client: Arc<RpcClient>,
    ) -> Vec<AccountDataSlice> {
        let dex_data = dex_json
            .iter()
            .filter(|json| json.owner == RAYDIUM_AMM_PROGRAM_ID)
            .collect::<Vec<_>>();
        if dex_data.is_empty() {
            return vec![];
        }
        info!("【RaydiumAMM】开始初始化Snapshot...");
        let mut pool_accounts = AHashSet::with_capacity(dex_data.len());
        let mut vault_accounts = AHashSet::with_capacity(dex_data.len() * 2);
        for json in dex_data.iter() {
            pool_accounts.insert(json.pool);
            vault_accounts.insert(json.vault_a);
            vault_accounts.insert(json.vault_b);
        }
        let mut all_pool_account_data = self
            .get_pool_snapshot_data(
                pool_accounts.into_iter().collect::<Vec<_>>(),
                rpc_client.clone(),
            )
            .await;
        let all_vault_account_data = self
            .get_mint_vault_snapshot_data(
                vault_accounts.into_iter().collect::<Vec<_>>(),
                rpc_client.clone(),
            )
            .await;
        info!(
            "【RaydiumAmm】初始化Snapshot完毕, 初始化池子数量 : {}",
            all_pool_account_data.len()
        );
        all_pool_account_data
            .into_iter()
            .chain(all_vault_account_data.into_iter())
            .collect()
    }

    fn print_snapshot(&self, dex_json: &[DexJson]) -> anyhow::Result<()> {
        if let Some(json) = dex_json
            .iter()
            .find(|json| &json.owner == DexType::RaydiumAMM.get_ref_program_id())
        {
            let pool = get_account_data::<AmmInfo>(&json.pool)
                .ok_or(anyhow!("{}找不到缓存数据", json.pool))?;
            info!(
                "【{}】【{:?}】, key : {:?}\ndata : {:#?}",
                DexType::RaydiumAMM,
                AccountType::Pool,
                json.pool,
                pool
            );
            let coin_vault = get_account_data::<MintVault>(&pool.coin_vault)
                .ok_or(anyhow!("{}找不到缓存数据", pool.coin_vault))?;
            info!(
                "【{}】【{:?}】, key : {:?}\ndata : {:#?}",
                DexType::RaydiumAMM,
                AccountType::MintVault,
                pool.coin_vault,
                coin_vault
            );
            let pc_vault = get_account_data::<MintVault>(&pool.pc_vault)
                .ok_or(anyhow!("{}找不到缓存数据", pool.pc_vault))?;
            info!(
                "【{}】【{:?}】, key : {:?}\ndata : {:#?}",
                DexType::RaydiumAMM,
                AccountType::MintVault,
                pool.pc_vault,
                pc_vault
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::dex::raydium_amm::{
        old_state, RaydiumAmmSnapshotInitializer, RAYDIUM_AMM_VAULT_OWNER,
    };
    use crate::dex::{init_data_slice_config, AmmInfo, FromCache, MintVault, SnapshotInitializer};
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
    async fn test_raydium_amm_snapshot_load() -> anyhow::Result<()> {
        init_data_slice_config()?;
        let dex_json = DexJson {
            pool: Pubkey::from_str("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2")?,
            owner: Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8")?,
            mint_a: Pubkey::from_str("So11111111111111111111111111111111111111112")?,
            mint_b: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?,
            vault_a: Pubkey::from_str("DQyrAcCrDXQ7NeoqGgDCZwBvWDcYmFCjSb9JtteuvPpz")?,
            vault_b: Pubkey::from_str("HLmqeL62xR1QoZ1HKKbXRrdN1p3phKpxRMb2VVopvBBz")?,
            address_lookup_table_address: Some(Pubkey::from_str(
                "E59uBXGqn83xN17kMbBVfU1M7T4wHG91eiygHb88Aovb",
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
            owner: RAYDIUM_AMM_VAULT_OWNER,
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
                    "owner": dex_json.owner.to_string(),
                    "executable": false,
                    "rentEpoch": 1
                }
                ]
            }),
        );
        let rpc_client = Arc::new(RpcClient::new_mock_with_mocks("success".to_string(), mocks));
        let mut snapshot_data = RaydiumAmmSnapshotInitializer
            .get_mint_vault_snapshot_data(vec![dex_json.vault_a], rpc_client.clone())
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
        let amm_info = old_state::pool::AmmInfo {
            fees: old_state::pool::Fees {
                swap_fee_numerator: 100,
                swap_fee_denominator: 1000,
                ..Default::default()
            },
            state_data: old_state::pool::StateData {
                need_take_pnl_coin: 10,
                need_take_pnl_pc: 20,
                ..Default::default()
            },
            coin_vault: dex_json.vault_a,
            pc_vault: dex_json.vault_b,
            coin_vault_mint: dex_json.mint_a,
            pc_vault_mint: dex_json.mint_b,
            ..Default::default()
        };
        let amm_info_data = bytemuck::bytes_of(&amm_info);
        mocks.insert(
            RpcRequest::GetMultipleAccounts,
            json!({
                "context": { "slot": 1 },
                "value": [
                    {
                    "lamports": 0,
                    "data": [base64::encode(amm_info_data), "base64"],
                    "owner": dex_json.owner.to_string(),
                    "executable": false,
                    "rentEpoch": 1
                }
                ]
            }),
        );
        let rpc_client = Arc::new(RpcClient::new_mock_with_mocks("failed".to_string(), mocks));
        let mut pool_snapshot_data = RaydiumAmmSnapshotInitializer
            .get_pool_snapshot_data(vec![dex_json.pool], rpc_client.clone())
            .await;
        assert_eq!(pool_snapshot_data.len(), 1);
        let data_slice = pool_snapshot_data.pop().unwrap();
        assert_eq!(data_slice.account_key, dex_json.pool);
        assert!(data_slice.static_slice_data.is_some());
        assert!(data_slice.dynamic_slice_data.is_some());
        let slice_amm_info = AmmInfo::from_cache(
            Some(Arc::new(data_slice.static_slice_data.unwrap())),
            Some(Arc::new(data_slice.dynamic_slice_data.unwrap())),
        )?;
        let slice_data = slice_amm_info.need_take_pnl_coin;
        let origin_data = amm_info.state_data.need_take_pnl_coin;
        assert_eq!(slice_data, origin_data);
        let slice_data = slice_amm_info.need_take_pnl_pc;
        let origin_data = amm_info.state_data.need_take_pnl_pc;
        assert_eq!(slice_data, origin_data);
        let slice_data = slice_amm_info.swap_fee_numerator;
        let origin_data = amm_info.fees.swap_fee_numerator;
        assert_eq!(slice_data, origin_data);
        let slice_data = slice_amm_info.swap_fee_denominator;
        let origin_data = amm_info.fees.swap_fee_denominator;
        assert_eq!(slice_data, origin_data);
        let slice_data = slice_amm_info.coin_vault;
        let origin_data = amm_info.coin_vault;
        assert_eq!(slice_data, origin_data);
        let slice_data = slice_amm_info.pc_vault;
        let origin_data = amm_info.pc_vault;
        assert_eq!(slice_data, origin_data);
        let slice_data = slice_amm_info.coin_vault_mint;
        let origin_data = amm_info.coin_vault_mint;
        assert_eq!(slice_data, origin_data);
        let slice_data = slice_amm_info.pc_vault_mint;
        let origin_data = amm_info.pc_vault_mint;
        assert_eq!(slice_data, origin_data);
        Ok(())
    }

    #[tokio::test]
    async fn test_raydium_amm_snapshot_load_error_owner() -> anyhow::Result<()> {
        let mut dex_json = vec![DexJson {
            pool: Pubkey::from_str("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2")?,
            owner: Pubkey::from_str("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA")?,
            mint_a: Pubkey::from_str("So11111111111111111111111111111111111111112")?,
            mint_b: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?,
            vault_a: Pubkey::from_str("DQyrAcCrDXQ7NeoqGgDCZwBvWDcYmFCjSb9JtteuvPpz")?,
            vault_b: Pubkey::from_str("HLmqeL62xR1QoZ1HKKbXRrdN1p3phKpxRMb2VVopvBBz")?,
            address_lookup_table_address: Some(Pubkey::from_str(
                "E59uBXGqn83xN17kMbBVfU1M7T4wHG91eiygHb88Aovb",
            )?),
        }];
        let data = RaydiumAmmSnapshotInitializer
            .init_snapshot(
                &mut dex_json,
                Arc::new(RpcClient::new_mock("aa".to_string())),
            )
            .await;
        assert_eq!(data.len(), 0);
        Ok(())
    }
}
