use crate::dex::raydium_amm::state::AmmInfo;
use crate::dex::raydium_amm::SERUM_PROGRAM_ID;
use crate::dex::{ATA_PROGRAM_ID, MINT_PROGRAM_ID};
use crate::metadata::get_keypair;
use anyhow::Result;
use solana_sdk::instruction::AccountMeta;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::Signer;

pub fn to_instruction(pool_id: Pubkey, swap_direction: bool) -> Result<Vec<AccountMeta>> {
    let wallet = get_keypair().pubkey();
    let amm_info = crate::global_cache::get_account_data::<AmmInfo>(&pool_id).unwrap();
    let mut accounts = Vec::with_capacity(17);
    // 1.mint program
    accounts.push(AccountMeta::new_readonly(MINT_PROGRAM_ID, false));
    // 2.pool
    accounts.push(AccountMeta::new(pool_id, false));
    // 3.authority id
    accounts.push(AccountMeta::new_readonly(
        crate::dex::raydium_amm::RAYDIUM_AMM_VAULT_OWNER,
        false,
    ));
    // 4.open order
    accounts.push(AccountMeta::new(pool_id, false));
    // 5.coin vault
    accounts.push(AccountMeta::new(amm_info.coin_vault, false));
    // 6.pc vault
    accounts.push(AccountMeta::new(amm_info.pc_vault, false));
    // 7.Serum Program Id
    accounts.push(AccountMeta::new(SERUM_PROGRAM_ID, false));
    // 8.Serum Market
    accounts.push(AccountMeta::new(pool_id, false));
    // 9.Serum Bids
    accounts.push(AccountMeta::new(pool_id, false));
    // 10.Serum Asks
    accounts.push(AccountMeta::new(pool_id, false));
    // 11.Serum Event Queue
    accounts.push(AccountMeta::new(pool_id, false));
    // 12.Serum Coin Vault Account
    accounts.push(AccountMeta::new(pool_id, false));
    // 13.Serum Pc Vault Account
    accounts.push(AccountMeta::new(pool_id, false));
    // 14.Serum Vault Signer
    accounts.push(AccountMeta::new(pool_id, false));
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
    Ok(accounts)
}
