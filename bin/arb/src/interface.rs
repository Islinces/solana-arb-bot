use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;
use std::fmt::{Debug, Display, Formatter};

pub const ATA_PROGRAM_ID: Pubkey = pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
pub const SYSTEM_PROGRAM_ID: Pubkey = pubkey!("11111111111111111111111111111111");
pub const MINT_PROGRAM_ID: Pubkey = spl_token::ID;
pub const MINT2022_PROGRAM_ID: Pubkey = spl_token_2022::ID;
pub const CLOCK_ID: Pubkey = pubkey!("SysvarC1ock11111111111111111111111111111111");

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub enum DexType {
    RaydiumAMM,
    RaydiumCLMM,
    PumpFunAMM,
    MeteoraDLMM,
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
}

impl Display for DexType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            DexType::RaydiumAMM => "RaydiumAMM",
            DexType::RaydiumCLMM => "RaydiumCLmm",
            DexType::PumpFunAMM => "PumpFunAMM",
            DexType::MeteoraDLMM => "MeteoraDLMM",
        })
    }
}

impl DexType {
    pub fn get_ref_program_id(&self) -> &Pubkey {
        match self {
            DexType::RaydiumAMM => &crate::dex::raydium_amm::RAYDIUM_AMM_PROGRAM_ID,
            DexType::RaydiumCLMM => &crate::dex::raydium_clmm::RAYDIUM_CLMM_PROGRAM_ID,
            DexType::PumpFunAMM => &crate::dex::pump_fun::PUMP_FUN_AMM_PROGRAM_ID,
            DexType::MeteoraDLMM => &crate::dex::meteora_dlmm::METEORA_DLMM_PROGRAM_ID,
        }
    }
}

#[inline]
pub fn get_dex_type_with_program_id(program_id: &Pubkey) -> Option<DexType> {
    if program_id == &crate::dex::raydium_clmm::RAYDIUM_CLMM_PROGRAM_ID {
        Some(DexType::RaydiumCLMM)
    } else if program_id == &crate::dex::raydium_amm::RAYDIUM_AMM_PROGRAM_ID {
        Some(DexType::RaydiumAMM)
    } else if program_id == &crate::dex::pump_fun::PUMP_FUN_AMM_PROGRAM_ID {
        Some(DexType::PumpFunAMM)
    } else if program_id == &crate::dex::meteora_dlmm::METEORA_DLMM_PROGRAM_ID {
        Some(DexType::MeteoraDLMM)
    } else {
        unreachable!()
    }
}
