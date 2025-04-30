use crate::interface::GrpcMessage;
use crate::dex::Defi;
use crate::arbitrage::Action;
use async_channel::Receiver;
use burberry::ActionSubmitter;
use eyre::Context;
use solana_program::pubkey::Pubkey;
use std::sync::Arc;
use tracing::info;

pub struct Arb {
    defi: Arc<Defi>,
    ready_grpc_data_receiver: Receiver<GrpcMessage>,
    swap_sender: Arc<dyn ActionSubmitter<Action>>,
}

impl Arb {
    pub fn new(
        defi: Arc<Defi>,
        ready_grpc_data_receiver: Receiver<GrpcMessage>,
        swap_sender: Arc<dyn ActionSubmitter<Action>>,
    ) -> Self {
        Self {
            defi,
            ready_grpc_data_receiver,
            swap_sender,
        }
    }

    #[tokio::main]
    pub async fn run(self) -> anyhow::Result<()> {
        loop {
            tokio::select! {
                message = self.ready_grpc_data_receiver.recv() => {
                    if let Some(paths) = self.defi.update_cache_and_find_route(message.context("").unwrap()).await{
                        info!("找到路由：{:?}",paths);
                        //TODO:触发交易
                        self.swap_sender.submit(Action::SWAP(Pubkey::default()));
                    }
                }else => {
                }
            }
        }
    }
}
