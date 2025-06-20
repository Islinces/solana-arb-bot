use arb::arb_bot;
use chrono::Local;
use mimalloc::MiMalloc;
use rayon::ThreadPoolBuilder;
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ThreadPoolBuilder::new()
    //     .num_threads(num_cpus::get() / 4)
    //     .build_global()?;
    let (non_blocking_writer, _guard) = {
        #[cfg(feature = "log_file")]
        {
            let file_appender = tracing_appender::rolling::RollingFileAppender::builder()
                .filename_prefix("app")
                .filename_suffix("log")
                .rotation(tracing_appender::rolling::Rotation::DAILY)
                .build("./logs")
                .expect("构建 file_appender 失败");
            non_blocking(file_appender)
        }
        #[cfg(not(feature = "log_file"))]
        {
            non_blocking(std::io::stdout())
        }
    };
    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_timer(MicrosecondFormatter)
                .with_writer(non_blocking_writer)
                .with_span_events(FmtSpan::NONE),
        )
        .with(EnvFilter::new("info"))
        .init();
    arb_bot::start_with_custom().await?;
    Ok(())
}
