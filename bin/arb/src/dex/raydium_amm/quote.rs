use crate::dex::global_cache::get_account_data;
use crate::dex::quoter::{QuoteResult, Quoter};
use crate::dex::raydium_amm::state::AmmInfo;
use crate::dex::utils::CheckedCeilDiv;
use crate::dex::MintVault;
use solana_sdk::pubkey::Pubkey;
use std::ops::{Add, Div, Mul, Sub};

#[derive(Debug)]
pub struct RaydiumAMMQuoter;

impl Quoter for RaydiumAMMQuoter {
    fn quote(&self, amount_in: u64, swap_direction: bool, pool_id: &Pubkey) -> Option<QuoteResult> {
        let amm_info = get_account_data::<AmmInfo>(pool_id)?;
        let coin_vault_amount = get_account_data::<MintVault>(&amm_info.coin_vault)?.amount;
        let pc_vault_amount = get_account_data::<MintVault>(&amm_info.pc_vault)?.amount;
        let amount_in = u128::from(amount_in);
        let swap_fee_numerator = u128::from(amm_info.swap_fee_numerator);
        let swap_fee_denominator = u128::from(amm_info.swap_fee_denominator);
        let swap_fee = amount_in
            .mul(swap_fee_numerator)
            .checked_ceil_div(swap_fee_denominator)
            .unwrap()
            .0;
        let swap_in_after_deduct_fee = amount_in.sub(swap_fee);

        let mint_0_amount_without_pnl =
            u128::from(coin_vault_amount.sub(amm_info.need_take_pnl_coin));
        let mint_1_amount_without_pnl = u128::from(pc_vault_amount.sub(amm_info.need_take_pnl_pc));
        let amount_out = if swap_direction {
            mint_1_amount_without_pnl
                .mul(swap_in_after_deduct_fee)
                .div(mint_0_amount_without_pnl.add(swap_in_after_deduct_fee))
        } else {
            mint_0_amount_without_pnl
                .mul(swap_in_after_deduct_fee)
                .div(mint_1_amount_without_pnl.add(swap_in_after_deduct_fee))
        };
        Some(QuoteResult {
            amount_out: u64::try_from(amount_out).ok()?,
        })
    }
}

#[cfg(test)]
mod test {
    use crate::dex::raydium_amm::quote::RaydiumAMMQuoter;
    use crate::dex::{init_global_cache, AmmInfo, GlobalCache, MintVault, Quoter};
    use crate::dex_data::DexJson;
    use solana_sdk::pubkey::Pubkey;
    use std::str::FromStr;

    #[test]
    fn test_raydium_amm_quote() -> anyhow::Result<()> {
        let dex_json = DexJson {
            pool: Pubkey::from_str("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2")?,
            owner: Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8")?,
            mint_a: Pubkey::from_str("So11111111111111111111111111111111111111112")?,
            mint_b: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?,
            vault_a: Pubkey::from_str("DQyrAcCrDXQ7NeoqGgDCZwBvWDcYmFCjSb9JtteuvPpz")?,
            vault_b: Pubkey::from_str("HLmqeL62xR1QoZ1HKKbXRrdN1p3phKpxRMb2VVopvBBz")?,
            address_lookup_table_address: Some(Pubkey::from_str(
                "E59uBXGqn83xN17kMbBVfU1M7T4wHG91eiygHb88Aovb",
            )?),
        };
        let amm_info = AmmInfo {
            need_take_pnl_coin: 10,
            need_take_pnl_pc: 20,
            swap_fee_numerator: 25,
            swap_fee_denominator: 10_000,
            coin_vault: dex_json.vault_a,
            pc_vault: dex_json.vault_b,
            ..Default::default()
        };
        let data = bytemuck::bytes_of(&amm_info);
        let static_data = &data[0..144];
        let dynamic_data = &data[144..];
        let mut global_cache = GlobalCache::init();
        global_cache.upsert_static(dex_json.pool, static_data.to_vec());
        global_cache.upsert_dynamic(dex_json.pool, dynamic_data.to_vec());

        let coin_vault_amount = MintVault {
            amount: 26_324 * 10_u64.pow(9),
        };
        global_cache.upsert_dynamic(
            dex_json.vault_a,
            bytemuck::bytes_of(&coin_vault_amount).to_vec(),
        );
        let pc_vault_amount = MintVault {
            amount: 3_524_576 * 10_u64.pow(6),
        };
        global_cache.upsert_dynamic(
            dex_json.vault_b,
            bytemuck::bytes_of(&pc_vault_amount).to_vec(),
        );
        init_global_cache(global_cache);
        let quote_result = RaydiumAMMQuoter
            .quote(10_u64.pow(9), true, &dex_json.pool)
            .unwrap();
        assert_eq!(quote_result.amount_out, 133552322);
        Ok(())
    }
}
