use solana_sdk::pubkey::Pubkey;
use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;

pub type SubscribeKey = (DexType, GrpcAccountUpdateType);

const RAYDIUM_AMM_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    75, 217, 73, 196, 54, 2, 195, 63, 32, 119, 144, 237, 22, 163, 82, 76, 161, 185, 151, 92, 241,
    33, 162, 169, 12, 255, 236, 125, 248, 182, 138, 205,
]);
const RAYDIUM_AMM_PROGRAM_ID_STR: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";

pub const RAYDIUM_AMM_VAULT_OWNER: Pubkey = Pubkey::new_from_array([
    75, 217, 73, 196, 54, 2, 195, 63, 32, 119, 144, 237, 22, 163, 82, 76, 161, 185, 151, 92, 241,
    33, 162, 169, 12, 255, 236, 125, 248, 182, 138, 205,
]);

const RAYDIUM_CLMM_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    165, 213, 202, 158, 4, 207, 93, 181, 144, 183, 20, 186, 47, 227, 44, 177, 89, 19, 63, 193, 193,
    146, 183, 34, 87, 253, 7, 211, 156, 176, 64, 30,
]);
const RAYDIUM_CLMM_PROGRAM_ID_STR: &str = "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK";

const PUMP_FUN_AMM_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    12, 20, 222, 252, 130, 94, 198, 118, 148, 37, 8, 24, 187, 101, 64, 101, 244, 41, 141, 49, 86,
    213, 113, 180, 212, 248, 9, 12, 24, 233, 168, 99,
]);
const PUMP_FUN_AMM_PROGRAM_ID_STR: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";

const METEORA_DLMM_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    4, 233, 225, 47, 188, 132, 232, 38, 201, 50, 204, 233, 226, 100, 12, 206, 21, 89, 12, 28, 98,
    115, 176, 146, 87, 8, 186, 59, 133, 32, 176, 188,
]);
const METEORA_DLMM_PROGRAM_ID_STR: &str = "LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo";

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

    pub fn get_str_program_id(&self) -> &str {
        match self {
            DexType::RaydiumAMM => RAYDIUM_AMM_PROGRAM_ID_STR,
            DexType::RaydiumCLmm => RAYDIUM_CLMM_PROGRAM_ID_STR,
            DexType::PumpFunAMM => PUMP_FUN_AMM_PROGRAM_ID_STR,
            DexType::MeteoraDLMM => METEORA_DLMM_PROGRAM_ID_STR,
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
