use chrono::Local;
use router::start_bot;
use std::env;
use tracing::info;
use tracing_appender::non_blocking;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

pub struct MicrosecondFormatter;

impl FormatTime for MicrosecondFormatter {
    fn format_time(&self, w: &mut fmt::format::Writer<'_>) -> std::fmt::Result {
        write!(w, "{}", Local::now().format("%Y-%m-%d %H:%M:%S%.6f"))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::new("info");
    // let file_appender = RollingFileAppender::builder()
    //     .filename_prefix("app")
    //     .filename_suffix("log")
    //     .rotation(Rotation::DAILY)
    //     .build("./logs")
    //     .expect("构建file_appender失败");
    // let (non_blocking_writer, _guard) = non_blocking(file_appender);
    let (non_blocking_writer, _guard) = non_blocking(std::io::stdout());
    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_timer(MicrosecondFormatter)
                .with_writer(non_blocking_writer)
                .with_span_events(FmtSpan::NONE),
        )
        .with(filter)
        .init();
    info!("arb-bot开始启动");
    start_bot::run().await?;
    Ok(())
}
