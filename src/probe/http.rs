//! HTTP probe (rustls; no OpenSSL).

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use reqwest::Client;
use url::Url;

use crate::config::Target;
use crate::probe::{Probe, ProbeResult, result};

/// Shared client (connection pool across probes).
static CLIENT: std::sync::OnceLock<Arc<Client>> = std::sync::OnceLock::new();

pub fn shared_client() -> Arc<Client> {
    Arc::clone(
        CLIENT.get_or_init(|| Arc::new(Client::builder().build().expect("build http client"))),
    )
}

pub struct HttpProbe {
    pub target: Target,
    pub url: Url,
    pub client: Arc<Client>,
}

impl HttpProbe {
    pub fn new(target: Target, url: Url, client: Arc<Client>) -> Self {
        HttpProbe {
            target,
            url,
            client,
        }
    }
}

#[async_trait]
impl Probe for HttpProbe {
    fn target(&self) -> &Target {
        &self.target
    }

    async fn run(&self) -> ProbeResult {
        let start = Instant::now();
        match self
            .client
            .get(self.url.clone())
            .timeout(Duration::from_secs(self.target.timeout))
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    let latency = start.elapsed().as_secs_f64() * 1000.0;
                    result(&self.target, true, Some(latency), "")
                } else {
                    result(
                        &self.target,
                        false,
                        None,
                        format!("status={}", status.as_u16()),
                    )
                }
            }
            Err(e) => result(
                &self.target,
                false,
                None,
                classify_http_err(&e, self.url.scheme() == "https"),
            ),
        }
    }
}

/// Classify a reqwest error into a short detail: timeout | tls=… | connect=… | error=…
fn classify_http_err(e: &reqwest::Error, https: bool) -> String {
    if e.is_timeout() {
        return "timeout".into();
    }
    // Walk the source chain manually (Error::chain is unstable in this toolchain).
    let mut parts: Vec<String> = Vec::new();
    let mut cur: Option<&(dyn std::error::Error + 'static)> = Some(e);
    while let Some(err) = cur {
        parts.push(err.to_string());
        cur = err.source();
    }
    let chain = parts.join(" | ");
    let lower = chain.to_lowercase();
    let short: String = chain.chars().take(80).collect();
    // Plain-TCP connect failures (refused/unreachable) are not TLS failures.
    let tcp_level = lower.contains("connection refused")
        || lower.contains("tcp connect")
        || lower.contains("network is unreachable")
        || lower.contains("no route to host");
    // For https URLs, a non-TCP connect error is a TLS-layer failure
    // (rustls reports e.g. "received corrupt message" for a plain-HTTP peer).
    let tls = lower.contains("tls")
        || lower.contains("certificate")
        || lower.contains("handshake")
        || lower.contains("rustls")
        || lower.contains("invalid peer")
        || lower.contains("alert")
        || lower.contains("corrupt message")
        || (https && e.is_connect() && !tcp_level);
    if tls {
        format!("tls={short}")
    } else if e.is_connect() {
        format!("connect={short}")
    } else {
        format!("error={short}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProbeType;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn target(url: &str) -> Target {
        Target {
            name: "t".into(),
            probe_type: ProbeType::Http,
            endpoint: url.into(),
            interval: 1,
            timeout: 3,
        }
    }

    /// Minimal HTTP/1.1 server: 204 for any path except `status_path` (→ 500).
    async fn spawn_server(
        status_path: &str,
    ) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
        let status_path = status_path.to_string();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            loop {
                let (mut stream, _) = match listener.accept().await {
                    Ok(x) => x,
                    Err(_) => break,
                };
                let status_path = status_path.clone();
                tokio::spawn(async move {
                    let mut buf = [0u8; 8192];
                    let n = match stream.read(&mut buf).await {
                        Ok(n) if n > 0 => n,
                        _ => return,
                    };
                    let req = String::from_utf8_lossy(&buf[..n]).to_string();
                    let (status, body) = if req.contains(&status_path) {
                        ("500 Internal Server Error", "err")
                    } else {
                        ("204 No Content", "")
                    };
                    let resp = format!(
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(resp.as_bytes()).await;
                });
            }
        });
        (addr, handle)
    }

    #[tokio::test]
    async fn http_204_ok_500_fail() {
        let (addr, handle) = spawn_server("/500").await;
        let url = format!("http://{addr}/ok");
        let p = HttpProbe::new(target(&url), Url::parse(&url).unwrap(), shared_client());
        let r = p.run().await;
        assert!(r.ok, "detail={}", r.detail);
        assert!(r.latency_ms.is_some());

        let url500 = format!("http://{addr}/500");
        let p2 = HttpProbe::new(
            target(&url500),
            Url::parse(&url500).unwrap(),
            shared_client(),
        );
        let r2 = p2.run().await;
        assert!(!r2.ok);
        assert_eq!(r2.detail, "status=500");
        handle.abort();
    }

    #[tokio::test]
    async fn http_tls_handshake_fail() {
        // https URL against a plain-HTTP port → TLS handshake failure.
        let (addr, handle) = spawn_server("/500").await;
        let url = format!("https://{addr}/ok");
        let p = HttpProbe::new(target(&url), Url::parse(&url).unwrap(), shared_client());
        let r = p.run().await;
        assert!(!r.ok);
        assert!(r.detail.starts_with("tls="), "got: {}", r.detail);
        handle.abort();
    }
}
