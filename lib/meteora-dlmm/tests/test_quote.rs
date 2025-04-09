use anchor_client::solana_client::rpc_client::RpcClient;
use anchor_spl::token_2022::spl_token_2022::extension::transfer_fee::TransferFeeConfig;
use anchor_spl::token_2022::spl_token_2022::extension::{
    BaseStateWithExtensions, StateWithExtensions,
};
use anyhow::Context;
use arrayref::array_ref;
use dex::interface::Pool;
use meteora_dlmm::dlmm_pool::DlmmPool;
use meteora_dlmm::sdk::commons::pda::{derive_bin_array_bitmap_extension, derive_lb_pair_pda2};
use meteora_dlmm::sdk::commons::quote::get_bin_array_pubkeys_for_swap;
use meteora_dlmm::sdk::interface::accounts::{
    BinArray, BinArrayAccount, BinArrayBitmapExtension, BinArrayBitmapExtensionAccount, LbPair,
    LbPairAccount,
};
use solana_program::clock::Clock;
use solana_program::pubkey::Pubkey;
use solana_program::sysvar::SysvarId;
use solana_sdk::account::Account;
use std::collections::HashMap;
use std::str::FromStr;

#[test]
fn test_meteora_dlmm_quote() {
    let dlmm_pool = build_pool();
    let quote = dlmm_pool.quote(
        10_u64.pow(9),
        Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
    );
    if let Some(result) = quote {
        println!(
            "amount_in : {} SOL, amount_out {:?} USDC",
            10_u64.pow(9),
            result
        );
    } else {
        println!("amount_out: None");
    }
}

fn build_pool() -> DlmmPool {
    let rpc_client = RpcClient::new("https://solana-rpc.publicnode.com".to_string());
    let mint_x = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();
    let mint_y = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
    let lb_pair_pubkey = derive_lb_pair_pda2(mint_x, mint_y, 80, 12_500).0;
    println!("lb_pair_pubkey : {:?}", lb_pair_pubkey);
    let bitmap_extension_pubkey = derive_bin_array_bitmap_extension(lb_pair_pubkey).0;
    let clock_id = Clock::id();
    let accounts = rpc_client
        .get_multiple_accounts(&[
            lb_pair_pubkey,
            mint_x,
            mint_y,
            bitmap_extension_pubkey,
            clock_id,
        ])
        .unwrap();
    let [lb_pair_account, mint_x_account, mint_y_account, bitmap_extension_account, clock_account] =
        array_ref![accounts, 0, 5];
    let lb_pair_state = LbPairAccount::deserialize(&lb_pair_account.as_ref().unwrap().data)
        .unwrap()
        .0;
    let bitmap_extension = match bitmap_extension_account {
        None => None,
        Some(account) => Some(
            BinArrayBitmapExtensionAccount::deserialize(&account.data)
                .unwrap()
                .0,
        ),
    };
    let clock: Clock =
        bincode::deserialize(&clock_account.as_ref().unwrap().data.as_ref()).unwrap();
    let mint_x_transfer_fee_config = mint_transfer_fee_config(
        lb_pair_state.token_mint_x_program_flag,
        mint_x_account.as_ref().unwrap(),
    );
    let mint_y_transfer_fee_config = mint_transfer_fee_config(
        lb_pair_state.token_mint_y_program_flag,
        mint_y_account.as_ref().unwrap(),
    );
    let left_bin_arrays = get_bin_arrays_by_swap_direction(
        lb_pair_pubkey,
        lb_pair_state.clone(),
        bitmap_extension.as_ref(),
        true,
        3,
        &rpc_client,
    );
    let right_bin_arrays = get_bin_arrays_by_swap_direction(
        lb_pair_pubkey,
        lb_pair_state.clone(),
        bitmap_extension.as_ref(),
        false,
        3,
        &rpc_client,
    );

    DlmmPool::new(
        lb_pair_pubkey,
        lb_pair_state,
        left_bin_arrays,
        right_bin_arrays,
        bitmap_extension,
        mint_x_transfer_fee_config,
        mint_y_transfer_fee_config,
        clock,
    )
}

fn mint_transfer_fee_config(
    token_mint_program_flag: u8,
    mint_account: &Account,
) -> Option<TransferFeeConfig> {
    match token_mint_program_flag {
        1 => {
            let token_mint_data = mint_account.data.as_ref();
            let token_mint_unpacked = StateWithExtensions::<
                anchor_spl::token_2022::spl_token_2022::state::Mint,
            >::unpack(token_mint_data)
            .unwrap();

            if let Ok(transfer_fee_config) =
                token_mint_unpacked.get_extension::<TransferFeeConfig>()
            {
                return Some(*transfer_fee_config);
            }
        }
        _ => {}
    }
    None
}

fn get_bin_arrays_by_swap_direction(
    lb_pair_pubkey: Pubkey,
    lb_pair_state: LbPair,
    bitmap_extension: Option<&BinArrayBitmapExtension>,
    swap_for_y: bool,
    take: u8,
    rpc_client: &RpcClient,
) -> HashMap<Pubkey, BinArray> {
    let bin_arrays_for_swap = get_bin_array_pubkeys_for_swap(
        lb_pair_pubkey,
        &lb_pair_state,
        bitmap_extension,
        swap_for_y,
        take,
    )
    .unwrap();
    rpc_client
        .get_multiple_accounts(&bin_arrays_for_swap)
        .unwrap()
        .into_iter()
        .zip(bin_arrays_for_swap.iter())
        .map(|(account, &key)| {
            let account = account.unwrap();
            Some((
                key,
                BinArrayAccount::deserialize(account.data.as_ref()).ok()?.0,
            ))
        })
        .collect::<Option<HashMap<Pubkey, BinArray>>>()
        .context("Failed to fetch bin arrays")
        .unwrap()
}
