use anyhow::anyhow;
use parking_lot::RwLockReadGuard;
use solana_sdk::instruction::AccountMeta;
use solana_sdk::message::AddressLookupTableAccount;
use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;
use std::fmt::{Display, Formatter};

mod account_relation;
mod data_slice;
mod global_cache;
mod meteora_dlmm;
mod orca_whirlpools;
mod pump_fun;
mod quoter;
mod raydium_amm;
mod raydium_clmm;
mod snapshot;
mod subscriber;
mod swap_instruction;
mod utils;

pub use account_relation::*;
pub use data_slice::*;
pub use global_cache::*;
pub use quoter::*;
pub use snapshot::*;
pub use subscriber::*;
pub use swap_instruction::*;

pub trait FromCache {
    fn from_cache(
        account_key: &Pubkey,
        static_cache: RwLockReadGuard<StaticCache>,
        dynamic_cache: &DynamicCache,
    ) -> Option<Self>
    where
        Self: Sized;
}

pub struct InstructionItem {
    pub dex_type: DexType,
    pub swap_direction: bool,
    pub account_meta: Vec<AccountMeta>,
    pub alts: Vec<AddressLookupTableAccount>,
}

impl InstructionItem {
    pub fn new(
        dex_type: DexType,
        swap_direction: bool,
        account_meta: Vec<AccountMeta>,
        alts: Vec<AddressLookupTableAccount>,
    ) -> Self {
        Self {
            dex_type,
            swap_direction,
            account_meta,
            alts,
        }
    }
}

pub(crate) fn get_transfer_fee(mint: &Pubkey, epoch: u64, pre_fee_amount: u64) -> u64 {
    if let Some(fee_config) = global_cache::get_token2022_data(mint) {
        fee_config
            .calculate_epoch_fee(epoch, pre_fee_amount)
            .unwrap()
    } else {
        0
    }
}

pub const ATA_PROGRAM_ID: Pubkey = pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
pub const SYSTEM_PROGRAM_ID: Pubkey = pubkey!("11111111111111111111111111111111");
pub const MINT_PROGRAM_ID: Pubkey = spl_token::ID;
pub const MINT2022_PROGRAM_ID: Pubkey = spl_token_2022::ID;
pub const CLOCK_ID: Pubkey = pubkey!("SysvarC1ock11111111111111111111111111111111");
pub const MEMO_PROGRAM: Pubkey = pubkey!("Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo");

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub enum DexType {
    RaydiumAMM,
    RaydiumCLMM,
    PumpFunAMM,
    MeteoraDLMM,
    OrcaWhirl,
}

impl TryFrom<&Pubkey> for DexType {
    type Error = anyhow::Error;

    fn try_from(owner: &Pubkey) -> Result<Self, Self::Error> {
        if owner == &spl_token::ID || owner == DexType::RaydiumAMM.get_ref_program_id() {
            Ok(DexType::RaydiumAMM)
        } else if owner == DexType::PumpFunAMM.get_ref_program_id() {
            Ok(DexType::PumpFunAMM)
        } else if owner == DexType::RaydiumCLMM.get_ref_program_id() {
            Ok(DexType::RaydiumCLMM)
        } else if owner == DexType::MeteoraDLMM.get_ref_program_id() {
            Ok(DexType::MeteoraDLMM)
        } else if owner == DexType::OrcaWhirl.get_ref_program_id() {
            Ok(DexType::OrcaWhirl)
        } else {
            Err(anyhow!("无效的Owner"))
        }
    }
}

#[derive(Debug, Clone)]
pub enum AccountType {
    // common
    Pool,
    MintVault,
    // clmm
    AmmConfig,
    TickArray,
    TickArrayBitmap,
    // pumpfun
    PumpFunGlobalConfig,
    // dlmm
    BinArray,
    BinArrayBitmap,
    Clock,
    // orca whirl
    Oracle,
}

impl Display for DexType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            DexType::RaydiumAMM => "RaydiumAMM",
            DexType::RaydiumCLMM => "RaydiumCLmm",
            DexType::PumpFunAMM => "PumpFunAMM",
            DexType::MeteoraDLMM => "MeteoraDLMM",
            DexType::OrcaWhirl => "OrcaWhirl",
        })
    }
}

impl DexType {
    pub fn get_ref_program_id(&self) -> &Pubkey {
        match self {
            DexType::RaydiumAMM => &raydium_amm::RAYDIUM_AMM_PROGRAM_ID,
            DexType::RaydiumCLMM => &raydium_clmm::RAYDIUM_CLMM_PROGRAM_ID,
            DexType::PumpFunAMM => &pump_fun::PUMP_FUN_AMM_PROGRAM_ID,
            DexType::MeteoraDLMM => &meteora_dlmm::METEORA_DLMM_PROGRAM_ID,
            DexType::OrcaWhirl => &orca_whirlpools::WHIRLPOOL_ID,
        }
    }
}

#[inline]
pub fn get_dex_type_with_program_id(program_id: &Pubkey) -> Option<DexType> {
    if program_id == DexType::RaydiumCLMM.get_ref_program_id() {
        Some(DexType::RaydiumCLMM)
    } else if program_id == DexType::RaydiumAMM.get_ref_program_id() {
        Some(DexType::RaydiumAMM)
    } else if program_id == DexType::PumpFunAMM.get_ref_program_id() {
        Some(DexType::PumpFunAMM)
    } else if program_id == DexType::MeteoraDLMM.get_ref_program_id() {
        Some(DexType::MeteoraDLMM)
    } else if program_id == DexType::OrcaWhirl.get_ref_program_id() {
        Some(DexType::OrcaWhirl)
    } else {
        unreachable!()
    }
}
