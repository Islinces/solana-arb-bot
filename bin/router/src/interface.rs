use crate::dex_data::DexJson;
use crate::state::DexMetadata;
use anyhow::anyhow;
use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;
use std::fmt::{Debug, Display, Formatter};
use std::fs::File;
use std::ptr;
use tokio::sync::OnceCell;
use tracing::error;

pub const RAYDIUM_AMM_PROGRAM_ID: Pubkey = pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");
pub const RAYDIUM_AMM_VAULT_OWNER: Pubkey = pubkey!("5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1");
pub const RAYDIUM_CLMM_PROGRAM_ID: Pubkey = pubkey!("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK");
pub const PUMP_FUN_AMM_PROGRAM_ID: Pubkey = pubkey!("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA");
pub const METEORA_DLMM_PROGRAM_ID: Pubkey = pubkey!("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo");

pub static RAYDIUM_AMM_POOL_DATA_SLICE: OnceCell<([(usize, usize); 2], usize)> =
    OnceCell::const_new();
pub static RAYDIUM_CLMM_POOL_DATA_SLICE: OnceCell<([(usize, usize); 4], usize)> =
    OnceCell::const_new();
pub static RAYDIUM_CLMM_TICK_ARRAY_STATE_DATA_SLICE: OnceCell<(Vec<(usize, usize)>, usize)> =
    OnceCell::const_new();

pub static MINT_VAULT_DATA_SLICE: OnceCell<([(usize, usize); 1], usize)> = OnceCell::const_new();

pub static DEX_METADATA: OnceCell<DexMetadata> = OnceCell::const_new();

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DexType {
    RaydiumAMM,
    RaydiumCLMM,
    PumpFunAMM,
    // MeteoraDLMM,
}

pub enum AccountType {
    Pool,
    MintVault,
    TickArrayState,
    TickArrayBitmapExtension,
}

impl Display for DexType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            DexType::RaydiumAMM => "RaydiumAMM",
            DexType::RaydiumCLMM => "RaydiumCLmm",
            DexType::PumpFunAMM => "PumpFunAMM",
        })
    }
}

impl DexType {
    pub fn get_ref_program_id(&self) -> &Pubkey {
        match self {
            DexType::RaydiumAMM => &RAYDIUM_AMM_PROGRAM_ID,
            DexType::RaydiumCLMM => &RAYDIUM_CLMM_PROGRAM_ID,
            DexType::PumpFunAMM => &PUMP_FUN_AMM_PROGRAM_ID,
        }
    }
}

#[inline]
pub fn is_follow_vault(vault_account: &Pubkey) -> Option<(Pubkey, DexType)> {
    DEX_METADATA
        .get()
        .unwrap()
        .get_dex_type_and_pool_id_for_vault(vault_account)
}

#[inline]
pub fn get_dex_type_with_program_id(program_id: &Pubkey) -> Option<DexType> {
    if program_id == &RAYDIUM_CLMM_PROGRAM_ID {
        Some(DexType::RaydiumCLMM)
    } else if program_id == &RAYDIUM_AMM_PROGRAM_ID {
        Some(DexType::RaydiumAMM)
    } else if program_id == &PUMP_FUN_AMM_PROGRAM_ID {
        Some(DexType::PumpFunAMM)
    } else {
        None
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
        Some((dex_type, account_type)) => match dex_type {
            DexType::RaydiumAMM => match account_type {
                AccountType::Pool => Ok(retain_intervals_unsafe(
                    data,
                    &RAYDIUM_AMM_POOL_DATA_SLICE.get().unwrap().0,
                    RAYDIUM_AMM_POOL_DATA_SLICE.get().unwrap().1,
                )),
                AccountType::MintVault => Ok(retain_intervals_unsafe(
                    data,
                    &MINT_VAULT_DATA_SLICE.get().unwrap().0,
                    MINT_VAULT_DATA_SLICE.get().unwrap().1,
                )),
                _ => Err(anyhow!("")),
            },
            DexType::RaydiumCLMM => match account_type {
                AccountType::Pool => Ok(retain_intervals_unsafe(
                    data,
                    &RAYDIUM_CLMM_POOL_DATA_SLICE.get().unwrap().0,
                    RAYDIUM_CLMM_POOL_DATA_SLICE.get().unwrap().1,
                )),
                AccountType::TickArrayState => Ok(retain_intervals_unsafe(
                    data,
                    &RAYDIUM_CLMM_TICK_ARRAY_STATE_DATA_SLICE
                        .get()
                        .unwrap()
                        .0
                        .as_slice(),
                    RAYDIUM_CLMM_TICK_ARRAY_STATE_DATA_SLICE.get().unwrap().1,
                )),
                // 不做切片
                AccountType::TickArrayBitmapExtension => Ok(data.to_vec()),
                _ => Err(anyhow!("")),
            },
            DexType::PumpFunAMM => match account_type {
                AccountType::MintVault => Ok(retain_intervals_unsafe(
                    data,
                    &MINT_VAULT_DATA_SLICE.get().unwrap().0,
                    MINT_VAULT_DATA_SLICE.get().unwrap().1,
                )),
                _ => Err(anyhow!("")),
            },
        },
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

pub fn init_dex_data(dex_json_path: String) -> anyhow::Result<Vec<DexJson>> {
    let dex_data: Vec<DexJson> = match File::open(dex_json_path.as_str()) {
        Ok(file) => serde_json::from_reader(file).expect("解析【dex_data.json】失败"),
        Err(e) => {
            error!("{}", e);
            vec![]
        }
    };
    if dex_data.is_empty() {
        Err(anyhow!("json文件无数据"))
    } else {
        init_raydium_amm_pool_data_slice();
        init_mint_vault_data_slice();
        init_raydium_clmm_data_slice();
        DEX_METADATA.set(DexMetadata::new(&dex_data)?)?;
        Ok(dex_data)
    }
}

fn init_raydium_amm_pool_data_slice() {
    RAYDIUM_AMM_POOL_DATA_SLICE
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
        .unwrap()
}

fn init_mint_vault_data_slice() {
    // amount
    MINT_VAULT_DATA_SLICE.set(([(64, 64 + 8)], 8)).unwrap()
}

fn init_raydium_clmm_data_slice() {
    RAYDIUM_CLMM_POOL_DATA_SLICE
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
    RAYDIUM_CLMM_TICK_ARRAY_STATE_DATA_SLICE
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
