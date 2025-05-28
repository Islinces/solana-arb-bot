use crate::arb::Arb;
use crate::dex_data::DexJson;
use crate::executor::jito::JitoExecutor;
use crate::executor::Executor;
use crate::grpc_processor::MessageProcessor;
use crate::grpc_subscribe::GrpcSubscribe;
use crate::state::GrpcMessage;
use anyhow::anyhow;
use clap::Parser;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::fs::File;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::task::JoinSet;
use tracing::{error, info};

#[derive(Parser, Debug)]
pub struct Command {
    #[arg(long, required = true)]
    grpc_subscribe_type: String,
    #[arg(long, required = true)]
    dex_json_path: String,
    #[arg(long, required = true)]
    keypair_path: String,
    #[arg(long)]
    grpc_url: Option<String>,
    #[arg(long)]
    rpc_url: Option<String>,
    #[arg(long)]
    pub jito_region: Option<String>,
    #[arg(long)]
    pub jito_uuid: Option<String>,
    #[arg(long)]
    follow_mints: Option<Vec<String>>,
    #[arg(long)]
    pub arb_bot_name: Option<String>,
    #[arg(long)]
    arb_size: Option<usize>,
    #[arg(long)]
    arb_channel_capacity: Option<usize>,
    #[arg(long)]
    arb_min_profit: Option<u64>,
    #[arg(long)]
    pub tip_bps_numerator: Option<u64>,
    #[arg(long)]
    pub tip_bps_denominator: Option<u64>,
    #[arg(long)]
    standard_program: Option<bool>,
    #[arg(long)]
    processor_size: Option<usize>,
    #[arg(long)]
    specify_pool: Option<String>,
}

pub async fn start_with_custom() -> anyhow::Result<()> {
    let command = Command::parse();
    let follow_mints =
        command
            .follow_mints
            .clone()
            .map_or(vec![spl_token::native_mint::ID], |mints| {
                mints
                    .into_iter()
                    .map(|v| Pubkey::from_str(&v).unwrap())
                    .collect()
            });
    let grpc_url = command
        .grpc_url
        .clone()
        .unwrap_or("https://solana-yellowstone-grpc.publicnode.com".to_string());
    let rpc_url = command
        .rpc_url
        .clone()
        .unwrap_or("https://solana-rpc.publicnode.com".to_string());
    let dex_json_path = command.dex_json_path.clone();
    let keypair_path = command.keypair_path.clone();
    let processor_size = command.processor_size.unwrap_or(1);
    let arb_size = command.arb_size.unwrap_or(1);
    let arb_min_profit = command.arb_min_profit.unwrap_or(100_000);
    // Account本地缓存更新后广播通道容量
    let arb_channel_capacity = command.arb_channel_capacity.unwrap_or(10_000);
    let single_mode = if command.grpc_subscribe_type == "single" {
        true
    } else {
        false
    };
    let executor = JitoExecutor::initialize(&command)?;
    let rpc_client = Arc::new(RpcClient::new(rpc_url));
    // 0.初始化钱包，ata账户，blockhash
    // 1.初始化各个Account的切片规则
    // 2.初始化snapshot，返回有效的DexJson(所有数据都合法的)
    // 3.初始化池子与DexType关系、金库与池子&DexType的关系，用于解析GRPC推送数据使用
    // 4.构建边
    let dex_data = init_start_data(
        keypair_path,
        dex_json_path,
        follow_mints.as_slice(),
        rpc_client,
    )
    .await?;
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
    Arb::new(arb_size, arb_min_profit, executor)
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
    Ok(())
}

pub async fn init_start_data(
    keypair_path: String,
    dex_json_path: String,
    follow_mints: &[Pubkey],
    rpc_client: Arc<RpcClient>,
) -> anyhow::Result<Vec<DexJson>> {
    // 初始化钱包
    // 初始化钱包关联的ATA账户余额
    // 初始化blockhash
    crate::metadata::init_metadata(keypair_path, rpc_client.clone()).await?;
    let mut dex_data: Vec<DexJson> = match File::open(dex_json_path.as_str()) {
        Ok(file) => serde_json::from_reader(file).expect("解析【dex_data.json】失败"),
        Err(e) => {
            error!("{}", e);
            vec![]
        }
    };
    if dex_data.is_empty() {
        Err(anyhow!("json文件无数据"))
    } else {
        // 删除不涉及关注的Mint的池子
        dex_data.retain(|v| follow_mints.contains(&v.mint_a) || follow_mints.contains(&v.mint_b));
        if dex_data.is_empty() {
            return Err(anyhow!(
                "json文件中无涉及程序关注的Mint的池子，程序关注的Mint : {:?}",
                follow_mints
            ));
        }
        // 各个Dex的Account切片规则(需要订阅的，不需要订阅的)
        crate::data_slice::init_data_slice_config();
        // 初始化snapshot，返回有效的DexJson
        let dex_data = crate::account_cache::init_snaphot(dex_data, rpc_client).await?;
        // 初始化account之间的关系，用于解析GRPC推送数据
        crate::account_relation::init(dex_data.as_slice())?;
        // 构建边
        crate::graph::init_graph(dex_data.as_slice(), follow_mints)?;
        Ok(dex_data)
    }
}
