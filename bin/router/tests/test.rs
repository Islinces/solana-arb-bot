use anyhow::Context;
use arrayref::array_ref;
use router::dex::meteora_dlmm::sdk::commons::pda::derive_bin_array_bitmap_extension;
use router::dex::meteora_dlmm::sdk::commons::quote::get_bin_array_pubkeys_for_swap;
use router::dex::meteora_dlmm::sdk::interface::accounts::{
    BinArray, BinArrayAccount, BinArrayBitmapExtension, BinArrayBitmapExtensionAccount, LbPair,
    LbPairAccount,
};
use solana_client::rpc_client::RpcClient;
use solana_program::clock::Clock;
use solana_program::pubkey::Pubkey;
use solana_program::sysvar::SysvarId;
use solana_sdk::account::Account;
use spl_token_2022::extension::transfer_fee::TransferFeeConfig;
use spl_token_2022::extension::{BaseStateWithExtensions, StateWithExtensions};
use std::collections::HashMap;
use std::mem::offset_of;
use std::str::FromStr;
use router::dex::meteora_dlmm::sdk::interface::typedefs::{StaticParameters, VariableParameters};

#[test]
fn test1() {
    println!(
        "{:?}",
        Pubkey::from([
            1, 61, 244, 118, 82, 182, 221, 78, 178, 56, 190, 138, 178, 50, 240, 238, 148, 5, 8,
            203, 46, 53, 64, 102, 156, 255, 0, 79, 166, 113, 16, 67
        ])
    );
}

#[test]
fn calac_lb_pair_sub_field_offset() {
    // 计算每个字段的offset和size
    let offsets = [
        (
            "v_parameters.volatility_accumulator",
            offset_of!(LbPair, v_parameters.volatility_accumulator) as usize,
            size_of::<u32>(),
        ),
        (
            "v_parameters.volatility_reference",
            offset_of!(LbPair, v_parameters.volatility_reference) as usize,
            size_of::<u32>(),
        ),
        (
            "v_parameters.index_reference",
            offset_of!(LbPair, v_parameters.index_reference) as usize,
            size_of::<i32>(),
        ),
        (
            "v_parameters.last_update_timestamp",
            offset_of!(LbPair, v_parameters.last_update_timestamp) as usize,
            size_of::<i64>(),
        ),
        (
            "pair_type",
            offset_of!(LbPair, pair_type) as usize,
            size_of::<u8>(),
        ),
        (
            "active_id",
            offset_of!(LbPair, active_id) as usize,
            size_of::<i32>(),
        ),
        (
            "bin_step",
            offset_of!(LbPair, bin_step) as usize,
            size_of::<u16>(),
        ),
        (
            "status",
            offset_of!(LbPair, status) as usize,
            size_of::<u8>(),
        ),
        (
            "activation_type",
            offset_of!(LbPair, activation_type) as usize,
            size_of::<u8>(),
        ),
        (
            "bin_array_bitmap",
            offset_of!(LbPair, bin_array_bitmap) as usize,
            size_of::<[u64; 16]>(),
        ),
        (
            "activation_point",
            offset_of!(LbPair, activation_point) as usize,
            size_of::<u64>(),
        ),
    ];
    let mut all_size = 0;
    // 打印结果
    for (field, offset, size) in offsets {
        all_size += size;
        println!(
            "Pool Field: {}, Offset: {} bytes, Size: {} bytes",
            field,
            offset + 8,
            size
        );
    }
    println!("all size: {}", all_size);
}

#[test]
fn test() {
    let rpc_client = RpcClient::new("https://solana-rpc.publicnode.com".to_string());
    let mint_x = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
    let mint_y = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();
    let lb_pair_pubkey = Pubkey::from_str("3msVd34R5KxonDzyNSV5nT19UtUeJ2RF1NaQhvVPNLxL").unwrap();
    let bitmap_extension_pubkey = derive_bin_array_bitmap_extension(lb_pair_pubkey).0;
    println!("lb_pair_pubkey : {:?}", lb_pair_pubkey);
    println!("bitmap_extension_pubkey : {:?}", bitmap_extension_pubkey);
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
        &lb_pair_state,
        bitmap_extension.as_ref(),
        true,
        3,
        &rpc_client,
    );
    let right_bin_arrays = get_bin_arrays_by_swap_direction(
        lb_pair_pubkey,
        &lb_pair_state,
        bitmap_extension.as_ref(),
        false,
        3,
        &rpc_client,
    );
    println!("left bin arrays : {:?}", &left_bin_arrays.keys());
    println!("right bin arrays : {:?}", &right_bin_arrays.keys());
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
    lb_pair_state: &LbPair,
    bitmap_extension: Option<&BinArrayBitmapExtension>,
    swap_for_y: bool,
    take: u8,
    rpc_client: &RpcClient,
) -> HashMap<Pubkey, BinArray> {
    let bin_arrays_for_swap = get_bin_array_pubkeys_for_swap(
        lb_pair_pubkey,
        lb_pair_state,
        bitmap_extension,
        swap_for_y,
        take,
    )
    .unwrap();
    println!(
        "{:?}",
        bin_arrays_for_swap
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
    );
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
