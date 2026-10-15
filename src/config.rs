//! Configuration: file layout, TOML parsing, validation, defaults.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use url::Url;

/// Errors loading or validating configuration.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("config file not found: {0}")]
    NotFound(PathBuf),
    #[error("invalid config: {0}")]
    Invalid(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Probe layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProbeType {
    Dns,
    Tcp,
    Http,
    Icmp,
}

impl std::fmt::Display for ProbeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ProbeType::Dns => "dns",
            ProbeType::Tcp => "tcp",
            ProbeType::Http => "http",
            ProbeType::Icmp => "icmp",
        };
        f.write_str(s)
    }
}

/// A single probe target.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub name: String,
    #[serde(rename = "type")]
    pub probe_type: ProbeType,
    /// dns: resolver IP (e.g. "1.1.1.1"); tcp: "host:port" or gateway
    /// "auto:443,80"; http: full URL; icmp: host.
    pub endpoint: String,
    #[serde(default = "default_interval")]
    pub interval: u64,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

fn default_interval() -> u64 {
    5
}

fn default_timeout() -> u64 {
    3
}

impl Target {
    /// True for the special gateway target whose endpoint is `auto:<ports>`.
    pub fn is_gateway(&self) -> bool {
        self.probe_type == ProbeType::Tcp && self.endpoint.starts_with("auto:")
    }

    /// Port list for a gateway target (e.g. `auto:443,80` → [443, 80]).
    pub fn gateway_ports(&self) -> Vec<u16> {
        self.endpoint
            .strip_prefix("auto:")
            .map(|p| {
                p.split(',')
                    .filter_map(|s| s.trim().parse::<u16>().ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// One-line description for status output.
    pub fn desc(&self) -> String {
        match self.probe_type {
            ProbeType::Dns => format!("resolver {}, TCP/53, {}s", self.endpoint, self.interval),
            ProbeType::Tcp if self.is_gateway() => {
                format!("{} (auto), {}s", self.endpoint, self.interval)
            }
            ProbeType::Tcp => format!("{}, {}s", self.endpoint, self.interval),
            ProbeType::Http => format!("{}s", self.interval),
            ProbeType::Icmp => format!("{}s", self.interval),
        }
    }
}

/// Top-level configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub fail_threshold: u32,
    pub retention_days: u32,
    /// Explicit db path; empty → platform default.
    pub db_path: String,
    /// Explicit log file path (its parent dir is the log dir); empty → default.
    pub log_path: String,
    /// Enable ICMP targets (requires the `icmp` build feature + OS permission).
    pub icmp: bool,
    /// TOML array-of-tables key is `[[target]]`.
    #[serde(rename = "target")]
    pub targets: Vec<Target>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            fail_threshold: 3,
            retention_days: 90,
            db_path: String::new(),
            log_path: String::new(),
            icmp: false,
            targets: default_targets(),
        }
    }
}

/// The seven default targets (roadmap §4.3).
pub fn default_targets() -> Vec<Target> {
    vec![
        Target {
            name: "dns-cloudflare".into(),
            probe_type: ProbeType::Dns,
            endpoint: "1.1.1.1".into(),
            interval: 5,
            timeout: 3,
        },
        Target {
            name: "dns-google".into(),
            probe_type: ProbeType::Dns,
            endpoint: "8.8.8.8".into(),
            interval: 5,
            timeout: 3,
        },
        Target {
            name: "tcp-cloudflare".into(),
            probe_type: ProbeType::Tcp,
            endpoint: "1.1.1.1:443".into(),
            interval: 5,
            timeout: 3,
        },
        Target {
            name: "http-gstatic".into(),
            probe_type: ProbeType::Http,
            endpoint: "https://connectivitycheck.gstatic.com/generate_204".into(),
            interval: 10,
            timeout: 5,
        },
        Target {
            name: "http-cloudflare".into(),
            probe_type: ProbeType::Http,
            endpoint: "https://www.cloudflare.com/cdn-cgi/trace".into(),
            interval: 10,
            timeout: 5,
        },
        Target {
            name: "gateway".into(),
            probe_type: ProbeType::Tcp,
            endpoint: "auto:443,80".into(),
            interval: 5,
            timeout: 2,
        },
        Target {
            name: "icmp-cloudflare".into(),
            probe_type: ProbeType::Icmp,
            endpoint: "1.1.1.1".into(),
            interval: 5,
            timeout: 3,
        },
    ]
}

/// Default data directory: ~/.local/share/urania (Linux) or
/// ~/Library/Application Support/urania (macOS).
pub fn default_data_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support/urania")
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
            let p = PathBuf::from(xdg);
            let base = if p.is_absolute() { p } else { home.join(p) };
            base.join("urania")
        } else {
            home.join(".local/share/urania")
        }
    }
}

