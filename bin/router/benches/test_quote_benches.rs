use criterion::{criterion_group, criterion_main, Criterion};
use router::defi::dex::Dex;
use router::defi::raydium_amm::math::{CheckedCeilDiv, SwapDirection};
use router::defi::raydium_amm::raydium_amm::RaydiumAmmDex;
use router::defi::types::{Mint, Pool, PoolExtra, Protocol};
use solana_program::pubkey::Pubkey;
use std::ops::{Add, Div, Mul, Sub};
use std::str::FromStr;
use std::time::Duration;

fn bench_route(c: &mut Criterion) {
    let in_mint = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
    let raydium_amm_dex = RaydiumAmmDex::new(
        Pool {
            protocol: Protocol::RaydiumAMM,
            pool_id: Pubkey::from_str("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2").unwrap(),
            tokens: vec![
                Mint {
                    mint: in_mint,
                    decimals: 9,
                },
                Mint {
                    mint: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
                    decimals: 6,
                },
            ],
            extra: PoolExtra::RaydiumAMM {
                mint_0_vault_amount: Some(1744849814023_u64),
                mint_1_vault_amount: Some(11752484441015_u64),
                mint_0_need_take_pnl: Some(0),
                mint_1_need_take_pnl: Some(0),
                mint_0_vault: None,
                mint_1_vault: None,
                swap_fee_numerator: 25,
                swap_fee_denominator: 10000,
            },
        },
        in_mint,
    );
    let mut group = c.benchmark_group("bench_route");
    group.sample_size(10000);
    group.warm_up_time(Duration::from_secs(5));
    group.measurement_time(std::time::Duration::from_secs(5));
    group.bench_function("bench_route", |b| {
        b.iter(|| {
            test(
                1_000_000_000_u64,
                // in_mint,
                // in_mint,
                25_u64,
                10000_u64,
                1744849814023_u64,
                11752484441015_u64,
                0,
                0,
                SwapDirection::Coin2PC,
                // true
            );
            // raydium_amm_dex.quote(1_000_000_000_u64);
        })
    });
    group.finish();
}

criterion_group!(benches, bench_route,);
criterion_main!(benches);

fn test(
    amount_in: u64,
    swap_fee_numerator: u64,
    swap_fee_denominator: u64,
    mint_0_vault_amount: u64,
    mint_1_vault_amount: u64,
    mint_0_need_take_pnl: u64,
    mint_1_need_take_pnl: u64,
    swap_direction: SwapDirection,
    // zero_to_one: bool,
) -> Option<u64> {
    let amount_in = u128::from(amount_in);
    let swap_fee = amount_in
        .checked_mul(u128::from(swap_fee_numerator))
        .unwrap()
        .checked_ceil_div(u128::from(swap_fee_denominator))
        .unwrap()
        .0;

    let swap_in_after_deduct_fee = amount_in.checked_sub(swap_fee)
        .unwrap()
        ;

    let mint_0_amount_without_pnl = u128::from(
        mint_0_vault_amount
            .checked_sub(mint_0_need_take_pnl)
            .unwrap(),
    );
    let mint_1_amount_without_pnl = u128::from(
        mint_1_vault_amount
            .checked_sub(mint_1_need_take_pnl)
            .unwrap(),
    );
    let amount_out = if let SwapDirection::PC2Coin=swap_direction {
        mint_1_amount_without_pnl
            .checked_mul(swap_in_after_deduct_fee)
            .unwrap()
            .checked_div(
                mint_0_amount_without_pnl.add(swap_in_after_deduct_fee), // .unwrap(),
            )
        .unwrap()
    } else {
        mint_0_amount_without_pnl
            .checked_mul(swap_in_after_deduct_fee)
            .unwrap()
            .checked_div(
                mint_1_amount_without_pnl.add(swap_in_after_deduct_fee), // .unwrap(),
            )
        .unwrap()
    };
    Some(amount_out.try_into().unwrap())
}

