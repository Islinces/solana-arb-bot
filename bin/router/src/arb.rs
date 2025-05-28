use crate::executor::Executor;
use crate::graph::TwoHopPath;
use crate::metadata::{get_keypair, get_last_blockhash, get_native_mint_ata, remove_already_ata};
use crate::quoter::QuoteResult;
use crate::state::{BalanceChangeInfo, GrpcTransactionMsg};
use crate::MINT_PROGRAM;
use ahash::AHashMap;
use anyhow::anyhow;
use base58::ToBase58;
use base64::engine::general_purpose;
use base64::Engine;
use chrono::{DateTime, Local};
use rand::Rng;
use rayon::iter::IntoParallelIterator;
use rayon::iter::ParallelIterator;
use reqwest::Client;
use serde_json::json;
use solana_sdk::address_lookup_table::AddressLookupTableAccount;
use solana_sdk::compute_budget::ComputeBudgetInstruction;
use solana_sdk::hash::Hash;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::message::v0::Message;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::VersionedTransaction;
use spl_associated_token_account::instruction::create_associated_token_account_idempotent;
use spl_associated_token_account::{get_associated_token_address_with_program_id, solana_program};
use spl_token::instruction::transfer;
use std::ops::{Div, Mul};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Sender;
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tokio::time::Instant;
use tracing::{error, info, warn};

pub struct Arb {
    arb_size: usize,
    arb_min_profit: u64,
    executor: Arc<dyn Executor>,
}

impl Arb {
    pub fn new(arb_size: usize, arb_min_profit: u64, executor: Arc<dyn Executor>) -> Self {
        Self {
            arb_size,
            arb_min_profit,
            executor,
        }
    }

    pub async fn start(
        &self,
        join_set: &mut JoinSet<()>,
        message_cached_sender: &Sender<(GrpcTransactionMsg, DateTime<Local>)>,
    ) {
        let arb_size = self.arb_size as u64;

        for _ in 0..arb_size {
            let executor = self.executor.clone();
            let arb_min_profit = self.arb_min_profit.clone();
            let mut receiver = message_cached_sender.subscribe();
            join_set.spawn(async move {
                loop {
                    match receiver.recv().await {
                        Ok((transaction_msg, send_timestamp)) => {
                            let incoming_arb_timestamp = Local::now();
                            let instant = Instant::now();
                            let tx = transaction_msg.transaction.unwrap();
                            let meta = transaction_msg.meta.unwrap();
                            let account_keys = tx
                                .message
                                .unwrap()
                                .account_keys
                                .into_iter()
                                .chain(meta.loaded_writable_addresses)
                                .chain(meta.loaded_readonly_addresses)
                                .map(|v| Pubkey::try_from(v).unwrap())
                                .collect::<Vec<_>>();
                            let changed_balances = meta
                                .pre_token_balances
                                .into_iter()
                                .zip(meta.post_token_balances.into_iter())
                                .filter_map(|(pre, post)| {
                                    BalanceChangeInfo::new(&pre, &post, &account_keys)
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
                            if !changed_balances.is_empty() {
                                // 触发路由计算
                                match Self::trigger_quote(arb_min_profit, changed_balances) {
                                    None => {}
                                    Some(quote_result) => {
                                        // 有获利路径后生成指令，发送指令
                                        match executor.execute(quote_result).await {
                                            Ok(_) => {}
                                            Err(e) => {
                                                error!("发送交易失败，原因：{}", e);
                                            }
                                        }
                                    }
                                }
                            }
                            // if specify_pool
                            //     .is_none_or(|v| changed_balances.iter().any(|t| &t.pool_id == &v))
                            // {
                            //     info!(
                            //         "Arb_{index} ==> \n🤝Transaction, 总耗时 : {:?}μs\n\
                            //             交易 : {:?}, GRPC推送时间 : {:?}\n\
                            //             GRPC到Processor通道耗时 : {:?}μs, \
                            //             Processor到Arb通道耗时 : {:?}μs, \
                            //             获取变化的Balances耗时 : {:?}ns\n\
                            //             Balance是否发生变化 : {:?}\n\
                            //             Balances : {:#?}",
                            //         grpc_to_processor_channel_cost
                            //             + processor_to_arb_channel_cost
                            //             + (get_change_balance_cost.div_ceil(1000)),
                            //         transaction_msg.signature.as_slice().to_base58(),
                            //         transaction_msg
                            //             .received_timestamp
                            //             .format("%Y-%m-%d %H:%M:%S%.9f")
                            //             .to_string(),
                            //         grpc_to_processor_channel_cost,
                            //         processor_to_arb_channel_cost,
                            //         get_change_balance_cost,
                            //         any_balance_change,
                            //         changed_balances,
                            //     );
                            // }
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

    fn trigger_quote(arb_min_profit: u64, balances: Vec<BalanceChangeInfo>) -> Option<QuoteResult> {
        // TODO 配置
        let amount_in_mint = spl_token::native_mint::ID;
        balances
            // 多个pool并行quote
            .into_par_iter()
            .filter_map(|balance_change_info| {
                crate::quoter::find_best_hop_path(
                    &balance_change_info.pool_id,
                    &amount_in_mint,
                    // TODO
                    10000,
                    arb_min_profit,
                )
            })
            .max_by_key(|quote_result| quote_result.profit)
    }
}
