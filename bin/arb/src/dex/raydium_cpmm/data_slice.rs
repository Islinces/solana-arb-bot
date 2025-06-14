use crate::dex::{retain_intervals_unsafe, AccountType, DataSliceInitializer, SliceType};
use anyhow::anyhow;
use tokio::sync::OnceCell;

// ========================= dynamic data 账户订阅的数据切片 =========================
// pool
static DYNAMIC_POOL_SLICE: OnceCell<(Vec<(usize, usize)>, usize)> = OnceCell::const_new();
// ========================= static data 账户未订阅的数据切片 =========================
// pool
static STATIC_POOL_SLICE: OnceCell<(Vec<(usize, usize)>, usize)> = OnceCell::const_new();
// amm config
static STATIC_AMM_CONFIG_SLICE: OnceCell<(Vec<(usize, usize)>, usize)> = OnceCell::const_new();

#[derive(Debug)]
pub struct RaydiumCPMMDataSlicer;

impl DataSliceInitializer for RaydiumCPMMDataSlicer {
    fn try_init_data_slice_config(&self) -> anyhow::Result<()> {
        init_pool_data_slice()?;
        init_amm_config_slice()?;
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
                AccountType::Pool => Ok(Some(DYNAMIC_POOL_SLICE.get().unwrap().1)),
                AccountType::MintVault => self.try_get_mint_vault_data_slice_size(slice_type),
                _ => Err(anyhow!("DexType和AccountType不匹配")),
            },
            SliceType::Unsubscribed => match account_type {
                AccountType::Pool => Ok(Some(STATIC_POOL_SLICE.get().unwrap().1)),
                AccountType::MintVault => self.try_get_mint_vault_data_slice_size(slice_type),
                AccountType::AmmConfig => Ok(Some(STATIC_AMM_CONFIG_SLICE.get().unwrap().1)),
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
                    &DYNAMIC_POOL_SLICE.get().unwrap().0,
                    DYNAMIC_POOL_SLICE.get().unwrap().1,
                )),
                AccountType::MintVault => self.try_mint_vault_slice_data(data),
                _ => Err(anyhow!("")),
            },
            SliceType::Unsubscribed => match account_type {
                AccountType::Pool => Ok(retain_intervals_unsafe(
                    data,
                    &STATIC_POOL_SLICE.get().unwrap().0,
                    STATIC_POOL_SLICE.get().unwrap().1,
                )),
                AccountType::AmmConfig => Ok(retain_intervals_unsafe(
                    data,
                    &STATIC_AMM_CONFIG_SLICE.get().unwrap().0,
                    STATIC_AMM_CONFIG_SLICE.get().unwrap().1,
                )),
                AccountType::MintVault => Err(anyhow!("")),
                _ => Err(anyhow!("")),
            },
        }
    }
}

fn init_pool_data_slice() -> anyhow::Result<()> {
    STATIC_POOL_SLICE.set({
        let slice = vec![
            // amm_config
            (8, 32),
            // token_0_vault
            (72, 32),
            // token_1_vault
            (104, 32),
            // token_0_mint
            (168, 32),
            // token_1_mint
            (200, 32),
            // token_0_program
            (232, 32),
            // token_1_program
            (264, 32),
            // observation_key
            (296, 32),
            // open_time
            (373, 8),
        ];
        let total_len = slice.iter().map(|(_, offset)| offset).sum();
        let slice = slice
            .into_iter()
            .map(|(start, offset)| (start, start + offset))
            .collect::<Vec<_>>();
        (slice, total_len)
    })?;
    DYNAMIC_POOL_SLICE.set({
        let slice = vec![
            // status
            (329, 1),
            // protocol_fees_token_0
            (341, 8),
            // protocol_fees_token_1
            (349, 8),
            // fund_fees_token_0
            (357, 8),
            // fund_fees_token_1
            (365, 8),
        ];
        let total_len = slice.iter().map(|(_, offset)| offset).sum();
        let slice = slice
            .into_iter()
            .map(|(start, offset)| (start, start + offset))
            .collect::<Vec<_>>();
        (slice, total_len)
    })?;
    Ok(())
}

fn init_amm_config_slice() -> anyhow::Result<()> {
    STATIC_AMM_CONFIG_SLICE.set({ (vec![(12, 20)], 8) })?;
    Ok(())
}
