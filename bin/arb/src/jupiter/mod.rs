use crate::dex::InstructionMaterial;
use crate::dex::{DexType, MINT_PROGRAM_ID};
use crate::graph::SearchResult;
use crate::jupiter::accounts_type::AccountsType;
use crate::jupiter::jupiter_route::RouteBuilder;
use crate::jupiter::remaining_accounts_info::RemainingAccountsInfo;
use crate::jupiter::remaining_accounts_slice::RemainingAccountsSlice;
use crate::jupiter::route_plan_step::RoutePlanStep;
use crate::jupiter::swap::Swap;
use crate::metadata::{get_arb_mint_ata, get_keypair, remove_already_ata, MintAtaPair};
use crate::HopPathSearchResult;
use ahash::AHashSet;
use anyhow::Result;
use solana_sdk::address_lookup_table::AddressLookupTableAccount;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::Signer;

mod accounts_type;
mod jupiter_route;
mod remaining_accounts_info;
mod remaining_accounts_slice;
mod route_plan_step;
mod side;
pub mod swap;

/// `jupiter` program ID.
const JUPITER_ID: Pubkey = pubkey!("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4");
const JUPITER_EVENT_AUTHORITY: Pubkey = pubkey!("D8cy77BBepLMngZx6ZukaTff5hCt1HrWyKk3Hnd9oitf");

pub fn build_jupiter_swap_ix(
    hop_path_search_result: HopPathSearchResult,
    tip: u64,
) -> Result<(
    Instruction,
    AHashSet<MintAtaPair>,
    Vec<AddressLookupTableAccount>,
)> {
    let mut remaining_accounts = Vec::with_capacity(100);
    let mut route_plan = Vec::with_capacity(2);
    let mut alts = Vec::with_capacity(2);
    let instruction_materials: Vec<InstructionMaterial> =
        hop_path_search_result.convert_to_instruction_materials()?;
    let mut used_atas = AHashSet::with_capacity(instruction_materials.len() * 2);
    for (index, mut material) in instruction_materials.into_iter().enumerate() {
        let (swap, append_jup_program) = get_jupiter_swap_type(&mut material)?;
        remaining_accounts.push(AccountMeta::new_readonly(
            material.dex_type.get_ref_program_id().clone(),
            false,
        ));
        remaining_accounts.extend(material.account_meta);
        if append_jup_program {
            remaining_accounts.push(AccountMeta::new_readonly(JUPITER_ID, false));
        }
        alts.extend(material.alts.unwrap_or(vec![]));
        route_plan.push(RoutePlanStep {
            swap,
            percent: 100,
            // TODO 多跳的时候index如何确定
            input_index: if index == 0 { 0 } else { 1 },
            output_index: if index == 0 { 1 } else { 0 },
        });
        used_atas.extend(material.used_atas);
    }
    remove_already_ata(&mut used_atas);
    let arb_mint_ata = get_arb_mint_ata();
    let (amount_in, amount_in_mint): (u64, Pubkey) = hop_path_search_result.amount_in();
    let instruction = RouteBuilder::new()
        .user_transfer_authority(get_keypair().pubkey())
        .user_source_token_account(arb_mint_ata)
        .user_destination_token_account(arb_mint_ata)
        .destination_mint(amount_in_mint)
        .destination_token_account(Some(JUPITER_ID))
        .platform_fee_account(Some(JUPITER_ID))
        .program(JUPITER_ID)
        .token_program(MINT_PROGRAM_ID)
        .event_authority(JUPITER_EVENT_AUTHORITY)
        .in_amount(amount_in)
        .quoted_out_amount(amount_in + tip + 10_000)
        .slippage_bps(0)
        .platform_fee_bps(0)
        .route_plan(route_plan)
        .add_remaining_accounts(remaining_accounts.as_slice())
        .instruction();
    Ok((instruction, used_atas, alts))
}

fn get_jupiter_swap_type(instruction_material: &mut InstructionMaterial) -> Result<(Swap, bool)> {
    match instruction_material.dex_type {
        DexType::RaydiumAMM => Ok((Swap::Raydium, false)),
        // DexType::RaydiumCLMM => Swap::RaydiumClmmV2,
        DexType::RaydiumCLMM => Ok((Swap::RaydiumClmm, true)),
        DexType::PumpFunAMM => Ok((
            if instruction_material.swap_direction {
                Swap::PumpdotfunAmmSell
            } else {
                Swap::PumpdotfunAmmBuy
            },
            false,
        )),
        DexType::MeteoraDLMM => Ok((Swap::MeteoraDlmm, true)),
        DexType::OrcaWhirl => Ok((
            Swap::WhirlpoolSwapV2 {
                a_to_b: instruction_material.swap_direction,
                // 设置remaining account的数量&类型
                remaining_accounts_info: {
                    instruction_material
                        .remaining_account_num
                        .map_or(None, |num| {
                            Some(RemainingAccountsInfo {
                                slices: vec![RemainingAccountsSlice {
                                    accounts_type: AccountsType::SupplementalTickArrays,
                                    length: num,
                                }],
                            })
                        })
                },
            },
            false,
        )),
        DexType::MeteoraDAMMV2 => Ok((Swap::MeteoraDammV2, false)),
        DexType::RaydiumCPMM => Ok((Swap::RaydiumCP, false)),
    }
}
