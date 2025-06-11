use crate::dex::meteora_dlmm::commons::quote::get_bin_array_pubkeys_for_swap;
use crate::dex::meteora_dlmm::interface::accounts::{BinArrayBitmapExtension, LbPair};
use crate::dex::meteora_dlmm::{METEORA_DLMM_EVENT_AUTHORITY_PROGRAM_ID, METEORA_DLMM_PROGRAM_ID};
use crate::dex::DexType::MeteoraDLMM;
use crate::dex::ATA_PROGRAM_ID;
use crate::global_cache::{get_alt, get_token_program};
use crate::metadata::{get_keypair, MintAtaPair};
use crate::{InstructionMaterial, InstructionMaterialConverter};
use anyhow::Result;
use solana_sdk::instruction::AccountMeta;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer;

pub struct MeteoraDLMMInstructionMaterialConverter;

impl InstructionMaterialConverter for MeteoraDLMMInstructionMaterialConverter {
    fn convert_to_instruction_material(
        &self,
        pool_id: &Pubkey,
        swap_direction: bool,
    ) -> Result<InstructionMaterial> {
        let wallet = get_keypair().pubkey();
        let lb_pair = crate::global_cache::get_account_data::<LbPair>(pool_id).unwrap();
        let mut accounts = Vec::with_capacity(20);
        // 1.lb pair
        accounts.push(AccountMeta::new(pool_id.clone(), false));
        // 2.bitmap extension
        let bitmap_extension_key =
            crate::dex::meteora_dlmm::commons::pda::derive_bin_array_bitmap_extension(pool_id);
        let bitmap_extension =
            crate::global_cache::get_account_data::<BinArrayBitmapExtension>(&bitmap_extension_key);
        accounts.push(AccountMeta::new_readonly(
            bitmap_extension
                .as_ref()
                .map_or(METEORA_DLMM_PROGRAM_ID, |_| bitmap_extension_key),
            false,
        ));
        // 3.reserve_x
        accounts.push(AccountMeta::new(lb_pair.reserve_x, false));
        // 4.reserve_y
        accounts.push(AccountMeta::new(lb_pair.reserve_y, false));
        let token_x_mint_program = get_token_program(&lb_pair.token_x_mint);
        let token_y_mint_program = get_token_program(&lb_pair.token_y_mint);
        let (token_x_ata, _) = Pubkey::find_program_address(
            &[
                wallet.as_ref(),
                token_x_mint_program.as_ref(),
                lb_pair.token_x_mint.as_ref(),
            ],
            &ATA_PROGRAM_ID,
        );
        let (token_y_ata, _) = Pubkey::find_program_address(
            &[
                wallet.as_ref(),
                token_y_mint_program.as_ref(),
                lb_pair.token_y_mint.as_ref(),
            ],
            &ATA_PROGRAM_ID,
        );
        if swap_direction {
            // 5.token_x ata
            accounts.push(AccountMeta::new(token_x_ata, false));
            // 6.token_y ata
            accounts.push(AccountMeta::new(token_y_ata, false));
        } else {
            // 5.token_y ata
            accounts.push(AccountMeta::new(token_y_ata, false));
            // 6.token_x ata
            accounts.push(AccountMeta::new(token_x_ata, false));
        }
        // 7.token_x
        accounts.push(AccountMeta::new_readonly(lb_pair.token_x_mint, false));
        // 8.token_y
        accounts.push(AccountMeta::new_readonly(lb_pair.token_y_mint, false));
        // 9.oracle
        accounts.push(AccountMeta::new(lb_pair.oracle, false));
        // 10.fee account
        accounts.push(AccountMeta::new_readonly(METEORA_DLMM_PROGRAM_ID, false));
        // 11.wallet
        accounts.push(AccountMeta::new(wallet, true));
        // 12.token_x program
        accounts.push(AccountMeta::new_readonly(token_x_mint_program, false));
        // 13.token_y program
        accounts.push(AccountMeta::new_readonly(token_y_mint_program, false));
        // 14.Event Authority
        accounts.push(AccountMeta::new_readonly(
            METEORA_DLMM_EVENT_AUTHORITY_PROGRAM_ID,
            false,
        ));
        // 15.program
        accounts.push(AccountMeta::new_readonly(METEORA_DLMM_PROGRAM_ID, false));
        // 16~~.current bin array
        let bin_arrays = get_bin_array_pubkeys_for_swap(
            &pool_id,
            &lb_pair,
            bitmap_extension.as_ref(),
            swap_direction,
            3,
        )?;
        accounts.extend(
            bin_arrays
                .into_iter()
                .map(|k| AccountMeta::new(k, false))
                .collect::<Vec<_>>(),
        );
        Ok(InstructionMaterial::new(
            MeteoraDLMM,
            swap_direction,
            accounts,
            None,
            get_alt(pool_id),
            vec![
                MintAtaPair::new(lb_pair.token_x_mint, token_x_ata),
                MintAtaPair::new(lb_pair.token_y_mint, token_y_ata)
            ],
        ))
    }
}
