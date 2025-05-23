use ahash::{AHashMap, AHashSet};
use burberry::{map_collector, map_executor, Engine};
use chrono::{DateTime, Local};
use clap::Parser;
use moka::sync::{Cache, CacheBuilder};
use router::collector::{CollectorType, SubscribeCollector};
use router::dex_data::{get_dex_data, DexJson};
use router::executor::{ExecutorType, SimpleExecutor};
use router::grpc_processor::MessageProcessor;
use router::grpc_subscribe::GrpcSubscribe;
use router::interface::DexType;
use router::state::{GrpcMessage, TxId};
use router::strategy::MessageStrategy;
use serde::Deserializer;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinSet;
use tracing_appender::non_blocking;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

pub struct MicrosecondFormatter;

impl FormatTime for MicrosecondFormatter {
    fn format_time(&self, w: &mut fmt::format::Writer<'_>) -> std::fmt::Result {
        write!(w, "{}", Local::now().format("%Y-%m-%d %H:%M:%S%.9f"))
    }
}

#[derive(Parser, Debug)]
pub struct Command {
    #[arg(long, required = true)]
    grpc_subscribe_type: String,
    #[arg(long, required = true)]
    dex_json_path: String,
    #[arg(long)]
    grpc_url: Option<String>,
    #[arg(long)]
    mod_value: Option<u64>,
    #[arg(long)]
    start_mode: Option<String>,
    #[arg(long)]
    specify_pool: Option<String>,
    #[arg(long)]
    use_stream_map: Option<bool>,
    #[arg(long)]
    standard_program: Option<bool>,
    #[arg(long)]
    processor_size: Option<usize>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (non_blocking_writer, _guard) = non_blocking(std::io::stdout());
    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_timer(MicrosecondFormatter)
                .with_writer(non_blocking_writer)
                .with_span_events(FmtSpan::NONE),
        )
        .with(EnvFilter::new("info"))
        .init();
    let command = Command::parse();
    let start_mode = command.start_mode.clone().unwrap_or("custom".to_string());
    if start_mode.as_str() == "engine" {
        start_with_engine(command).await;
    } else {
        start_with_custom(command).await;
    }
    Ok(())
}

async fn start_with_custom(command: Command) {
    // grpc消息消费通道，无界
    let (message_sender, message_receiver) = flume::unbounded::<GrpcMessage>();
    let grpc_url = command
        .grpc_url
        .unwrap_or("https://solana-yellowstone-grpc.publicnode.com".to_string());
    let single_mode = if command.grpc_subscribe_type == "single" {
        true
    } else {
        false
    };
    let specify_pool = Arc::new(
        command
            .specify_pool
            .clone()
            .map_or(None, |v| Some(Pubkey::from_str(&v).unwrap())),
    );
    let processor_size = command.processor_size.unwrap_or(num_cpus::get()/2);
    let dex_data = get_dex_data(command.dex_json_path.clone());
    let (_pool_ids, vault_to_pool) = vault_to_pool(&dex_data);
    let vault_to_pool = Arc::new(vault_to_pool);
    let mut join_set = JoinSet::new();
    // 创建processor
    for index in 0..processor_size {
        let vault_to_pool = vault_to_pool.clone();
        let specify_pool = specify_pool.clone();
        let message_receiver = message_receiver.clone();
        join_set.spawn(async move {
            MessageProcessor::new(vault_to_pool, specify_pool, index)
                .start(message_receiver)
                .await
        });
    }
    // 等待所有processor初始化完成
    let _ = join_set.join_all().await;
    // 订阅GRPC
    let subscribe = GrpcSubscribe {
        grpc_url,
        dex_json_path: command.dex_json_path.clone(),
        message_sender: message_sender.clone(),
        single_mode,
        specify_pool: command.specify_pool.clone(),
        use_stream_map: command.use_stream_map.unwrap_or(true),
        standard_program: command.standard_program.unwrap_or(true),
    };
    subscribe.subscribe(dex_data).await;
}

fn vault_to_pool(
    dex_data: &Vec<DexJson>,
) -> (AHashSet<Pubkey>, AHashMap<Pubkey, (Pubkey, Pubkey)>) {
    let mut pool_ids: AHashSet<Pubkey> = AHashSet::with_capacity(dex_data.len());
    let mut vault_to_pool: AHashMap<Pubkey, (Pubkey, Pubkey)> =
        AHashMap::with_capacity(dex_data.len() * 2);
    for json in dex_data.iter() {
        if &json.owner == DexType::PumpFunAMM.get_ref_program_id()
            || &json.owner == DexType::RaydiumAMM.get_ref_program_id()
        {
            pool_ids.insert(json.pool);
            vault_to_pool.insert(json.vault_a, (json.pool, json.owner));
            vault_to_pool.insert(json.vault_b, (json.pool, json.owner));
        }
    }
    (pool_ids, vault_to_pool)
}

async fn start_with_engine(command: Command) {
    let grpc_url = command
        .grpc_url
        .unwrap_or("https://solana-yellowstone-grpc.publicnode.com".to_string());
    let single_mode = if command.grpc_subscribe_type == "single" {
        true
    } else {
        false
    };
    let dex_data = get_dex_data(command.dex_json_path.clone());
    let (pool_ids, vault_to_pool) = vault_to_pool(&dex_data);
    let mut engine = Engine::default();
    engine.add_executor(map_executor!(SimpleExecutor, ExecutorType::Simple));
    engine.add_collector(map_collector!(
        SubscribeCollector {
            grpc_url,
            single_mode,
            dex_json_path: command.dex_json_path.clone(),
            specify_pool: command.specify_pool.clone(),
            dex_data,
            standard_program: command.standard_program.unwrap_or(true),
        },
        CollectorType::Message
    ));
    engine.add_strategy(Box::new(MessageStrategy {
        receiver_msg: AHashMap::with_capacity(10000),
        mod_value: command.mod_value,
        single_mode,
        specify_pool: command
            .specify_pool
            .clone()
            .map_or(None, |v| Some(Pubkey::from_str(&v).unwrap())),
        pool_ids,
        vault_to_pool,
        standard_program: command.standard_program.unwrap_or(true),
    }));

    engine.run_and_join().await.unwrap();
}
