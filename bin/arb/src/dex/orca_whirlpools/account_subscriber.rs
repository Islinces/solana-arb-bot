use crate::dex::orca_whirlpools::WHIRLPOOL_ID;
use crate::dex_data::DexJson;
use crate::{AccountSubscriber, SubscriptionAccounts};
use solana_sdk::clock::Clock;
use solana_sdk::sysvar::SysvarId;
use std::collections::HashMap;
use yellowstone_grpc_proto::geyser::subscribe_request_filter_accounts_filter::Filter;
use yellowstone_grpc_proto::geyser::subscribe_request_filter_accounts_filter::Filter::Datasize;
use yellowstone_grpc_proto::geyser::{
    subscribe_request_filter_accounts_filter_memcmp, SubscribeRequestFilterAccounts,
    SubscribeRequestFilterAccountsFilter, SubscribeRequestFilterAccountsFilterMemcmp,
};

pub struct OrcaWhirlAccountSubscriber;

impl AccountSubscriber for OrcaWhirlAccountSubscriber {
    fn get_subscription_accounts(&self, dex_json: &[DexJson]) -> Option<SubscriptionAccounts> {
        let dex_json = dex_json
            .iter()
            .filter(|json| json.owner == WHIRLPOOL_ID)
            .collect::<Vec<_>>();
        if dex_json.is_empty() {
            return None;
        }
        let mut orca_whirl_account_keys = Vec::with_capacity(dex_json.len());
        let mut orca_whirl_oracle_account_keys = Vec::with_capacity(dex_json.len());

        for json in dex_json.iter() {
            // pool
            orca_whirl_account_keys.push(json.pool);
            // oracle
            orca_whirl_oracle_account_keys
                .push(crate::dex::orca_whirlpools::get_oracle_address(&json.pool).unwrap());
        }
        // orca whirl tick array
        let mut tick_array_sub_accounts = HashMap::with_capacity(dex_json.len());
        for pool_id in orca_whirl_account_keys.iter() {
            tick_array_sub_accounts.insert(
                format!("{}:{}","WH-TK",pool_id.to_string()),
                SubscribeRequestFilterAccounts {
                    owner: vec![WHIRLPOOL_ID.to_string()],
                    filters: vec![
                        // TickArray data大小为10136
                        SubscribeRequestFilterAccountsFilter {
                            filter: Some(Datasize(9988)),
                        },
                        // 订阅关注的池子的TickArray
                        SubscribeRequestFilterAccountsFilter {
                            filter: Some(
                                Filter::Memcmp(SubscribeRequestFilterAccountsFilterMemcmp {
                                    offset: 9956,
                                    data: Some(
                                        subscribe_request_filter_accounts_filter_memcmp::Data::Bytes(
                                            pool_id.to_bytes().to_vec(),
                                        ),
                                    ),
                                }),
                            ),
                        },
                    ],
                    ..Default::default()
                },
            );
        }
        orca_whirl_account_keys.extend(orca_whirl_oracle_account_keys);
        let mut unified_accounts = Vec::from(orca_whirl_account_keys.clone());
        unified_accounts.push(Clock::id());
        Some(SubscriptionAccounts::new(
            unified_accounts,
            Some(tick_array_sub_accounts),
            orca_whirl_account_keys,
        ))
    }
}
