use crate::data_slice::{retain_intervals_unsafe, SliceType};
use crate::interface::AccountType;
use anyhow::anyhow;
use borsh::BorshDeserialize;
use tokio::sync::OnceCell;

// ========================= dynamic data 账户订阅的数据切片 =========================
// clmm pool
static DYNAMIC_RAYDIUM_CLMM_POOL_SLICE: OnceCell<([(usize, usize); 4], usize)> =
    OnceCell::const_new();
// clmm bitmap extension
static DYNAMIC_RAYDIUM_CLMM_BITMAP_EXTENSION_SLICE: OnceCell<([(usize, usize); 1], usize)> =
    OnceCell::const_new();
// clmm tick array
static DYNAMIC_RAYDIUM_CLMM_TICK_ARRAY_STATE_SLICE: OnceCell<(Vec<(usize, usize)>, usize)> =
    OnceCell::const_new();
// ========================= static data 账户未订阅的数据切片 =========================
// clmm pool
static STATIC_RAYDIUM_CLMM_POOL_SLICE: OnceCell<([(usize, usize); 7], usize)> =
    OnceCell::const_new();
// clmm amm config
static STATIC_RAYDIUM_CLMM_AMM_CONFIG_SLICE: OnceCell<([(usize, usize); 3], usize)> =
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
                &DYNAMIC_RAYDIUM_CLMM_POOL_SLICE.get().unwrap().0,
                DYNAMIC_RAYDIUM_CLMM_POOL_SLICE.get().unwrap().1,
            )),
            AccountType::TickArray => {
                let borsh_data =
                    crate::dex::raydium_clmm::copy_tick_array::TickArrayState::try_from_slice(
                        &data[8..],
                    );
                let result = Ok(retain_intervals_unsafe(
                    data,
                    &DYNAMIC_RAYDIUM_CLMM_TICK_ARRAY_STATE_SLICE
                        .get()
                        .unwrap()
                        .0
                        .as_slice(),
                    DYNAMIC_RAYDIUM_CLMM_TICK_ARRAY_STATE_SLICE.get().unwrap().1,
                ));

                // info!("borsh_data : {:#?}", borsh_data);
                // info!("slice_data : {:#?}", unsafe {
                //     ptr::read_unaligned(result.as_ref().unwrap().as_slice().as_ptr() as *const TickArrayState)
                // });
                result
            }
            AccountType::TickArrayBitmap => Ok(retain_intervals_unsafe(
                data,
                &DYNAMIC_RAYDIUM_CLMM_BITMAP_EXTENSION_SLICE
                    .get()
                    .unwrap()
                    .0
                    .as_slice(),
                DYNAMIC_RAYDIUM_CLMM_BITMAP_EXTENSION_SLICE.get().unwrap().1,
            )),
            _ => Err(anyhow!("")),
        },
        SliceType::Unsubscribed => match account_type {
            AccountType::Pool => Ok(retain_intervals_unsafe(
                data,
                &STATIC_RAYDIUM_CLMM_POOL_SLICE.get().unwrap().0,
                STATIC_RAYDIUM_CLMM_POOL_SLICE.get().unwrap().1,
            )),
            AccountType::AmmConfig => Ok(retain_intervals_unsafe(
                data,
                &STATIC_RAYDIUM_CLMM_AMM_CONFIG_SLICE.get().unwrap().0,
                STATIC_RAYDIUM_CLMM_AMM_CONFIG_SLICE.get().unwrap().1,
            )),
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
            AccountType::Pool => Ok(Some(DYNAMIC_RAYDIUM_CLMM_POOL_SLICE.get().unwrap().1)),
            AccountType::AmmConfig => Ok(None),
            AccountType::TickArray => Ok(Some(
                DYNAMIC_RAYDIUM_CLMM_TICK_ARRAY_STATE_SLICE.get().unwrap().1,
            )),
            AccountType::TickArrayBitmap => Ok(Some(
                DYNAMIC_RAYDIUM_CLMM_BITMAP_EXTENSION_SLICE.get().unwrap().1,
            )),
            _ => Err(anyhow!("DexType和AccountType不匹配")),
        },
        SliceType::Unsubscribed => match account_type {
            AccountType::Pool => Ok(Some(STATIC_RAYDIUM_CLMM_POOL_SLICE.get().unwrap().1)),
            AccountType::AmmConfig => {
                Ok(Some(STATIC_RAYDIUM_CLMM_AMM_CONFIG_SLICE.get().unwrap().1))
            }
            AccountType::TickArray => Ok(None),
            AccountType::TickArrayBitmap => Ok(None),
            _ => Err(anyhow!("DexType和AccountType不匹配")),
        },
    }
}

pub fn init_raydium_clmm_data_slice() {
    DYNAMIC_RAYDIUM_CLMM_POOL_SLICE
        .set({
            (
                [
                    // liquidity
                    (237, 237 + 16),
                    // sqrt_price_x64
                    (253, 253 + 16),
                    // tick_current
                    (269, 269 + 4),
                    // tick_array_bitmap
                    (904, 904 + 128),
                ],
                16 + 16 + 4 + 128,
            )
        })
        .unwrap();
    DYNAMIC_RAYDIUM_CLMM_BITMAP_EXTENSION_SLICE
        .set(([(8, 1832)], 1832 - 8))
        .unwrap();
    DYNAMIC_RAYDIUM_CLMM_TICK_ARRAY_STATE_SLICE
        .set({
            let mut data_slice: Vec<(usize, usize)> = Vec::new();
            let mut total_len = 0;
            // pool_id
            data_slice.push((8, 8 + 32));
            total_len += 32;
            // start_tick_index
            data_slice.push((40, 40 + 4));
            total_len += 4;
            // ticks
            let mut start_index = 40 + 4;
            // ticks 60个
            for _ in 0..60 {
                // tick
                data_slice.push((start_index, start_index + 4));
                start_index += 4;
                total_len += 4;
                // liquidity_net
                data_slice.push((start_index, start_index + 16));
                start_index += 16;
                total_len += 16;
                // liquidity_gross
                data_slice.push((start_index, start_index + 16));
                start_index += 16;
                total_len += 16;

                // fee_growth_outside_0_x64
                start_index += 16;
                // fee_growth_outside_1_x64
                start_index += 16;
                // reward_growths_outside_x64
                start_index += 16 * 3;
                // padding
                start_index += 13 * 4;
            }
            // initialized_tick_count
            // data_slice.push((start_index, start_index + 1));
            // total_len += 1;
            (data_slice, total_len)
        })
        .unwrap();
    STATIC_RAYDIUM_CLMM_POOL_SLICE
        .set({
            {
                (
                    [
                        // amm_config
                        (9, 9 + 32),
                        // token_mint_0
                        (73, 73 + 32),
                        // token_mint_1
                        (105, 105 + 32),
                        // token_vault_0
                        (137, 137 + 32),
                        // token_vault_1
                        (169, 169 + 32),
                        // observation_key
                        (201, 201 + 32),
                        // tick_spacing
                        (235, 235 + 2),
                    ],
                    32 * 6 + 2,
                )
            }
        })
        .unwrap();
    STATIC_RAYDIUM_CLMM_AMM_CONFIG_SLICE
        .set({
            (
                [
                    // protocol_fee_rate
                    (43, 43 + 4),
                    // trade_fee_rate
                    (47, 47 + 4),
                    // fund_fee_rate
                    (53, 53 + 4),
                ],
                4 * 3,
            )
        })
        .unwrap();
}
