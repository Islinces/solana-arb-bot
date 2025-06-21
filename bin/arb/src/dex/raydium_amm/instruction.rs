use crate::dex::global_cache::get_alt;
use crate::dex::raydium_amm::state::AmmInfo;
use crate::dex::raydium_amm::SERUM_PROGRAM_ID;
use crate::dex::swap_instruction::{InstructionMaterial, InstructionMaterialConverter};
use crate::dex::DexType::RaydiumAMM;
use crate::dex::{ATA_PROGRAM_ID, MINT_PROGRAM_ID};
use crate::metadata::{get_keypair, MintAtaPair};
use anyhow::Result;
use solana_sdk::instruction::AccountMeta;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::Signer;

pub struct RaydiumAMMInstructionMaterialConverter;

impl InstructionMaterialConverter for RaydiumAMMInstructionMaterialConverter {
    fn convert_to_instruction_material(
        &self,
        pool_id: &Pubkey,
        swap_direction: bool,
    ) -> Result<InstructionMaterial> {
        let wallet = get_keypair().pubkey();
        let amm_info = crate::dex::global_cache::get_account_data::<AmmInfo>(pool_id).unwrap();
        let mut accounts = Vec::with_capacity(17);
        // 1.mint program
        accounts.push(AccountMeta::new_readonly(MINT_PROGRAM_ID, false));
        // 2.pool
        accounts.push(AccountMeta::new(pool_id.clone(), false));
        // 3.authority id
        accounts.push(AccountMeta::new_readonly(
            crate::dex::raydium_amm::RAYDIUM_AMM_VAULT_OWNER,
            false,
        ));
        // 4.open order
        accounts.push(AccountMeta::new(pool_id.clone(), false));
        // 5.coin vault
        accounts.push(AccountMeta::new(amm_info.coin_vault, false));
        // 6.pc vault
        accounts.push(AccountMeta::new(amm_info.pc_vault, false));
        // 7.Serum Program Id
        accounts.push(AccountMeta::new(SERUM_PROGRAM_ID, false));
        // 8.Serum Market
        accounts.push(AccountMeta::new(pool_id.clone(), false));
        // 9.Serum Bids
        accounts.push(AccountMeta::new(pool_id.clone(), false));
        // 10.Serum Asks
        accounts.push(AccountMeta::new(pool_id.clone(), false));
        // 11.Serum Event Queue
        accounts.push(AccountMeta::new(pool_id.clone(), false));
        // 12.Serum Coin Vault Account
        accounts.push(AccountMeta::new(pool_id.clone(), false));
        // 13.Serum Pc Vault Account
        accounts.push(AccountMeta::new(pool_id.clone(), false));
        // 14.Serum Vault Signer
        accounts.push(AccountMeta::new(pool_id.clone(), false));
        let (coin_ata, _) = Pubkey::find_program_address(
            &[
                wallet.as_ref(),
                MINT_PROGRAM_ID.as_ref(),
                amm_info.coin_vault_mint.as_ref(),
            ],
            &ATA_PROGRAM_ID,
        );
        let (pc_ata, _) = Pubkey::find_program_address(
            &[
                wallet.as_ref(),
                MINT_PROGRAM_ID.as_ref(),
                amm_info.pc_vault_mint.as_ref(),
            ],
            &ATA_PROGRAM_ID,
        );
        if swap_direction {
            // 15.coin mint ata
            accounts.push(AccountMeta::new(coin_ata, false));
            // 16.pc mint ata
            accounts.push(AccountMeta::new(pc_ata, false));
        } else {
            // 15.pc mint ata
            accounts.push(AccountMeta::new(pc_ata, false));
            // 16.coin mint ata
            accounts.push(AccountMeta::new(coin_ata, false));
        }
        // 17.wallet
        accounts.push(AccountMeta::new(wallet, true));
        Ok(InstructionMaterial::new(
            RaydiumAMM,
            swap_direction,
            accounts,
            None,
            get_alt(pool_id),
            vec![
                MintAtaPair::new(amm_info.coin_vault_mint, coin_ata),
                MintAtaPair::new(amm_info.pc_vault_mint, pc_ata),
            ],
        ))
    }
}

