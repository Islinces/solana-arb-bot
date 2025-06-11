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
