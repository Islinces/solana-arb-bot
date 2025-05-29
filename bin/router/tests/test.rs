use router::state::{GrpcAccountMsg, GrpcMessage, GrpcTransactionMsg};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::time::Duration;
use base58::ToBase58;
use tokio::sync::broadcast;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio_stream::StreamExt;
use tracing::{error, info};
use tracing_appender::non_blocking;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient, Interceptor};
use yellowstone_grpc_proto::geyser::subscribe_update::UpdateOneof;
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestFilterAccounts,
};

#[tokio::test]
async fn main() {
    let (non_blocking_writer, _guard) = non_blocking(std::io::stdout());
    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_writer(non_blocking_writer)
                .with_span_events(FmtSpan::NONE),
        )
        .with(EnvFilter::new("info"))
        .init();
    let (sender, _) = broadcast::channel(1000);

    // 多个消费者（比如策略模块）
    for i in 0..3 {
        let rx = sender.subscribe();
        Test1.start(i, rx).await;
        // tokio::spawn(async move {
        //     while let Ok(update) = rx.recv().await {
        //         info!("Strategy {} got update from pool {:?}", i, update);
        //     }
        // });
    }

    let (flume_sender, flume_receiver) = flume::unbounded();
    tokio::spawn(async move {
        loop {
            let _ = flume_sender.send(1);
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    });
    // 多个生产者模拟并发更新
    for x in 0..2 {
        let tx1 = sender.clone();
        let receiver = flume_receiver.clone();
        Test2.start(x, tx1, receiver).await;
    }
    // 模拟运行
    tokio::signal::ctrl_c().await.unwrap();
}

struct Test1;

impl Test1 {
    pub async fn start(&self, index: usize, mut rx: Receiver<i32>) {
        tokio::spawn(async move {
            while let Ok(update) = rx.recv().await {
                info!("Strategy {} got update from pool {:?}", index, update);
            }
        });
    }
}

struct Test2;

impl Test2 {
    pub async fn start(&self, _index: i32, tx: Sender<i32>, flume_receiver: flume::Receiver<i32>) {
        tokio::spawn(async move {
            loop {
                match flume_receiver.recv_async().await {
                    Ok(value) => {
                        let _ = tx.send(value);
                    }
                    Err(e) => {
                        error!("{:?}", e)
                    }
                }
            }
        });
    }
}

#[tokio::test]
async fn test() {
    let (non_blocking_writer, _guard) = non_blocking(std::io::stdout());
    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_writer(non_blocking_writer)
                .with_span_events(FmtSpan::NONE),
        )
        .with(EnvFilter::new("info"))
        .init();
    let mut grpc_client =
        create_grpc_client("https://solana-yellowstone-grpc.publicnode.com".to_string()).await;
    let mut accounts = HashMap::new();
    // 所有池子、金库订阅
    accounts.insert(
        "accounts".to_string(),
        SubscribeRequestFilterAccounts {
            account: vec!["Gf7sXMoP8iRw4iiXmJ1nq4vxcRycbGXy5RL8a8LnTd3v".to_string()],
            ..Default::default()
        },
    );
    let subscribe_request = SubscribeRequest {
        accounts,
        commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
        ..Default::default()
    };
    let (_, mut stream) = grpc_client
        .subscribe_with_request(Some(subscribe_request))
        .await
        .unwrap();
    while let Some(message) = stream.next().await {
        match message {
            Ok(data) => {
                if let Some(UpdateOneof::Account(account)) = data.update_oneof {
                    info!("{:?}",account.account.unwrap().txn_signature.unwrap().as_slice().to_base58());
                } else if let Some(UpdateOneof::Transaction(transaction)) = data.update_oneof {
                    match transaction.transaction {
                        None => {}
                        Some(tx) => {}
                    }
                }
            }
            Err(e) => {
                error!("grpc推送消息失败，原因：{}", e)
            }
        }
    }
}

async fn create_grpc_client(grpc_url: String) -> GeyserGrpcClient<impl Interceptor + Sized> {
    let use_tls = grpc_url.starts_with("https://");
    let mut builder = GeyserGrpcClient::build_from_shared(grpc_url).unwrap();
    if use_tls {
        builder = builder
            .tls_config(ClientTlsConfig::new().with_native_roots())
            .unwrap();
    }
    builder
        .max_decoding_message_size(100 * 1024 * 1024) // 100MB
        .connect_timeout(Duration::from_secs(10))
        .buffer_size(64 * 1024) // 64KB buffer
        .http2_adaptive_window(true)
        .http2_keep_alive_interval(Duration::from_secs(15))
        .initial_connection_window_size(2 * 1024 * 1024) // 2MB
        .initial_stream_window_size(2 * 1024 * 1024) // 2MB
        .keep_alive_timeout(Duration::from_secs(30))
        .keep_alive_while_idle(true)
        .tcp_keepalive(Some(Duration::from_secs(30)))
        .tcp_nodelay(true)
        .timeout(Duration::from_secs(10))
        .connect()
        .await
        .map_err(|e| {
            error!("GRPC订阅: 连接GRPC服务器失败，原因: {e}");
            anyhow::anyhow!(e)
        })
        .unwrap()
}
