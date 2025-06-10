use crate::dex::AccountType;
use crate::{retain_intervals_unsafe, DataSliceInitializer, SliceType};
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

#[derive(Debug)]
pub struct RaydiumAMMDataSlicer;

impl DataSliceInitializer for RaydiumAMMDataSlicer {
    fn try_init_data_slice_config(&self) -> anyhow::Result<()> {
        init_pool_data_slice()?;
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
                AccountType::Pool => Ok(Some(DYNAMIC_RAYDIUM_AMM_POOL_SLICE.get().unwrap().1)),
                AccountType::MintVault => self.try_get_mint_vault_data_slice_size(slice_type),
                _ => Err(anyhow!("DexType和AccountType不匹配")),
            },
            SliceType::Unsubscribed => match account_type {
                AccountType::Pool => Ok(Some(STATIC_RAYDIUM_AMM_POOL_SLICE.get().unwrap().1)),
                AccountType::MintVault => self.try_get_mint_vault_data_slice_size(slice_type),
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
                AccountType::Pool => Ok(retain_intervals_unsafe(
                    data,
                    &DYNAMIC_RAYDIUM_AMM_POOL_SLICE.get().unwrap().0,
                    DYNAMIC_RAYDIUM_AMM_POOL_SLICE.get().unwrap().1,
                )),
                AccountType::MintVault => self.try_mint_vault_slice_data(data),
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
}

fn init_pool_data_slice() -> anyhow::Result<()> {
    DYNAMIC_RAYDIUM_AMM_POOL_SLICE.set({
        (
            [
                // state_data.need_take_pnl_coin
                (192, 192 + 8),
                // state_data.need_take_pnl_pc
                (200, 200 + 8),
            ],
            8 + 8,
        )
    })?;
    STATIC_RAYDIUM_AMM_POOL_SLICE.set({
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
    })?;
    Ok(())
}