#[cfg(test)]
mod test {
    use crate::dex::raydium_amm::instruction::RaydiumAMMInstructionMaterialConverter;
    use crate::dex::raydium_amm::{RAYDIUM_AMM_VAULT_OWNER, SERUM_PROGRAM_ID};
    use crate::dex::{
        init_global_cache, AmmInfo, DexType, GlobalCache, InstructionMaterialConverter,
        ATA_PROGRAM_ID, MINT_PROGRAM_ID,
    };
    use crate::dex_data::DexJson;
    use crate::metadata::{MintAtaPair, KEYPAIR};
    use solana_sdk::pubkey::Pubkey;
    use solana_sdk::signature::Keypair;
    use solana_sdk::signer::Signer;
    use std::str::FromStr;
    use std::sync::Arc;

    #[test]
    fn test_instruction() -> anyhow::Result<()> {
        let dex_json = DexJson {
            pool: Pubkey::from_str("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2")?,
            owner: Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8")?,
            mint_a: Pubkey::from_str("So11111111111111111111111111111111111111112")?,
            mint_b: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?,
            vault_a: Pubkey::from_str("DQyrAcCrDXQ7NeoqGgDCZwBvWDcYmFCjSb9JtteuvPpz")?,
            vault_b: Pubkey::from_str("HLmqeL62xR1QoZ1HKKbXRrdN1p3phKpxRMb2VVopvBBz")?,
            address_lookup_table_address: Some(Pubkey::from_str(
                "E59uBXGqn83xN17kMbBVfU1M7T4wHG91eiygHb88Aovb",
            )?),
        };
        let keypair = Keypair::new();
        let wallet = keypair.pubkey();
        KEYPAIR.set(Arc::new(keypair))?;

        let amm_info = AmmInfo {
            coin_vault: dex_json.vault_a,
            pc_vault: dex_json.vault_b,
            coin_vault_mint: dex_json.mint_a,
            pc_vault_mint: dex_json.mint_b,
            ..Default::default()
        };
        let data = bytemuck::bytes_of(&amm_info);
        let static_data = &data[0..144];
        let dynamic_data = &data[144..];
        let mut global_cache = GlobalCache::init();
        global_cache.upsert_static(dex_json.pool, static_data.to_vec());
        global_cache.upsert_dynamic(dex_json.pool, dynamic_data.to_vec());
        init_global_cache(global_cache);

        let con_vault_ata = Pubkey::find_program_address(
            &[
                wallet.as_ref(),
                MINT_PROGRAM_ID.as_ref(),
                amm_info.coin_vault_mint.as_ref(),
            ],
            &ATA_PROGRAM_ID,
        )
        .0;
        let pc_vault_ata = Pubkey::find_program_address(
            &[
                wallet.as_ref(),
                MINT_PROGRAM_ID.as_ref(),
                amm_info.pc_vault_mint.as_ref(),
            ],
            &ATA_PROGRAM_ID,
        )
        .0;
        let a_to_b_result = RaydiumAMMInstructionMaterialConverter
            .convert_to_instruction_material(&dex_json.pool, true);

        assert!(a_to_b_result.is_ok());
        let a_to_b_result = a_to_b_result?;
        assert_eq!(a_to_b_result.dex_type, DexType::RaydiumAMM);
        assert_eq!(a_to_b_result.swap_direction, true);
        assert!(a_to_b_result.remaining_account_num.is_none());
        assert_eq!(a_to_b_result.account_meta.len(), 17);
        assert_eq!(a_to_b_result.used_atas.len(), 2);
        assert!(vec![
            MintAtaPair {
                mint: amm_info.coin_vault_mint,
                ata: con_vault_ata,
            },
            MintAtaPair {
                mint: amm_info.pc_vault_mint,
                ata: pc_vault_ata,
            }
        ]
        .iter()
        .all(|a| a_to_b_result.used_atas.contains(a)));
        let b_to_a_result = RaydiumAMMInstructionMaterialConverter
            .convert_to_instruction_material(&dex_json.pool, false);
        assert!(b_to_a_result.is_ok());
        let b_to_a_result = b_to_a_result?;
        assert_eq!(b_to_a_result.dex_type, DexType::RaydiumAMM);
        assert_eq!(b_to_a_result.swap_direction, false);
        assert!(b_to_a_result.remaining_account_num.is_none());
        assert_eq!(b_to_a_result.account_meta.len(), 17);
        assert_eq!(b_to_a_result.used_atas.len(), 2);
        assert!(vec![
            MintAtaPair {
                mint: amm_info.coin_vault_mint,
                ata: con_vault_ata,
            },
            MintAtaPair {
                mint: amm_info.pc_vault_mint,
                ata: pc_vault_ata,
            }
        ]
        .iter()
        .all(|a| b_to_a_result.used_atas.contains(a)));
        a_to_b_result
            .account_meta
            .iter()
            .zip(b_to_a_result.account_meta.iter())
            .enumerate()
            .for_each(
                |((index, (a_to_b_account_meta, b_to_a_account_meta)))| match index + 1 {
                    1 => {
                        assert!(
                            a_to_b_account_meta.pubkey == MINT_PROGRAM_ID
                                && !a_to_b_account_meta.is_signer
                                && !a_to_b_account_meta.is_writable
                        );
                        assert_eq!(a_to_b_account_meta, b_to_a_account_meta);
                    }
                    2 => {
                        assert!(
                            a_to_b_account_meta.pubkey == dex_json.pool
                                && !a_to_b_account_meta.is_signer
                                && a_to_b_account_meta.is_writable
                        );
                        assert_eq!(a_to_b_account_meta, b_to_a_account_meta);
                    }
                    3 => {
                        assert!(
                            a_to_b_account_meta.pubkey == RAYDIUM_AMM_VAULT_OWNER
                                && !a_to_b_account_meta.is_signer
                                && !a_to_b_account_meta.is_writable
                        );
                        assert_eq!(a_to_b_account_meta, b_to_a_account_meta);
                    }
                    4 => {
                        assert!(
                            a_to_b_account_meta.pubkey == dex_json.pool
                                && !a_to_b_account_meta.is_signer
                                && a_to_b_account_meta.is_writable
                        );
                        assert_eq!(a_to_b_account_meta, b_to_a_account_meta);
                    }
                    5 => {
                        assert!(
                            a_to_b_account_meta.pubkey == dex_json.vault_a
                                && !a_to_b_account_meta.is_signer
                                && a_to_b_account_meta.is_writable
                        );
                        assert_eq!(a_to_b_account_meta, b_to_a_account_meta);
                    }
                    6 => {
                        assert!(
                            a_to_b_account_meta.pubkey == dex_json.vault_b
                                && !a_to_b_account_meta.is_signer
                                && a_to_b_account_meta.is_writable
                        );
                        assert_eq!(a_to_b_account_meta, b_to_a_account_meta);
                    }
                    7 => {
                        assert!(
                            a_to_b_account_meta.pubkey == SERUM_PROGRAM_ID
                                && !a_to_b_account_meta.is_signer
                                && a_to_b_account_meta.is_writable
                        );
                        assert_eq!(a_to_b_account_meta, b_to_a_account_meta);
                    }
                    8..=14 => {
                        assert!(
                            a_to_b_account_meta.pubkey == dex_json.pool
                                && !a_to_b_account_meta.is_signer
                                && a_to_b_account_meta.is_writable
                        );
                        assert_eq!(a_to_b_account_meta, b_to_a_account_meta);
                    }
                    15 => {
                        assert!(
                            a_to_b_account_meta.pubkey == con_vault_ata
                                && !a_to_b_account_meta.is_signer
                                && a_to_b_account_meta.is_writable
                        );
                        assert!(
                            b_to_a_account_meta.pubkey == pc_vault_ata
                                && !b_to_a_account_meta.is_signer
                                && b_to_a_account_meta.is_writable
                        );
                    }
                    16 => {
                        assert!(
                            a_to_b_account_meta.pubkey == pc_vault_ata
                                && !a_to_b_account_meta.is_signer
                                && a_to_b_account_meta.is_writable
                        );
                        assert!(
                            b_to_a_account_meta.pubkey == con_vault_ata
                                && !b_to_a_account_meta.is_signer
                                && b_to_a_account_meta.is_writable
                        );
                    }
                    17 => {
                        assert!(
                            a_to_b_account_meta.pubkey == wallet
                                && a_to_b_account_meta.is_signer
                                && a_to_b_account_meta.is_writable
                        );
                        assert_eq!(a_to_b_account_meta, b_to_a_account_meta);
                    }
                    _ => {}
                },
            );
        Ok(())
    }
}
