use crate::dex::meteora_damm_v2::DAMM_V2_PROGRAM_ID;
use crate::dex::{AccountSubscriber, DexType, SubscriptionAccounts};
use crate::dex_data::DexJson;

pub struct MeteoraDAMMV2AccountSubscriber;

impl AccountSubscriber for MeteoraDAMMV2AccountSubscriber {
    fn get_subscription_accounts(&self, dex_json: &[DexJson]) -> Option<SubscriptionAccounts> {
        let subscribed_accounts = dex_json
            .iter()
            .filter(|json| &json.owner == DexType::MeteoraDAMMV2.get_ref_program_id())
            .map(|json| json.pool)
            .collect::<Vec<_>>();
        if subscribed_accounts.is_empty() {
            return None;
        }
        Some(SubscriptionAccounts {
            tx_include_accounts: vec![DAMM_V2_PROGRAM_ID],
            account_subscribe_owners: vec![DAMM_V2_PROGRAM_ID],
            subscribed_accounts,
            need_clock: true,
        })
    }
}
