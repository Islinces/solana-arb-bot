use crate::executor::Executor;
use crate::graph::HopPath;
use crate::grpc_processor::BalanceChangeInfo;
use crate::grpc_subscribe::GrpcTransactionMsg;
use crate::metadata::get_arb_mint_ata_amount;
use crate::{HopPathSearchResult, HopPathTypes, SearchResult};
use base58::ToBase58;
use parking_lot::RwLock;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use solana_sdk::pubkey::Pubkey;
use std::ops::{Div, Mul};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Sender;
use tokio::task::JoinSet;
use tokio::time::Instant;
use tracing::{error, info, warn};

pub struct Arb {
    arb_size: usize,
    arb_amount_in: u64,
    arb_min_profit: u64,
    arb_mint: Arc<Pubkey>,
    arb_mint_bps_numerator: u64,
    arb_mint_bps_denominator: u64,
    executor: Arc<dyn Executor>,
    hop_paths: Arc<Vec<RwLock<HopPathTypes>>>,
}

impl Arb {
    pub fn new(
        arb_size: usize,
        arb_amount_in: u64,
        arb_min_profit: u64,
        arb_mint: Pubkey,
        arb_mint_bps_numerator: u64,
        arb_mint_bps_denominator: u64,
        executor: Arc<dyn Executor>,
        hop_paths: Arc<Vec<RwLock<HopPathTypes>>>,
    ) -> Self {
        Self {
            arb_size,
            arb_amount_in,
            arb_min_profit,
            arb_mint: Arc::new(arb_mint),
            arb_mint_bps_numerator,
            arb_mint_bps_denominator,
            executor,
            hop_paths,
        }
    }

    pub async fn start(
        &self,
        join_set: &mut JoinSet<()>,
        cached_message_receiver: flume::Receiver<GrpcTransactionMsg>,
    ) {
        let arb_size = self.arb_size as u64;
        for index in 0..arb_size {
            let arb_amount_in = self.arb_amount_in;
            let executor = self.executor.clone();
            let arb_min_profit = self.arb_min_profit.clone();
            let arb_mint = self.arb_mint.clone();
            let arb_mint_bps_numerator = self.arb_mint_bps_numerator.clone();
            let arb_mint_bps_denominator = self.arb_mint_bps_denominator.clone();
            let mut receiver = cached_message_receiver.clone();
            let hop_paths = self.hop_paths.clone();
            join_set.spawn(async move {
                loop {
                    match receiver.recv_async().await {
                        Ok(transaction_msg) => {
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
                            if !changed_balances.is_empty() {
                                // 触发路由计算
                                let trigger_instant = Instant::now();
                                if let Some(best_path) = Self::trigger_quote(
                                    hop_paths.clone(),
                                    arb_mint.clone(),
                                    arb_amount_in,
                                    arb_min_profit,
                                    arb_mint_bps_numerator,
                                    arb_mint_bps_denominator,
                                    changed_balances,
                                ) {
                                    let trigger_quote_cost = trigger_instant.elapsed();
                                    let quote_info: String = best_path.information();
                                    let tx = transaction_msg
                                        .signature
                                        .as_slice()
                                        .to_base58()
                                        .chars()
                                        .take(4)
                                        .collect::<String>();
                                    // 有获利路径后生成指令，发送指令
                                    let msg = executor
                                        .execute(best_path, tx.clone(), transaction_msg.slot)
                                        .await
                                        .unwrap_or_else(|e| format!("发送交易失败，原因：{}", e));
                                    let all_cost = transaction_msg.instant.elapsed().as_millis();
                                    let quote_cost = trigger_quote_cost.as_micros();
                                    info!(
                                        "\nArb_{index} ==> 耗时 : {:>4.2}ms, \
                                        路由 : {:>4.2}μs, \
                                        {} \n路径 : {}, tx : {},  Slot : {}, Time: {}",
                                        all_cost,
                                        quote_cost,
                                        msg,
                                        quote_info,
                                        tx,
                                        transaction_msg.slot,
                                        transaction_msg
                                            .received_timestamp
                                            .format("%Y-%m-%d %H:%M:%S.%3f")
                                    );
                                }
                            }
                        }
                        Err(_) => {
                            error!("Arb_{index} 接收消息失败，原因：所有的Processor关闭");
                            break;
                        }
                    }
                }
            });
        }
    }

    fn trigger_quote(
        hop_paths: Arc<Vec<RwLock<HopPathTypes>>>,
        arb_mint: Arc<Pubkey>,
        arb_amount_in: u64,
        arb_min_profit: u64,
        arb_mint_bps_numerator: u64,
        arb_mint_bps_denominator: u64,
        balances: Vec<BalanceChangeInfo>,
    ) -> Option<HopPathSearchResult> {
        let arb_max_amount_in = get_arb_mint_ata_amount()?
            .mul(arb_mint_bps_numerator)
            .div(arb_mint_bps_denominator);
        hop_paths
            .par_iter()
            .filter_map(|best_hop_path_searcher| {
                best_hop_path_searcher.read().find_best_hop_path(
                    balances.first().unwrap().pool_id,
                    arb_mint.clone(),
                    arb_amount_in,
                    arb_max_amount_in,
                    arb_min_profit,
                )
            })
            .max_by_key(|a| a.profit())
    }
}
