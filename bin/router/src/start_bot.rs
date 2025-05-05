use crate::arbitrage::arb_strategy::ArbStrategy;
use crate::arbitrage::jito_arb_executor::{JitoArbExecutor, JitoConfig};
use crate::arbitrage::message_collector::GrpcMessageCollector;
use crate::arbitrage::Action;
use crate::interface::SourceMessage;
use burberry::{map_collector, map_executor, Engine};
use dashmap::DashMap;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::hash::Hash;
use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use std::ops::Mul;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{error, info};

pub enum ExecutorType {
    JITO(JitoConfig),
}

pub async fn run() -> anyhow::Result<()> {
    let grpc_url = "https://solana-yellowstone-grpc.publicnode.com";
    let rpc_url = "https://solana-rpc.publicnode.com";
    let mut engine = Engine::default();
    let rpc_client = Arc::new(RpcClient::new(rpc_url.to_string()));
    let message_collector = GrpcMessageCollector::new(rpc_client.clone(), grpc_url);
    let initial_blockhash = rpc_client.get_latest_blockhash().await?;
    let cached_blockhash = Arc::new(Mutex::new(initial_blockhash));

    let refresh_interval = Duration::from_secs(10);
    let blockhash_client = rpc_client.clone();
    let blockhash_cache = cached_blockhash.clone();
    tokio::spawn(async move {
        blockhash_refresher(blockhash_client, blockhash_cache, refresh_interval).await;
    });
    // TODO：钱包传递方式
    let key_pair = Keypair::new();
    let bot_name = Some(String::from("arb-bot"));
    let arb_worker_size = 5;
    let max_amount_in_numerator = 9;
    let max_amount_in_denominator = 10;
    let profit_threshold = 100_000;
    let native_ata_amount = Arc::new(100_u64.mul(10_u64.pow(9)));
    let executor_type = ExecutorType::JITO(JitoConfig {
        jito_regin: "mainnet".to_string(),
        jito_uuid: None,
    });
    // 钱包已有的mint的ata账户
    let mut mint_ata: Arc<DashMap<Pubkey, Pubkey>> = Arc::new(DashMap::new());
    // TODO 随便写的，测试用的
    mint_ata.insert(
        Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
        Pubkey::new_unique(),
    );
    let arb_executor = match executor_type {
        ExecutorType::JITO(jito_config) => JitoArbExecutor::new(
            bot_name,
            key_pair,
            mint_ata.clone(),
            rpc_client.clone(),
            cached_blockhash.clone(),
            jito_config,
        ),
    };
    engine.add_collector(map_collector!(message_collector, SourceMessage::Account));
    engine.add_executor(map_executor!(arb_executor, Action::SWAP));
    engine.add_strategy(Box::new(ArbStrategy::new(
        rpc_client.clone(),
        arb_worker_size,
        native_ata_amount,
        max_amount_in_numerator,
        max_amount_in_denominator,
        profit_threshold,
        None,
        None,
    )));
    engine.run_and_join().await.unwrap();
    Ok(())
}

async fn blockhash_refresher(
    rpc_client: Arc<RpcClient>,
    cached_blockhash: Arc<Mutex<Hash>>,
    refresh_interval: Duration,
) {
    loop {
        match rpc_client.get_latest_blockhash().await {
            Ok(blockhash) => {
                let mut guard = cached_blockhash.lock().await;
                *guard = blockhash;
                // info!("Blockhash refreshed: {}", blockhash);
            }
            Err(e) => {
                error!("Failed to refresh blockhash: {:?}", e);
            }
        }
        tokio::time::sleep(refresh_interval).await;
    }
}
