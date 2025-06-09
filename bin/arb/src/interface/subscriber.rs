use crate::dex::meteora_dlmm::MeteoraDLMMAccountSubscriber;
use crate::dex::orca_whirlpools::OrcaWhirlAccountSubscriber;
use crate::dex::pump_fun::PumpFunAMMAccountSubscriber;
use crate::dex::raydium_amm::RaydiumAMMAccountSubscriber;
use crate::dex::raydium_clmm::RaydiumCLMMAccountSubscriber;
use crate::dex_data::DexJson;
use enum_dispatch::enum_dispatch;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use yellowstone_grpc_proto::geyser::SubscribeRequestFilterAccounts;

#[enum_dispatch]
pub trait AccountSubscriber {
    fn get_subscription_accounts(&self, dex_json: &[DexJson]) -> Option<SubscriptionAccounts>;
}

#[enum_dispatch(AccountSubscriber)]
pub enum Subscriber {
    MeteoraDLMMAccountSubscriber,
    PumpFunAMMAccountSubscriber,
    RaydiumAMMAccountSubscriber,
    RaydiumCLMMAccountSubscriber,
    OrcaWhirlAccountSubscriber,
}

pub fn get_subscribers() -> Option<Vec<Subscriber>> {
    Some(vec![
        Subscriber::from(MeteoraDLMMAccountSubscriber),
        Subscriber::from(PumpFunAMMAccountSubscriber),
        Subscriber::from(RaydiumAMMAccountSubscriber),
        Subscriber::from(RaydiumCLMMAccountSubscriber),
        Subscriber::from(OrcaWhirlAccountSubscriber),
    ])
}

pub struct SubscriptionAccounts {
    // 放在一个SubscribeRequestFilterAccounts中
    pub unified_accounts: Vec<Pubkey>,
    // 每个value单独一个SubscribeRequestFilterAccounts，TickArray，BinArray等订阅
    pub account_with_owner_and_filter: Option<HashMap<String, SubscribeRequestFilterAccounts>>,
    // 订阅tx包含的账户
    pub tx_include_accounts: Vec<Pubkey>,
}

impl SubscriptionAccounts {
    pub fn new(
        unified_accounts: Vec<Pubkey>,
        account_with_owner_and_filter: Option<HashMap<String, SubscribeRequestFilterAccounts>>,
        tx_include_accounts: Vec<Pubkey>,
    ) -> Self {
        Self {
            unified_accounts,
            account_with_owner_and_filter,
            tx_include_accounts,
        }
    }
}
