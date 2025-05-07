use crate::arbitrage::Action;
use crate::dex::DexData;
use crate::interface::GrpcMessage;
use async_channel::Receiver;
use burberry::ActionSubmitter;
use eyre::Context;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::warn;

pub struct Arb {
    dex: Arc<DexData>,
    ready_grpc_data_receiver: Receiver<GrpcMessage>,
    swap_sender: Arc<dyn ActionSubmitter<Action>>,
    profit_threshold: u64,
    start_amount_in: u64,
    sol_ata_amount: Arc<Mutex<u64>>,
}

impl Arb {
    pub fn new(
        dex: Arc<DexData>,
        ready_grpc_data_receiver: Receiver<GrpcMessage>,
        swap_sender: Arc<dyn ActionSubmitter<Action>>,
        profit_threshold: u64,
        start_amount_in: u64,
        sol_ata_amount: Arc<Mutex<u64>>,
    ) -> Self {
        Self {
            dex,
            ready_grpc_data_receiver,
            swap_sender,
            profit_threshold,
            start_amount_in,
            sol_ata_amount,
        }
    }

    #[tokio::main]
    pub async fn run(self) -> anyhow::Result<()> {
        loop {
            tokio::select! {
                message = self.ready_grpc_data_receiver.recv() => {
                    let guard = self.sol_ata_amount.lock().await;
                    let sol_ata_amount = guard.clone();
                    drop(guard);
                    if let Some(quote_result) = self.dex.update_cache_and_find_route(
                        message.context("").unwrap(),
                        self.profit_threshold,
                        self.start_amount_in,
                        sol_ata_amount
                    ).await {
                            self.swap_sender.submit(Action::SWAP(quote_result));
                    }
                } else=> warn!("ready_grpc_data_receiver closed")
            }
        }
    }
}
