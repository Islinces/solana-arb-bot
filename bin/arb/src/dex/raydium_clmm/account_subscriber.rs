use crate::dex::raydium_clmm::RAYDIUM_CLMM_PROGRAM_ID;
use crate::dex_data::DexJson;
use crate::grpc_subscribe::POOL_TICK_ARRAY_BITMAP_SEED;
use crate::{AccountSubscriber, SubscriptionAccounts};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use yellowstone_grpc_proto::geyser::subscribe_request_filter_accounts_filter::Filter;
use yellowstone_grpc_proto::geyser::subscribe_request_filter_accounts_filter::Filter::Datasize;
use yellowstone_grpc_proto::geyser::{
    subscribe_request_filter_accounts_filter_memcmp, SubscribeRequestFilterAccounts,
    SubscribeRequestFilterAccountsFilter, SubscribeRequestFilterAccountsFilterMemcmp,
};

pub struct RaydiumCLMMAccountSubscriber;

impl AccountSubscriber for RaydiumCLMMAccountSubscriber {
    fn get_subscription_accounts(&self, dex_json: &[DexJson]) -> Option<SubscriptionAccounts> {
        let dex_json = dex_json
            .iter()
            .filter(|json| json.owner == RAYDIUM_CLMM_PROGRAM_ID)
            .collect::<Vec<_>>();
        if dex_json.is_empty() {
            return None;
        }
        let mut pool_keys = Vec::with_capacity(dex_json.len());
        let mut bitmap_extension_keys = Vec::with_capacity(dex_json.len());

        for json in dex_json.iter() {
            // pool
            pool_keys.push(json.pool);
            // TickArrayBitmapExtension
            bitmap_extension_keys.push(
                Pubkey::find_program_address(
                    &[POOL_TICK_ARRAY_BITMAP_SEED.as_bytes(), json.pool.as_ref()],
                    &RAYDIUM_CLMM_PROGRAM_ID,
                )
                .0,
            );
        }
        let mut tick_array_sub_accounts = HashMap::with_capacity(dex_json.len());
        for pool_id in pool_keys.iter() {
            tick_array_sub_accounts.insert(
                format!("{}:{}","CLMM-TK",pool_id.to_string()),
                SubscribeRequestFilterAccounts {
                    owner: vec![RAYDIUM_CLMM_PROGRAM_ID.to_string()],
                    filters: vec![
                        // TickArrayState data大小为10240
                        SubscribeRequestFilterAccountsFilter {
                            filter: Some(Datasize(10240)),
                        },
                        // 订阅关注的池子的TickArrayState
                        SubscribeRequestFilterAccountsFilter {
                            filter: Some(
                                Filter::Memcmp(SubscribeRequestFilterAccountsFilterMemcmp {
                                    offset: 8,
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
        Some(SubscriptionAccounts::new(
            pool_keys.clone(),
            Some(tick_array_sub_accounts),
            pool_keys,
        ))
    }
}
