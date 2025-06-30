use crate::dex::raydium_amm::{RAYDIUM_AMM_PROGRAM_ID, RAYDIUM_AMM_VAULT_OWNER};
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
        let mut subscribed_accounts = Vec::with_capacity(dex_json.len() * 3);
        for json in dex_json.iter() {
            subscribed_accounts.push(json.pool);
            subscribed_accounts.push(json.vault_a);
            subscribed_accounts.push(json.vault_b);
        }
        Some(SubscriptionAccounts {
            tx_include_accounts: vec![RAYDIUM_AMM_PROGRAM_ID],
            account_subscribe_owners: vec![RAYDIUM_AMM_PROGRAM_ID],
            vault_subscribe_owners: vec![RAYDIUM_AMM_VAULT_OWNER],
            subscribed_accounts,
            need_clock: false,
        })
    }
}
