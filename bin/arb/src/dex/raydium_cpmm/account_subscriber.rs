use crate::dex::raydium_cpmm::{RAYDIUM_CPMM_AUTHORITY_ID, RAYDIUM_CPMM_PROGRAM_ID};
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
        let mut subscribed_accounts = Vec::with_capacity(dex_json.len() * 3);

        for json in dex_json.iter() {
            subscribed_accounts.push(json.pool);
            subscribed_accounts.push(json.vault_a);
            subscribed_accounts.push(json.vault_b);
        }
        Some(SubscriptionAccounts {
            tx_include_accounts: vec![RAYDIUM_CPMM_PROGRAM_ID, RAYDIUM_CPMM_AUTHORITY_ID],
            account_subscribe_owners: vec![RAYDIUM_CPMM_PROGRAM_ID],
            subscribed_accounts,
            need_clock: false,
        })
    }
}
