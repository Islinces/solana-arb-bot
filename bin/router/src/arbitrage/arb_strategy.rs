use crate::arbitrage::arb_worker::Arb;
use crate::arbitrage::message_processor::GrpcDataProcessor;
use crate::arbitrage::Action;
use crate::dex;
use crate::dex::DexData;
use crate::interface::{AccountUpdate, DexType, GrpcMessage, SourceMessage};
use async_channel::Sender;
use burberry::{ActionSubmitter, Strategy};
use futures_util::TryFutureExt;
use solana_client::nonblocking::rpc_client::RpcClient;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::runtime::{Builder, Handle, RuntimeFlavor};
use tracing::error;

#[macro_export]
macro_rules! run_in_tokio {
    ($code:expr) => {
        match Handle::try_current() {
            Ok(handle) => match handle.runtime_flavor() {
                RuntimeFlavor::CurrentThread => std::thread::scope(move |s| {
                    s.spawn(move || {
                        Builder::new_current_thread()
                            .enable_all()
                            .build()
                            .unwrap()
                            .block_on(async move { $code.await })
                    })
                    .join()
                    .unwrap()
                }),
                _ => {
                    tokio::task::block_in_place(move || handle.block_on(async move { $code.await }))
                }
            },
            Err(_) => Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move { $code.await }),
        }
    };
}

pub struct ArbStrategy {
    event_capacity: u64,
    event_expired_mills: u64,
    arb_worker_size: usize,
    sol_ata_amount: Arc<u64>,
    max_amount_in_numerator: u8,
    max_amount_in_denominator: u8,
    profit_threshold: u64,
    protocol_grpc_sender: Option<HashMap<DexType, Sender<AccountUpdate>>>,
    rpc_client: Arc<RpcClient>,
}

impl ArbStrategy {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        rpc_client: Arc<RpcClient>,
        arb_worker_size: usize,
        sol_ata_amount: Arc<u64>,
        max_amount_in_numerator: u8,
        max_amount_in_denominator: u8,
        profit_threshold: u64,
        event_expired_mills: Option<u64>,
        event_capacity: Option<u64>,
    ) -> Self {
        Self {
            event_expired_mills: event_expired_mills.unwrap_or(1000),
            event_capacity: event_capacity.unwrap_or(1000),
            arb_worker_size,
            sol_ata_amount,
            max_amount_in_numerator,
            max_amount_in_denominator,
            profit_threshold,
            rpc_client,
            protocol_grpc_sender: None,
        }
    }
}

#[burberry::async_trait]
impl Strategy<SourceMessage, Action> for ArbStrategy {
    fn name(&self) -> &str {
        "arb_strategy"
    }

    async fn sync_state(
        &mut self,
        submitter: Arc<dyn ActionSubmitter<Action>>,
    ) -> eyre::Result<()> {
        let protocols = dex::supported_protocols();
        let protocol_size = protocols.len();
        let (init_tx, mut init_rx) = tokio::sync::mpsc::channel(protocol_size);
        let (ready_data_sender, ready_data_receiver) = async_channel::unbounded::<GrpcMessage>();
        let mut grpc_message_sender_maps = HashMap::new();
        let event_expired_mills = self.event_expired_mills;
        let event_capacity = self.event_capacity;
        for protocol in protocols {
            let (grpc_message_sender, grpc_message_receiver) =
                async_channel::unbounded::<AccountUpdate>();
            grpc_message_sender_maps.insert(protocol.clone(), grpc_message_sender);
            let ready_grpc_data_sender = ready_data_sender.clone();
            let init_tx = init_tx.clone();
            let use_cache = protocol.use_cache();
            let _ = std::thread::Builder::new()
                .stack_size(128 * 1024 * 1024) // 128 MB
                .name(format!("cache-worker-{:?}", protocol))
                .spawn(move || {
                    run_in_tokio!(init_tx.send(())).unwrap();
                    let data_process_worker = GrpcDataProcessor::new(
                        use_cache,
                        event_capacity,
                        event_expired_mills,
                        grpc_message_receiver,
                        ready_grpc_data_sender,
                    );
                    let _ = data_process_worker.run().unwrap_or_else(|e| {
                        panic!("thread of [cache-worker-{}] panicked: {e:?}", protocol)
                    });
                });
        }
        for _ in 0..protocol_size {
            init_rx.recv().await.expect("worker initialization failed");
        }
        self.protocol_grpc_sender = Some(grpc_message_sender_maps);
        let (init_tx, mut init_rx) = tokio::sync::mpsc::channel(self.arb_worker_size);
        for index in 0..self.arb_worker_size {
            let swap_action_sender = submitter.clone();
            let ready_grpc_data_receiver = ready_data_receiver.clone();
            let init_tx = init_tx.clone();
            let rpc_client = self.rpc_client.clone();
            let sol_ata_amount = self.sol_ata_amount.clone();
            let max_amount_in_numerator = self.max_amount_in_numerator;
            let max_amount_in_denominator = self.max_amount_in_denominator;
            let profit_threshold = self.profit_threshold;
            let _ = std::thread::Builder::new()
                .stack_size(128 * 1024 * 1024) // 128 MB
                .name(format!("route-worker-{:?}", index))
                .spawn(move || {
                    let dex = Arc::new(
                        run_in_tokio!({
                            DexData::new(
                                rpc_client,
                                sol_ata_amount,
                                max_amount_in_numerator,
                                max_amount_in_denominator,
                            )
                        })
                        .unwrap(),
                    );
                    run_in_tokio!(init_tx.send(())).unwrap();
                    let arb_worker = Arb::new(
                        dex,
                        ready_grpc_data_receiver,
                        swap_action_sender,
                        profit_threshold,
                    );
                    let _ = arb_worker.run().unwrap_or_else(|e| {
                        panic!("worker of [arb-worker-{index}] panicked: {e:?}")
                    });
                });
        }
        for _ in 0..self.arb_worker_size {
            init_rx.recv().await.expect("worker initialization failed");
        }
        Ok(())
    }

    async fn process_event(
        &mut self,
        event: SourceMessage,
        _submitter: Arc<dyn ActionSubmitter<Action>>,
    ) {
        if let SourceMessage::Account(account) = event {
            if let Some(sender) = self
                .protocol_grpc_sender
                .as_ref()
                .unwrap()
                .get(&account.protocol)
            {
                if let Err(e) = sender.send(account).await {
                    error!("发送失败：{}", e);
                }
            }
        }
    }
}
