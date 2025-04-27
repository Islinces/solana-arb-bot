use crate::defi;
use crate::defi::types::SourceMessage;
use crate::strategy::grpc_message_collector::GrpcMessageCollector;
use crate::strategy::arb_executor::GrpcMessageExecutor;
use crate::strategy::grpc_subscribe_strategy::GrpcSubscribeStrategy;
use crate::strategy::Action;
use burberry::{map_collector, map_executor, Engine};
use solana_program::pubkey::Pubkey;
use std::str::FromStr;

pub async fn run() -> anyhow::Result<()> {
    let grpc_url = "https://solana-yellowstone-grpc.publicnode.com";
    let rpc_url = "https://solana-rpc.publicnode.com";
    let mints = vec![
        Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
        Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
    ];
    let mut engine = Engine::default();
    let message_collector =
        GrpcMessageCollector::new(rpc_url.to_string(), mints.clone(), grpc_url, 10);
    let message_executor = GrpcMessageExecutor::new();
    engine.add_collector(map_collector!(message_collector, SourceMessage::Account));
    engine.add_executor(map_executor!(message_executor, Action::SWAP));
    engine.add_strategy(Box::new(GrpcSubscribeStrategy::new(
        rpc_url.to_string(),
        mints,
        defi::supported_protocols(),
        None,
        None,
        5,
    )));
    engine.run_and_join().await.unwrap();
    Ok(())
}
