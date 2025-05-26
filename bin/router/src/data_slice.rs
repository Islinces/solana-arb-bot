use crate::interface::{AccountType, DexType, DEX_METADATA};
use anyhow::anyhow;
use solana_sdk::pubkey::Pubkey;
use std::ptr;
use tokio::sync::OnceCell;

// ========================= 账户订阅的数据切片 =========================
pub static RAYDIUM_AMM_POOL_DYNAMIC_SLICE: OnceCell<([(usize, usize); 2], usize)> =
    OnceCell::const_new();
pub static RAYDIUM_CLMM_POOL_DYNAMIC_SLICE: OnceCell<([(usize, usize); 4], usize)> =
    OnceCell::const_new();
pub static RAYDIUM_CLMM_TICK_ARRAY_STATE_DYNAMIC_SLICE: OnceCell<(Vec<(usize, usize)>, usize)> =
    OnceCell::const_new();
pub static MINT_VAULT_DYNAMIC_SLICE: OnceCell<([(usize, usize); 1], usize)> = OnceCell::const_new();
// ========================= 账户未订阅的数据切片 =========================
// raydium amm 池子
pub static RAYDIUM_AMM_POOL_DATA_STATIC_SLICE: OnceCell<([(usize, usize); 6], usize)> =
    OnceCell::const_new();
// pump fun 池子
pub static PUMP_FUN_POOL_STATIC_SLICE: OnceCell<([(usize, usize); 5], usize)> =
    OnceCell::const_new();
// pump fun global config
pub static PUMP_FUN_GLOBAL_CONFIG_STATIC_SLICE: OnceCell<([(usize, usize); 2], usize)> =
    OnceCell::const_new();

pub fn slice_data_with_dex_type_and_account_type_for_static(
    dex_type: DexType,
    account_type: AccountType,
    data: &[u8],
) -> anyhow::Result<Vec<u8>> {
    match dex_type {
        DexType::RaydiumAMM => match account_type {
            AccountType::Pool => Ok(retain_intervals_unsafe(
                data,
                &RAYDIUM_AMM_POOL_DATA_STATIC_SLICE.get().unwrap().0,
                RAYDIUM_AMM_POOL_DATA_STATIC_SLICE.get().unwrap().1,
            )),
            AccountType::MintVault => Err(anyhow!("")),
            _ => Err(anyhow!("")),
        },
        DexType::RaydiumCLMM => match account_type {
            AccountType::Pool => todo!(),
            _ => Err(anyhow!("")),
        },
        DexType::PumpFunAMM => match account_type {
            AccountType::Pool => Ok(retain_intervals_unsafe(
                data,
                &PUMP_FUN_POOL_STATIC_SLICE.get().unwrap().0,
                PUMP_FUN_POOL_STATIC_SLICE.get().unwrap().1,
            )),
            AccountType::PumpFunGlobalConfig => Ok(retain_intervals_unsafe(
                data,
                &PUMP_FUN_GLOBAL_CONFIG_STATIC_SLICE.get().unwrap().0,
                PUMP_FUN_GLOBAL_CONFIG_STATIC_SLICE.get().unwrap().1,
            )),
            AccountType::MintVault => Err(anyhow!("")),
            _ => Err(anyhow!("")),
        },
    }
}

#[inline]
pub fn slice_data_with_dex_type_and_account_type_for_dynamic(
    dex_type: DexType,
    account_type: AccountType,
    data: &[u8],
) -> anyhow::Result<Vec<u8>> {
    match dex_type {
        DexType::RaydiumAMM => match account_type {
            AccountType::Pool => Ok(retain_intervals_unsafe(
                data,
                &RAYDIUM_AMM_POOL_DYNAMIC_SLICE.get().unwrap().0,
                RAYDIUM_AMM_POOL_DYNAMIC_SLICE.get().unwrap().1,
            )),
            AccountType::MintVault => Ok(retain_intervals_unsafe(
                data,
                &MINT_VAULT_DYNAMIC_SLICE.get().unwrap().0,
                MINT_VAULT_DYNAMIC_SLICE.get().unwrap().1,
            )),
            _ => Err(anyhow!("")),
        },
        DexType::RaydiumCLMM => match account_type {
            AccountType::Pool => Ok(retain_intervals_unsafe(
                data,
                &RAYDIUM_CLMM_POOL_DYNAMIC_SLICE.get().unwrap().0,
                RAYDIUM_CLMM_POOL_DYNAMIC_SLICE.get().unwrap().1,
            )),
            AccountType::TickArrayState => Ok(retain_intervals_unsafe(
                data,
                &RAYDIUM_CLMM_TICK_ARRAY_STATE_DYNAMIC_SLICE
                    .get()
                    .unwrap()
                    .0
                    .as_slice(),
                RAYDIUM_CLMM_TICK_ARRAY_STATE_DYNAMIC_SLICE.get().unwrap().1,
            )),
            // 不做切片
            AccountType::TickArrayBitmapExtension => Ok(data.to_vec()),
            _ => Err(anyhow!("")),
        },
        DexType::PumpFunAMM => match account_type {
            AccountType::MintVault => Ok(retain_intervals_unsafe(
                data,
                &MINT_VAULT_DYNAMIC_SLICE.get().unwrap().0,
                MINT_VAULT_DYNAMIC_SLICE.get().unwrap().1,
            )),
            _ => Err(anyhow!("")),
        },
    }
}

#[inline]
pub fn slice_data(account_key: &Pubkey, owner: &Pubkey, data: &[u8]) -> anyhow::Result<Vec<u8>> {
    match DEX_METADATA
        .get()
        .unwrap()
        .get_dex_type_and_account_type(owner, account_key)
    {
        None => Err(anyhow!("")),
        Some((dex_type, account_type)) => {
            slice_data_with_dex_type_and_account_type_for_dynamic(dex_type, account_type, data)
        }
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

pub fn init_data_slice_config() {
    init_raydium_amm_pool_data_slice();
    init_raydium_clmm_data_slice();
    init_pump_fun_data_slice();
}

fn init_raydium_amm_pool_data_slice() {
    RAYDIUM_AMM_POOL_DYNAMIC_SLICE
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
    init_mint_vault_data_slice();
    RAYDIUM_AMM_POOL_DATA_STATIC_SLICE
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

fn init_pump_fun_data_slice() {
    init_mint_vault_data_slice();
    PUMP_FUN_POOL_STATIC_SLICE
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
    PUMP_FUN_GLOBAL_CONFIG_STATIC_SLICE
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
        .unwrap()
}

fn init_raydium_clmm_data_slice() {
    RAYDIUM_CLMM_POOL_DYNAMIC_SLICE
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
    RAYDIUM_CLMM_TICK_ARRAY_STATE_DYNAMIC_SLICE
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
            data_slice.push((start_index, start_index + 1));
            total_len += 1;
            (data_slice, total_len)
        })
        .unwrap();
}

fn init_mint_vault_data_slice() {
    // amount
    MINT_VAULT_DYNAMIC_SLICE.set(([(64, 64 + 8)], 8)).unwrap()
}
