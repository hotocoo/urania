//! Signal handling: SIGTERM or SIGINT triggers a graceful shutdown.

use tokio::signal::unix::{SignalKind, signal};

pub async fn wait_terminate() {
    let mut term = signal(SignalKind::terminate()).expect("install SIGTERM handler");
    let mut int = signal(SignalKind::interrupt()).expect("install SIGINT handler");
    tokio::select! {
        _ = term.recv() => {}
        _ = int.recv() => {}
    }
}
