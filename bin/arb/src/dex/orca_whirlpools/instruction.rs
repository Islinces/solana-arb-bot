use crate::global_cache::{get_account_data, get_token_program};
use crate::dex::orca_whirlpools::{
    get_oracle_address, get_tick_array_address, get_tick_array_start_tick_index
    , Whirlpool, TICK_ARRAY_SIZE,
};
use crate::interface1::{ATA_PROGRAM_ID, MEMO_PROGRAM, MINT_PROGRAM_ID};
use crate::metadata::get_keypair;
use anyhow::anyhow;
use solana_sdk::instruction::AccountMeta;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::Signer;

pub fn to_instruction(pool_id: Pubkey, swap_direction: bool) -> anyhow::Result<Vec<AccountMeta>> {
    let wallet = get_keypair().pubkey();
    let pool = get_account_data::<Whirlpool>(&pool_id).unwrap();
    let mut accounts = Vec::with_capacity(17);
    let token_program_a = get_token_program(&pool.token_mint_a);
    let token_program_b = get_token_program(&pool.token_mint_b);
    // 1.token_program_a
    accounts.push(AccountMeta::new_readonly(token_program_a, false));
    // 2.token_program_b
    accounts.push(AccountMeta::new_readonly(token_program_b, false));
    // 3.memo
    accounts.push(AccountMeta::new_readonly(MEMO_PROGRAM, false));
    // 4.wallet
    accounts.push(AccountMeta::new(wallet, true));
    // 5.pool
    accounts.push(AccountMeta::new(pool_id, false));
    // 6.token_mint_a
    accounts.push(AccountMeta::new_readonly(pool.token_mint_a, false));
    // 7.token_mint_a
    accounts.push(AccountMeta::new_readonly(pool.token_mint_b, false));
    // 8.token_mint_a_ata
    let (token_mint_a_ata, _) = Pubkey::find_program_address(
        &[
            wallet.as_ref(),
            MINT_PROGRAM_ID.as_ref(),
            pool.token_mint_a.as_ref(),
        ],
        &ATA_PROGRAM_ID,
    );
    accounts.push(AccountMeta::new(token_mint_a_ata, false));
    // 9.token_vault_a
    accounts.push(AccountMeta::new(pool.token_vault_a, false));
    // 10.token_mint_a_ata
    let (token_mint_b_ata, _) = Pubkey::find_program_address(
        &[
            wallet.as_ref(),
            MINT_PROGRAM_ID.as_ref(),
            pool.token_mint_b.as_ref(),
        ],
        &ATA_PROGRAM_ID,
    );
    accounts.push(AccountMeta::new(token_mint_b_ata, false));
    // 11.token_vault_b
    accounts.push(AccountMeta::new(pool.token_vault_b, false));
    // tick_arrays
    let mut tick_array_keys = get_tick_arrays_or_default(
        &pool_id,
        pool.tick_current_index,
        pool.tick_spacing,
        swap_direction,
    );
    // 12.tick_array_0
    accounts.push(AccountMeta::new(
        tick_array_keys
            .pop()
            .map_or(Err(anyhow!("tick_array_0 pop failed")), |v| Ok(v))?,
        false,
    ));
    // 13.tick_array_1
    accounts.push(AccountMeta::new(
        tick_array_keys
            .pop()
            .map_or(Err(anyhow!("tick_array_1 pop failed")), |v| Ok(v))?,
        false,
    ));
    // 14.tick_array_2
    accounts.push(AccountMeta::new(
        tick_array_keys
            .pop()
            .map_or(Err(anyhow!("tick_array_2 pop failed")), |v| Ok(v))?,
        false,
    ));
    // 15.oracle
    accounts.push(AccountMeta::new(get_oracle_address(&pool_id)?, false));
    // 16..remaining tick_array TODO 发jupiter的话需要放在Swap枚举类里
    Ok(accounts)
}

fn get_tick_arrays_or_default(
    whirlpool_address: &Pubkey,
    tick_current_index: i32,
    tick_spacing: u16,
    swap_direction: bool,
) -> Vec<Pubkey> {
    let tick_array_start_index = get_tick_array_start_tick_index(tick_current_index, tick_spacing);
    let offset = tick_spacing as i32 * TICK_ARRAY_SIZE as i32;

    let tick_array_indexes = if swap_direction {
        [
            tick_array_start_index,
            tick_array_start_index - offset,
            tick_array_start_index - offset * 2,
            // tick_array_start_index - offset * 3,
            // tick_array_start_index - offset * 4,
            // tick_array_start_index - offset * 5,
        ]
    } else {
        [
            tick_array_start_index,
            tick_array_start_index + offset,
            tick_array_start_index + offset * 2,
            // tick_array_start_index + offset * 3,
            // tick_array_start_index + offset * 4,
            // tick_array_start_index + offset * 5,
        ]
    };

    tick_array_indexes
        .iter()
        .map(|tick_index| {
            get_tick_array_address(whirlpool_address, *tick_index)
                .unwrap()
                .0
        })
        .collect::<Vec<_>>()
}
