use crate::dex::pump_fun::PUMP_FUN_AMM_PROGRAM_ID;
use crate::dex::subscriber::{AccountSubscriber, SubscriptionAccounts};
use crate::dex_data::DexJson;

pub struct PumpFunAMMAccountSubscriber;

impl AccountSubscriber for PumpFunAMMAccountSubscriber {
    fn get_subscription_accounts(&self, dex_json: &[DexJson]) -> Option<SubscriptionAccounts> {
        let dex_json = dex_json
            .iter()
            .filter(|json| json.owner == PUMP_FUN_AMM_PROGRAM_ID)
            .collect::<Vec<_>>();
        if dex_json.is_empty() {
            return None;
        }
        let mut subscribed_accounts = Vec::with_capacity(dex_json.len() * 2);
        let mut account_subscribe_owners = Vec::with_capacity(dex_json.len() + 1);
        let mut vault_subscribe_owners = Vec::with_capacity(dex_json.len());
        account_subscribe_owners.push(PUMP_FUN_AMM_PROGRAM_ID);
        for json in dex_json.iter() {
            vault_subscribe_owners.push(json.pool);
            account_subscribe_owners.push(json.pool);
            subscribed_accounts.push(json.vault_a);
            subscribed_accounts.push(json.vault_b);
        }
        Some(SubscriptionAccounts {
            tx_include_accounts: vec![PUMP_FUN_AMM_PROGRAM_ID],
            account_subscribe_owners,
            vault_subscribe_owners,
            subscribed_accounts,
            need_clock: false,
        })
    }
}
