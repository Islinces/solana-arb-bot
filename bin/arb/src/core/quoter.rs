use crate::dex::meteora_dlmm::quote::MeteoraDLMMQuoter;
use crate::dex::orca_whirlpools::OrcaWhirlQuoter;
use crate::dex::pump_fun::quote::PumpFunAMMQuoter;
use crate::dex::raydium_amm::quote::RaydiumAMMQuoter;
use crate::dex::raydium_clmm::quote::RaydiumCLMMQuoter;
use crate::dex::DexType;
use enum_dispatch::enum_dispatch;
use solana_sdk::pubkey::Pubkey;

#[enum_dispatch]
pub trait Quoter {
    fn quote(&self, amount_in: u64, swap_direction: bool, pool_id: &Pubkey) -> Option<QuoteResult>;
}

#[derive(Debug)]
#[enum_dispatch(Quoter)]
pub enum QuoterType {
    MeteoraDLMM(MeteoraDLMMQuoter),
    OrcaWhirl(OrcaWhirlQuoter),
    PumpFunAMM(PumpFunAMMQuoter),
    RaydiumAmm(RaydiumAMMQuoter),
    RaydiumCLMM(RaydiumCLMMQuoter),
}

pub fn get_quoter_type(dex_type: DexType) -> anyhow::Result<QuoterType> {
    match dex_type {
        DexType::RaydiumAMM => Ok(QuoterType::from(RaydiumAMMQuoter)),
        DexType::RaydiumCLMM => Ok(QuoterType::from(RaydiumCLMMQuoter)),
        DexType::PumpFunAMM => Ok(QuoterType::from(PumpFunAMMQuoter)),
        DexType::MeteoraDLMM => Ok(QuoterType::from(MeteoraDLMMQuoter)),
        DexType::OrcaWhirl => Ok(QuoterType::from(OrcaWhirlQuoter)),
    }
}

pub struct QuoteResult {
    pub amount_out: u64,
}
