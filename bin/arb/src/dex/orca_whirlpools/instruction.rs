use crate::dex::global_cache::{get_account_data, get_alt, get_token_program};
use crate::dex::oracle::get_oracle_address;
use crate::dex::orca_whirlpools::math::get_tick_array_start_tick_index;
use crate::dex::swap_instruction::{InstructionMaterial, InstructionMaterialConverter};
use crate::dex::tick_array::{get_tick_array_address, TICK_ARRAY_SIZE};
use crate::dex::whirlpool::Whirlpool;
use crate::dex::DexType::OrcaWhirl;
use crate::dex::{ATA_PROGRAM_ID, MEMO_PROGRAM_V2, MINT_PROGRAM_ID};
use crate::metadata::{get_keypair, MintAtaPair};
use anyhow::anyhow;
use solana_sdk::instruction::AccountMeta;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::Signer;

pub struct OrcaWhirlInstructionMaterialConverter;

impl InstructionMaterialConverter for OrcaWhirlInstructionMaterialConverter {
    fn convert_to_instruction_material(
        &self,
        pool_id: &Pubkey,
        swap_direction: bool,
    ) -> anyhow::Result<InstructionMaterial> {
        let wallet = get_keypair().pubkey();
        let pool = get_account_data::<Whirlpool>(pool_id)
            .ok_or(anyhow!("缓存中找不到池子[{}]数据", pool_id))?;
        let mut accounts = Vec::with_capacity(17);
        let token_program_a = get_token_program(&pool.token_mint_a);
        let token_program_b = get_token_program(&pool.token_mint_b);
        // 1.token_program_a
        accounts.push(AccountMeta::new_readonly(token_program_a, false));
        // 2.token_program_b
        accounts.push(AccountMeta::new_readonly(token_program_b, false));
        // 3.memo
        accounts.push(AccountMeta::new_readonly(MEMO_PROGRAM_V2, false));
        // 4.wallet
        accounts.push(AccountMeta::new(wallet, true));
        // 5.pool
        accounts.push(AccountMeta::new(pool_id.clone(), false));
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
            pool_id,
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
        accounts.push(AccountMeta::new(get_oracle_address(pool_id)?, false));
        // 16..remaining tick_array
        // accounts.push(AccountMeta::new(
        //     tick_array_keys
        //         .pop()
        //         .map_or(Err(anyhow!("tick_array_3 pop failed")), |v| Ok(v))?,
        //     false,
        // ));
        // accounts.push(AccountMeta::new(
        //     tick_array_keys
        //         .pop()
        //         .map_or(Err(anyhow!("tick_array_4 pop failed")), |v| Ok(v))?,
        //     false,
        // ));
        Ok(InstructionMaterial::new(
            OrcaWhirl,
            swap_direction,
            accounts,
            None,
            get_alt(pool_id),
            vec![
                MintAtaPair::new(pool.token_mint_a, token_mint_a_ata),
                MintAtaPair::new(pool.token_mint_b, token_mint_b_ata),
            ],
        ))
    }
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
