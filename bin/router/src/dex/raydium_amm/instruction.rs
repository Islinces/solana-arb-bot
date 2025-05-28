use crate::dex::raydium_amm::state::AmmInfo;
use crate::dex::InstructionItem;
use crate::interface::DexType;
use crate::metadata::get_keypair;
use crate::{ATA_PROGRAM, MINT_PROGRAM};
use solana_sdk::instruction::AccountMeta;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::Signer;
use std::str::FromStr;

pub fn to_instruction(pool_id: Pubkey, swap_direction: bool) -> Option<InstructionItem> {
    let wallet = get_keypair().pubkey();
    let amm_info = crate::account_cache::get_account_data::<AmmInfo>(&pool_id).unwrap();
    let mut accounts = Vec::with_capacity(17);
    // 1.mint program
    accounts.push(AccountMeta::new_readonly(MINT_PROGRAM, false));
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
    accounts.push(AccountMeta::new(
        Pubkey::from_str("opnb2LAfJYbRMAHHvqjCwQxanZn7ReEHp1k81EohpZb").unwrap(),
        false,
    ));
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
            &wallet.to_bytes(),
            &MINT_PROGRAM.to_bytes(),
            &amm_info.coin_vault_mint.to_bytes(),
        ],
        &ATA_PROGRAM,
    );
    let (pc_ata, _) = Pubkey::find_program_address(
        &[
            &wallet.to_bytes(),
            &MINT_PROGRAM.to_bytes(),
            &amm_info.pc_vault_mint.to_bytes(),
        ],
        &ATA_PROGRAM,
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
    Some(InstructionItem::new(
        DexType::RaydiumAMM,
        swap_direction,
        accounts,
        crate::account_cache::get_alt(&pool_id)?,
    ))
}
