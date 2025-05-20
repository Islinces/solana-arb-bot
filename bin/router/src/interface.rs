use solana_sdk::pubkey::Pubkey;
use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;

pub type SubscribeKey = (DexType, GrpcAccountUpdateType);

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
            DexType::PumpFunAMM => "PumpFunAM",
            DexType::MeteoraDLMM => "MeteoraDLMM",
        })
    }
}

impl DexType {
    pub fn get_owner(&self) -> Pubkey {
        match self {
            DexType::RaydiumAMM => Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8").unwrap(),
            DexType::RaydiumCLmm => Pubkey::from_str("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK").unwrap(),
            DexType::PumpFunAMM => Pubkey::from_str("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA").unwrap(),
            DexType::MeteoraDLMM => Pubkey::from_str("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo").unwrap(),
        }
    }

    pub fn get_program_id(&self) -> Pubkey {
        self.get_owner()
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
