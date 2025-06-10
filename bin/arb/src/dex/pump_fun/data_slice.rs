use crate::dex::AccountType;
use crate::{retain_intervals_unsafe, DataSliceInitializer, SliceType};
use anyhow::anyhow;
use tokio::sync::OnceCell;

// ========================= static data 账户未订阅的数据切片 =========================
// pump fun pool
static STATIC_PUMP_FUN_POOL_SLICE: OnceCell<([(usize, usize); 5], usize)> = OnceCell::const_new();
// pump fun global config
static STATIC_PUMP_FUN_GLOBAL_CONFIG_SLICE: OnceCell<([(usize, usize); 2], usize)> =
    OnceCell::const_new();

#[derive(Debug)]
pub struct PumpFunAMMDataSlicer;

impl DataSliceInitializer for PumpFunAMMDataSlicer {
    fn try_init_data_slice_config(&self) -> anyhow::Result<()> {
        init_pool_data_slice()?;
        init_global_config_slice()?;
        self.try_init_mint_vault_data_slice()?;
        Ok(())
    }

    fn try_get_data_slice_size(
        &self,
        account_type: AccountType,
        slice_type: SliceType,
    ) -> anyhow::Result<Option<usize>> {
        match slice_type {
            SliceType::Subscribed => match account_type {
                AccountType::Pool => Ok(None),
                AccountType::MintVault => self.try_get_mint_vault_data_slice_size(slice_type),
                AccountType::PumpFunGlobalConfig => Ok(None),
                _ => Err(anyhow!("DexType和AccountType不匹配")),
            },
            SliceType::Unsubscribed => match account_type {
                AccountType::Pool => Ok(Some(STATIC_PUMP_FUN_POOL_SLICE.get().unwrap().1)),
                AccountType::MintVault => self.try_get_mint_vault_data_slice_size(slice_type),
                AccountType::PumpFunGlobalConfig => {
                    Ok(Some(STATIC_PUMP_FUN_GLOBAL_CONFIG_SLICE.get().unwrap().1))
                }
                _ => Err(anyhow!("DexType和AccountType不匹配")),
            },
        }
    }

    fn try_slice_data(
        &self,
        account_type: AccountType,
        data: Vec<u8>,
        slice_type: SliceType,
    ) -> anyhow::Result<Vec<u8>> {
        match slice_type {
            SliceType::Subscribed => match account_type {
                AccountType::MintVault => self.try_mint_vault_slice_data(data),
                _ => Err(anyhow!("")),
            },
            SliceType::Unsubscribed => match account_type {
                AccountType::Pool => Ok(retain_intervals_unsafe(
                    data,
                    &STATIC_PUMP_FUN_POOL_SLICE.get().unwrap().0,
                    STATIC_PUMP_FUN_POOL_SLICE.get().unwrap().1,
                )),
                AccountType::PumpFunGlobalConfig => Ok(retain_intervals_unsafe(
                    data,
                    &STATIC_PUMP_FUN_GLOBAL_CONFIG_SLICE.get().unwrap().0,
                    STATIC_PUMP_FUN_GLOBAL_CONFIG_SLICE.get().unwrap().1,
                )),
                AccountType::MintVault => Err(anyhow!("")),
                _ => Err(anyhow!("")),
            },
        }
    }
}

fn init_pool_data_slice() -> anyhow::Result<()> {
    STATIC_PUMP_FUN_POOL_SLICE.set({
        (
            [
                // base_mint
                (43, 43 + 32),
                // quote_mint
                (75, 75 + 32),
                // pool_base_token_account
                (139, 139 + 32),
                // pool_quote_tiken_account
                (171, 171 + 32),
                // coin_creator
                (211, 211 + 32),
            ],
            32 * 5,
        )
    })?;
    Ok(())
}

fn init_global_config_slice() -> anyhow::Result<()> {
    STATIC_PUMP_FUN_GLOBAL_CONFIG_SLICE.set({
        (
            [
                // lp_fee_basis_points
                (40, 40 + 8),
                // protocol_fee_basis_points
                (48, 48 + 8),
            ],
            8 * 2,
        )
    })?;
    Ok(())
}
