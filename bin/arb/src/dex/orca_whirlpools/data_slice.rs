use crate::interface1::AccountType;
use crate::{retain_intervals_unsafe, DataSliceInitializer, SliceType};
use anyhow::anyhow;
use tokio::sync::OnceCell;

// ========================= dynamic data 账户订阅的数据切片 =========================
// pool
static DYNAMIC_POOL_SLICE: OnceCell<([(usize, usize); 3], usize)> = OnceCell::const_new();
// oracle
static DYNAMIC_ORACLE_SLICE: OnceCell<([(usize, usize); 5], usize)> = OnceCell::const_new();
// tick array
static DYNAMIC_TICK_ARRAY_SLICE: OnceCell<(Vec<(usize, usize)>, usize)> = OnceCell::const_new();
// ========================= static data 账户未订阅的数据切片 =========================
// pool
static STATIC_POOL_SLICE: OnceCell<([(usize, usize); 9], usize)> = OnceCell::const_new();
// oracle
static STATIC_ORACLE_SLICE: OnceCell<([(usize, usize); 8], usize)> = OnceCell::const_new();

#[derive(Debug)]
pub struct OrcaWhirlDataSlicer;

impl DataSliceInitializer for OrcaWhirlDataSlicer {
    fn try_init_data_slice_config(&self) -> anyhow::Result<()> {
        pool_data_slice()?;
        oracle_data_slice()?;
        tick_array_data_slice()?;
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
                AccountType::Oracle => Ok(Some(DYNAMIC_ORACLE_SLICE.get().unwrap().1)),
                AccountType::TickArray => Ok(Some(DYNAMIC_TICK_ARRAY_SLICE.get().unwrap().1)),
                _ => Err(anyhow!("DexType和AccountType不匹配")),
            },
            SliceType::Unsubscribed => match account_type {
                AccountType::Pool => Ok(Some(STATIC_POOL_SLICE.get().unwrap().1)),
                AccountType::Oracle => Ok(Some(STATIC_ORACLE_SLICE.get().unwrap().1)),
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
                AccountType::Oracle => Ok(retain_intervals_unsafe(
                    data,
                    &DYNAMIC_ORACLE_SLICE.get().unwrap().0,
                    DYNAMIC_ORACLE_SLICE.get().unwrap().1,
                )),
                AccountType::TickArray => Ok(retain_intervals_unsafe(
                    data,
                    &DYNAMIC_TICK_ARRAY_SLICE.get().unwrap().0,
                    DYNAMIC_TICK_ARRAY_SLICE.get().unwrap().1,
                )),
                _ => Err(anyhow!("")),
            },
            SliceType::Unsubscribed => match account_type {
                AccountType::Pool => Ok(retain_intervals_unsafe(
                    data,
                    &STATIC_POOL_SLICE.get().unwrap().0,
                    STATIC_POOL_SLICE.get().unwrap().1,
                )),
                AccountType::Oracle => Ok(retain_intervals_unsafe(
                    data,
                    &STATIC_ORACLE_SLICE.get().unwrap().0,
                    STATIC_ORACLE_SLICE.get().unwrap().1,
                )),
                _ => Err(anyhow!("")),
            },
        }
    }
}

fn tick_array_data_slice() -> anyhow::Result<()> {
    DYNAMIC_TICK_ARRAY_SLICE.set({
        let mut data_slice: Vec<(usize, usize)> = Vec::new();
        let mut total_len = 0;
        // start_tick_index
        data_slice.push((8, 8 + 4));
        total_len += 4;
        // ticks
        let mut start_index = 12;
        // ticks 88
        for _ in 0..88 {
            // initialized
            data_slice.push((start_index, start_index + 1));
            start_index += 1;
            total_len += 1;
            // liquidity_net
            data_slice.push((start_index, start_index + 16));
            start_index += 16;
            total_len += 16;
            // liquidity_gross
            data_slice.push((start_index, start_index + 16));
            start_index += 16;
            total_len += 16;
            // 不需要订阅的
            // fee_a
            start_index += 16;
            // fee_b
            start_index += 16;
            // reward
            start_index += 16 * 3;
        }
        // whirlpool
        data_slice.push((9956, 9956 + 32));
        total_len += 32;
        (data_slice, total_len)
    })?;
    Ok(())
}

fn oracle_data_slice() -> anyhow::Result<()> {
    STATIC_ORACLE_SLICE.set({
        (
            [
                // whirlpool
                (8, 8 + 32),
                // filter_period
                (40, 40 + 2),
                // decay_period
                (42, 42 + 2),
                // reduction_factor
                (44, 44 + 2),
                // adaptive_fee_control_factor
                (46, 46 + 4),
                // max_volatility_accumulator
                (50, 50 + 4),
                // tick_group_size
                (54, 54 + 2),
                // major_swap_threshold_ticks
                (56, 56 + 16),
            ],
            32 + 2 + 2 + 2 + 4 + 4 + 2 + 16,
        )
    })?;
    DYNAMIC_ORACLE_SLICE.set({
        (
            [
                // last_reference_update_timestamp
                (74, 74 + 8),
                // last_major_swap_timestamp
                (82, 82 + 8),
                // volatility_reference
                (90, 90 + 4),
                // tick_group_index_reference
                (94, 94 + 4),
                // volatility_accumulator
                (98, 98 + 4),
            ],
            8 + 8 + 4 + 4 + 4,
        )
    })?;
    Ok(())
}

fn pool_data_slice() -> anyhow::Result<()> {
    DYNAMIC_POOL_SLICE.set({
        (
            [
                // liquidity
                (49, 49 + 16),
                // sqrt_price
                (65, 65 + 16),
                // tick_current_index
                (81, 81 + 4),
            ],
            16 + 16 + 4,
        )
    })?;
    STATIC_POOL_SLICE.set({
        (
            [
                // tick_spacing
                (41, 41 + 2),
                // fee_tier_index_seed
                (43, 43 + 2),
                // fee_rate
                (45, 45 + 2),
                // protocol_fee_owed_a
                (85, 85 + 8),
                // protocol_fee_owed_b
                (93, 93 + 8),
                // token_mint_a
                (101, 101 + 32),
                // token_vault_a
                (133, 133 + 32),
                // token_mint_b
                (181, 181 + 32),
                // token_vault_b
                (213, 213 + 32),
            ],
            2 + 2 + 2 + 8 + 8 + 32 + 32 + 32 + 32,
        )
    })?;
    Ok(())
}
