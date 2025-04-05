use arrayref::array_ref;
use raydium_amm::amm_pool::AmmPool;
use raydium_amm::raydium_amm_dex::RaydiumAmmDex;
use raydium_clmm::clmm_pool::ClmmPool;
use raydium_clmm::raydium_clmm_dex::RaydiumClmmDex;
use raydium_clmm::tickarray_bitmap_extension::TickArrayBitmapExtension;
use raydium_clmm::utils::deserialize_anchor_account;
use raydium_clmm::{config, pool};
use router::router::Routing;
use solana_client::rpc_client::RpcClient;
use solana_program::pubkey::Pubkey;
use spl_token_2022::extension::StateWithExtensions;
use spl_token_2022::state::Mint;
use std::alloc::*;
use std::str::FromStr;

#[test]
fn test_build_routing() {
    let mut amm_pools = Vec::<AmmPool>::new();
    let mut clmm_pools = Vec::<ClmmPool>::new();
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
    amm_pools.push(pool_1);
    let pool_2 = new_amm_pool(
        Pubkey::from_str("5oAvct85WyF7Sj73VYHbyFJkdRJ28D8m4z4Sxjvzuc6n").unwrap(),
        Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8").unwrap(),
        sol.0,
        usdc.0,
        sol.1,
        usdc.1,
        (3000.4374 * (10_u64.pow(sol.1 as u32)) as f64) as u64,
        (430000.2 * (10_u64.pow(usdc.1 as u32)) as f64) as u64,
    );
    amm_pools.push(pool_2);
    // println!("pool_2 amount_out : {:#?}", pool_2.quote(1000000000_u64, sol.0));
    let pool_3 = build_clmm_pool(
        RpcClient::new("https://solana-rpc.publicnode.com".to_string()),
        Pubkey::from_str("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK").unwrap(),
        Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
        Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
        6,
    );
    clmm_pools.push(pool_3);
    let pool_4 = build_clmm_pool(
        RpcClient::new("https://solana-rpc.publicnode.com".to_string()),
        Pubkey::from_str("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK").unwrap(),
        Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
        Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
        4,
    );
    clmm_pools.push(pool_4);
    let pool_5 = build_clmm_pool(
        RpcClient::new("https://solana-rpc.publicnode.com".to_string()),
        Pubkey::from_str("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK").unwrap(),
        Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
        Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
        2,
    );
    clmm_pools.push(pool_5);
    // let pool_6 = build_clmm_pool(
    //     RpcClient::new("https://solana-rpc.publicnode.com".to_string()),
    //     Pubkey::from_str("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK").unwrap(),
    //     Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
    //     Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
    //     0,
    // );
    // clmm_pools.push(pool_6);
    // let pool_7 = build_clmm_pool(
    //     RpcClient::new("https://solana-rpc.publicnode.com".to_string()),
    //     Pubkey::from_str("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK").unwrap(),
    //     Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
    //     Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
    //     9,
    // );
    // clmm_pools.push(pool_7);

    let routing = Routing::new(vec![
        Box::new(RaydiumAmmDex::new(amm_pools)),
        Box::new(RaydiumClmmDex::new(clmm_pools)),
    ]);
    // println!("routing : {:#?}", routing);
    let route_step = routing.find_route(
        sol.0,
        10_u64.pow(sol.1 as u32),
        Some(Pubkey::from_str("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2").unwrap()),
    );
    if let Some(step) = route_step {
        println!("Step {:?} :\n {:#?}", 1, step.0);
        println!("Step {:?} :\n {:#?}", 2, step.1);
    }
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

fn build_clmm_pool(
    rpc_client: RpcClient,
    program_id: Pubkey,
    mint_0: Pubkey,
    mint_1: Pubkey,
    amm_config_index: u16,
) -> ClmmPool {
    let mut mint0 = Some(mint_0);
    let mut mint1 = Some(mint_1);
    let (amm_config_key, __bump) = Pubkey::find_program_address(
        &[
            config::AMM_CONFIG_SEED.as_bytes(),
            &amm_config_index.to_be_bytes(),
        ],
        &program_id,
    );

    // 根据 AMM CONFIG ACCOUNT + MIN-0 + MINT-1 计算出池子地址 POOL ACCOUNT
    let pool_id_account = if mint0 != None && mint1 != None {
        if mint0.unwrap() > mint1.unwrap() {
            let temp_mint = mint0;
            mint0 = mint1;
            mint1 = temp_mint;
        }

        Some(
            Pubkey::find_program_address(
                &[
                    pool::POOL_SEED.as_bytes(),
                    amm_config_key.to_bytes().as_ref(),
                    mint0.unwrap().to_bytes().as_ref(),
                    mint1.unwrap().to_bytes().as_ref(),
                ],
                &program_id,
            )
            .0,
        )
    } else {
        None
    };

    // 根据 POOL ACCOUNT 计算
    let tickarray_bitmap_extension = if pool_id_account != None {
        Some(
            Pubkey::find_program_address(
                &[
                    pool::POOL_TICK_ARRAY_BITMAP_SEED.as_bytes(),
                    pool_id_account.unwrap().to_bytes().as_ref(),
                ],
                &program_id,
            )
            .0,
        )
    } else {
        None
    };
    let load_accounts = vec![
        amm_config_key,
        pool_id_account.unwrap(),
        tickarray_bitmap_extension.unwrap(),
        mint0.unwrap(),
        mint1.unwrap(),
    ];
    let rsps = rpc_client.get_multiple_accounts(&load_accounts).unwrap();
    let [amm_config_account, pool_account, tickarray_bitmap_extension_account, mint0_account, mint1_account] =
        array_ref![rsps, 0, 5];
    let mint0_data = mint0_account.clone().unwrap().data;
    let mint0_state = StateWithExtensions::<Mint>::unpack(&mint0_data).unwrap();
    let mint1_data = mint1_account.clone().unwrap().data;
    let mint1_state = StateWithExtensions::<Mint>::unpack(&mint1_data).unwrap();
    let amm_config_state =
        deserialize_anchor_account::<config::AmmConfig>(amm_config_account.as_ref().unwrap())
            .unwrap();
    let pool_state =
        deserialize_anchor_account::<pool::PoolState>(pool_account.as_ref().unwrap()).unwrap();
    let tickarray_bitmap_extension = deserialize_anchor_account::<TickArrayBitmapExtension>(
        tickarray_bitmap_extension_account.as_ref().unwrap(),
    )
    .unwrap();
    let zero_to_one_tick_arrays = raydium_clmm::utils::load_cur_and_next_specify_count_tick_array(
        &rpc_client,
        5,
        &pool_id_account.unwrap(),
        &program_id,
        &pool_state,
        &tickarray_bitmap_extension,
        true,
    );
    let one_to_zero_tick_arrays = raydium_clmm::utils::load_cur_and_next_specify_count_tick_array(
        &rpc_client,
        5,
        &pool_id_account.unwrap(),
        &program_id,
        &pool_state,
        &tickarray_bitmap_extension,
        false,
    );
    ClmmPool {
        pool_id: pool_id_account.unwrap(),
        owner_id: program_id,
        amm_config: amm_config_key,
        mint_0: mint0.unwrap(),
        mint_1: mint1.unwrap(),
        mint_0_decimals: mint0_state.base.decimals,
        mint_1_decimals: mint1_state.base.decimals,
        tick_spacing: pool_state.tick_spacing,
        liquidity: pool_state.liquidity,
        sqrt_price_x64: pool_state.sqrt_price_x64,
        tick_current: pool_state.tick_current,
        tick_array_bitmap: pool_state.tick_array_bitmap,
        tick_array_bitmap_extension: tickarray_bitmap_extension,
        zero_to_one_tick_arays: zero_to_one_tick_arrays,
        one_to_zero_tick_arays: one_to_zero_tick_arrays,
        trade_fee_rate: amm_config_state.trade_fee_rate,
    }
}
