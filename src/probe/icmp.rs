//! ICMP probe (optional, feature `icmp`). Uses the `ping` crate (dgram
//! socket with raw fallback). Where ICMP needs privileges, the probe
//! reports `permission` and the daemon skips the target at startup.

use std::net::{IpAddr, ToSocketAddrs};
use std::time::Duration;

use async_trait::async_trait;
use ping::{Pinger, Request};

use crate::config::Target;
use crate::probe::{Probe, ProbeResult, result};

pub struct IcmpProbe {
    pub target: Target,
    pub host: String,
}

impl IcmpProbe {
    pub fn new(target: Target, host: String) -> Self {
        IcmpProbe { target, host }
    }
}

fn resolve(host: &str) -> Result<IpAddr, std::io::Error> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(ip);
    }
    let sa = host
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no addresses"))?;
    Ok(sa.ip())
}

/// True when the error is a socket permission failure (EPERM/EACCES).
fn is_permission(e: &ping::Error) -> bool {
    matches!(
        e,
        ping::Error::IoError { error }
            if matches!(error.raw_os_error(), Some(libc::EPERM) | Some(libc::EACCES))
    )
}

/// One-shot capability check (blocking; call on a thread). A timeout means
/// the socket works (permissions ok) even if the network is down.
pub fn available(host: &str, timeout_secs: u64) -> bool {
    let host = host.to_string();
    std::thread::Builder::new()
        .name("urania-icmp-check".into())
        .spawn(move || {
            let Ok(ip) = resolve(&host) else {
                return false;
            };
            let mut pinger = Pinger::new();
            match pinger.ping(Request::new(ip), Duration::from_secs(timeout_secs)) {
                Ok(_) | Err(ping::Error::Timeout) => true,
                Err(e) => !is_permission(&e),
            }
        })
        .map(|j| j.join().unwrap_or(false))
        .unwrap_or(false)
}

#[async_trait]
impl Probe for IcmpProbe {
    fn target(&self) -> &Target {
        &self.target
    }

    async fn run(&self) -> ProbeResult {
        let host = self.host.clone();
        let timeout = Duration::from_secs(self.target.timeout);
        let res = tokio::task::spawn_blocking(move || -> Result<ping::Reply, ping::Error> {
            let ip = resolve(&host).map_err(|error| ping::Error::IoError { error })?;
            let mut pinger = Pinger::new();
            pinger.ping(Request::new(ip), timeout)
        })
        .await;
        match res {
            Err(_) => result(&self.target, false, None, "error"),
            Ok(Err(ping::Error::Timeout)) => result(&self.target, false, None, "timeout"),
            Ok(Err(e)) if is_permission(&e) => result(&self.target, false, None, "permission"),
            Ok(Err(_)) => result(&self.target, false, None, "error"),
            Ok(Ok(reply)) => result(
                &self.target,
                true,
                Some(reply.rtt.as_secs_f64() * 1000.0),
                "",
            ),
        }
    }
}
