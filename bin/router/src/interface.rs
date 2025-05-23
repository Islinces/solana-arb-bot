use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;
use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;

pub type SubscribeKey = (DexType, GrpcAccountUpdateType);
const RAYDIUM_AMM_PROGRAM_ID: Pubkey = pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");

const RAYDIUM_CLMM_PROGRAM_ID: Pubkey =pubkey!("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK");

const PUMP_FUN_AMM_PROGRAM_ID: Pubkey = pubkey!("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA");

const METEORA_DLMM_PROGRAM_ID: Pubkey = pubkey!("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo");

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DexType {
    RaydiumAMM,
    RaydiumCLmm,
    PumpFunAMM,
    MeteoraDLMM,
}

impl Display for DexType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            DexType::RaydiumAMM => "RaydiumAMM",
            DexType::RaydiumCLmm => "RaydiumCLmm",
            DexType::PumpFunAMM => "PumpFunAMM",
            DexType::MeteoraDLMM => "MeteoraDLMM",
        })
    }
}

#[test]
fn test() {
    println!(
        "{:?}",
        Pubkey::from_str("5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1")
            .unwrap()
            .to_bytes()
    );
}

impl DexType {
    pub fn get_ref_program_id(&self) -> &Pubkey {
        match self {
            DexType::RaydiumAMM => &RAYDIUM_AMM_PROGRAM_ID,
            DexType::RaydiumCLmm => &RAYDIUM_CLMM_PROGRAM_ID,
            DexType::PumpFunAMM => &PUMP_FUN_AMM_PROGRAM_ID,
            DexType::MeteoraDLMM => &METEORA_DLMM_PROGRAM_ID,
        }
    }
}

#[derive(Eq, PartialEq, Hash, Debug, Clone)]
pub enum GrpcAccountUpdateType {
    Pool,
    BinArray,
    TickArrayState,
    MintVault,
    Clock,
    Transaction,
}
