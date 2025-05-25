use crate::interface::DexType;
use crate::state::{BalanceChangeInfo, CacheValue, GrpcMessage, GrpcTransactionMsg, TxId};
use ahash::{AHashMap, AHasher, HashMap, HashSet};
use base58::ToBase58;
use chrono::{DateTime, Local};
use solana_sdk::pubkey::Pubkey;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Sender;
use tokio::task::JoinSet;
use tokio::time::Instant;
use tracing::{error, info, warn};

pub struct Arb {
    pub arb_size: usize,
    pub vault_to_pool: Arc<AHashMap<Pubkey, (Pubkey, Pubkey)>>,
    pub specify_pool: Arc<Option<Pubkey>>,
}

impl Arb {
    pub fn new(
        arb_size: usize,
        vault_to_pool: AHashMap<Pubkey, (Pubkey, Pubkey)>,
        specify_pool: Option<Pubkey>,
    ) -> Self {
        Self {
            arb_size,
            vault_to_pool: Arc::new(vault_to_pool),
            specify_pool: Arc::new(specify_pool),
        }
    }

    pub async fn start(
        &self,
        join_set: &mut JoinSet<()>,
        message_cached_sender: &Sender<(GrpcTransactionMsg, DateTime<Local>)>,
    ) {
        let arb_size = self.arb_size as u64;
        for index in 0..arb_size {
            let vault_to_pool = self.vault_to_pool.clone();
            let specify_pool = self.specify_pool.clone();
            let mut receiver = message_cached_sender.subscribe();
            join_set.spawn(async move {
                while let data = receiver.recv().await {
                    let incoming_arb_timestamp = Local::now();
                    match data {
                        Ok((transaction_msg, send_timestamp)) => {
                            let instant = Instant::now();
                            let tx = transaction_msg.transaction.unwrap();
                            let meta = transaction_msg.meta.unwrap();
                            let account_keys = tx.message.unwrap().account_keys;
                            let writable_accounts = meta.loaded_writable_addresses;
                            let readable_accounts = meta.loaded_readonly_addresses;
                            let account_keys = account_keys
                                .iter()
                                .chain(writable_accounts.iter())
                                .chain(readable_accounts.iter())
                                .map(|v| Pubkey::try_from(v.as_slice()).unwrap().to_string())
                                .collect::<Vec<_>>();
                            let pre_token_balances = meta.pre_token_balances;
                            let post_token_balances = meta.post_token_balances;
                            let changed_balances = pre_token_balances
                                .into_iter()
                                .zip(post_token_balances.into_iter())
                                .filter_map(|(pre, post)| {
                                    BalanceChangeInfo::new(
                                        &pre,
                                        &post,
                                        &account_keys,
                                        vault_to_pool.clone(),
                                    )
                                })
                                .collect::<Vec<_>>();
                            let get_change_balance_cost = instant.elapsed().as_nanos();

                            let grpc_to_processor_channel_cost =
                                (send_timestamp - transaction_msg.received_timestamp)
                                    .num_microseconds()
                                    .unwrap() as u128;
                            let processor_to_arb_channel_cost =
                                (incoming_arb_timestamp - send_timestamp)
                                    .num_microseconds()
                                    .unwrap() as u128;
                            let any_balance_change = !changed_balances.is_empty();
                            if specify_pool.is_none_or(|v| {
                                    changed_balances.iter().any(|t| &t.pool_id == &v)
                                })
                            {
                                info!(
                                    "Arb_{index} ==> \n🤝Transaction, 总耗时 : {:?}μs\n\
                                        交易 : {:?}, GRPC推送时间 : {:?}\n\
                                        GRPC到Processor通道耗时 : {:?}μs, \
                                        Processor到Arb通道耗时 : {:?}μs, \
                                        获取变化的Balances耗时 : {:?}ns\n\
                                        Balance是否发生变化 : {:?}\n\
                                        Balances : {:#?}",
                                    grpc_to_processor_channel_cost
                                        + processor_to_arb_channel_cost
                                        + (get_change_balance_cost.div_ceil(1000)),
                                    transaction_msg.signature.as_slice().to_base58(),
                                    transaction_msg
                                        .received_timestamp
                                        .format("%Y-%m-%d %H:%M:%S%.9f")
                                        .to_string(),
                                    grpc_to_processor_channel_cost,
                                    processor_to_arb_channel_cost,
                                    get_change_balance_cost,
                                    any_balance_change,
                                    changed_balances,
                                );
                            }
                        }
                        Err(RecvError::Closed) => {
                            error!("action channel closed!");
                            break;
                        }
                        Err(RecvError::Lagged(num)) => {
                            warn!("action channel lagged by {num}")
                        }
                    }
                }
            });
        }
    }
}
