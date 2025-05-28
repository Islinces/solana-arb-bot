use crate::data_slice::{retain_intervals_unsafe, DYNAMIC_MINT_VAULT_SLICE};
use crate::interface::{AccountType, DexType};
use anyhow::anyhow;
use tokio::sync::OnceCell;

// ========================= static data 账户未订阅的数据切片 =========================
// pump fun pool
static STATIC_PUMP_FUN_POOL_SLICE: OnceCell<([(usize, usize); 5], usize)> = OnceCell::const_new();
// pump fun global config
static STATIC_PUMP_FUN_GLOBAL_CONFIG_SLICE: OnceCell<([(usize, usize); 2], usize)> =
    OnceCell::const_new();

pub fn slice_data_for_static(account_type: AccountType, data: &[u8]) -> anyhow::Result<Vec<u8>> {
    match account_type {
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
    }
}

pub fn slice_data_for_dynamic(account_type: AccountType, data: &[u8]) -> anyhow::Result<Vec<u8>> {
    match account_type {
        AccountType::MintVault => Ok(retain_intervals_unsafe(
            data,
            &DYNAMIC_MINT_VAULT_SLICE.get().unwrap().0,
            DYNAMIC_MINT_VAULT_SLICE.get().unwrap().1,
        )),
        _ => Err(anyhow!("")),
    }
}

pub fn get_slice_size(
    account_type: AccountType,
    dynamic_flag: bool,
) -> anyhow::Result<Option<usize>> {
    match account_type {
        AccountType::Pool => {
            if dynamic_flag {
                Ok(None)
            } else {
                Ok(Some(STATIC_PUMP_FUN_POOL_SLICE.get().unwrap().1))
            }
        }
        AccountType::MintVault => {
            if dynamic_flag {
                Ok(Some(DYNAMIC_MINT_VAULT_SLICE.get().unwrap().1))
            } else {
                Ok(None)
            }
        }
        AccountType::PumpFunGlobalConfig => {
            if dynamic_flag {
                Ok(None)
            } else {
                Ok(Some(STATIC_PUMP_FUN_GLOBAL_CONFIG_SLICE.get().unwrap().1))
            }
        }
        _ => Err(anyhow!("DexType和AccountType不匹配")),
    }
}

pub fn init_pump_fun_data_slice() {
    STATIC_PUMP_FUN_POOL_SLICE
        .set({
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
        })
        .unwrap();
    STATIC_PUMP_FUN_GLOBAL_CONFIG_SLICE
        .set({
            (
                [
                    // lp_fee_basis_points
                    (40, 40 + 8),
                    // protocol_fee_basis_points
                    (48, 48 + 8),
                ],
                8 * 2,
            )
        })
        .unwrap();
}
