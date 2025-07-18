use crate::dex::meteora_damm_v2::DAMM_V2_PROGRAM_ID;
use crate::dex::{AccountSubscriber, DexType, SubscriptionAccounts};
use crate::dex_data::DexJson;

pub struct MeteoraDAMMV2AccountSubscriber;

impl AccountSubscriber for MeteoraDAMMV2AccountSubscriber {
    fn get_subscription_accounts(&self, dex_json: &[DexJson]) -> Option<SubscriptionAccounts> {
        None
    }
}
