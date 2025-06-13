use crate::dex::{retain_intervals_unsafe, AccountType, DataSliceInitializer, SliceType};
use anyhow::anyhow;
use tokio::sync::OnceCell;

// ========================= dynamic data 账户订阅的数据切片 =========================
// pool
static DYNAMIC_POOL_SLICE: OnceCell<(Vec<(usize, usize)>, usize)> = OnceCell::const_new();
// ========================= static data 账户未订阅的数据切片 =========================
// pool
static STATIC_POOL_SLICE: OnceCell<(Vec<(usize, usize)>, usize)> = OnceCell::const_new();

#[derive(Debug)]
pub struct MeteoraDAMMV2DataSlicer;

impl DataSliceInitializer for MeteoraDAMMV2DataSlicer {
    fn try_init_data_slice_config(&self) -> anyhow::Result<()> {
        STATIC_POOL_SLICE.set({
            let data_slice = [
                // cliff_fee_numerator
                (8, 8),
                // fee_scheduler_mode
                (16, 1),
                // number_of_period
                (22, 2),
                // period_frequency
                (24, 8),
                // reduction_factor
                (32, 8),
                // initialized
                (56, 1),
                // variable_fee_control
                (68, 4),
                // bin_step
                (72, 2),
                // filter_period
                (74, 2),
                // decay_period
                (76, 2),
                // reduction_factor
                (78, 2),
                // token_a_mint
                (168, 32),
                // token_b_mint
                (200, 32),
                // token_a_vault
                (232, 32),
                // token_b_vault
                (264, 32),
                // sqrt_min_price
                (424, 16),
                // sqrt_max_price
                (440, 16),
                // activation_point
                (472, 8),
                // activation_type
                (480, 1),
                // token_a_flag
                (482, 1),
                // token_b_flag
                (483, 1),
                // collect_fee_mode
                (484, 1),
            ];
            let total_len = data_slice.iter().map(|(_, offset)| offset).sum();
            (
                data_slice
                    .into_iter()
                    .map(|(start, offset)| (start, start + offset))
                    .collect::<Vec<_>>(),
                total_len,
            )
        })?;
        DYNAMIC_POOL_SLICE.set({
            let data_slice = [
                // last_update_timestamp
                (80, 8),
                // sqrt_price_reference
                (104, 16),
                // volatility_accumulator
                (120, 16),
                // volatility_reference
                (136, 16),
                // liquidity
                (360, 16),
                // sqrt_price
                (456, 16),
                // pool_status
                (481, 1),
            ];
            let total_len = data_slice.iter().map(|(_, offset)| offset).sum();
            (
                data_slice
                    .into_iter()
                    .map(|(start, offset)| (start, start + offset))
                    .collect::<Vec<_>>(),
                total_len,
            )
        })?;
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
                _ => Err(anyhow!("")),
            },
            SliceType::Unsubscribed => match account_type {
                AccountType::Pool => Ok(Some(STATIC_POOL_SLICE.get().unwrap().1)),
                _ => Err(anyhow!("")),
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
                _ => Err(anyhow!("")),
            },
            SliceType::Unsubscribed => match account_type {
                AccountType::Pool => Ok(retain_intervals_unsafe(
                    data,
                    &STATIC_POOL_SLICE.get().unwrap().0,
                    STATIC_POOL_SLICE.get().unwrap().1,
                )),
                _ => Err(anyhow!("")),
            },
        }
    }
}
