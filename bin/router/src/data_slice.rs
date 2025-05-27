use crate::interface::{AccountType, DexType};
use anyhow::anyhow;
use futures_util::future::ok;
use solana_sdk::pubkey::Pubkey;
use std::ptr;
use tokio::sync::OnceCell;

// ========================= 账户订阅的数据切片 =========================
// amm pool
static DYNAMIC_RAYDIUM_AMM_POOL_SLICE: OnceCell<([(usize, usize); 2], usize)> =
    OnceCell::const_new();
// clmm pool
static DYNAMIC_RAYDIUM_CLMM_POOL_SLICE: OnceCell<([(usize, usize); 4], usize)> =
    OnceCell::const_new();
// clmm bitmap extension
static DYNAMIC_RAYDIUM_CLMM_BITMAP_EXTENSION_SLICE: OnceCell<([(usize, usize); 1], usize)> =
    OnceCell::const_new();
// clmm tick array
static DYNAMIC_RAYDIUM_CLMM_TICK_ARRAY_STATE_SLICE: OnceCell<(Vec<(usize, usize)>, usize)> =
    OnceCell::const_new();
// mint vault
static DYNAMIC_MINT_VAULT_SLICE: OnceCell<([(usize, usize); 1], usize)> = OnceCell::const_new();
// ========================= 账户未订阅的数据切片 =========================
// amm pool
static STATIC_RAYDIUM_AMM_POOL_SLICE: OnceCell<([(usize, usize); 6], usize)> =
    OnceCell::const_new();
// pump fun pool
static STATIC_PUMP_FUN_POOL_SLICE: OnceCell<([(usize, usize); 5], usize)> = OnceCell::const_new();
// pump fun global config
static STATIC_PUMP_FUN_GLOBAL_CONFIG_SLICE: OnceCell<([(usize, usize); 2], usize)> =
    OnceCell::const_new();
// clmm pool
static STATIC_RAYDIUM_CLMM_POOL_SLICE: OnceCell<([(usize, usize); 7], usize)> =
    OnceCell::const_new();
// clmm amm config
static STATIC_RAYDIUM_CLMM_AMM_CONFIG_SLICE: OnceCell<([(usize, usize); 1], usize)> =
    OnceCell::const_new();

