use crate::defi::types::{AccountUpdate, PoolCache, Protocol, SourceMessage};
use crate::defi::Defi;
use crate::strategy::arb::Arb;
use crate::strategy::grpc_message_processor::{GrpcDataProcessor, GrpcMessage};
use crate::strategy::Action;
use async_channel::Sender;
use burberry::{ActionSubmitter, Strategy};
use solana_program::pubkey::Pubkey;
use std::collections::HashMap;
use std::sync::Arc;
use futures_util::TryFutureExt;
use tokio::runtime::{Builder, Handle, RuntimeFlavor};
use tracing::error;
use yellowstone_grpc_proto::geyser::SubscribeUpdateAccountInfo;

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

pub struct GrpcSubscribeStrategy {
    event_capacity: u64,
    event_expired_mills: u64,
    grpc_worker_size: usize,
    protocol_grpc_sender: Option<HashMap<Protocol, Sender<AccountUpdate>>>,
    protocols: Vec<Protocol>,
    rpc_url: String,
    subscribe_mints: Vec<Pubkey>,
}

impl GrpcSubscribeStrategy {
    pub fn new(
        rpc_url: String,
        subscribe_mints: Vec<Pubkey>,
        protocol: Vec<Protocol>,
        event_expired_mills: Option<u64>,
        event_capacity: Option<u64>,
        grpc_worker_size: usize,
    ) -> Self {
        Self {
            event_expired_mills: event_expired_mills.unwrap_or(1000),
            event_capacity: event_capacity.unwrap_or(1000),
            grpc_worker_size,
            protocols: protocol,
            rpc_url,
            subscribe_mints,
            protocol_grpc_sender: None,
        }
    }
}

#[burberry::async_trait]
impl Strategy<SourceMessage, Action> for GrpcSubscribeStrategy {
    fn name(&self) -> &str {
        "grpc_subscribe_strategy"
    }

    async fn sync_state(
        &mut self,
        submitter: Arc<dyn ActionSubmitter<Action>>,
    ) -> eyre::Result<()> {
        let (init_tx, mut init_rx) = tokio::sync::mpsc::channel(self.protocols.len());
        let (ready_data_sender, ready_data_receiver) = async_channel::unbounded::<GrpcMessage>();
        let mut grpc_message_sender_maps = HashMap::new();
        let event_expired_mills = self.event_expired_mills;
        let event_capacity = self.event_capacity;
        for protocol in self.protocols.clone() {
            let (grpc_message_sender, grpc_message_receiver) =
                async_channel::unbounded::<AccountUpdate>();
            grpc_message_sender_maps.insert(protocol.clone(), grpc_message_sender);
            let ready_grpc_data_sender = ready_data_sender.clone();
            let init_tx = init_tx.clone();
            let worker_name = format!("{}", protocol.clone());

            let _ = std::thread::Builder::new()
                .stack_size(128 * 1024 * 1024) // 128 MB
                .name(format!("grpc-data-{:?}", worker_name))
                .spawn(move || {
                    run_in_tokio!(init_tx.send(())).unwrap();
                    let data_process_worker = GrpcDataProcessor::new(
                        event_capacity,
                        event_expired_mills,
                        grpc_message_receiver,
                        ready_grpc_data_sender,
                    );
                    let _ = data_process_worker
                        .run()
                        .unwrap_or_else(|e| panic!("worker {worker_name} panicked: {e:?}"));
                });
        }
        for _ in 0..self.protocols.len() {
            init_rx.recv().await.expect("worker initialization failed");
        }
        self.protocol_grpc_sender = Some(grpc_message_sender_maps);
        let (init_tx, mut init_rx) = tokio::sync::mpsc::channel(self.grpc_worker_size);
        for index in 0..self.grpc_worker_size {
            let swap_action_sender = submitter.clone();
            let ready_grpc_data_receiver = ready_data_receiver.clone();
            let init_tx = init_tx.clone();
            let rpc_url = self.rpc_url.clone();
            let subscribe_mints = self.subscribe_mints.clone();
            let _ = std::thread::Builder::new()
                .stack_size(128 * 1024 * 1024) // 128 MB
                .name(format!("worker-{:?}", index))
                .spawn(move || {
                    let defi =
                        Arc::new(run_in_tokio!({ Defi::new(&rpc_url, &subscribe_mints) }).unwrap());
                    run_in_tokio!(init_tx.send(())).unwrap();
                    let arb_worker = Arb::new(defi, ready_grpc_data_receiver, swap_action_sender);
                    let _ = arb_worker
                        .run()
                        .unwrap_or_else(|e| panic!("worker {index} panicked: {e:?}"));
                });
        }
        for _ in 0..self.grpc_worker_size {
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
