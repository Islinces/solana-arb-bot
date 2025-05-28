use solana_sdk::pubkey::Pubkey;
use tokio::sync::broadcast;
use tokio::sync::broadcast::{Receiver, Sender};
use tracing::{error, info};
use tracing_appender::non_blocking;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

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
    pub async fn start(
        &self,
        _index: i32,
        tx: Sender<i32>,
        flume_receiver: flume::Receiver<i32>,
    ) {
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

#[test]
fn test(){

    println!("{:?}",Pubkey::from([ 6,
        155,
        136,
        87,
        254,
        171,
        129,
        132,
        251,
        104,
        127,
        99,
        70,
        24,
        192,
        53,
        218,
        196,
        57,
        220,
        26,
        235,
        59,
        85,
        152,
        160,
        240,
        0,
        0,
        0,
        0,
        1,]));
}