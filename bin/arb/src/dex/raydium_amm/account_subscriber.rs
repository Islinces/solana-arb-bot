use crate::dex::raydium_amm::{RAYDIUM_AMM_PROGRAM_ID, RAYDIUM_AMM_VAULT_OWNER};
use crate::dex::subscriber::{AccountSubscriber, SubscriptionAccounts};
use crate::dex_data::DexJson;

pub struct RaydiumAMMAccountSubscriber;

impl AccountSubscriber for RaydiumAMMAccountSubscriber {
    fn get_subscription_accounts(&self, dex_json: &[DexJson]) -> Option<SubscriptionAccounts> {
        None
    }
}
