use burberry::{map_collector, map_executor, Engine};
use chrono::Local;
use clap::Parser;
use router::collector::{CollectorType, MultiSubscribeCollector, SingleSubscribeCollector};
use router::executor::{ExecutorType, SimpleExecutor};
use router::strategy::{MultiStrategy, SingleStrategy};
use std::collections::HashMap;
use tokio::sync::broadcast;
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
    let grpc_url = command
        .grpc_url
        .unwrap_or("https://solana-yellowstone-grpc.publicnode.com".to_string());
    let mut engine = Engine::default();
    engine.add_executor(map_executor!(SimpleExecutor, ExecutorType::Simple));
    if command.grpc_subscribe_type == "single" {
        engine.add_collector(map_collector!(
            SingleSubscribeCollector(command.dex_json_path, grpc_url),
            CollectorType::Single
        ));
        engine.add_strategy(Box::new(SingleStrategy {
            receiver_msg: HashMap::default(),
            mod_value: command.mod_value,
        }));
    } else {
        engine.add_collector(map_collector!(
            MultiSubscribeCollector(command.dex_json_path, grpc_url),
            CollectorType::Multiple
        ));
        engine.add_strategy(Box::new(MultiStrategy {
            receiver_msg: HashMap::default(),
            mod_value: command.mod_value,
        }));
    }
    engine.run_and_join().await.unwrap();
    Ok(())
}
