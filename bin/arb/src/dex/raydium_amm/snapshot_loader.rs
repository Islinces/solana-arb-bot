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
use std::sync::Arc;
use tracing::info;

pub struct RaydiumAmmSnapshotInitializer;

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
        let mut invalid_pool = AHashSet::with_capacity(dex_data.len());
        let mut pool_accounts = Vec::with_capacity(dex_data.len());
        let mut vault_to_pool = AHashMap::with_capacity(dex_data.len() * 2);
        for json in dex_data.iter() {
            pool_accounts.push(json.pool);
            vault_to_pool.insert(json.vault_a, json.pool);
            vault_to_pool.insert(json.vault_b, json.pool);
        }
        let mut all_pool_account_data = self
            .get_account_data_with_data_slice(
                pool_accounts,
                DexType::RaydiumAMM,
                AccountType::Pool,
                rpc_client.clone(),
            )
            .await;
        all_pool_account_data.retain(|account| {
            if account.static_slice_data.as_ref().is_none()
                || account.dynamic_slice_data.as_ref().is_none()
            {
                invalid_pool.insert(account.account_key);
                false
            } else {
                true
            }
        });
        let all_vault_accounts = all_pool_account_data
            .iter()
            .map(|account| {
                let option = dex_data
                    .iter()
                    .find(|a| a.pool == account.account_key)
                    .unwrap();
                vec![option.vault_a, option.vault_b]
            })
            .flatten()
            .collect::<Vec<_>>();
        let mut all_vault_account_data = self
            .get_account_data_with_data_slice(
                all_vault_accounts,
                DexType::RaydiumAMM,
                AccountType::MintVault,
                rpc_client,
            )
            .await;
        all_vault_account_data.retain(|account| {
            if account.dynamic_slice_data.as_ref().is_none() {
                invalid_pool.insert(vault_to_pool.get(&account.account_key).unwrap().clone());
                false
            } else {
                true
            }
        });
        dex_json.retain(|json| !invalid_pool.contains(&json.pool));
        info!(
            "【RaydiumAmm】初始化Snapshot完毕, 初始化池子数量 : {}",
            all_pool_account_data.len()
        );
        if dex_json.is_empty() {
            vec![]
        } else {
            all_pool_account_data
                .into_iter()
                .chain(all_vault_account_data.into_iter())
                .collect()
        }
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

mod test {
    use crate::dex::raydium_amm::{old_state, RaydiumAMMDataSlicer};
    use crate::dex::{AccountType, AmmInfo, DataSliceInitializer, FromCache, MintVault, SliceType};
    use serde::{Deserialize, Serialize};
    use solana_rpc_client::rpc_client::RpcClient;
    use solana_sdk::program_pack::Pack;
    use solana_sdk::pubkey::Pubkey;
    use spl_token::state::Account;
    use std::fs::File;
    use std::io::Write;
    use std::str::FromStr;
    use std::sync::Arc;

    #[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
    struct TestData {
        pool_id: Pubkey,
        amm_info: old_state::pool::AmmInfo,
        vault_a: Pubkey,
        vault_a_data: Vec<u8>,
        vault_b: Pubkey,
        vault_b_data: Vec<u8>,
    }
    const FILE_PATH: &str = "test_data/raydium_amm_test_data.json";

    // #[test]
    fn test_init_test_data() -> anyhow::Result<()> {
        let pool_id = Pubkey::from_str("9ViX1VductEoC2wERTSp2TuDxXPwAf69aeET8ENPJpsN")?;
        let vault_a = Pubkey::from_str("9jbyBXHinaAah2SthksJTYGzTQNRLA7HdT2A7VMF91Wu")?;
        let vault_b = Pubkey::from_str("9v9FpQYd46LS9zHJitTtnPDDQrHfkSdW2PRbbEbKd2gw")?;
        let rpc_client = RpcClient::new("https://solana-rpc.publicnode.com");
        let account_data = rpc_client.get_account_data(&pool_id)?;
        let amm_info =
            bytemuck::from_bytes::<old_state::pool::AmmInfo>(account_data.as_slice()).clone();
        let vault_a_data = rpc_client.get_account_data(&vault_a)?;
        let vault_b_data = rpc_client.get_account_data(&vault_b)?;
        let test_data = TestData {
            pool_id,
            amm_info,
            vault_a,
            vault_a_data,
            vault_b,
            vault_b_data,
        };
        // println!("{:#?}", test_data);
        let mut file = File::create(FILE_PATH)?;
        let json_string = serde_json::to_string_pretty(&test_data)?;
        file.write_all(json_string.as_bytes())?;
        let from_file = serde_json::from_reader::<File, TestData>(File::open(FILE_PATH)?)?;
        assert!(from_file.eq(&test_data));
        Ok(())
    }

    #[test]
    fn test_snapshot_load() -> anyhow::Result<()> {
        let test_data = serde_json::from_reader::<File, TestData>(File::open(FILE_PATH)?)?;
        let test_amm_info = test_data.amm_info;
        let data_slicer = RaydiumAMMDataSlicer;
        data_slicer.try_init_data_slice_config()?;
        let amm_info_bytes = bytemuck::bytes_of(&test_amm_info);
        let amm_info_dynamic_data = data_slicer.try_slice_data(
            AccountType::Pool,
            amm_info_bytes.to_vec(),
            SliceType::Subscribed,
        )?;
        let amm_info_static_data = data_slicer.try_slice_data(
            AccountType::Pool,
            amm_info_bytes.to_vec(),
            SliceType::Unsubscribed,
        )?;
        let slice_amm_info = AmmInfo::from_cache(
            Some(Arc::new(amm_info_static_data)),
            Some(Arc::new(amm_info_dynamic_data)),
        )?;
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
        // 金库校验
        let vault_data = test_data.vault_a_data;
        let vault_amount = Account::unpack(vault_data.as_slice())?.amount;
        let vault_bytes = data_slicer.try_slice_data(
            AccountType::MintVault,
            vault_data,
            SliceType::Subscribed,
        )?;
        let slice_vault_amount = MintVault::from_cache(None, Some(Arc::new(vault_bytes)))?.amount;
        assert!(slice_vault_amount.eq(&vault_amount));

        let vault_data = test_data.vault_b_data;
        let vault_amount = Account::unpack(vault_data.as_slice())?.amount;
        let vault_bytes = data_slicer.try_slice_data(
            AccountType::MintVault,
            vault_data,
            SliceType::Subscribed,
        )?;
        let slice_vault_amount = MintVault::from_cache(None, Some(Arc::new(vault_bytes)))?.amount;
        assert!(slice_vault_amount.eq(&vault_amount));

        Ok(())
    }
}
