use crate::dex::raydium_cpmm::{RAYDIUM_CPMM_AUTHORITY_ID, RAYDIUM_CPMM_PROGRAM_ID};
use crate::dex::{AccountSubscriber, DexType, SubscriptionAccounts};
use crate::dex_data::DexJson;

pub struct RaydiumCPMMAccountSubscriber;

impl AccountSubscriber for RaydiumCPMMAccountSubscriber {
    fn get_subscription_accounts(&self, dex_json: &[DexJson]) -> Option<SubscriptionAccounts> {
        None
    }
}
