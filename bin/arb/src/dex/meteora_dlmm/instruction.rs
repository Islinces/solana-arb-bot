// fn converter(
//     &self,
//     wallet: Pubkey,
//     instruction_item: InstructionItem,
// ) -> Option<(
//     Vec<AccountMeta>,
//     [(Pubkey, Pubkey); 2],
//     Vec<AddressLookupTableAccount>,
// )> {
//     match instruction_item {
//         InstructionItem::MeteoraDLMM(item) => {
//             let mut accounts = Vec::with_capacity(20);
//             // 1.lb pair
//             accounts.push(AccountMeta::new(item.pool_id, false));
//             // 2.bitmap extension
//             // accounts.push(AccountMeta::new(item.bitmap_extension, false));
//             accounts.push(AccountMeta::new_readonly(
//                 DexType::MeteoraDLMM.get_program_id(),
//                 false,
//             ));
//             // 3.mint_0 vault
//             accounts.push(AccountMeta::new(item.mint_0_vault, false));
//             // 4.mint_1 vault
//             accounts.push(AccountMeta::new(item.mint_1_vault, false));
//             let (mint_0_ata, _) = Pubkey::find_program_address(
//                 &[
//                     &wallet.to_bytes(),
//                     &get_mint_program().to_bytes(),
//                     &item.mint_0.to_bytes(),
//                 ],
//                 &get_ata_program(),
//             );
//             let (mint_1_ata, _) = Pubkey::find_program_address(
//                 &[
//                     &wallet.to_bytes(),
//                     &get_mint_program().to_bytes(),
//                     &item.mint_1.to_bytes(),
//                 ],
//                 &get_ata_program(),
//             );
//             if item.zero_to_one {
//                 // 5.mint_0 ata
//                 accounts.push(AccountMeta::new(mint_0_ata, false));
//                 // 6.mint_1 ata
//                 accounts.push(AccountMeta::new(mint_1_ata, false));
//             } else {
//                 // 5.mint_1 ata
//                 accounts.push(AccountMeta::new(mint_1_ata, false));
//                 // 6.mint_0 ata
//                 accounts.push(AccountMeta::new(mint_0_ata, false));
//             }
//             // 7.mint_0
//             accounts.push(AccountMeta::new_readonly(item.mint_0, false));
//             // 8.mint_1
//             accounts.push(AccountMeta::new_readonly(item.mint_1, false));
//             // 9.oracle
//             accounts.push(AccountMeta::new(item.oracle, false));
//             // 10.fee account
//             accounts.push(AccountMeta::new_readonly(
//                 DexType::MeteoraDLMM.get_program_id(),
//                 false,
//             ));
//             // 11.wallet
//             accounts.push(AccountMeta::new(wallet, true));
//             // 12.mint_0 program
//             accounts.push(AccountMeta::new_readonly(get_mint_program(), false));
//             // 13.mint_1 program
//             accounts.push(AccountMeta::new_readonly(get_mint_program(), false));
//             // 14.Event Authority
//             accounts.push(AccountMeta::new_readonly(
//                 Pubkey::from_str("D1ZN9Wj1fRSUQfCjhvnu1hqDMT7hzjzBBpi12nVniYD6").unwrap(),
//                 false,
//             ));
//             // 15.program
//             accounts.push(AccountMeta::new_readonly(
//                 DexType::MeteoraDLMM.get_program_id(),
//                 false,
//             ));
//             // 16~~.current bin array
//             let bin_arrays = item
//                 .bin_arrays
//                 .into_iter()
//                 .map(|k| AccountMeta::new(k, false))
//                 .collect::<Vec<_>>();
//             accounts.extend(bin_arrays);
//             Some((
//                 accounts,
//                 [(item.mint_0, mint_0_ata), (item.mint_1, mint_1_ata)],
//                 vec![item.alt],
//             ))
//         }
//         _ => None,
//     }
// }