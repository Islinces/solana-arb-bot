use crate::arb::Arb;
use crate::dex_data::DexJson;
use crate::executor::jito::JitoExecutor;
use crate::executor::Executor;
use crate::global_cache::{get_global_cache, init_global_cache, GlobalCache};
use crate::grpc_processor::MessageProcessor;
use crate::grpc_subscribe::{GrpcMessage, GrpcSubscribe, GrpcTransactionMsg};
use crate::keypair::KeypairVault;
use anyhow::anyhow;
use clap::Parser;
use rpassword::read_password;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use std::fs::File;
use std::path::PathBuf;
use std::process::exit;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::task::JoinSet;
use tokio::time::Instant;
use tracing::{error, info};

#[derive(Parser, Debug)]
pub struct Command {
    #[arg(long, required = true)]
    dex_json_path: String,
    #[arg(long, required = true)]
    keypair_path: String,
    #[arg(long, default_value = "https://solana-yellowstone-grpc.publicnode.com")]
    grpc_url: String,
    #[arg(long, default_value = "https://solana-rpc.publicnode.com")]
    rpc_url: String,
    #[arg(long, default_value = "mainnet")]
    pub jito_region: String,
    #[arg(long,num_args = 1..)]
    pub jito_uuid: Option<Vec<String>>,
    #[arg(long, default_values = ["So11111111111111111111111111111111111111112"])]
    follow_mints: Vec<Pubkey>,
    #[arg(long)]
    pub arb_bot_name: Option<String>,
    #[arg(long, required = true)]
    arb_amount_in: u64,
    #[arg(long, default_value = "1")]
    arb_size: usize,
    #[arg(long, default_value = "So11111111111111111111111111111111111111112")]
    arb_mint: Pubkey,
    #[arg(long, default_value = "70")]
    arb_mint_bps_numerator: u64,
    #[arg(long, default_value = "100")]
    arb_mint_bps_denominator: u64,
    #[arg(long, default_value = "1000")]
    arb_channel_capacity: usize,
    #[arg(long, default_value = "100000")]
    arb_min_profit: u64,
    #[arg(long, default_value = "70")]
    pub tip_bps_numerator: u64,
    #[arg(long, default_value = "100")]
    pub tip_bps_denominator: u64,
    #[arg(long, default_value = "false")]
    standard_program: bool,
    #[arg(long, default_value = "1")]
    processor_size: usize,
}

pub async fn start_with_custom() -> anyhow::Result<()> {
    let command = Command::parse();
    info!("{:#?}", command);
    let grpc_url = command.grpc_url.clone();
    let rpc_url = command.rpc_url.clone();
    let arb_mint = command.arb_mint.clone();
    let follow_mints = command.follow_mints.clone();
    let dex_json_path = command.dex_json_path.clone();
    let keypair_path = command.keypair_path.clone();
    let processor_size = command.processor_size;
    let arb_size = command.arb_size;
    let arb_amount_in = command.arb_amount_in;
    let arb_mint_bps_numerator = command.arb_mint_bps_numerator;
    let arb_mint_bps_denominator = command.arb_mint_bps_denominator;
    let arb_min_profit = command.arb_min_profit;
    // Account本地缓存更新后广播通道容量
    let arb_channel_capacity = command.arb_channel_capacity;
    let rpc_client = Arc::new(RpcClient::new(rpc_url));
    // 0.初始化钱包，ata账户，blockhash
    // 1.初始化各个Account的切片规则
    // 2.初始化snapshot，返回有效的DexJson(所有数据都合法的)
    // 3.初始化池子与DexType关系、金库与池子&DexType的关系，用于解析GRPC推送数据使用
    // 4.构建边
    let dex_data = init_start_data(
        keypair_path,
        dex_json_path,
        &arb_mint,
        follow_mints.as_slice(),
        rpc_client,
    )
    .await?;
    // grpc消息消费通道
    let (grpc_message_sender, grpc_message_receiver) = flume::unbounded::<GrpcMessage>();
    // Account本地缓存更新后广播通道
    let (cached_message_sender, cached_message_receiver) =
        flume::bounded::<GrpcTransactionMsg>(arb_channel_capacity);
    // 接收发生改变的缓存数据，判断是否需要触发route
    let mut join_set = JoinSet::new();
    // 将GRPC通过过来的数据保存到本地缓存中
    // 缓存数据发生改变，将数据发送出来
    MessageProcessor::new(processor_size)
        .start(
            &mut join_set,
            &grpc_message_receiver,
            cached_message_sender,
            cached_message_receiver.clone(),
        )
        .await;
    // 接收更新缓存的Account信息，判断是否需要触发route
    Arb::new(
        arb_size,
        arb_amount_in,
        arb_min_profit,
        arb_mint,
        arb_mint_bps_numerator,
        arb_mint_bps_denominator,
        JitoExecutor::initialize(&command)?,
    )
    .start(&mut join_set, cached_message_receiver)
    .await;
    join_set.spawn(async move {
        // 订阅GRPC
        GrpcSubscribe
            .subscribe(grpc_url, dex_data, grpc_message_sender)
            .await;
    });
    while let Some(event) = join_set.join_next().await {
        if let Err(err) = event {
            error!("task terminated unexpectedly: {err:#}");
            // 退出程序
            exit(-1);
        }
    }
    Ok(())
}

pub async fn init_start_data(
    keypair_path: String,
    dex_json_path: String,
    arb_mint: &Pubkey,
    follow_mints: &[Pubkey],
    rpc_client: Arc<RpcClient>,
) -> anyhow::Result<Vec<DexJson>> {
    // 1.初始化钱包
    let keypair = crate::keypair::get_keypair(keypair_path)?;
    // 2.加载DexJson
    let mut dex_data = crate::dex_data::load_dex_json(dex_json_path, follow_mints)?;
    // 3.各个Dex的Account切片规则(需要订阅的，不需要订阅的)
    crate::interface::init_data_slice_config()?;
    // 4.初始化全局缓存，未填充数据
    init_global_cache();
    // 5.初始化Snapshot，填充全局缓存，移除无效DexJson
    crate::interface::init_snapshot(&mut dex_data, rpc_client.clone(), get_global_cache()).await?;
    // 初始化钱包关联的ATA账户余额
    // 初始化blockhash
    crate::metadata::init_metadata(keypair, arb_mint, dex_data.as_slice(), rpc_client.clone())
        .await?;
    // 初始化account之间的关系，用于解析GRPC推送数据
    crate::account_relation::init(dex_data.as_slice())?;
    // 构建边
    crate::graph::init_graph(dex_data.as_slice(), follow_mints)?;
    Ok(dex_data)
}
