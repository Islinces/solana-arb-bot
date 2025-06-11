use crate::dex::meteora_dlmm::instruction::MeteoraDLMMInstructionMaterialConverter;
use crate::dex::orca_whirlpools::OrcaWhirlInstructionMaterialConverter;
use crate::dex::pump_fun::instruction::PumpFunAMMInstructionMaterialConverter;
use crate::dex::raydium_amm::instruction::RaydiumAMMInstructionMaterialConverter;
use crate::dex::raydium_clmm::instruction::RaydiumCLMMInstructionMaterialConverter;
use crate::dex::DexType;
use enum_dispatch::enum_dispatch;
use smallvec::SmallVec;
use solana_sdk::address_lookup_table::AddressLookupTableAccount;
use solana_sdk::instruction::AccountMeta;
use solana_sdk::pubkey::Pubkey;
use crate::metadata::MintAtaPair;

#[enum_dispatch]
pub trait InstructionMaterialConverter {
    fn convert_to_instruction_material(
        &self,
        pool_id: &Pubkey,
        swap_direction: bool,
    ) -> anyhow::Result<InstructionMaterial>;
}

#[enum_dispatch(InstructionMaterialConverter)]
pub enum SwapInstructionType {
    MeteoraDLMM(MeteoraDLMMInstructionMaterialConverter),
    OrcaWhirl(OrcaWhirlInstructionMaterialConverter),
    PumpFunAMM(PumpFunAMMInstructionMaterialConverter),
    RaydiumAmm(RaydiumAMMInstructionMaterialConverter),
    RaydiumCLMM(RaydiumCLMMInstructionMaterialConverter),
}

pub struct InstructionMaterial {
    pub dex_type: DexType,
    pub swap_direction: bool,
    pub account_meta: Vec<AccountMeta>,
    pub remaining_account_num: Option<u8>,
    pub alts: Option<Vec<AddressLookupTableAccount>>,
    pub used_atas: Vec<MintAtaPair>,
}

impl InstructionMaterial {
    pub fn new(
        dex_type: DexType,
        swap_direction: bool,
        account_meta: Vec<AccountMeta>,
        remaining_account_num: Option<u8>,
        alts: Option<Vec<AddressLookupTableAccount>>,
        used_atas: Vec<MintAtaPair>,
    ) -> Self {
        Self {
            dex_type,
            swap_direction,
            account_meta,
            remaining_account_num,
            alts,
            used_atas,
        }
    }
}

pub fn get_instruction_builder(dex_type: &DexType) -> anyhow::Result<SwapInstructionType> {
    match dex_type {
        DexType::RaydiumAMM => Ok(SwapInstructionType::from(
            RaydiumAMMInstructionMaterialConverter,
        )),
        DexType::RaydiumCLMM => Ok(SwapInstructionType::from(
            RaydiumCLMMInstructionMaterialConverter,
        )),
        DexType::PumpFunAMM => Ok(SwapInstructionType::from(
            PumpFunAMMInstructionMaterialConverter,
        )),
        DexType::MeteoraDLMM => Ok(SwapInstructionType::from(
            MeteoraDLMMInstructionMaterialConverter,
        )),
        DexType::OrcaWhirl => Ok(SwapInstructionType::from(
            OrcaWhirlInstructionMaterialConverter,
        )),
    }
}
