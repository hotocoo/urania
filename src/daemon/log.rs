//! Logging: rolling file for the daemon, stderr for interactive commands.

use std::path::Path;

use tracing_appender::rolling;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Rolling log in `dir`: urania-YYYY-MM-DD, keep 2 files.
/// (tracing-appender 0.2 has no size-based rotation; at info level the daily
/// files stay far below the 5 MB target — see REBRAND.md deviations.)
pub fn init_file(dir: &Path) -> anyhow::Result<tracing_appender::non_blocking::WorkerGuard> {
    std::fs::create_dir_all(dir)?;
    let appender = rolling::RollingFileAppender::builder()
        .rotation(rolling::Rotation::DAILY)
        .filename_prefix("urania")
        .max_log_files(2)
        .build(dir)
        .map_err(|e| anyhow::anyhow!("log init failed: {e:?}"))?;
    let (non_blocking, guard) = tracing_appender::non_blocking(appender);
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false),
        )
        .init();
    Ok(guard)
}

pub fn init_stderr() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();
}
