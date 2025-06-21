use crate::dex::raydium_amm::RAYDIUM_AMM_PROGRAM_ID;
use crate::dex::subscriber::{AccountSubscriber, SubscriptionAccounts};
use crate::dex_data::DexJson;

pub struct RaydiumAMMAccountSubscriber;

impl AccountSubscriber for RaydiumAMMAccountSubscriber {
    fn get_subscription_accounts(&self, dex_json: &[DexJson]) -> Option<SubscriptionAccounts> {
        let dex_json = dex_json
            .iter()
            .filter(|json| json.owner == RAYDIUM_AMM_PROGRAM_ID)
            .collect::<Vec<_>>();
        if dex_json.is_empty() {
            return None;
        }
        let mut raydium_amm_account_keys = Vec::with_capacity(dex_json.len() * 3);

        for json in dex_json.iter() {
            raydium_amm_account_keys.push(json.pool);
            raydium_amm_account_keys.push(json.vault_a);
            raydium_amm_account_keys.push(json.vault_b);
        }
        Some(SubscriptionAccounts::new(
            raydium_amm_account_keys.clone(),
            None,
            raydium_amm_account_keys,
        ))
    }
}

#[cfg(test)]
mod test {
    use crate::dex::raydium_amm::RaydiumAMMAccountSubscriber;
    use crate::dex::AccountSubscriber;
    use crate::dex_data::DexJson;
    use solana_sdk::pubkey::Pubkey;
    use std::str::FromStr;

    #[test]
    fn test_raydium_amm_subscribe() -> anyhow::Result<()> {
        let mut dex_json = vec![DexJson {
            pool: Pubkey::from_str("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2")?,
            owner: Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8")?,
            mint_a: Pubkey::from_str("So11111111111111111111111111111111111111112")?,
            mint_b: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?,
            vault_a: Pubkey::from_str("DQyrAcCrDXQ7NeoqGgDCZwBvWDcYmFCjSb9JtteuvPpz")?,
            vault_b: Pubkey::from_str("HLmqeL62xR1QoZ1HKKbXRrdN1p3phKpxRMb2VVopvBBz")?,
            address_lookup_table_address: Some(Pubkey::from_str(
                "E59uBXGqn83xN17kMbBVfU1M7T4wHG91eiygHb88Aovb",
            )?),
        }];
        let sub_accounts = RaydiumAMMAccountSubscriber
            .get_subscription_accounts(dex_json.as_slice())
            .unwrap();
        let dex_json = dex_json.pop().unwrap();
        let must_account = vec![dex_json.pool, dex_json.vault_a, dex_json.vault_a];
        assert!(must_account
            .iter()
            .all(|a| sub_accounts.unified_accounts.contains(a)));
        assert!(must_account
            .iter()
            .all(|a| sub_accounts.tx_include_accounts.contains(a)));
        assert!(sub_accounts.account_with_owner_and_filter.is_none());
        Ok(())
    }

    #[test]
    fn test_raydium_amm_subscribe_error_owner() -> anyhow::Result<()> {
        let mut dex_json = vec![DexJson {
            pool: Pubkey::from_str("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2")?,
            owner: Pubkey::from_str("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA")?,
            mint_a: Pubkey::from_str("So11111111111111111111111111111111111111112")?,
            mint_b: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?,
            vault_a: Pubkey::from_str("DQyrAcCrDXQ7NeoqGgDCZwBvWDcYmFCjSb9JtteuvPpz")?,
            vault_b: Pubkey::from_str("HLmqeL62xR1QoZ1HKKbXRrdN1p3phKpxRMb2VVopvBBz")?,
            address_lookup_table_address: Some(Pubkey::from_str(
                "E59uBXGqn83xN17kMbBVfU1M7T4wHG91eiygHb88Aovb",
            )?),
        }];
        let sub_accounts =
            RaydiumAMMAccountSubscriber.get_subscription_accounts(dex_json.as_slice());
        assert!(sub_accounts.is_none());
        Ok(())
    }
}
