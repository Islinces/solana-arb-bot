use crate::arb::Arb;
use crate::grpc_processor::MessageProcessor;
use crate::grpc_subscribe::GrpcSubscribe;
use crate::interface::init_dex_data;
use crate::state::GrpcMessage;
use clap::Parser;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use tokio::sync::broadcast;
use tokio::task::JoinSet;
use tracing::error;


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

pub async fn start_with_custom() {
    let command = Command::parse();
    let grpc_url = command
        .grpc_url
        .unwrap_or("https://solana-yellowstone-grpc.publicnode.com".to_string());
    let dex_json_path = command.dex_json_path;
    let processor_size = command.processor_size.unwrap_or(1);
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
    let dex_data = init_dex_data(dex_json_path).unwrap();
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
    Arb::new(arb_size, specify_pool)
        .start(&mut join_set, &cached_message_sender)
        .await;
    join_set.spawn(async move {
        // 订阅GRPC
        let subscribe = GrpcSubscribe {
            grpc_url,
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