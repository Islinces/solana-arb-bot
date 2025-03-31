use dex::interface::Pool;
use raydium_amm::amm_pool::AmmPool;
use solana_program::pubkey::Pubkey;
use std::str::FromStr;

#[test]
fn test_amm_pool_quote() {
    let sol = (
        Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
        9,
    );
    let usdc = (
        Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
        6,
    );
    let pool_1 = new_amm_pool(
        Pubkey::from_str("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2").unwrap(),
        Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8").unwrap(),
        sol.0,
        usdc.0,
        sol.1,
        usdc.1,
        (26_324.87 * (10_u64.pow(sol.1 as u32)) as f64) as u64,
        (3_524_576.3 * (10_u64.pow(usdc.1 as u32)) as f64) as u64,
    );
    let pool_2 = new_amm_pool(
        Pubkey::from_str("5oAvct85WyF7Sj73VYHbyFJkdRJ28D8m4z4Sxjvzuc6n").unwrap(),
        Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8").unwrap(),
        sol.0,
        usdc.0,
        sol.1,
        usdc.1,
        (3.4374 * (10_u64.pow(sol.1 as u32)) as f64) as u64,
        (430.2 * (10_u64.pow(usdc.1 as u32)) as f64) as u64,
    );
    let amount_in = 10_u64.pow(sol.1 as u32);
    let amount_out = pool_1.quote(amount_in, sol.0);
    println!(
        "pool : {:?}, amount_in : {:?}, amount_out : {:?}",
        pool_1.get_pool_id(),
        amount_in,
        amount_out
    );
    let pool_2_amount_out = pool_2.quote(amount_in, sol.0);
    println!(
        "pool : {:?}, amount_in : {:?}, amount_out : {:?}",
        pool_2.get_pool_id(),
        amount_in,
        pool_2_amount_out
    );
}

fn new_amm_pool(
    pool_id: Pubkey,
    pool_owner: Pubkey,
    mint_0: Pubkey,
    mint_1: Pubkey,
    mint_0_decimals: u8,
    mint_1_decimals: u8,
    mint_0_vault: u64,
    mint_1_vault: u64,
) -> AmmPool {
    AmmPool::new(
        pool_id,
        pool_owner,
        mint_0_vault,
        mint_1_vault,
        mint_0,
        mint_1,
        mint_0_decimals,
        mint_1_decimals,
        25,
        10_000,
        0,
        0,
    )
}