/// Default config file path: ~/.config/urania/config.toml (both platforms).
pub fn config_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            let p = PathBuf::from(xdg);
            let base = if p.is_absolute() { p } else { home.join(p) };
            return base.join("urania/config.toml");
        }
    }
    home.join(".config/urania/config.toml")
}

/// Database path (explicit config wins over platform default).
pub fn db_path(cfg: &Config) -> PathBuf {
    if cfg.db_path.is_empty() {
        default_data_dir().join("urania.db")
    } else {
        PathBuf::from(&cfg.db_path)
    }
}

/// Lock file path (sibling of the db).
pub fn lock_path(cfg: &Config) -> PathBuf {
    db_path(cfg).with_file_name("urania.lock")
}

/// Log directory (explicit log_path's parent wins over the data dir).
pub fn log_dir(cfg: &Config) -> PathBuf {
    if cfg.log_path.is_empty() {
        db_path(cfg)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        PathBuf::from(&cfg.log_path)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

/// Load config: explicit path (must exist), else default path if present,
/// else built-in defaults.
pub fn load(cli_path: Option<&Path>) -> Result<Config, ConfigError> {
    let path = match cli_path {
        Some(p) => {
            if !p.exists() {
                return Err(ConfigError::NotFound(p.to_path_buf()));
            }
            p.to_path_buf()
        }
        None => {
            let d = config_path();
            if d.exists() {
                d
            } else {
                return Ok(Config::default());
            }
        }
    };
    let text = std::fs::read_to_string(&path)?;
    let cfg: Config = toml::from_str(&text).map_err(|e| ConfigError::Invalid(e.to_string()))?;
    cfg.validate()?;
    Ok(cfg)
}

impl Config {
    /// Validate global fields and every target.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.fail_threshold == 0 {
            return Err(ConfigError::Invalid("fail_threshold must be >= 1".into()));
        }
        if self.retention_days == 0 {
            return Err(ConfigError::Invalid("retention_days must be >= 1".into()));
        }
        let mut names = HashSet::new();
        for t in &self.targets {
            if !names.insert(t.name.as_str()) {
                return Err(ConfigError::Invalid(format!(
                    "duplicate target name: {}",
                    t.name
                )));
            }
            if t.interval < 1 {
                return Err(ConfigError::Invalid(format!(
                    "target {}: interval must be >= 1",
                    t.name
                )));
            }
            if t.timeout < 1 {
                return Err(ConfigError::Invalid(format!(
                    "target {}: timeout must be >= 1",
                    t.name
                )));
            }
            match t.probe_type {
                ProbeType::Dns => {
                    t.endpoint.parse::<std::net::IpAddr>().map_err(|_| {
                        ConfigError::Invalid(format!(
                            "target {}: dns endpoint must be a resolver IP, got {}",
                            t.name, t.endpoint
                        ))
                    })?;
                }
                ProbeType::Tcp => {
                    if t.is_gateway() {
                        if t.gateway_ports().is_empty() {
                            return Err(ConfigError::Invalid(format!(
                                "target {}: gateway endpoint must be auto:<port,...>",
                                t.name
                            )));
                        }
                    } else {
                        parse_host_port(&t.endpoint).map_err(|_| {
                            ConfigError::Invalid(format!(
                                "target {}: tcp endpoint must be host:port, got {}",
                                t.name, t.endpoint
                            ))
                        })?;
                    }
                }
                ProbeType::Http => {
                    let u = Url::parse(&t.endpoint).map_err(|_| {
                        ConfigError::Invalid(format!(
                            "target {}: bad http endpoint: {}",
                            t.name, t.endpoint
                        ))
                    })?;
                    if !matches!(u.scheme(), "http" | "https") {
                        return Err(ConfigError::Invalid(format!(
                            "target {}: http endpoint must be http(s), got {}",
                            t.name, t.endpoint
                        )));
                    }
                }
                ProbeType::Icmp => {
                    if t.endpoint.trim().is_empty() {
                        return Err(ConfigError::Invalid(format!(
                            "target {}: icmp endpoint empty",
                            t.name
                        )));
                    }
                }
            }
        }
        Ok(())
    }
}

