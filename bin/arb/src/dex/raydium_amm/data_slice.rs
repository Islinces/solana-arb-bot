use crate::data_slice::{retain_intervals_unsafe, SliceType, DYNAMIC_MINT_VAULT_SLICE};
use crate::interface::AccountType;
use anyhow::anyhow;
use tokio::sync::OnceCell;

// ========================= dynamic data 账户订阅的数据切片 =========================
// amm pool
static DYNAMIC_RAYDIUM_AMM_POOL_SLICE: OnceCell<([(usize, usize); 2], usize)> =
    OnceCell::const_new();
// ========================= static data 账户未订阅的数据切片 =========================
// amm pool
static STATIC_RAYDIUM_AMM_POOL_SLICE: OnceCell<([(usize, usize); 6], usize)> =
    OnceCell::const_new();

pub fn slice_data(
    account_type: AccountType,
    data: &[u8],
    slice_type: SliceType,
) -> anyhow::Result<Vec<u8>> {
    match slice_type {
        SliceType::Subscribed => match account_type {
            AccountType::Pool => Ok(retain_intervals_unsafe(
                data,
                &DYNAMIC_RAYDIUM_AMM_POOL_SLICE.get().unwrap().0,
                DYNAMIC_RAYDIUM_AMM_POOL_SLICE.get().unwrap().1,
            )),
            AccountType::MintVault => Ok(retain_intervals_unsafe(
                data,
                &DYNAMIC_MINT_VAULT_SLICE.get().unwrap().0,
                DYNAMIC_MINT_VAULT_SLICE.get().unwrap().1,
            )),
            _ => Err(anyhow!("")),
        },
        SliceType::Unsubscribed => match account_type {
            AccountType::Pool => Ok(retain_intervals_unsafe(
                data,
                &STATIC_RAYDIUM_AMM_POOL_SLICE.get().unwrap().0,
                STATIC_RAYDIUM_AMM_POOL_SLICE.get().unwrap().1,
            )),
            AccountType::MintVault => Err(anyhow!("")),
            _ => Err(anyhow!("")),
        },
    }
}

pub fn get_slice_size(
    account_type: AccountType,
    slice_type: SliceType,
) -> anyhow::Result<Option<usize>> {
    match slice_type {
        SliceType::Subscribed => match account_type {
            AccountType::Pool => Ok(Some(DYNAMIC_RAYDIUM_AMM_POOL_SLICE.get().unwrap().1)),
            AccountType::MintVault => Ok(None),
            _ => Err(anyhow!("DexType和AccountType不匹配")),
        },
        SliceType::Unsubscribed => match account_type {
            AccountType::Pool => Ok(Some(STATIC_RAYDIUM_AMM_POOL_SLICE.get().unwrap().1)),
            AccountType::MintVault => Ok(Some(DYNAMIC_MINT_VAULT_SLICE.get().unwrap().1)),
            _ => Err(anyhow!("DexType和AccountType不匹配")),
        },
    }
}

pub fn init_raydium_amm_data_slice() {
    DYNAMIC_RAYDIUM_AMM_POOL_SLICE
        .set({
            (
                [
                    // state_data.need_take_pnl_coin
                    (192, 192 + 8),
                    // state_data.need_take_pnl_pc
                    (200, 200 + 8),
                ],
                8 + 8,
            )
        })
        .unwrap();
    STATIC_RAYDIUM_AMM_POOL_SLICE
        .set({
            (
                [
                    // swap_fee_numerator
                    (176, 176 + 8),
                    // swap_fee_denominator
                    (184, 184 + 8),
                    // coin_vault
                    (336, 336 + 32),
                    // pc_vault
                    (368, 368 + 32),
                    // coin_vault_mint
                    (400, 400 + 32),
                    // pc_vault_mint
                    (432, 432 + 32),
                ],
                8 + 8 + 32 + 32 + 32 + 32,
            )
        })
        .unwrap();
}
