use flume::{RecvError, TrySendError};
use std::time::Duration;
use tokio::io::join;
use tokio::task::JoinSet;
use tokio::time::{sleep, Instant};
use tracing::info;
use tracing_appender::non_blocking;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

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
    let (sender, receiver) = flume::bounded::<usize>(1);
    let mut join_set = JoinSet::new();
    let drop_receiver = receiver.clone();
    join_set.spawn(async move {
        let mut start_index = 0;
        loop {
            start_index += 1;
            info!("try_send : {}", start_index);
            match sender.try_send(start_index) {
                Ok(_) => {}
                Err(TrySendError::Full(size)) => {
                    // info!("drop before : {:?}", drop_receiver.iter().collect::<Vec<_>>());
                    drop_receiver.try_recv().ok();
                    // info!("drop after : {:?}", drop_receiver.iter().collect::<Vec<_>>());
                    info!("drop {}",size);
                }
                _ => {}
            }
            sleep(Duration::from_secs(1)).await;
        }
    });
    join_set.spawn(async move {
        loop {
            match receiver.recv_async().await {
                Ok(value) => {
                    info!("recv value : {}", value);
                    sleep(Duration::from_secs(2)).await;
                }
                Err(_) => {}
            }
        }
    });
    join_set.join_all().await;
}
