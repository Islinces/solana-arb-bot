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
        let mut pump_fun_account_keys = Vec::with_capacity(dex_json.len() * 2);

        for json in dex_json.iter() {
            pump_fun_account_keys.push(json.vault_a);
            pump_fun_account_keys.push(json.vault_b);
        }
        Some(SubscriptionAccounts::new(
            pump_fun_account_keys.clone(),
            None,
            pump_fun_account_keys,
        ))
    }
}
