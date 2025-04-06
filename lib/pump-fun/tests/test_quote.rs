use dex::interface::Pool;
use pump_fun::pump_fun_pool::PumpFunPool;
use solana_program::pubkey::Pubkey;
use std::str::FromStr;

#[test]
fn test_quote() {
    let pool = PumpFunPool::new(
        Pubkey::from_str("Gf7sXMoP8iRw4iiXmJ1nq4vxcRycbGXy5RL8a8LnTd3v").unwrap(),
        Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
        Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
        1_513_490_960_000_u64,
        12_775_840_000_000_u64,
        20_u64,
        5_u64,
    );
    let output_mint_unit = 10_u64.pow(6);
    let input_mint_unit = 10_u64.pow(9);
    let amount_in = 1 * input_mint_unit;
    let amount_out = pool.quote(
        amount_in,
        Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
    );
    if let Some(out) = amount_out {
        println!(
            "amount_in : {:?} SOL \namount_out: {:?} USDC",
            amount_in / input_mint_unit,
            (out as f64) / (output_mint_unit as f64)
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
    let amm_program_id = Pubkey::from_str("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA").unwrap();
    println!(
        "global_config_pda: {}",
        Pubkey::find_program_address(&[b"global_config"], &amm_program_id).0
    )
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
    let amm_program_id = Pubkey::from_str("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA").unwrap();
    // creator
    let owner = Pubkey::from_str("CbAF99HGQnAtnvw67nHesZH7ZwaQZeboYceCH5xNhr4s").unwrap();
    let base_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();
    let quote_mint = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();

    println!(
        "pool_pda: {}",
        Pubkey::find_program_address(
            &[
                b"pool",
                &0_u16.to_be_bytes(),
                &owner.to_bytes(),
                &base_mint.to_bytes(),
                &quote_mint.to_bytes()
            ],
            &amm_program_id
        )
        .0
    )
}
