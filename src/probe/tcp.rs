//! TCP connect probe with optional port-list fallback (gateway 443 → 80, T3).

use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::config::Target;
use crate::probe::{Probe, ProbeResult, result};

pub struct TcpProbe {
    pub target: Target,
    /// Candidate (host, port) pairs; the first success wins.
    pub candidates: Vec<(String, u16)>,
}

impl TcpProbe {
    pub fn new(target: Target, candidates: Vec<(String, u16)>) -> Self {
        TcpProbe { target, candidates }
    }
}

#[async_trait]
impl Probe for TcpProbe {
    fn target(&self) -> &Target {
        &self.target
    }

    async fn run(&self) -> ProbeResult {
        let dur = Duration::from_secs(self.target.timeout);
        let fallback = self.candidates.len() > 1;
        let mut last_detail = "no-candidates".to_string();
        for (host, port) in &self.candidates {
            let start = Instant::now();
            match timeout(dur, TcpStream::connect((host.as_str(), *port))).await {
                Ok(Ok(_stream)) => {
                    let latency = start.elapsed().as_secs_f64() * 1000.0;
                    // Record which port responded (the evidence states what was measured).
                    let detail = if fallback {
                        format!("port={port}")
                    } else {
                        String::new()
                    };
                    return result(&self.target, true, Some(latency), detail);
                }
                Ok(Err(e)) => {
                    last_detail = format!("{}@{}:{}", classify_io(&e), host, port);
                }
                Err(_) => {
                    last_detail = format!("timeout@{}:{}", host, port);
                }
            }
        }
        result(&self.target, false, None, last_detail)
    }
}

fn classify_io(e: &std::io::Error) -> &'static str {
    match e.kind() {
        std::io::ErrorKind::ConnectionRefused => "refused",
        std::io::ErrorKind::NetworkUnreachable | std::io::ErrorKind::HostUnreachable => {
            "unreachable"
        }
        std::io::ErrorKind::TimedOut => "timeout",
        _ => "connect",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProbeType;
    use tokio::io::AsyncReadExt;

    fn target(endpoint: &str) -> Target {
        Target {
            name: "t".into(),
            probe_type: ProbeType::Tcp,
            endpoint: endpoint.into(),
            interval: 1,
            timeout: 2,
        }
    }

    #[tokio::test]
    async fn tcp_ok_and_refused() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let _ = s.read(&mut [0u8; 1]).await; // hold the connection briefly
        });
        let p = TcpProbe::new(
            target(&addr.to_string()),
            vec![(addr.ip().to_string(), addr.port())],
        );
        let r = p.run().await;
        assert!(r.ok, "expected ok, got detail={}", r.detail);
        assert!(r.latency_ms.is_some());
        assert_eq!(r.detail, "");
        handle.abort();

        // Nothing listens on loopback port 1 → refused.
        let p2 = TcpProbe::new(target("127.0.0.1:1"), vec![("127.0.0.1".into(), 1)]);
        let r2 = p2.run().await;
        assert!(!r2.ok);
        assert!(r2.detail.starts_with("refused"), "got: {}", r2.detail);
    }

    #[tokio::test]
    async fn tcp_fallback_port() {
        // First candidate refused, second ok → success with port= detail.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let _ = s.read(&mut [0u8; 1]).await;
        });
        let p = TcpProbe::new(
            target(&addr.to_string()),
            vec![
                ("127.0.0.1".into(), 1),
                (addr.ip().to_string(), addr.port()),
            ],
        );
        let r = p.run().await;
        assert!(r.ok, "detail={}", r.detail);
        assert_eq!(r.detail, format!("port={}", addr.port()));
        handle.abort();
    }

    #[tokio::test]
    async fn tcp_timeout_unroutable() {
        // 10.255.255.1 is unroutable → SYN blackhole → timeout (or unreachable).
        let t = Target {
            timeout: 1,
            ..target("10.255.255.1:443")
        };
        let p = TcpProbe::new(t, vec![("10.255.255.1".into(), 443)]);
        let r = p.run().await;
        assert!(!r.ok);
        assert!(
            r.detail.starts_with("timeout") || r.detail.starts_with("unreachable"),
            "got: {}",
            r.detail
        );
    }
}
