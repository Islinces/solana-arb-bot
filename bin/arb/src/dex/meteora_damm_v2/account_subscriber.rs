use crate::dex::{AccountSubscriber, DexType, SubscriptionAccounts, CLOCK_ID};
use crate::dex_data::DexJson;

pub struct MeteoraDAMMV2AccountSubscriber;

impl AccountSubscriber for MeteoraDAMMV2AccountSubscriber {
    fn get_subscription_accounts(&self, dex_json: &[DexJson]) -> Option<SubscriptionAccounts> {
        let dex_json = dex_json
            .iter()
            .filter(|json| &json.owner == DexType::MeteoraDAMMV2.get_ref_program_id())
            .collect::<Vec<_>>();
        if dex_json.is_empty() {
            return None;
        }
        let mut pool_keys = Vec::with_capacity(dex_json.len() * 2);
        for json in dex_json.iter() {
            // pool
            pool_keys.push(json.pool);
        }
        let mut unified_accounts = Vec::from(pool_keys.clone());
        unified_accounts.push(CLOCK_ID);
        Some(SubscriptionAccounts::new(unified_accounts, None, pool_keys))
    }
}
