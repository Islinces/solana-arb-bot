use ahash::{AHashMap, AHashSet};
use chrono::Local;
use clap::Parser;
use mimalloc::MiMalloc;
use router::arb::Arb;
use router::dex_data::{get_dex_data, DexJson};
use router::grpc_processor::MessageProcessor;
use router::grpc_subscribe::GrpcSubscribe;
use router::interface::DexType;
use router::state::GrpcMessage;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use tokio::sync::broadcast;
use tokio::task::JoinSet;
use tracing::error;
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

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[derive(Parser, Debug)]
pub struct Command {
    #[arg(long, required = true)]
    grpc_subscribe_type: String,
    #[arg(long, required = true)]
    dex_json_path: String,
    #[arg(long)]
    grpc_url: Option<String>,
    // #[arg(long)]
    // start_mode: Option<String>,
    #[arg(long)]
    specify_pool: Option<String>,
    #[arg(long)]
    standard_program: Option<bool>,
    #[arg(long)]
    processor_size: Option<usize>,
    #[arg(long)]
    arb_size: Option<usize>,
    #[arg(long)]
    arb_channel_capacity: Option<usize>,
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
    // let start_mode = command.start_mode.clone().unwrap_or("custom".to_string());
    // if start_mode.as_str() == "engine" {
    //     start_with_engine(command).await;
    // } else {
    //
    // }
    start_with_custom(command).await;
    Ok(())
}

async fn start_with_custom(command: Command) {
    let grpc_url = command
        .grpc_url
        .unwrap_or("https://solana-yellowstone-grpc.publicnode.com".to_string());
    let processor_size = command.processor_size.unwrap_or(num_cpus::get() / 4);
    let arb_size = command.arb_size.unwrap_or(1);
    // Account本地缓存更新后广播通道容量
    let arb_channel_capacity = command.arb_channel_capacity.unwrap_or(10_000);
    let single_mode = if command.grpc_subscribe_type == "single" {
        true
    } else {
        false
    };
    let specify_pool = command
        .specify_pool
        .clone()
        .map_or(None, |v| Some(Pubkey::from_str(&v).unwrap()));
    let dex_data = get_dex_data(command.dex_json_path.clone());
    let (_pool_ids, vault_to_pool) = vault_to_pool(&dex_data);
    // grpc消息消费通道
    let (grpc_message_sender, grpc_message_receiver) = flume::unbounded::<GrpcMessage>();
    // Account本地缓存更新后广播通道
    let (cached_message_sender, _) = broadcast::channel(arb_channel_capacity);
    // 接收发生改变的缓存数据，判断是否需要触发route
    let mut join_set = JoinSet::new();
    // 将GRPC通过过来的数据保存到本地缓存中
    // 缓存数据发生改变，将数据发送出来
    MessageProcessor::new(processor_size)
        .start(
            &mut join_set,
            &grpc_message_receiver,
            &cached_message_sender,
        )
        .await;
    // 接收更新缓存的Account信息，判断是否需要触发route
    Arb::new(arb_size, vault_to_pool, specify_pool)
        .start(&mut join_set, &cached_message_sender)
        .await;
    join_set.spawn(async move {
        // 订阅GRPC
        let subscribe = GrpcSubscribe {
            grpc_url,
            dex_json_path: command.dex_json_path.clone(),
            single_mode,
            specify_pool: command.specify_pool.clone(),
            standard_program: command.standard_program.unwrap_or(true),
        };
        subscribe.subscribe(dex_data, grpc_message_sender).await
    });
    while let Some(event) = join_set.join_next().await {
        if let Err(err) = event {
            error!("task terminated unexpectedly: {err:#}");
        }
    }
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