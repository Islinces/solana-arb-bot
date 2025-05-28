use crate::dex::pump_fun::data_slice::init_pump_fun_data_slice;
use crate::dex::raydium_amm::data_slice::init_raydium_amm_data_slice;
use crate::dex::raydium_clmm::data_slice::init_raydium_clmm_data_slice;
use crate::interface::{AccountType, DexType};
use anyhow::anyhow;
use solana_sdk::pubkey::Pubkey;
use std::ptr;
use tokio::sync::OnceCell;

// ========================= dynamic data 账户订阅的数据切片 =========================
// mint vault
pub static DYNAMIC_MINT_VAULT_SLICE: OnceCell<([(usize, usize); 1], usize)> = OnceCell::const_new();

pub fn slice_data_for_static(
    dex_type: DexType,
    account_type: AccountType,
    data: &[u8],
) -> anyhow::Result<Vec<u8>> {
    match dex_type {
        DexType::RaydiumAMM => {
            crate::dex::raydium_amm::data_slice::slice_data_for_static(account_type, data)
        }
        DexType::RaydiumCLMM => {
            crate::dex::raydium_clmm::data_slice::slice_data_for_static(account_type, data)
        }
        DexType::PumpFunAMM => {
            crate::dex::pump_fun::data_slice::slice_data_for_static(account_type, data)
        }
        DexType::MeteoraDLMM => {
            unreachable!()
        }
    }
}

pub fn slice_data_for_dynamic(
    dex_type: DexType,
    account_type: AccountType,
    data: &[u8],
) -> anyhow::Result<Vec<u8>> {
    match dex_type {
        DexType::RaydiumAMM => {
            crate::dex::raydium_amm::data_slice::slice_data_for_dynamic(account_type, data)
        }
        DexType::RaydiumCLMM => {
            crate::dex::raydium_clmm::data_slice::slice_data_for_dynamic(account_type, data)
        }
        DexType::PumpFunAMM => {
            crate::dex::pump_fun::data_slice::slice_data_for_dynamic(account_type, data)
        }
        DexType::MeteoraDLMM => {
            unreachable!()
        }
    }
}

pub fn slice_data(account_key: &Pubkey, owner: &Pubkey, data: &[u8]) -> anyhow::Result<Vec<u8>> {
    match crate::account_relation::get_dex_type_and_account_type(owner, account_key) {
        None => Err(anyhow!("")),
        Some((dex_type, account_type)) => slice_data_for_dynamic(dex_type, account_type, data),
    }
}

#[inline]
pub fn retain_intervals_unsafe(
    src: &[u8],
    intervals: &[(usize, usize)],
    total_len: usize,
) -> Vec<u8> {
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
    init_mint_vault_data_slice();
    init_raydium_amm_data_slice();
    init_raydium_clmm_data_slice();
    init_pump_fun_data_slice();
}

fn init_mint_vault_data_slice() {
    // amount
    DYNAMIC_MINT_VAULT_SLICE
        .set({ ([(64, 64 + 8)], 8) })
        .unwrap();
}

pub fn get_slice_size(
    dex_type: DexType,
    account_type: AccountType,
    dynamic_flag: bool,
) -> anyhow::Result<Option<usize>> {
    match dex_type {
        DexType::RaydiumAMM => {
            crate::dex::raydium_amm::data_slice::get_slice_size(account_type, dynamic_flag)
        }
        DexType::RaydiumCLMM => {
            crate::dex::raydium_clmm::data_slice::get_slice_size(account_type, dynamic_flag)
        }
        DexType::PumpFunAMM => {
            crate::dex::pump_fun::data_slice::get_slice_size(account_type, dynamic_flag)
        }
        DexType::MeteoraDLMM => {
            unimplemented!()
        }
    }
}
