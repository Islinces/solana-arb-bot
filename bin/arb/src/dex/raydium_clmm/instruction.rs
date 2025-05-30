use crate::dex::raydium_clmm::state::{
    pda_bit_map_extension_key, PoolState, TickArrayBitmapExtension,
};
use crate::dex::raydium_clmm::utils::load_cur_and_next_specify_count_tick_array_key;
use crate::dex::InstructionItem;
use crate::interface::{DexType, ATA_PROGRAM_ID, MINT_PROGRAM_ID};
use crate::metadata::get_keypair;
use solana_sdk::instruction::AccountMeta;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::Signer;

pub fn to_instruction(pool_id: Pubkey, swap_direction: bool) -> Option<InstructionItem> {
    let wallet = get_keypair().pubkey();
    let pool_state = crate::account_cache::get_account_data::<PoolState>(&pool_id).unwrap();
    let mut accounts = Vec::with_capacity(11);
    // 1. wallet
    accounts.push(AccountMeta::new(wallet, true));
    // 2.amm config
    accounts.push(AccountMeta::new_readonly(pool_state.amm_config, false));
    // 3.pool state
    accounts.push(AccountMeta::new(pool_id, false));
    let (token_mint_0_ata, _) = Pubkey::find_program_address(
        &[
            &wallet.to_bytes(),
            &MINT_PROGRAM_ID.to_bytes(),
            &pool_state.token_mint_0.to_bytes(),
        ],
        &ATA_PROGRAM_ID,
    );
    let (token_mint_1_ata, _) = Pubkey::find_program_address(
        &[
            &wallet.to_bytes(),
            &MINT_PROGRAM_ID.to_bytes(),
            &pool_state.token_mint_1.to_bytes(),
        ],
        &ATA_PROGRAM_ID,
    );
    if swap_direction {
        // 4.token_mint_0_ata
        accounts.push(AccountMeta::new(token_mint_0_ata, false));
        // 5.token_mint_1_ata
        accounts.push(AccountMeta::new(token_mint_1_ata, false));
        // 6.token vault 0
        accounts.push(AccountMeta::new(pool_state.token_vault_0, false));
        // 7.token vault 1
        accounts.push(AccountMeta::new(pool_state.token_vault_1, false));
    } else {
        // 4.token_mint_1_ata
        accounts.push(AccountMeta::new(token_mint_1_ata, false));
        // 5.token_mint_0_ata
        accounts.push(AccountMeta::new(token_mint_0_ata, false));
        // 6.token vault 1
        accounts.push(AccountMeta::new(pool_state.token_vault_1, false));
        // 7.token vault 0
        accounts.push(AccountMeta::new(pool_state.token_vault_0, false));
    }
    // 8.Observation State
    accounts.push(AccountMeta::new(pool_state.observation_key, false));
    // 9.token program
    accounts.push(AccountMeta::new_readonly(MINT_PROGRAM_ID, false));
    // 10.current tick array
    let bit_map_extension_key = pda_bit_map_extension_key(&pool_id);
    let mut tick_arrays = load_cur_and_next_specify_count_tick_array_key(
        3,
        &pool_id,
        &pool_state,
        &crate::account_cache::get_account_data::<TickArrayBitmapExtension>(&bit_map_extension_key),
        swap_direction,
    )
    .map_or(None, |keys| {
        Some(
            keys.into_iter()
                .map(|k| AccountMeta::new(k, false))
                .collect::<Vec<_>>(),
        )
    })?;
    accounts.push(tick_arrays.remove(0));
    // 11.bitmap_extension
    accounts.push(AccountMeta::new(bit_map_extension_key, false));
    accounts.extend(tick_arrays);
    Some(InstructionItem::new(
        DexType::RaydiumCLMM,
        swap_direction,
        accounts,
        crate::account_cache::get_alt(&pool_id)?,
    ))
}
