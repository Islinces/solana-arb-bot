use crate::executor::Executor;
use crate::metadata::get_arb_mint_ata_amount;
use crate::quoter::QuoteResult;
use crate::state::{BalanceChangeInfo, GrpcTransactionMsg};
use base58::ToBase58;
use solana_sdk::pubkey::Pubkey;
use std::ops::{Div, Mul};
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
    ) -> Self {
        Self {
            arb_size,
            arb_amount_in,
            arb_min_profit,
            arb_mint: Arc::new(arb_mint),
            arb_mint_bps_numerator,
            arb_mint_bps_denominator,
            executor,
        }
    }

    pub async fn start(
        &self,
        join_set: &mut JoinSet<()>,
        message_cached_sender: &Sender<(GrpcTransactionMsg, Duration, Instant)>,
    ) {
        let arb_size = self.arb_size as u64;

        for index in 0..arb_size {
            let arb_amount_in = self.arb_amount_in;
            let executor = self.executor.clone();
            let arb_min_profit = self.arb_min_profit.clone();
            let arb_mint = self.arb_mint.clone();
            let arb_mint_bps_numerator = self.arb_mint_bps_numerator.clone();
            let arb_mint_bps_denominator = self.arb_mint_bps_denominator.clone();
            let mut receiver = message_cached_sender.subscribe();
            join_set.spawn(async move {
                loop {
                    match receiver.recv().await {
                        Ok((transaction_msg, _grpc_to_processor_cost, processor_send_instant)) => {
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
                                let trigger_cost = if let Some(quote_result) = Self::trigger_quote(
                                    arb_mint.clone(),
                                    arb_amount_in,
                                    arb_min_profit,
                                    arb_mint_bps_numerator,
                                    arb_mint_bps_denominator,
                                    changed_balances,
                                )
                                .await
                                {
                                    let trigger_quote_cost = trigger_instant.elapsed();
                                    let quote_info = format!("{:?}", quote_result);
                                    if quote_result.profit <= 0 {
                                        (Some(trigger_quote_cost), None, Some(quote_info), None)
                                    } else {
                                        // 有获利路径后生成指令，发送指令
                                        let msg =
                                            executor.execute(quote_result).await.unwrap_or_else(
                                                |e| format!("发送交易失败，原因：{}", e),
                                            );
                                        (
                                            Some(trigger_quote_cost),
                                            Some(trigger_instant.elapsed() - trigger_quote_cost),
                                            Some(quote_info),
                                            Some(msg),
                                        )
                                    }
                                } else {
                                    (Some(trigger_instant.elapsed()), None, None, None)
                                };
                                let all_cost = transaction_msg.instant.elapsed().as_micros();
                                let quote_cost =
                                    trigger_cost.0.unwrap_or(Duration::from_secs(0)).as_micros();
                                let execute_cost =
                                    trigger_cost.1.unwrap_or(Duration::from_secs(0)).as_micros();
                                let quote_info = trigger_cost.2.unwrap_or("".to_string());
                                let execute_msg = trigger_cost.3.unwrap_or("".to_string());
                                info!(
                                    "Arb_{index} ==> 🤝耗时 : {:>3}μs\n\
                                        路由 : {:>3}μs, \
                                        发送 : {:>3}μs,\n\
                                        交易路径 : {}\n\
                                        执行结果 : {}",
                                    all_cost, quote_cost, execute_cost, quote_info, execute_msg,
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

    async fn trigger_quote(
        arb_mint: Arc<Pubkey>,
        arb_amount_in: u64,
        arb_min_profit: u64,
        arb_mint_bps_numerator: u64,
        arb_mint_bps_denominator: u64,
        balances: Vec<BalanceChangeInfo>,
    ) -> Option<QuoteResult> {
        let arb_max_amount_in = get_arb_mint_ata_amount()
            .await?
            .mul(arb_mint_bps_numerator)
            .div(arb_mint_bps_denominator);
        crate::quoter::find_best_hop_path(
            balances.first().unwrap().pool_id,
            arb_mint,
            arb_amount_in,
            arb_max_amount_in,
            arb_min_profit,
        )
        .await

        // // TODO 所有的？
        // balances
        //     // 多个pool并行quote
        //     .into_par_iter()
        //     .filter_map(|balance_info| {
        //         crate::quoter::find_best_hop_path(
        //             &balance_info.pool_id,
        //             &amount_in_mint,
        //             // TODO
        //             10000,
        //             arb_min_profit,
        //         )
        //     })
        //     .max_by_key(|quote_result| quote_result.profit)
    }
}
