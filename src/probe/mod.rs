//! Probe layer: DNS / TCP / HTTP / ICMP (feature-gated) + scheduler.

pub mod dns;
pub mod http;
#[cfg(feature = "icmp")]
pub mod icmp;
pub mod tcp;

use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::task::JoinHandle;

use crate::config::{ProbeType, Target};
use crate::util::now_ms;

/// One probe result (what gets stored and fed to the detector).
#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub ts_wall_ms: i64,
    /// Milliseconds since the previous result for this target (0 in one-shot modes).
    pub ts_mono_delta_ms: i64,
    pub target: String,
    pub probe_type: ProbeType,
    pub ok: bool,
    pub latency_ms: Option<f64>,
    /// Failure detail, e.g. "query-timeout@1.1.1.1", "timeout@host:443", "status=503".
    pub detail: String,
}

/// A probe bound to one target.
#[async_trait]
pub trait Probe: Send + Sync {
    fn target(&self) -> &Target;
    async fn run(&self) -> ProbeResult;
}

pub(crate) fn result(
    target: &Target,
    ok: bool,
    latency_ms: Option<f64>,
    detail: impl Into<String>,
) -> ProbeResult {
    ProbeResult {
        ts_wall_ms: now_ms(),
        ts_mono_delta_ms: 0,
        target: target.name.clone(),
        probe_type: target.probe_type,
        ok,
        latency_ms,
        detail: detail.into(),
    }
}

/// Build a concrete probe for a configured target. `gateway_ip` is the
/// auto-detected gateway for `auto:<ports>` targets.
pub fn build_probe(t: &Target, gateway_ip: &Option<IpAddr>) -> Option<Arc<dyn Probe>> {
    match t.probe_type {
        ProbeType::Tcp if t.is_gateway() => {
            let Some(ip) = gateway_ip else {
                return None;
            };
            let ports = t.gateway_ports();
            if ports.is_empty() {
                return None;
            }
            let candidates = ports.iter().map(|p| (ip.to_string(), *p)).collect();
            Some(Arc::new(tcp::TcpProbe::new(t.clone(), candidates)))
        }
        ProbeType::Tcp => {
            let (host, port) = crate::config::parse_host_port(&t.endpoint).ok()?;
            Some(Arc::new(tcp::TcpProbe::new(t.clone(), vec![(host, port)])))
        }
        ProbeType::Http => {
            let url = url::Url::parse(&t.endpoint).ok()?;
            Some(Arc::new(http::HttpProbe::new(
                t.clone(),
                url,
                http::shared_client(),
            )))
        }
        ProbeType::Dns => {
            let resolver: IpAddr = t.endpoint.parse().ok()?;
            Some(Arc::new(dns::DnsProbe::new(
                t.clone(),
                resolver,
                dns::QUERY_DOMAIN.to_string(),
            )))
        }
        ProbeType::Icmp => {
            #[cfg(feature = "icmp")]
            {
                Some(Arc::new(icmp::IcmpProbe::new(
                    t.clone(),
                    t.endpoint.clone(),
                )))
            }
            #[cfg(not(feature = "icmp"))]
            {
                tracing::warn!(
                    "target {}: icmp ignored (build with --features icmp)",
                    t.name
                );
                None
            }
        }
    }
}

/// Run one probe round against all probes concurrently; results in input order.
pub async fn run_round(probes: &[Arc<dyn Probe>]) -> Vec<ProbeResult> {
    let handles: Vec<JoinHandle<ProbeResult>> = probes
        .iter()
        .map(|p| {
            let p = Arc::clone(p);
            tokio::spawn(async move { p.run().await })
        })
        .collect();
    let mut out = Vec::with_capacity(handles.len());
    for h in handles {
        match h.await {
            Ok(r) => out.push(r),
            Err(e) => tracing::error!("probe task failed: {e}"),
        }
    }
    out
}

/// Spawn the per-target scheduler tasks: each fires on its own interval and
/// sends results over `tx`. Tasks exit when the channel is closed.
pub fn spawn_scheduler(
    probes: Vec<Arc<dyn Probe>>,
    tx: tokio::sync::mpsc::Sender<ProbeResult>,
) -> Vec<JoinHandle<()>> {
    probes
        .into_iter()
        .map(|p| {
            let tx = tx.clone();
            let interval = Duration::from_secs(p.target().interval.max(1));
            tokio::spawn(async move {
                let mut tick = tokio::time::interval(interval);
                tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                let mut last: Option<Instant> = None;
                loop {
                    tick.tick().await;
                    let mut res = p.run().await;
                    let now = Instant::now();
                    res.ts_mono_delta_ms = last
                        .map(|l| now.duration_since(l).as_millis() as i64)
                        .unwrap_or(0);
                    last = Some(now);
                    if tx.send(res).await.is_err() {
                        return; // receiver dropped: shutting down
                    }
                }
            })
        })
        .collect()
}
