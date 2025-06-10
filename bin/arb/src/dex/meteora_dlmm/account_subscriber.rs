use crate::dex::meteora_dlmm::METEORA_DLMM_PROGRAM_ID;
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

pub struct MeteoraDLMMAccountSubscriber;

impl AccountSubscriber for MeteoraDLMMAccountSubscriber {
    fn get_subscription_accounts(&self, dex_json: &[DexJson]) -> Option<SubscriptionAccounts> {
        let dex_json = dex_json
            .iter()
            .filter(|json| json.owner == METEORA_DLMM_PROGRAM_ID)
            .collect::<Vec<_>>();
        if dex_json.is_empty() {
            return None;
        }
        let mut pool_keys = Vec::with_capacity(dex_json.len() * 2);
        let mut bitmap_extension_keys = Vec::with_capacity(dex_json.len() * 2);

        for json in dex_json.iter() {
            // pool
            pool_keys.push(json.pool);
            // BinArrayBitmapExtension
            bitmap_extension_keys.push(
                crate::dex::meteora_dlmm::commons::pda::derive_bin_array_bitmap_extension(
                    &json.pool,
                ),
            );
        }
        let mut bin_array_sub_accounts = HashMap::with_capacity(dex_json.len());
        for pool_id in pool_keys.iter() {
            bin_array_sub_accounts.insert(
                format!("{}:{}","DLMM-BIN",pool_id.to_string()),
                SubscribeRequestFilterAccounts {
                    owner: vec![METEORA_DLMM_PROGRAM_ID.to_string()],
                    filters: vec![
                        // BinArray data大小为10136
                        SubscribeRequestFilterAccountsFilter {
                            filter: Some(Datasize(10136)),
                        },
                        // 订阅关注的池子的BinArray
                        SubscribeRequestFilterAccountsFilter {
                            filter: Some(
                                Filter::Memcmp(SubscribeRequestFilterAccountsFilterMemcmp {
                                    offset: 24,
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
        pool_keys.extend(bitmap_extension_keys);
        let mut unified_accounts = Vec::from(pool_keys.clone());
        unified_accounts.push(Clock::id());
        Some(SubscriptionAccounts::new(
            unified_accounts,
            Some(bin_array_sub_accounts),
            pool_keys,
        ))
    }
}
