use std::collections::HashMap;
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestAccountsDataSlice,
    SubscribeRequestFilterAccounts,
};
use crate::cache::Pool;

pub struct MintVaultSubscribe {}

impl MintVaultSubscribe {
    pub fn mint_vault_subscribe_request(pools: &[Pool]) -> SubscribeRequest {
        SubscribeRequest {
            accounts: pools
                .iter()
                .filter_map(|pool| {
                    let pool_id = pool.pool_id;
                    if let Some((mint_0_vault, mint_1_vault)) = pool.mint_vault_pair() {
                        Some([
                            (
                                // mint_vault账户上没有关联的pool_id信息
                                // 通过filter_name在grpc推送消息时确定关联的pool
                                format!("{}:{}", pool_id, 0),
                                SubscribeRequestFilterAccounts {
                                    account: vec![mint_0_vault.to_string()],
                                    ..Default::default()
                                },
                            ),
                            (
                                format!("{}:{}", pool_id, 1),
                                SubscribeRequestFilterAccounts {
                                    account: vec![mint_1_vault.to_string()],
                                    ..Default::default()
                                },
                            ),
                        ])
                    } else {
                        None
                    }
                })
                .flatten()
                .collect::<HashMap<_, _>>(),
            commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
            accounts_data_slice: vec![
                // mint
                SubscribeRequestAccountsDataSlice {
                    offset: 0,
                    length: 32,
                },
                // amount
                SubscribeRequestAccountsDataSlice {
                    offset: 64,
                    length: 8,
                },
                // state
                SubscribeRequestAccountsDataSlice {
                    offset: 108,
                    length: 1,
                },
            ],
            ..Default::default()
        }
    }
}
