use crate::dex::raydium_clmm::state::{
    pda_bit_map_extension_key, PoolState, TickArrayBitmapExtension,
};
use crate::dex::raydium_clmm::utils::load_cur_and_next_specify_count_tick_array_key;
use crate::dex::swap_instruction::{InstructionMaterial, InstructionMaterialConverter};
use crate::dex::DexType::RaydiumCLMM;
use crate::dex::{ATA_PROGRAM_ID, MINT_PROGRAM_ID};
use crate::dex::global_cache::get_alt;
use crate::metadata::{get_keypair, MintAtaPair};
use anyhow::{anyhow, Result};
use solana_sdk::instruction::AccountMeta;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::Signer;

pub struct RaydiumCLMMInstructionMaterialConverter;

impl InstructionMaterialConverter for RaydiumCLMMInstructionMaterialConverter {
    fn convert_to_instruction_material(
        &self,
        pool_id: &Pubkey,
        swap_direction: bool,
    ) -> Result<InstructionMaterial> {
        let wallet = get_keypair().pubkey();
        let pool_state = crate::dex::global_cache::get_account_data::<PoolState>(pool_id)
            .ok_or(anyhow!("缓存中找不到池子[{}]数据", pool_id))?;
        let mut accounts = Vec::with_capacity(11);
        // 1. wallet
        accounts.push(AccountMeta::new(wallet, true));
        // 2.amm config
        accounts.push(AccountMeta::new_readonly(pool_state.amm_config, false));
        // 3.pool state
        accounts.push(AccountMeta::new(pool_id.clone(), false));
        let (token_mint_0_ata, _) = Pubkey::find_program_address(
            &[
                wallet.as_ref(),
                MINT_PROGRAM_ID.as_ref(),
                pool_state.token_mint_0.as_ref(),
            ],
            &ATA_PROGRAM_ID,
        );
        let (token_mint_1_ata, _) = Pubkey::find_program_address(
            &[
                wallet.as_ref(),
                MINT_PROGRAM_ID.as_ref(),
                pool_state.token_mint_1.as_ref(),
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
        let bit_map_extension_key = pda_bit_map_extension_key(pool_id);
        let mut tick_arrays = load_cur_and_next_specify_count_tick_array_key(
            2,
            pool_id,
            &pool_state,
            &crate::dex::global_cache::get_account_data::<TickArrayBitmapExtension>(
                &bit_map_extension_key,
            ),
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
        accounts.push(tick_arrays.remove(0));
        // 11.bitmap_extension
        accounts.push(AccountMeta::new(bit_map_extension_key, false));
        accounts.extend(tick_arrays);
        Ok(InstructionMaterial::new(
            RaydiumCLMM,
            swap_direction,
            accounts,
            None,
            get_alt(pool_id),
            vec![
                MintAtaPair::new(pool_state.token_mint_0, token_mint_0_ata),
                MintAtaPair::new(pool_state.token_mint_1, token_mint_1_ata),
            ],
        ))
    }
}
