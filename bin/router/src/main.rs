use burberry::{map_collector, map_executor, Engine};
use chrono::{DateTime, Local};
use clap::Parser;
use router::collector::{CollectorType, SubscribeCollector};
use router::executor::{ExecutorType, SimpleExecutor};
use router::grpc_processor::MessageProcessor;
use router::grpc_subscribe::GrpcSubscribe;
use router::strategy::MessageStrategy;
use std::collections::HashMap;
use tokio::time::Instant;
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
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<(
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<String>,
        DateTime<Local>,
        Instant,
    )>();
    let grpc_url = command
        .grpc_url
        .unwrap_or("https://solana-yellowstone-grpc.publicnode.com".to_string());
    let single_mode = if command.grpc_subscribe_type == "single" {
        true
    } else {
        false
    };
    MessageProcessor(single_mode, command.specify_pool.clone())
        .start(receiver, HashMap::with_capacity(10000))
        .await;
    let subscribe = GrpcSubscribe {
        grpc_url,
        dex_json_path: command.dex_json_path.clone(),
        message_sender: sender.clone(),
        single_mode,
        specify_pool: command.specify_pool.clone(),
        use_stream_map: command.use_stream_map.unwrap_or(true),
    };
    subscribe.subscribe().await;
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
    let mut engine = Engine::default();
    engine.add_executor(map_executor!(SimpleExecutor, ExecutorType::Simple));
    engine.add_collector(map_collector!(
        SubscribeCollector {
            grpc_url,
            single_mode,
            dex_json_path: command.dex_json_path.clone(),
            specify_pool: command.specify_pool.clone()
        },
        CollectorType::Message
    ));
    engine.add_strategy(Box::new(MessageStrategy {
        receiver_msg: HashMap::default(),
        mod_value: command.mod_value,
        single_mode,
        specify_pool: command.specify_pool.clone(),
    }));

    engine.run_and_join().await.unwrap();
}
