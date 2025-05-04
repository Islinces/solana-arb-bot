use crate::arbitrage::Action;
use crate::dex::DexData;
use crate::interface::GrpcMessage;
use async_channel::Receiver;
use burberry::ActionSubmitter;
use eyre::Context;
use std::sync::Arc;

pub struct Arb {
    dex: Arc<DexData>,
    ready_grpc_data_receiver: Receiver<GrpcMessage>,
    swap_sender: Arc<dyn ActionSubmitter<Action>>,
    profit_threshold: u64,
}

impl Arb {
    pub fn new(
        dex: Arc<DexData>,
        ready_grpc_data_receiver: Receiver<GrpcMessage>,
        swap_sender: Arc<dyn ActionSubmitter<Action>>,
        profit_threshold: u64,
    ) -> Self {
        Self {
            dex,
            ready_grpc_data_receiver,
            swap_sender,
            profit_threshold,
        }
    }

    #[tokio::main]
    pub async fn run(self) -> anyhow::Result<()> {
        loop {
            tokio::select! {
                message = self.ready_grpc_data_receiver.recv() => {
                    if let Some(quote_result) = self.dex.update_cache_and_find_route(message.context("").unwrap(),self.profit_threshold).await{
                        self.swap_sender.submit(Action::SWAP(quote_result));
                    }
                }
            }
        }
    }
}
