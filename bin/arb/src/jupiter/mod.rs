use crate::dex::InstructionItem;
use crate::interface::{DexType, MINT_PROGRAM_ID};
use crate::jupiter::jupiter_route::RouteBuilder;
use crate::jupiter::route_plan_step::RoutePlanStep;
use crate::jupiter::swap::Swap;
use crate::metadata::{get_arb_mint_ata, get_keypair};
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
    instructions: Vec<InstructionItem>,
    amount_in_mint: Pubkey,
    amount_in: u64,
    tip: u64,
) -> Option<(Instruction, Vec<AddressLookupTableAccount>)> {
    let mut route_builder = RouteBuilder::new();
    let mut remaining_accounts = Vec::with_capacity(100);
    let mut route_plan = Vec::with_capacity(2);
    let mut alts = Vec::with_capacity(2);
    for (index, instruction_item) in instructions.into_iter().enumerate() {
        let swap =
            get_jupiter_swap_type(&instruction_item.dex_type, instruction_item.swap_direction);
        remaining_accounts.push(AccountMeta::new_readonly(
            instruction_item.dex_type.get_ref_program_id().clone(),
            false,
        ));
        remaining_accounts.extend(instruction_item.account_meta);
        if swap == Swap::MeteoraDlmm || swap == Swap::RaydiumClmm {
            remaining_accounts.push(AccountMeta::new_readonly(JUPITER_ID, false));
        }
        alts.extend(instruction_item.alts);
        route_plan.push(RoutePlanStep {
            swap,
            percent: 100,
            input_index: if index == 0 { 0 } else { 1 },
            output_index: if index == 0 { 1 } else { 0 },
        })
    }
    let arb_mint_ata = get_arb_mint_ata();
    route_builder
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
        .add_remaining_accounts(remaining_accounts.as_slice());
    Some((route_builder.instruction(), alts))
}

fn get_jupiter_swap_type(dex_type: &DexType, swap_direction: bool) -> Swap {
    match dex_type {
        DexType::RaydiumAMM => Swap::Raydium,
        // DexType::RaydiumCLMM => Swap::RaydiumClmmV2,
        DexType::RaydiumCLMM => Swap::RaydiumClmm,
        DexType::PumpFunAMM => {
            if swap_direction {
                Swap::PumpdotfunAmmSell
            } else {
                Swap::PumpdotfunAmmBuy
            }
        }
        DexType::MeteoraDLMM => Swap::MeteoraDlmm,
        DexType::OrcaWhirl => Swap::WhirlpoolSwapV2 {
            //TODO:
            a_to_b: swap_direction,
            remaining_accounts_info: None,
        },
    }
}
