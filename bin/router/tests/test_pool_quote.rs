// use arrayref::array_ref;
// use meteora_dlmm::dlmm_pool::DlmmPool;
// use meteora_dlmm::meteora_dlmm_dex::MeteoraDlmmDex;
// use meteora_dlmm::sdk::commons::pda::{derive_bin_array_bitmap_extension, derive_lb_pair_pda2};
// use meteora_dlmm::sdk::commons::quote::get_bin_array_pubkeys_for_swap;
// use meteora_dlmm::sdk::interface::accounts::{
//     BinArray, BinArrayAccount, BinArrayBitmapExtension, BinArrayBitmapExtensionAccount, LbPair,
//     LbPairAccount,
// };
// use pump_fun::pump_fun_dex::PumpFunDex;
// use pump_fun::pump_fun_pool::PumpFunPool;
// use pump_fun::{GlobalConfig, Pool as PumpPool};
// use raydium_amm::amm_pool::AmmPool;
// use raydium_amm::raydium_amm_dex::RaydiumAmmDex;
// use raydium_amm::state::{AmmInfo, Loadable};
// use raydium_clmm::clmm_pool::ClmmPool;
// use raydium_clmm::raydium_clmm_dex::RaydiumClmmDex;
// use raydium_clmm::sdk::tickarray_bitmap_extension::TickArrayBitmapExtension;
// use raydium_clmm::sdk::utils::deserialize_anchor_account;
// use router::router::Routing;
// use solana_client::rpc_client::RpcClient;
// use solana_program::clock::Clock;
// use solana_program::program_pack::Pack;
// use solana_program::pubkey::Pubkey;
// use solana_program::sysvar::SysvarId;
// use solana_sdk::account::{Account, ReadableAccount};
// use spl_token_2022::extension::transfer_fee::TransferFeeConfig;
// use spl_token_2022::extension::{BaseStateWithExtensions, StateWithExtensions};
// use spl_token_2022::state::Mint;
// use std::alloc::*;
// use std::collections::HashMap;
// use std::ops::Div;
// use std::str::FromStr;
// use std::sync::Arc;
// use raydium_clmm::sdk::{config, pool};
//
// #[test]
// fn test_build_routing() {
//     let sol = (
//         Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
//         9,
//     );
//     let usdc = (
//         Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
//         6,
//     );
//     let amount_in_mint = sol.0;
//     let amount_in = 10_u64.pow(sol.1 as u32).div(10_u64.pow(6));
//     let mut amm_pools = Vec::<AmmPool>::new();
//     // let mut clmm_pools = Vec::<ClmmPool>::new();
//     // let mut pump_fun_pools = Vec::<PumpFunPool>::new();
//     // let mut dlmm_pools = Vec::<DlmmPool>::new();
//     let rpc_client = RpcClient::new("https://solana-rpc.publicnode.com".to_string());
//
//     {
//         let pool_1 = new_amm_pool(
//             Pubkey::from_str("3gSjs6MqyHFsp8DXvaKvVUJjV7qg5itf9qmUGuhnSaWH").unwrap(),
//             &rpc_client,
//         );
//         amm_pools.push(pool_1);
//         let pool_2 = new_amm_pool(
//             Pubkey::from_str("9JZdkfK4gUtq6QzP3Pq82PjDYPW9eRtqHNtjZn23Nc51").unwrap(),
//             &rpc_client,
//         );
//         amm_pools.push(pool_2);
//         let pool_1 = new_amm_pool(
//             Pubkey::from_str("5fGDRDhRhkbiTdgww4v4wRq6HZTYeV6qyVi1PJVV9qpu").unwrap(),
//             &rpc_client,
//         );
//         amm_pools.push(pool_1);
//         let pool_2 = new_amm_pool(
//             Pubkey::from_str("H2p2de3UXq42To4XA5ByCY8C2NgJzXX6nzNvxbrMESBW").unwrap(),
//             &rpc_client,
//         );
//         amm_pools.push(pool_2);
//         let pool_2 = new_amm_pool(
//             Pubkey::from_str("FFhoDyFx1TvZhJHMUbU6BSi7dkrxEJucRFVYRnsb9xBy").unwrap(),
//             &rpc_client,
//         );
//         amm_pools.push(pool_2);
//         let pool_2 = new_amm_pool(
//             Pubkey::from_str("8vVQ4G39TeBqiJguKPxYto6a997KQSaSwrscqxR6QtGT").unwrap(),
//             &rpc_client,
//         );
//         amm_pools.push(pool_2);
//         let pool_2 = new_amm_pool(
//             Pubkey::from_str("EUvnsWhMnhY3S5EnV3tLATNRvTM2xBMWZQQguzpFNwYT").unwrap(),
//             &rpc_client,
//         );
//         amm_pools.push(pool_2);
//         let pool_2 = new_amm_pool(
//             Pubkey::from_str("AbbG2aR8iNhy2prC32iDRW7pKJjzqhUtri8rV5HboHUY").unwrap(),
//             &rpc_client,
//         );
//         amm_pools.push(pool_2);
//         let pool_2 = new_amm_pool(
//             Pubkey::from_str("7s2pco2iYjNPKeN7QYaHD8ZSRb8jZgypspwg7DNkZhZJ").unwrap(),
//             &rpc_client,
//         );
//         amm_pools.push(pool_2);
//     }
//     // {
//     //     let pool_3 = build_clmm_pool(
//     //         &rpc_client,
//     //         Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
//     //         Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
//     //         0,
//     //     );
//     //     clmm_pools.push(pool_3);
//     //     let pool_4 = build_clmm_pool(
//     //         &rpc_client,
//     //         Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
//     //         Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
//     //         1,
//     //     );
//     //     clmm_pools.push(pool_4);
//     //     let pool_5 = build_clmm_pool(
//     //         &rpc_client,
//     //         Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
//     //         Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
//     //         2,
//     //     );
//     //     clmm_pools.push(pool_5);
//     //     let pool_4 = build_clmm_pool(
//     //         &rpc_client,
//     //         Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
//     //         Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
//     //         4,
//     //     );
//     //     clmm_pools.push(pool_4);
//     //     let pool_5 = build_clmm_pool(
//     //         &rpc_client,
//     //         Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
//     //         Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
//     //         5,
//     //     );
//     //     clmm_pools.push(pool_5);
//     //     let pool_3 = build_clmm_pool(
//     //         &rpc_client,
//     //         Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
//     //         Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
//     //         6,
//     //     );
//     //     clmm_pools.push(pool_3);
//     //     let pool_5 = build_clmm_pool(
//     //         &rpc_client,
//     //         Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
//     //         Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
//     //         8,
//     //     );
//     //     clmm_pools.push(pool_5);
//     // }
//     // {
//     //     let pool_1 = build_pump_fun_pool(
//     //         &rpc_client,
//     //         Pubkey::from_str("FJgYJ23aemGm67iNAr2dZ8H7uLZ8Y1CorPW4KehrHhtM").unwrap(),
//     //     );
//     //     pump_fun_pools.push(pool_1);
//     //     let pool_2 = build_pump_fun_pool(
//     //         &rpc_client,
//     //         Pubkey::from_str("3s5pmAP8Zaphm92gdEdwnAcj6fJCNGuS8tdVYLRwwk4c").unwrap(),
//     //     );
//     //     pump_fun_pools.push(pool_2);
//     //     let pool_3 = build_pump_fun_pool(
//     //         &rpc_client,
//     //         Pubkey::from_str("6ef59PhPsXgre7d8BUB2J6GGK6RM3ABek6XSR7J3Z6kX").unwrap(),
//     //     );
//     //     pump_fun_pools.push(pool_3);
//     //     let pool_4 = build_pump_fun_pool(
//     //         &rpc_client,
//     //         Pubkey::from_str("FBirun32CEFCQXeZCWjZVWzCs628Co3DqY3vV8u9Xhdz").unwrap(),
//     //     );
//     //     pump_fun_pools.push(pool_4);
//     // }
//     // {
//     //     let pool_1 = build_dlmm_pool(&rpc_client, usdc.0, sol.0, 80, 12_500);
//     //     dlmm_pools.push(pool_1);
//     //     let pool_1 = build_dlmm_pool(&rpc_client, usdc.0, sol.0, 400, 12_500);
//     //     dlmm_pools.push(pool_1);
//     //     let pool_1 = build_dlmm_pool(&rpc_client, usdc.0, sol.0, 80, 62_500);
//     //     dlmm_pools.push(pool_1);
//     //     let pool_1 = build_dlmm_pool(&rpc_client, usdc.0, sol.0, 2, 15_000);
//     //     dlmm_pools.push(pool_1);
//     // }
//     let routing = Routing::new(vec![
//         Arc::new(RaydiumAmmDex::new(amm_pools)),
//         Arc::new(RaydiumClmmDex::new(clmm_pools)),
//         Arc::new(PumpFunDex::new(pump_fun_pools)),
//         Arc::new(MeteoraDlmmDex::new(dlmm_pools)),
//     ]);
//     // println!("routing : {:#?}", routing);
//     let route_step = routing.find_route(
//         amount_in_mint,
//         amount_in,
//         Some(Pubkey::from_str("Gf7sXMoP8iRw4iiXmJ1nq4vxcRycbGXy5RL8a8LnTd3v").unwrap()),
//     );
//     if let Some(step) = route_step {
//         println!("Step {:?} :\n {:#?}", 1, step.0);
//         println!("Step {:?} :\n {:#?}", 2, step.1);
//     }
// }
//
// fn new_amm_pool(pool_id: Pubkey, rpc_client: &RpcClient) -> AmmPool {
//     let account = rpc_client.get_account(&pool_id).unwrap();
//     let amm_info = AmmInfo::load_from_bytes(&account.data()).unwrap();
//     let vault_accounts = rpc_client
//         .get_multiple_accounts(&[amm_info.coin_vault, amm_info.pc_vault])
//         .unwrap();
//     let [coin_vault, pc_vault] = array_ref![vault_accounts, 0, 2];
//     println!("amm pool_id : {:?}", pool_id);
//     AmmPool::new(
//         pool_id,
//         amm_info.amm_owner,
//         spl_token::state::Account::unpack(&coin_vault.as_ref().unwrap().data)
//             .unwrap()
//             .amount,
//         spl_token::state::Account::unpack(&pc_vault.as_ref().unwrap().data)
//             .unwrap()
//             .amount,
//         amm_info.coin_vault_mint,
//         amm_info.pc_vault_mint,
//         amm_info.coin_decimals as u8,
//         amm_info.pc_decimals as u8,
//         amm_info.fees.swap_fee_numerator,
//         amm_info.fees.swap_fee_denominator,
//         amm_info.state_data.need_take_pnl_coin,
//         amm_info.state_data.need_take_pnl_pc,
//     )
// }
//
// fn build_clmm_pool(
//     rpc_client: &RpcClient,
//     mint_0: Pubkey,
//     mint_1: Pubkey,
//     amm_config_index: u16,
// ) -> ClmmPool {
//     let program_id = Pubkey::from_str("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK").unwrap();
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
//     println!("clmm pool_id : {}", pool_id_account.unwrap());
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
//         pump_fun::utils::deserialize_anchor_account::<PumpPool>(accounts[0].as_ref().unwrap()).unwrap();
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
//     println!("pump func pool_id : {:?}", pool_id);
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
//     println!("dlmm pool_id : {:?}", lb_pair_pubkey);
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
