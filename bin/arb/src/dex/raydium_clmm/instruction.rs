use crate::account_cache::get_token_program;
use crate::dex::raydium_clmm::state::{
    pda_bit_map_extension_key, PoolState, TickArrayBitmapExtension,
};
use crate::dex::raydium_clmm::utils::load_cur_and_next_specify_count_tick_array_key;
use crate::dex::raydium_clmm::RAYDIUM_CLMM_MEMO_PROGRAM_ID;
use crate::interface::{ATA_PROGRAM_ID, MINT2022_PROGRAM_ID, MINT_PROGRAM_ID};
use crate::metadata::get_keypair;
use anyhow::{anyhow, Result};
use solana_sdk::instruction::AccountMeta;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::Signer;

pub fn to_instruction(pool_id: Pubkey, swap_direction: bool) -> Result<Vec<AccountMeta>> {
    let wallet = get_keypair().pubkey();
    let pool_state = crate::account_cache::get_account_data::<PoolState>(&pool_id).unwrap();
    let mut accounts = Vec::with_capacity(11);
    // 1. wallet
    accounts.push(AccountMeta::new(wallet, true));
    // 2.amm config
    accounts.push(AccountMeta::new_readonly(pool_state.amm_config, false));
    // 3.pool state
    accounts.push(AccountMeta::new(pool_id, false));
    let token_mint_0_program = get_token_program(&pool_state.token_mint_0);
    let token_mint_1_program = get_token_program(&pool_state.token_mint_1);
    let (token_mint_0_ata, _) = Pubkey::find_program_address(
        &[
            wallet.as_ref(),
            token_mint_0_program.as_ref(),
            pool_state.token_mint_0.as_ref(),
        ],
        &ATA_PROGRAM_ID,
    );
    let (token_mint_1_ata, _) = Pubkey::find_program_address(
        &[
            wallet.as_ref(),
            token_mint_1_program.as_ref(),
            pool_state.token_mint_1.as_ref(),
        ],
        &ATA_PROGRAM_ID,
    );
    let (input_mint,output_mint)=if swap_direction {
        // 4.token_mint_0_ata
        accounts.push(AccountMeta::new(token_mint_0_ata, false));
        // 5.token_mint_1_ata
        accounts.push(AccountMeta::new(token_mint_1_ata, false));
        // 6.token vault 0
        accounts.push(AccountMeta::new(pool_state.token_vault_0, false));
        // 7.token vault 1
        accounts.push(AccountMeta::new(pool_state.token_vault_1, false));
        (pool_state.token_mint_0,pool_state.token_mint_1)
    } else {
        // 4.token_mint_1_ata
        accounts.push(AccountMeta::new(token_mint_1_ata, false));
        // 5.token_mint_0_ata
        accounts.push(AccountMeta::new(token_mint_0_ata, false));
        // 6.token vault 1
        accounts.push(AccountMeta::new(pool_state.token_vault_1, false));
        // 7.token vault 0
        accounts.push(AccountMeta::new(pool_state.token_vault_0, false));
        (pool_state.token_mint_1,pool_state.token_mint_0)
    };
    // 8.Observation State
    accounts.push(AccountMeta::new(pool_state.observation_key, false));
    // 9.token_program
    accounts.push(AccountMeta::new_readonly(
        MINT_PROGRAM_ID,
        false,
    ));
    // 10.token_program_2022
    accounts.push(AccountMeta::new_readonly(
        MINT2022_PROGRAM_ID,
        false,
    ));
    // 11.memo program
    accounts.push(AccountMeta::new_readonly(
        RAYDIUM_CLMM_MEMO_PROGRAM_ID,
        false,
    ));
    // 12.input mint
    accounts.push(AccountMeta::new(input_mint,false));
    // 13.output mint
    accounts.push(AccountMeta::new(output_mint,false));
    let bit_map_extension_key = pda_bit_map_extension_key(&pool_id);
    // TODO 仅加载需要的
    let mut tick_arrays = load_cur_and_next_specify_count_tick_array_key(
        3,
        &pool_id,
        &pool_state,
        &crate::account_cache::get_account_data::<TickArrayBitmapExtension>(&bit_map_extension_key),
        swap_direction,
    )
    .map_or(
        Err(anyhow!("生成指令，获取TickArray失败")),
        |keys| {
            Ok(keys
                .into_iter()
                .map(|k| AccountMeta::new(k, false))
                .collect::<Vec<_>>())
        },
    )?;
    // 12.current tick array
    accounts.push(tick_arrays.remove(0));
    // 13.bitmap_extension
    accounts.push(AccountMeta::new(bit_map_extension_key, false));
    // 14.remaining tick array
    accounts.extend(tick_arrays);
    Ok(accounts)
}
