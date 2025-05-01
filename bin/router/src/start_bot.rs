use crate::arbitrage::arb_executor::GrpcMessageExecutor;
use crate::arbitrage::arb_strategy::ArbStrategy;
use crate::arbitrage::message_collector::GrpcMessageCollector;
use crate::arbitrage::Action;
use crate::dex;
use crate::interface::SourceMessage;
use burberry::{map_collector, map_executor, Engine};

pub async fn run() -> anyhow::Result<()> {
    let grpc_url = "https://solana-yellowstone-grpc.publicnode.com";
    let rpc_url = "https://solana-rpc.publicnode.com";
    let mut engine = Engine::default();
    let message_collector =
        GrpcMessageCollector::new(rpc_url.to_string(), grpc_url);
    let message_executor = GrpcMessageExecutor::new();
    engine.add_collector(map_collector!(message_collector, SourceMessage::Account));
    engine.add_executor(map_executor!(message_executor, Action::SWAP));
    engine.add_strategy(Box::new(ArbStrategy::new(
        rpc_url.to_string(),
        dex::supported_protocols(),
        None,
        None,
        5,
    )));
    engine.run_and_join().await.unwrap();
    Ok(())
}