/// Parse "host:port" (IPv4, IPv6, or hostname).
pub fn parse_host_port(endpoint: &str) -> Result<(String, u16), String> {
    let (host, port) = endpoint
        .rsplit_once(':')
        .ok_or_else(|| format!("expected host:port, got {endpoint}"))?;
    if host.is_empty() {
        return Err(format!("empty host in {endpoint}"));
    }
    let port: u16 = port
        .parse()
        .map_err(|_| format!("bad port in {endpoint}"))?;
    Ok((host.to_string(), port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_have_seven_targets() {
        let c = Config::default();
        assert_eq!(c.targets.len(), 7);
        assert!(c.targets.iter().any(|t| t.is_gateway()));
        assert_eq!(c.fail_threshold, 3);
        assert_eq!(c.retention_days, 90);
        assert!(c.validate().is_ok());
    }

    #[test]
    fn partial_toml_merges_defaults() {
        let toml = r#"
            fail_threshold = 5
            [[target]]
            name = "t1"
            type = "tcp"
            endpoint = "1.1.1.1:443"
            interval = 2
            timeout = 2
        "#;
        let c: Config = toml::from_str(toml).unwrap();
        assert_eq!(c.fail_threshold, 5);
        assert_eq!(c.targets.len(), 1);
        assert_eq!(c.retention_days, 90);
        assert!(c.validate().is_ok());
    }

    #[test]
    fn missing_explicit_file_is_notfound() {
        let e = load(Some(Path::new("/nonexistent/urania-config.toml"))).unwrap_err();
        assert!(matches!(e, ConfigError::NotFound(_)));
    }

    #[test]
    fn bad_http_endpoint_rejected() {
        let c = Config {
            targets: vec![Target {
                name: "x".into(),
                probe_type: ProbeType::Http,
                endpoint: "not a url".into(),
                interval: 5,
                timeout: 5,
            }],
            ..Default::default()
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn interval_below_one_rejected() {
        let c = Config {
            targets: vec![Target {
                name: "x".into(),
                probe_type: ProbeType::Tcp,
                endpoint: "1.1.1.1:443".into(),
                interval: 0,
                timeout: 5,
            }],
            ..Default::default()
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn duplicate_names_rejected() {
        let t = Target {
            name: "x".into(),
            probe_type: ProbeType::Tcp,
            endpoint: "1.1.1.1:443".into(),
            interval: 5,
            timeout: 5,
        };
        let c = Config {
            targets: vec![t.clone(), t],
            ..Default::default()
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn gateway_ports_parsed() {
        let t = Target {
            name: "g".into(),
            probe_type: ProbeType::Tcp,
            endpoint: "auto:443,80".into(),
            interval: 5,
            timeout: 2,
        };
        assert!(t.is_gateway());
        assert_eq!(t.gateway_ports(), vec![443, 80]);
    }

    #[test]
    fn parse_host_port_roundtrip() {
        assert_eq!(
            parse_host_port("1.1.1.1:443").unwrap(),
            ("1.1.1.1".to_string(), 443)
        );
        assert!(parse_host_port("nocolon").is_err());
        assert!(parse_host_port("1.1.1.1:99999").is_err());
    }
}
