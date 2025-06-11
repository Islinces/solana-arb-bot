use crate::dex::pump_fun::state::{global_config_key, Pool};
use crate::dex::DexType::PumpFunAMM;
use crate::dex::{ATA_PROGRAM_ID, MINT_PROGRAM_ID, SYSTEM_PROGRAM_ID};
use crate::global_cache::get_alt;
use crate::metadata::{get_keypair, MintAtaPair};
use crate::{InstructionMaterial, InstructionMaterialConverter};
use anyhow::Result;
use solana_sdk::instruction::AccountMeta;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::Signer;
use std::str::FromStr;

pub struct PumpFunAMMInstructionMaterialConverter;

impl InstructionMaterialConverter for PumpFunAMMInstructionMaterialConverter {
    fn convert_to_instruction_material(
        &self,
        pool_id: &Pubkey,
        swap_direction: bool,
    ) -> Result<InstructionMaterial> {
        let wallet = get_keypair().pubkey();
        let pool = crate::global_cache::get_account_data::<Pool>(pool_id).unwrap();
        let mut accounts = Vec::with_capacity(17);
        // 1.pool
        accounts.push(AccountMeta::new_readonly(pool_id.clone(), false));
        // 2. wallet
        accounts.push(AccountMeta::new(wallet, true));
        // 3. global config
        accounts.push(AccountMeta::new_readonly(global_config_key(), false));
        // 4.base mint
        accounts.push(AccountMeta::new_readonly(pool.base_mint, false));
        // 5.quote mint
        accounts.push(AccountMeta::new_readonly(pool.quote_mint, false));
        // 6.base mint ata
        let (base_ata, _) = Pubkey::find_program_address(
            &[
                wallet.as_ref(),
                MINT_PROGRAM_ID.as_ref(),
                pool.base_mint.as_ref(),
            ],
            &ATA_PROGRAM_ID,
        );
        accounts.push(AccountMeta::new(base_ata, false));
        // 7.quote mint ata
        let (quote_ata, _) = Pubkey::find_program_address(
            &[
                wallet.as_ref(),
                MINT_PROGRAM_ID.as_ref(),
                pool.quote_mint.as_ref(),
            ],
            &ATA_PROGRAM_ID,
        );
        accounts.push(AccountMeta::new(quote_ata, false));
        // 8.base mint vault
        accounts.push(AccountMeta::new(pool.pool_base_token_account, false));
        // 9.quote mint vault
        accounts.push(AccountMeta::new(pool.pool_quote_token_account, false));
        // 10.fee account
        let fee_account = crate::dex::pump_fun::get_fee_account_with_rand();
        accounts.push(AccountMeta::new_readonly(fee_account.clone(), false));
        // 11.pump fun sol ata 小费账户
        accounts.push(AccountMeta::new(
            Pubkey::find_program_address(
                &[
                    &fee_account.to_bytes(),
                    &MINT_PROGRAM_ID.to_bytes(),
                    &spl_token::native_mint::id().to_bytes(),
                ],
                &ATA_PROGRAM_ID,
            )
            .0,
            false,
        ));
        // 12.base quote program
        accounts.push(AccountMeta::new_readonly(MINT_PROGRAM_ID, false));
        // 13.quote quote program
        accounts.push(AccountMeta::new_readonly(MINT_PROGRAM_ID, false));
        // 14.system program
        accounts.push(AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false));
        // 15.system program
        accounts.push(AccountMeta::new_readonly(ATA_PROGRAM_ID, false));
        // 16.event authority
        accounts.push(AccountMeta::new_readonly(
            Pubkey::from_str("GS4CU59F31iL7aR2Q8zVS8DRrcRnXX1yjQ66TqNVQnaR")?,
            false,
        ));
        // 17.pump fun program
        accounts.push(AccountMeta::new_readonly(
            crate::dex::pump_fun::PUMP_FUN_AMM_PROGRAM_ID,
            false,
        ));
        // 18.Coin Creator Vault Ata
        accounts.push(AccountMeta::new(pool.coin_creator_vault_ata, false));
        // 19.Coin Creator Vault Authority
        accounts.push(AccountMeta::new_readonly(
            pool.coin_creator_vault_authority,
            false,
        ));
        Ok(InstructionMaterial::new(
            PumpFunAMM,
            swap_direction,
            accounts,
            None,
            get_alt(pool_id),
            vec![
                MintAtaPair::new(pool.base_mint, base_ata),
                MintAtaPair::new(pool.quote_mint, quote_ata),
            ],
        ))
    }
}
