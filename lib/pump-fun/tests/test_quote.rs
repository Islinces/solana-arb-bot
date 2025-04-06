use anchor_lang::Discriminator;
use anchor_spl::token::spl_token;
use dex::interface::Pool;
use pump_fun::pump_fun_pool::PumpFunPool;
use pump_fun::utils::deserialize_anchor_account;
use pump_fun::{GlobalConfig, Pool as PumpPool};
use solana_client::rpc_client::RpcClient;
use solana_program::program_pack::Pack;
use solana_program::pubkey::Pubkey;
use std::ops::Mul;
use std::str::FromStr;

#[test]
fn test() {
    println!("{:?}", PumpPool::discriminator());
}

#[test]
fn test_quote() {
    let rpc_client = RpcClient::new("https://solana-rpc.publicnode.com".to_string());
    let pool_id = Pubkey::from_str("Gf7sXMoP8iRw4iiXmJ1nq4vxcRycbGXy5RL8a8LnTd3v").unwrap();
    let accounts = vec![pool_id, global_config_pda()];
    let accounts = rpc_client.get_multiple_accounts(&accounts).unwrap();
    let pool = deserialize_anchor_account::<PumpPool>(accounts[0].as_ref().unwrap()).unwrap();
    let global_config =
        deserialize_anchor_account::<GlobalConfig>(accounts[1].as_ref().unwrap()).unwrap();
    let vault_accounts = rpc_client
        .get_multiple_accounts(&[pool.pool_base_token_account, pool.pool_quote_token_account])
        .unwrap();
    let mint_0_vault =
        spl_token::state::Account::unpack(vault_accounts[0].as_ref().unwrap().data.as_slice())
            .unwrap();
    let mint_1_vault =
        spl_token::state::Account::unpack(vault_accounts[1].as_ref().unwrap().data.as_slice())
            .unwrap();
    let pool = PumpFunPool::new(
        pool_id,
        pool.base_mint,
        pool.quote_mint,
        // 1_523_851_760_000_u64,
        // 12_690_500_000_000_u64,
        mint_0_vault.amount,
        mint_1_vault.amount,
        global_config.lp_fee_basis_points,
        global_config.protocol_fee_basis_points,
    );
    let sol_unit = 10_u64.pow(9);
    let usdc_unit = 10_u64.pow(6);
    let amount_in = 1.mul(sol_unit);
    let amount_out = pool.quote(
        amount_in,
        Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
    );
    if let Some(out) = amount_out {
        println!(
            "amount_in : {:?} SOL \namount_out: {:?} USDC",
            amount_in ,
            out
        )
    }
}

#[test]
fn generate_lp_mint_pda() {
    let pool_id = Pubkey::from_str("Gf7sXMoP8iRw4iiXmJ1nq4vxcRycbGXy5RL8a8LnTd3v").unwrap();
    let amm_program_id = Pubkey::from_str("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA").unwrap();
    println!(
        "lp_mint_pda: {}",
        Pubkey::find_program_address(&[b"pool_lp_mint", &pool_id.to_bytes(),], &amm_program_id).0
    )
}

#[test]
fn generate_global_config_pda() {
    println!("global_config_pda: {}", global_config_pda())
}

fn global_config_pda() -> Pubkey {
    let amm_program_id = Pubkey::from_str("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA").unwrap();
    Pubkey::find_program_address(&[b"global_config"], &amm_program_id).0
}

#[test]
fn generate_pool_authority_pda() {
    let program_id = Pubkey::from_str("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P").unwrap();
    let base_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();
    // 结果不对
    println!(
        "pool_authority_pda: {}",
        Pubkey::find_program_address(&[b"pool-authority", &base_mint.to_bytes()], &program_id).0
    )
}

#[test]
fn generate_pool_pda() {
    println!("pool_pda: {}", pool_pda())
}

fn pool_pda() -> Pubkey {
    let amm_program_id = Pubkey::from_str("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA").unwrap();
    // creator
    let owner = Pubkey::from_str("CbAF99HGQnAtnvw67nHesZH7ZwaQZeboYceCH5xNhr4s").unwrap();
    let base_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();
    let quote_mint = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
    Pubkey::find_program_address(
        &[
            b"pool",
            &0_u16.to_be_bytes(),
            &owner.to_bytes(),
            &base_mint.to_bytes(),
            &quote_mint.to_bytes(),
        ],
        &amm_program_id,
    )
    .0
}