pub fn slice_data_for_static(
    dex_type: DexType,
    account_type: AccountType,
    data: &[u8],
) -> anyhow::Result<Vec<u8>> {
    match dex_type {
        DexType::RaydiumAMM => match account_type {
            AccountType::Pool => Ok(retain_intervals_unsafe(
                data,
                &STATIC_RAYDIUM_AMM_POOL_SLICE.get().unwrap().0,
                STATIC_RAYDIUM_AMM_POOL_SLICE.get().unwrap().1,
            )),
            AccountType::MintVault => Err(anyhow!("")),
            _ => Err(anyhow!("")),
        },
        DexType::RaydiumCLMM => match account_type {
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
        DexType::PumpFunAMM => match account_type {
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

#[inline]
pub fn slice_data_for_dynamic(
    dex_type: DexType,
    account_type: AccountType,
    data: &[u8],
) -> anyhow::Result<Vec<u8>> {
    match dex_type {
        DexType::RaydiumAMM => match account_type {
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
        DexType::RaydiumCLMM => match account_type {
            AccountType::Pool => Ok(retain_intervals_unsafe(
                data,
                &DYNAMIC_RAYDIUM_CLMM_POOL_SLICE.get().unwrap().0,
                DYNAMIC_RAYDIUM_CLMM_POOL_SLICE.get().unwrap().1,
            )),
            AccountType::TickArrayState => Ok(retain_intervals_unsafe(
                data,
                &DYNAMIC_RAYDIUM_CLMM_TICK_ARRAY_STATE_SLICE
                    .get()
                    .unwrap()
                    .0
                    .as_slice(),
                DYNAMIC_RAYDIUM_CLMM_TICK_ARRAY_STATE_SLICE.get().unwrap().1,
            )),
            // 不做切片
            AccountType::TickArrayBitmapExtension => Ok(retain_intervals_unsafe(
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
        DexType::PumpFunAMM => match account_type {
            AccountType::MintVault => Ok(retain_intervals_unsafe(
                data,
                &DYNAMIC_MINT_VAULT_SLICE.get().unwrap().0,
                DYNAMIC_MINT_VAULT_SLICE.get().unwrap().1,
            )),
            _ => Err(anyhow!("")),
        },
    }
}

#[inline]
pub fn slice_data(account_key: &Pubkey, owner: &Pubkey, data: &[u8]) -> anyhow::Result<Vec<u8>> {
    match crate::account_relation::get_dex_type_and_account_type(owner, account_key) {
        None => Err(anyhow!("")),
        Some((dex_type, account_type)) => slice_data_for_dynamic(dex_type, account_type, data),
    }
}

fn retain_intervals_unsafe(src: &[u8], intervals: &[(usize, usize)], total_len: usize) -> Vec<u8> {
    // 创建目标 Vec，先设置容量
    let mut result = Vec::with_capacity(total_len);
    unsafe {
        // 获取目标 vec 的写入指针
        let mut dst = result.as_mut_ptr();
        for &(start, end) in intervals {
            let len = end - start;
            if start < end && end <= src.len() {
                // 源地址
                let src_ptr = src.as_ptr().add(start);
                // 拷贝
                ptr::copy_nonoverlapping(src_ptr, dst, len);
                // 移动目标指针
                dst = dst.add(len);
            }
        }
        // 设置 vec 实际长度（安全）
        result.set_len(total_len);
    }
    result
}

pub async fn init_data_slice_config() {
    init_raydium_amm_pool_data_slice().await;
    init_raydium_clmm_data_slice().await;
    init_pump_fun_data_slice().await;
}

async fn init_raydium_amm_pool_data_slice() {
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
    init_mint_vault_data_slice().await;
    STATIC_RAYDIUM_AMM_POOL_SLICE
        .get_or_init(|| async {
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
        .await;
}

async fn init_pump_fun_data_slice() {
    init_mint_vault_data_slice().await;
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
        .get_or_init(|| async {
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
        .await;
}

async fn init_raydium_clmm_data_slice() {
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
        .set({ ([(8, 1832)], 1832 - 8) })
        .unwrap();
    DYNAMIC_RAYDIUM_CLMM_TICK_ARRAY_STATE_SLICE
        .get_or_init(|| async {
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
            data_slice.push((start_index, start_index + 1));
            total_len += 1;
            (data_slice, total_len)
        })
        .await;
    STATIC_RAYDIUM_CLMM_POOL_SLICE
        .get_or_init(|| async {
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
        .await;
    STATIC_RAYDIUM_CLMM_AMM_CONFIG_SLICE
        .get_or_init(|| async {
            (
                [
                    // trade_fee_rate
                    (47, 47 + 4),
                ],
                4,
            )
        })
        .await;
}

async fn init_mint_vault_data_slice() {
    // amount
    DYNAMIC_MINT_VAULT_SLICE
        .get_or_init(|| async { ([(64, 64 + 8)], 8) })
        .await;
}

pub fn get_slice_size(
    dex_type: DexType,
    account_type: AccountType,
    dynamic_flag: bool,
) -> anyhow::Result<Option<usize>> {
    match dex_type {
        DexType::RaydiumAMM => match account_type {
            AccountType::Pool => Ok(Some(if dynamic_flag {
                DYNAMIC_RAYDIUM_AMM_POOL_SLICE.get().unwrap().1
            } else {
                STATIC_RAYDIUM_AMM_POOL_SLICE.get().unwrap().1
            })),
            AccountType::MintVault => {
                if dynamic_flag {
                    Ok(Some(DYNAMIC_MINT_VAULT_SLICE.get().unwrap().1))
                } else {
                    Ok(None)
                }
            }
            _ => Err(anyhow!("DexType和AccountType不匹配")),
        },
        DexType::RaydiumCLMM => match account_type {
            AccountType::Pool => Ok(Some(if dynamic_flag {
                DYNAMIC_RAYDIUM_CLMM_POOL_SLICE.get().unwrap().1
            } else {
                STATIC_RAYDIUM_CLMM_POOL_SLICE.get().unwrap().1
            })),
            AccountType::AmmConfig => {
                if dynamic_flag {
                    Ok(None)
                } else {
                    Ok(Some(STATIC_RAYDIUM_CLMM_AMM_CONFIG_SLICE.get().unwrap().1))
                }
            }
            AccountType::TickArrayState => {
                if dynamic_flag {
                    Ok(Some(
                        DYNAMIC_RAYDIUM_CLMM_TICK_ARRAY_STATE_SLICE.get().unwrap().1,
                    ))
                } else {
                    Ok(None)
                }
            }
            AccountType::TickArrayBitmapExtension => {
                if dynamic_flag {
                    Ok(Some(
                        DYNAMIC_RAYDIUM_CLMM_BITMAP_EXTENSION_SLICE.get().unwrap().1,
                    ))
                } else {
                    Ok(None)
                }
            }
            _ => Err(anyhow!("DexType和AccountType不匹配")),
        },
        DexType::PumpFunAMM => match account_type {
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
        },
    }
}
