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
    arb_min_profit: u64,
    arb_mint: Arc<Pubkey>,
    arb_mint_bps_numerator: u64,
    arb_mint_bps_denominator: u64,
    executor: Arc<dyn Executor>,
}

impl Arb {
    pub fn new(
        arb_size: usize,
        arb_min_profit: u64,
        arb_mint: Pubkey,
        arb_mint_bps_numerator: u64,
        arb_mint_bps_denominator: u64,
        executor: Arc<dyn Executor>,
    ) -> Self {
        Self {
            arb_size,
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
            let executor = self.executor.clone();
            let arb_min_profit = self.arb_min_profit.clone();
            let arb_mint = self.arb_mint.clone();
            let arb_mint_bps_numerator = self.arb_mint_bps_numerator.clone();
            let arb_mint_bps_denominator = self.arb_mint_bps_denominator.clone();
            let mut receiver = message_cached_sender.subscribe();
            join_set.spawn(async move {
                loop {
                    match receiver.recv().await {
                        Ok((transaction_msg, grpc_to_processor_cost, processor_send_instant)) => {
                            let processor_to_arb_cost = processor_send_instant.elapsed();
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
                            let get_change_balance_cost =
                                processor_send_instant.elapsed() - processor_to_arb_cost;
                            let any_changed = !changed_balances.is_empty();
                            let trigger_cost = if any_changed {
                                // 触发路由计算
                                let trigger_instant = Instant::now();
                                if let Some(quote_result) = Self::trigger_quote(
                                    arb_min_profit,
                                    arb_mint.clone(),
                                    arb_mint_bps_numerator,
                                    arb_mint_bps_denominator,
                                    changed_balances,
                                )
                                .await
                                {
                                    let trigger_quote_cost = trigger_instant.elapsed();
                                    let quote_info = format!("{:?}", quote_result);
                                    // 有获利路径后生成指令，发送指令
                                    let msg = executor
                                        .execute(quote_result)
                                        .await
                                        .unwrap_or_else(|e| format!("发送交易失败，原因：{}", e));
                                    (
                                        Some(trigger_quote_cost),
                                        Some(trigger_instant.elapsed() - trigger_quote_cost),
                                        Some(quote_info),
                                        Some(msg),
                                    )
                                } else {
                                    (Some(trigger_instant.elapsed()), None, None, None)
                                }
                            } else {
                                (None, None, None, None)
                            };
                            let quote_cost = trigger_cost.0.unwrap_or(Duration::from_secs(0));
                            let execute_cost = trigger_cost.1.unwrap_or(Duration::from_secs(0));
                            let quote_info = trigger_cost.2.unwrap_or("".to_string());
                            let execute_msg = trigger_cost.3.unwrap_or("".to_string());
                            info!(
                                "Arb_{index} ==> \n🤝Transaction, 总耗时 : {:?}μs\n\
                                        交易 : {:?}, GRPC推送时间 : {:?}\n\
                                        GRPC到Processor通道耗时 : {:?}μs, \
                                        Processor到Arb通道耗时 : {:?}μs, \
                                        获取变化的Balances耗时 : {:?}ns, \
                                        计算路由耗时 : {:?}μs, \
                                        发送交易耗时 : {:?}μs,\n\
                                        交易路径 : {:?}\n\
                                        执行结果 : {:?}",
                                (grpc_to_processor_cost
                                    + processor_to_arb_cost
                                    + get_change_balance_cost
                                    + quote_cost
                                    + execute_cost)
                                    .as_micros(),
                                transaction_msg.signature.as_slice().to_base58(),
                                transaction_msg
                                    .received_timestamp
                                    .format("%Y-%m-%d %H:%M:%S%.9f")
                                    .to_string(),
                                grpc_to_processor_cost.as_micros(),
                                processor_to_arb_cost.as_micros(),
                                get_change_balance_cost.as_nanos(),
                                quote_cost.as_micros(),
                                execute_cost.as_micros(),
                                quote_info,
                                execute_msg,
                            );
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
        arb_min_profit: u64,
        arb_mint: Arc<Pubkey>,
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
            // TODO
            10_u64.pow(9),
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
