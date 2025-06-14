use crate::dex::raydium_amm::RAYDIUM_AMM_PROGRAM_ID;
use crate::dex::{AccountSubscriber, DexType, SubscriptionAccounts};
use crate::dex_data::DexJson;

pub struct RaydiumCPMMAccountSubscriber;

impl AccountSubscriber for RaydiumCPMMAccountSubscriber {
    fn get_subscription_accounts(&self, dex_json: &[DexJson]) -> Option<SubscriptionAccounts> {
        let dex_json = dex_json
            .iter()
            .filter(|json| &json.owner == DexType::RaydiumCPMM.get_ref_program_id())
            .collect::<Vec<_>>();
        if dex_json.is_empty() {
            return None;
        }
        let mut account_keys = Vec::with_capacity(dex_json.len() * 3);

        for json in dex_json.iter() {
            account_keys.push(json.pool);
            account_keys.push(json.vault_a);
            account_keys.push(json.vault_b);
        }
        Some(SubscriptionAccounts::new(
            account_keys.clone(),
            None,
            account_keys,
        ))
    }
}
