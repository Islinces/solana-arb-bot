// fn quote(
//     &self,
//     amount_in: u64,
//     in_mint: Pubkey,
//     _out_mint: Pubkey,
//     pool: &Pool,
//     clock: Arc<Clock>,
// ) -> Option<u64> {
//     let mint_0 = pool.mint_0();
//     let mint_1 = pool.mint_1();
//     if amount_in == u64::MIN || (in_mint != mint_0 && in_mint != mint_1) {
//         return None;
//     }
//     let swap_for_y = in_mint == mint_0;
//     let lp_pair_state: LbPair = pool_state.clone().into();
//     match pool_state.get_bin_array_map(swap_for_y, 3) {
//         Ok(bin_array_map) => {
//             let result = quote_exact_in(
//                 pool.pool_id,
//                 lp_pair_state,
//                 amount_in,
//                 swap_for_y,
//                 bin_array_map,
//                 pool_state.bin_array_bitmap_extension,
//                 clock.clone(),
//                 pool_state.mint_x_transfer_fee_config,
//                 pool_state.mint_y_transfer_fee_config,
//             );
//             match result {
//                 Ok(quote) => Some(quote.amount_out),
//                 Err(_) => None,
//             }
//         }
//         Err(_) => None,
//     }
// }