// fn test_build_routing() -> (RaydiumAmmDex, Pubkey, u8) {
//     let mut amm_pools = Vec::<RaydiumAmmDex>::new();
//     // let mut clmm_pools = Vec::<ClmmPool>::new();
//     // let mut pump_fun_pools = Vec::<PumpFunPool>::new();
//     // let mut dlmm_pools = Vec::<DlmmPool>::new();
//     let rpc_client = RpcClient::new("https://solana-rpc.publicnode.com".to_string());
//     let sol = (
//         Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
//         9,
//     );
//     let usdc = (
//         Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
//         6,
//     );
//     {
//         let pool_1 = new_amm_pool(
//             Pubkey::from_str("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2").unwrap(),
//             Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8").unwrap(),
//             sol.0,
//             usdc.0,
//             sol.1,
//             usdc.1,
//             (26_324.87 * (10_u64.pow(sol.1 as u32)) as f64) as u64,
//             (3_524_576.3 * (10_u64.pow(usdc.1 as u32)) as f64) as u64,
//         );
//         amm_pools.push(pool_1);
//         // let pool_2 = new_amm_pool(
//         //     Pubkey::from_str("5oAvct85WyF7Sj73VYHbyFJkdRJ28D8m4z4Sxjvzuc6n").unwrap(),
//         //     Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8").unwrap(),
//         //     sol.0,
//         //     usdc.0,
//         //     sol.1,
//         //     usdc.1,
//         //     (3000.4374 * (10_u64.pow(sol.1 as u32)) as f64) as u64,
//         //     (430000.2 * (10_u64.pow(usdc.1 as u32)) as f64) as u64,
//         // );
//         // amm_pools.push(pool_2);
//     }
//     // {
//     //     let pool_3 = build_clmm_pool(
//     //         &rpc_client,
//     //         Pubkey::from_str("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK").unwrap(),
//     //         Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
//     //         Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
//     //         6,
//     //     );
//     //     clmm_pools.push(pool_3);
//     //     let pool_4 = build_clmm_pool(
//     //         &rpc_client,
//     //         Pubkey::from_str("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK").unwrap(),
//     //         Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
//     //         Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
//     //         4,
//     //     );
//     //     clmm_pools.push(pool_4);
//     //     let pool_5 = build_clmm_pool(
//     //         &rpc_client,
//     //         Pubkey::from_str("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK").unwrap(),
//     //         Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
//     //         Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
//     //         2,
//     //     );
//     //     clmm_pools.push(pool_5);
//     // }
//     // {
//     //     let pool_1 = build_pump_fun_pool(
//     //         &rpc_client,
//     //         Pubkey::from_str("Gf7sXMoP8iRw4iiXmJ1nq4vxcRycbGXy5RL8a8LnTd3v").unwrap(),
//     //     );
//     //     pump_fun_pools.push(pool_1);
//     // }
//     // {
//     //     let pool_1 = build_dlmm_pool(&rpc_client, usdc.0, sol.0, 80, 12_500);
//     //     dlmm_pools.push(pool_1);
//     // }
//     // (
//     //     Routing::new(vec![
//     //         Arc::new(RaydiumAmmDex::new(amm_pools)),
//     //         Arc::new(RaydiumClmmDex::new(clmm_pools)),
//     //         Arc::new(PumpFunDex::new(pump_fun_pools)),
//     //         Arc::new(MeteoraDlmmDex::new(dlmm_pools)),
//     //     ]),
//     //     sol.0,
//     //     sol.1,
//     // )
// }

