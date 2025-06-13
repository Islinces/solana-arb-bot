use crate::dex::meteora_damm_v2::MeteoraDAMMV2DataSlicer;
use crate::dex::meteora_dlmm::MeteoraDLMMDataSlicer;
use crate::dex::orca_whirlpools::OrcaWhirlDataSlicer;
use crate::dex::pump_fun::PumpFunAMMDataSlicer;
use crate::dex::raydium_amm::RaydiumAMMDataSlicer;
use crate::dex::raydium_clmm::RaydiumCLMMDataSlicer;
use crate::dex::{AccountType, DexType};
use ahash::AHashMap;
use anyhow::anyhow;
use enum_dispatch::enum_dispatch;
use solana_sdk::clock::Clock;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::SysvarId;
use std::ptr;
use tokio::sync::{OnceCell, SetError};

static DATA_SLICE_PROCESSOR: OnceCell<AHashMap<DexType, DataSlice>> = OnceCell::const_new();
// ========================= dynamic data 账户订阅的数据切片 =========================
// mint vault
static DYNAMIC_MINT_VAULT_SLICE: OnceCell<([(usize, usize); 1], usize)> = OnceCell::const_new();

#[enum_dispatch]
pub trait DataSliceInitializer {
    fn try_init_data_slice_config(&self) -> anyhow::Result<()>;
    fn try_get_data_slice_size(
        &self,
        account_type: AccountType,
        slice_type: SliceType,
    ) -> anyhow::Result<Option<usize>>;
    fn try_slice_data(
        &self,
        account_type: AccountType,
        data: Vec<u8>,
        slice_type: SliceType,
    ) -> anyhow::Result<Vec<u8>>;

    fn try_init_mint_vault_data_slice(&self) -> anyhow::Result<()> {
        // amount
        match DYNAMIC_MINT_VAULT_SLICE.set(([(64, 64 + 8)], 8)) {
            Ok(_) => Ok(()),
            Err(SetError::AlreadyInitializedError(_)) => Ok(()),
            Err(e) => Err(anyhow!(e)),
        }
    }

    fn try_mint_vault_slice_data(&self, data: Vec<u8>) -> anyhow::Result<Vec<u8>> {
        Ok(retain_intervals_unsafe(
            data,
            &DYNAMIC_MINT_VAULT_SLICE.get().unwrap().0,
            DYNAMIC_MINT_VAULT_SLICE.get().unwrap().1,
        ))
    }

    fn try_get_mint_vault_data_slice_size(
        &self,
        slice_type: SliceType,
    ) -> anyhow::Result<Option<usize>> {
        match slice_type {
            SliceType::Subscribed => Ok(Some(DYNAMIC_MINT_VAULT_SLICE.get().unwrap().1)),
            SliceType::Unsubscribed => Ok(None),
        }
    }
}

#[derive(Debug)]
#[enum_dispatch(DataSliceInitializer)]
pub enum DataSlice {
    MeteoraDLMM(MeteoraDLMMDataSlicer),
    MeteoraDAMMV2(MeteoraDAMMV2DataSlicer),
    OrcaWhirl(OrcaWhirlDataSlicer),
    PumpFunAMM(PumpFunAMMDataSlicer),
    RaydiumAmm(RaydiumAMMDataSlicer),
    RaydiumCLMM(RaydiumCLMMDataSlicer),
}

pub enum SliceType {
    Subscribed,
    Unsubscribed,
}

pub fn init_data_slice_config() -> anyhow::Result<()> {
    let mut data_slicer = AHashMap::<DexType, DataSlice>::new();
    data_slicer.insert(DexType::MeteoraDLMM, DataSlice::from(MeteoraDLMMDataSlicer));
    data_slicer.insert(
        DexType::MeteoraDAMMV2,
        DataSlice::from(MeteoraDAMMV2DataSlicer),
    );
    data_slicer.insert(DexType::OrcaWhirl, DataSlice::from(OrcaWhirlDataSlicer));
    data_slicer.insert(DexType::PumpFunAMM, DataSlice::from(PumpFunAMMDataSlicer));
    data_slicer.insert(DexType::RaydiumAMM, DataSlice::from(RaydiumAMMDataSlicer));
    data_slicer.insert(DexType::RaydiumCLMM, DataSlice::from(RaydiumCLMMDataSlicer));
    DATA_SLICE_PROCESSOR.set(data_slicer)?;
    DATA_SLICE_PROCESSOR
        .get()
        .unwrap()
        .values()
        .for_each(|slice| {
            slice.try_init_data_slice_config().unwrap();
        });
    Ok(())
}

pub fn get_data_slice_size(
    dex_type: DexType,
    account_type: AccountType,
    slice_type: SliceType,
) -> anyhow::Result<Option<usize>> {
    pick_data_slicer(dex_type)?.try_get_data_slice_size(account_type, slice_type)
}

pub fn try_slice_data(
    dex_type: DexType,
    account_type: AccountType,
    data: Vec<u8>,
    slice_type: SliceType,
) -> anyhow::Result<Vec<u8>> {
    pick_data_slicer(dex_type)?.try_slice_data(account_type, data, slice_type)
}

fn pick_data_slicer(dex_type: DexType) -> anyhow::Result<&'static DataSlice> {
    DATA_SLICE_PROCESSOR
        .get()
        .unwrap()
        .get(&dex_type)
        .map_or(Err(anyhow!("")), |a| Ok(a))
}

pub fn slice_data_auto_get_dex_type(
    account_key: &Pubkey,
    owner: &Pubkey,
    data: Vec<u8>,
    slice_type: SliceType,
) -> anyhow::Result<Vec<u8>> {
    if account_key == &Clock::id() {
        Ok(data)
    } else {
        match crate::dex::account_relation::get_dex_type_and_account_type(owner, account_key) {
            None => Err(anyhow!("")),
            Some((dex_type, account_type)) => {
                try_slice_data(dex_type, account_type, data, slice_type)
            }
        }
    }
}

#[inline]
pub fn retain_intervals_unsafe(
    src: Vec<u8>,
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
