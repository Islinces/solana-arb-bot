use crate::dex::pump_fun::PUMP_FUN_AMM_PROGRAM_ID;
use crate::dex::subscriber::{AccountSubscriber, SubscriptionAccounts};
use crate::dex_data::DexJson;

pub struct PumpFunAMMAccountSubscriber;

impl AccountSubscriber for PumpFunAMMAccountSubscriber {
    fn get_subscription_accounts(&self, dex_json: &[DexJson]) -> Option<SubscriptionAccounts> {
        None
    }
}