// fn new_amm_pool(
//     pool_id: Pubkey,
//     pool_owner: Pubkey,
//     mint_0: Pubkey,
//     mint_1: Pubkey,
//     mint_0_decimals: u8,
//     mint_1_decimals: u8,
//     mint_0_vault: u64,
//     mint_1_vault: u64,
// ) -> AmmPool {
//     AmmPool::new(
//         pool_id,
//         pool_owner,
//         mint_0_vault,
//         mint_1_vault,
//         mint_0,
//         mint_1,
//         mint_0_decimals,
//         mint_1_decimals,
//         25,
//         10_000,
//         0,
//         0,
//     )
// }
//
// fn build_clmm_pool(
//     rpc_client: &RpcClient,
//     program_id: Pubkey,
//     mint_0: Pubkey,
//     mint_1: Pubkey,
//     amm_config_index: u16,
// ) -> ClmmPool {
//     let mut mint0 = Some(mint_0);
//     let mut mint1 = Some(mint_1);
//     let (amm_config_key, __bump) = Pubkey::find_program_address(
//         &[
//             config::AMM_CONFIG_SEED.as_bytes(),
//             &amm_config_index.to_be_bytes(),
//         ],
//         &program_id,
//     );
//
//     // 根据 AMM CONFIG ACCOUNT + MIN-0 + MINT-1 计算出池子地址 POOL ACCOUNT
//     let pool_id_account = if mint0 != None && mint1 != None {
//         if mint0.unwrap() > mint1.unwrap() {
//             let temp_mint = mint0;
//             mint0 = mint1;
//             mint1 = temp_mint;
//         }
//
//         Some(
//             Pubkey::find_program_address(
//                 &[
//                     pool::POOL_SEED.as_bytes(),
//                     amm_config_key.to_bytes().as_ref(),
//                     mint0.unwrap().to_bytes().as_ref(),
//                     mint1.unwrap().to_bytes().as_ref(),
//                 ],
//                 &program_id,
//             )
//             .0,
//         )
//     } else {
//         None
//     };
//
//     // 根据 POOL ACCOUNT 计算
//     let tickarray_bitmap_extension = if pool_id_account != None {
//         Some(
//             Pubkey::find_program_address(
//                 &[
//                     pool::POOL_TICK_ARRAY_BITMAP_SEED.as_bytes(),
//                     pool_id_account.unwrap().to_bytes().as_ref(),
//                 ],
//                 &program_id,
//             )
//             .0,
//         )
//     } else {
//         None
//     };
//     let load_accounts = vec![
//         amm_config_key,
//         pool_id_account.unwrap(),
//         tickarray_bitmap_extension.unwrap(),
//         mint0.unwrap(),
//         mint1.unwrap(),
//     ];
//     let rsps = rpc_client.get_multiple_accounts(&load_accounts).unwrap();
//     let [amm_config_account, pool_account, tickarray_bitmap_extension_account, mint0_account, mint1_account] =
//         array_ref![rsps, 0, 5];
//     let mint0_data = mint0_account.clone().unwrap().data;
//     let mint0_state = StateWithExtensions::<Mint>::unpack(&mint0_data).unwrap();
//     let mint1_data = mint1_account.clone().unwrap().data;
//     let mint1_state = StateWithExtensions::<Mint>::unpack(&mint1_data).unwrap();
//     let amm_config_state =
//         deserialize_anchor_account::<config::AmmConfig>(amm_config_account.as_ref().unwrap())
//             .unwrap();
//     let pool_state =
//         deserialize_anchor_account::<pool::PoolState>(pool_account.as_ref().unwrap()).unwrap();
//     let tickarray_bitmap_extension = deserialize_anchor_account::<TickArrayBitmapExtension>(
//         tickarray_bitmap_extension_account.as_ref().unwrap(),
//     )
//     .unwrap();
//     let zero_to_one_tick_arrays = raydium_clmm::sdk::utils::load_cur_and_next_specify_count_tick_array(
//         &rpc_client,
//         5,
//         &pool_id_account.unwrap(),
//         &program_id,
//         &pool_state,
//         &tickarray_bitmap_extension,
//         true,
//     );
//     let one_to_zero_tick_arrays = raydium_clmm::sdk::utils::load_cur_and_next_specify_count_tick_array(
//         &rpc_client,
//         5,
//         &pool_id_account.unwrap(),
//         &program_id,
//         &pool_state,
//         &tickarray_bitmap_extension,
//         false,
//     );
//     ClmmPool {
//         pool_id: pool_id_account.unwrap(),
//         owner_id: program_id,
//         amm_config: amm_config_key,
//         mint_0: mint0.unwrap(),
//         mint_1: mint1.unwrap(),
//         mint_0_decimals: mint0_state.base.decimals,
//         mint_1_decimals: mint1_state.base.decimals,
//         tick_spacing: pool_state.tick_spacing,
//         liquidity: pool_state.liquidity,
//         sqrt_price_x64: pool_state.sqrt_price_x64,
//         tick_current: pool_state.tick_current,
//         tick_array_bitmap: pool_state.tick_array_bitmap,
//         tick_array_bitmap_extension: tickarray_bitmap_extension,
//         tick_array_states: zero_to_one_tick_arrays,
//         one_to_zero_tick_arays: one_to_zero_tick_arrays,
//         trade_fee_rate: amm_config_state.trade_fee_rate,
//     }
// }
//
// fn build_pump_fun_pool(rpc_client: &RpcClient, pool_id: Pubkey) -> PumpFunPool {
//     fn global_config_pda() -> Pubkey {
//         let amm_program_id =
//             Pubkey::from_str("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA").unwrap();
//         Pubkey::find_program_address(&[b"global_config"], &amm_program_id).0
//     }
//     let accounts = vec![pool_id, global_config_pda()];
//     let accounts = rpc_client.get_multiple_accounts(&accounts).unwrap();
//     let pool =
//         pump_fun::utils::deserialize_anchor_account::<Pool>(accounts[0].as_ref().unwrap()).unwrap();
//     let global_config =
//         pump_fun::utils::deserialize_anchor_account::<GlobalConfig>(accounts[1].as_ref().unwrap())
//             .unwrap();
//     let vault_accounts = rpc_client
//         .get_multiple_accounts(&[pool.pool_base_token_account, pool.pool_quote_token_account])
//         .unwrap();
//     let mint_0_vault =
//         spl_token::state::Account::unpack(vault_accounts[0].as_ref().unwrap().data.as_slice())
//             .unwrap();
//     let mint_1_vault =
//         spl_token::state::Account::unpack(vault_accounts[1].as_ref().unwrap().data.as_slice())
//             .unwrap();
//     PumpFunPool::new(
//         pool_id,
//         pool.base_mint,
//         pool.quote_mint,
//         mint_0_vault.amount,
//         mint_1_vault.amount,
//         global_config.lp_fee_basis_points,
//         global_config.protocol_fee_basis_points,
//     )
// }
//
// fn build_dlmm_pool(
//     rpc_client: &RpcClient,
//     mint_x: Pubkey,
//     mint_y: Pubkey,
//     bin_step: u16,
//     base_factor: u16,
// ) -> DlmmPool {
//     let lb_pair_pubkey = derive_lb_pair_pda2(mint_x, mint_y, bin_step, base_factor).0;
//     let bitmap_extension_pubkey = derive_bin_array_bitmap_extension(lb_pair_pubkey).0;
//     let clock_id = Clock::id();
//     let accounts = rpc_client
//         .get_multiple_accounts(&[
//             lb_pair_pubkey,
//             mint_x,
//             mint_y,
//             bitmap_extension_pubkey,
//             clock_id,
//         ])
//         .unwrap();
//     let [lb_pair_account, mint_x_account, mint_y_account, bitmap_extension_account, clock_account] =
//         array_ref![accounts, 0, 5];
//     let lb_pair_state = LbPairAccount::deserialize(&lb_pair_account.as_ref().unwrap().data)
//         .unwrap()
//         .0;
//     let bitmap_extension = match bitmap_extension_account {
//         None => None,
//         Some(account) => Some(
//             BinArrayBitmapExtensionAccount::deserialize(&account.data)
//                 .unwrap()
//                 .0,
//         ),
//     };
//     let clock: Clock =
//         bincode::deserialize(&clock_account.as_ref().unwrap().data.as_ref()).unwrap();
//     let mint_x_transfer_fee_config = mint_transfer_fee_config(
//         lb_pair_state.token_mint_x_program_flag,
//         mint_x_account.as_ref().unwrap(),
//     );
//     let mint_y_transfer_fee_config = mint_transfer_fee_config(
//         lb_pair_state.token_mint_y_program_flag,
//         mint_y_account.as_ref().unwrap(),
//     );
//     let left_bin_arrays = get_bin_arrays_by_swap_direction(
//         lb_pair_pubkey,
//         &lb_pair_state,
//         bitmap_extension.as_ref(),
//         true,
//         3,
//         &rpc_client,
//     );
//     let right_bin_arrays = get_bin_arrays_by_swap_direction(
//         lb_pair_pubkey,
//         &lb_pair_state,
//         bitmap_extension.as_ref(),
//         false,
//         3,
//         &rpc_client,
//     );
//
//     DlmmPool::new(
//         lb_pair_pubkey,
//         lb_pair_state,
//         left_bin_arrays,
//         right_bin_arrays,
//         bitmap_extension,
//         mint_x_transfer_fee_config,
//         mint_y_transfer_fee_config,
//         clock,
//     )
// }
//
// fn mint_transfer_fee_config(
//     token_mint_program_flag: u8,
//     mint_account: &Account,
// ) -> Option<TransferFeeConfig> {
//     match token_mint_program_flag {
//         1 => {
//             let token_mint_data = mint_account.data.as_ref();
//             let token_mint_unpacked = StateWithExtensions::<
//                 anchor_spl::token_2022::spl_token_2022::state::Mint,
//             >::unpack(token_mint_data)
//             .unwrap();
//
//             if let Ok(transfer_fee_config) =
//                 token_mint_unpacked.get_extension::<TransferFeeConfig>()
//             {
//                 return Some(*transfer_fee_config);
//             }
//         }
//         _ => {}
//     }
//     None
// }
//
// fn get_bin_arrays_by_swap_direction(
//     lb_pair_pubkey: Pubkey,
//     lb_pair_state: &LbPair,
//     bitmap_extension: Option<&BinArrayBitmapExtension>,
//     swap_for_y: bool,
//     take: u8,
//     rpc_client: &RpcClient,
// ) -> HashMap<Pubkey, BinArray> {
//     let bin_arrays_for_swap = get_bin_array_pubkeys_for_swap(
//         lb_pair_pubkey,
//         lb_pair_state,
//         bitmap_extension,
//         swap_for_y,
//         take,
//     )
//     .unwrap();
//     rpc_client
//         .get_multiple_accounts(&bin_arrays_for_swap)
//         .unwrap()
//         .into_iter()
//         .zip(bin_arrays_for_swap.iter())
//         .map(|(account, &key)| {
//             let account = account.unwrap();
//             Some((
//                 key,
//                 BinArrayAccount::deserialize(account.data.as_ref()).ok()?.0,
//             ))
//         })
//         .collect::<Option<HashMap<Pubkey, BinArray>>>()
//         .unwrap()
// }
