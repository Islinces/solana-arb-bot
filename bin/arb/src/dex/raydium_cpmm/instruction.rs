use crate::dex::raydium_cpmm::states::PoolState;
use crate::dex::raydium_cpmm::RAYDIUM_CPMM_AUTHORITY_ID;
use crate::dex::{
    get_alt, DexType, InstructionMaterial, InstructionMaterialConverter, ATA_PROGRAM_ID,
};
use crate::metadata::{get_keypair, MintAtaPair};
use solana_sdk::instruction::AccountMeta;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::Signer;

pub struct RaydiumCPMMInstructionMaterialConverter;

impl InstructionMaterialConverter for RaydiumCPMMInstructionMaterialConverter {
    fn convert_to_instruction_material(
        &self,
        pool_id: &Pubkey,
        swap_direction: bool,
    ) -> anyhow::Result<InstructionMaterial> {
        let wallet = get_keypair().pubkey();
        let pool_state = crate::dex::global_cache::get_account_data::<PoolState>(pool_id).unwrap();
        let mut accounts = Vec::with_capacity(13);
        // 1.wallet
        accounts.push(AccountMeta::new(wallet, true));
        // 2.authority
        accounts.push(AccountMeta::new_readonly(RAYDIUM_CPMM_AUTHORITY_ID, false));
        // 3.amm config
        accounts.push(AccountMeta::new_readonly(pool_state.amm_config, false));
        // 4.pool state
        accounts.push(AccountMeta::new(pool_id.clone(), false));
        let user_token_0_mint_ata = Pubkey::find_program_address(
            &[
                wallet.as_ref(),
                pool_state.token_0_program.as_ref(),
                pool_state.token_0_mint.as_ref(),
            ],
            &ATA_PROGRAM_ID,
        )
        .0;
        let user_token_1_mint_ata = Pubkey::find_program_address(
            &[
                wallet.as_ref(),
                pool_state.token_1_program.as_ref(),
                pool_state.token_1_mint.as_ref(),
            ],
            &ATA_PROGRAM_ID,
        )
        .0;
        let (
            input_token_mint,
            output_token_mint,
            input_token_program,
            output_token_program,
            input_vault,
            output_vault,
            input_token_account,
            output_token_account,
        ) = if swap_direction {
            (
                pool_state.token_0_mint,
                pool_state.token_1_mint,
                pool_state.token_0_program,
                pool_state.token_1_program,
                pool_state.token_0_vault,
                pool_state.token_1_vault,
                user_token_0_mint_ata,
                user_token_1_mint_ata,
            )
        } else {
            (
                pool_state.token_1_mint,
                pool_state.token_0_mint,
                pool_state.token_1_program,
                pool_state.token_0_program,
                pool_state.token_1_vault,
                pool_state.token_0_vault,
                user_token_1_mint_ata,
                user_token_0_mint_ata,
            )
        };
        // 5.input token account
        accounts.push(AccountMeta::new(input_token_account, false));
        // 6.output token account
        accounts.push(AccountMeta::new(output_token_account, false));
        // 7.input vault
        accounts.push(AccountMeta::new(input_vault, false));
        // 8.output vault
        accounts.push(AccountMeta::new(output_vault, false));
        // 9.input token program
        accounts.push(AccountMeta::new_readonly(input_token_program, false));
        // 10.output token program
        accounts.push(AccountMeta::new_readonly(output_token_program, false));
        // 11.input token mint
        accounts.push(AccountMeta::new_readonly(input_token_mint, false));
        // 12.output token mint
        accounts.push(AccountMeta::new_readonly(output_token_mint, false));
        // 13.observation state
        accounts.push(AccountMeta::new(pool_state.observation_key, false));
        Ok(InstructionMaterial::new(
            DexType::RaydiumCPMM,
            swap_direction,
            accounts,
            None,
            get_alt(pool_id),
            vec![
                MintAtaPair::new(pool_state.token_0_mint, user_token_0_mint_ata),
                MintAtaPair::new(pool_state.token_1_mint, user_token_1_mint_ata),
            ],
        ))
    }
}
