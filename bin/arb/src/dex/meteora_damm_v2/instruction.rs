use crate::dex::meteora_damm_v2::{
    DAMM_V2_EVENT_AUTHORITY, DAMM_V2_POOL_AUTHORITY, DAMM_V2_PROGRAM_ID,
};
use crate::dex::{
    get_alt, get_token_program, DexType, InstructionMaterial,
    InstructionMaterialConverter, ATA_PROGRAM_ID,
};
use crate::metadata::{get_keypair, MintAtaPair};
use solana_sdk::instruction::AccountMeta;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::Signer;
use crate::dex::meteora_damm_v2::state::pool::Pool;

pub struct MeteoraDAMMV2InstructionMaterialConverter;

impl InstructionMaterialConverter for MeteoraDAMMV2InstructionMaterialConverter {
    fn convert_to_instruction_material(
        &self,
        pool_id: &Pubkey,
        swap_direction: bool,
    ) -> anyhow::Result<InstructionMaterial> {
        let wallet = get_keypair().pubkey();
        let pool = crate::dex::global_cache::get_account_data::<Pool>(pool_id).unwrap();
        let mut accounts = Vec::with_capacity(17);
        // 1.pool authority
        accounts.push(AccountMeta::new_readonly(DAMM_V2_POOL_AUTHORITY, false));
        // 2.pool
        accounts.push(AccountMeta::new(pool_id.clone(), false));
        // 3.input token account
        let input_token_program = get_token_program(&pool.token_a_mint);
        let (input_token_account, _) = Pubkey::find_program_address(
            &[
                wallet.as_ref(),
                input_token_program.as_ref(),
                pool.token_a_mint.as_ref(),
            ],
            &ATA_PROGRAM_ID,
        );
        accounts.push(AccountMeta::new(input_token_account, false));
        // 4.output token account
        let out_token_program = get_token_program(&pool.token_b_mint);
        let (output_token_account, _) = Pubkey::find_program_address(
            &[
                wallet.as_ref(),
                out_token_program.as_ref(),
                pool.token_b_mint.as_ref(),
            ],
            &ATA_PROGRAM_ID,
        );
        accounts.push(AccountMeta::new(output_token_account, false));
        // 5.token a vault
        accounts.push(AccountMeta::new(pool.token_a_vault, false));
        // 6.token b vault
        accounts.push(AccountMeta::new(pool.token_b_vault, false));
        // 7.token a mint
        accounts.push(AccountMeta::new_readonly(pool.token_a_mint, false));
        // 8.token b mint
        accounts.push(AccountMeta::new_readonly(pool.token_b_mint, false));
        // 9.wallet
        accounts.push(AccountMeta::new(wallet, true));
        // 10. token a program
        accounts.push(AccountMeta::new_readonly(input_token_program, false));
        // 11. token b program
        accounts.push(AccountMeta::new_readonly(out_token_program, false));
        // 12.referral token account
        accounts.push(AccountMeta::new(DAMM_V2_PROGRAM_ID, false));
        // 13.event authority
        accounts.push(AccountMeta::new(DAMM_V2_EVENT_AUTHORITY, false));
        // 14.program
        accounts.push(AccountMeta::new(DAMM_V2_PROGRAM_ID, false));
        Ok(InstructionMaterial::new(
            DexType::MeteoraDAMMV2,
            swap_direction,
            accounts,
            None,
            get_alt(pool_id),
            vec![
                MintAtaPair::new(pool.token_a_mint, input_token_account),
                MintAtaPair::new(pool.token_b_mint, output_token_account),
            ],
        ))
    }
}
