use anyhow::anyhow;
use solana_sdk::instruction::AccountMeta;
use solana_sdk::message::AddressLookupTableAccount;
use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

mod account_relation;
mod data_slice;
mod global_cache;
pub mod meteora_damm_v2;
pub mod meteora_dlmm;
pub mod orca_whirlpools;
mod pump_fun;
mod quoter;
pub mod raydium_amm;
pub mod raydium_clmm;
pub mod raydium_cpmm;
mod snapshot;
mod subscriber;
mod swap_instruction;
mod utils;

pub use account_relation::*;
pub use data_slice::*;
pub use global_cache::*;
pub use meteora_dlmm::{BinArray, BinArrayBitmapExtension, LbPair};
pub use orca_whirlpools::accounts::*;
pub use pump_fun::state::*;
pub use quoter::*;
pub use raydium_amm::state::*;
pub use raydium_clmm::state::*;
pub use snapshot::*;
pub use subscriber::*;
pub use swap_instruction::*;
pub use utils::read_from;

pub const ATA_PROGRAM_ID: Pubkey = pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
pub const SYSTEM_PROGRAM_ID: Pubkey = pubkey!("11111111111111111111111111111111");
pub const MINT_PROGRAM_ID: Pubkey = spl_token::ID;
pub const MINT2022_PROGRAM_ID: Pubkey = spl_token_2022::ID;
pub const CLOCK_ID: Pubkey = pubkey!("SysvarC1ock11111111111111111111111111111111");
pub const MEMO_PROGRAM: Pubkey = pubkey!("Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo");
pub const MEMO_PROGRAM_V2: Pubkey = pubkey!("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr");

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub enum DexType {
    RaydiumAMM,
    RaydiumCLMM,
    RaydiumCPMM,
    PumpFunAMM,
    MeteoraDLMM,
    MeteoraDAMMV2,
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
        } else if owner == DexType::MeteoraDAMMV2.get_ref_program_id() {
            Ok(DexType::MeteoraDAMMV2)
        } else if owner == DexType::RaydiumCPMM.get_ref_program_id() {
            Ok(DexType::RaydiumCPMM)
        } else {
            Err(anyhow!("无效的Owner"))
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Copy)]
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
            DexType::MeteoraDAMMV2 => "MeteoraDAMMV2",
            DexType::OrcaWhirl => "OrcaWhirl",
            DexType::RaydiumCPMM => "RaydiumCPMM",
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
            DexType::MeteoraDAMMV2 => &meteora_damm_v2::DAMM_V2_PROGRAM_ID,
            DexType::RaydiumCPMM => &raydium_cpmm::RAYDIUM_CPMM_PROGRAM_ID,
        }
    }
}

pub(crate) fn get_transfer_fee(mint: &Pubkey, epoch: u64, pre_fee_amount: u64) -> u64 {
    if let Some(fee_config) = get_token2022_data(mint) {
        fee_config
            .calculate_epoch_fee(epoch, pre_fee_amount)
            .unwrap()
    } else {
        0
    }
}

pub trait FromCache {
    fn from_cache(
        static_cache: Option<Arc<Vec<u8>>>,
        dynamic_cache: Option<Arc<Vec<u8>>>,
    ) -> anyhow::Result<Self>
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
