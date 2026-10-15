//! Shared time/host helpers.

use chrono::{DateTime, Local, TimeZone, Utc};

/// Milliseconds since the Unix epoch (wall clock).
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// RFC3339 local timestamp, e.g. "2026-10-15T16:30:00+08:00".
pub fn now_iso() -> String {
    DateTime::<Local>::from(Utc::now())
        .format("%Y-%m-%dT%H:%M:%S%:z")
        .to_string()
}

/// Parse an RFC3339 timestamp into epoch milliseconds.
pub fn parse_iso(s: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.timestamp_millis())
}

/// Local time for display, e.g. "2026-10-15 16:30:00".
pub fn fmt_ts(ms: i64) -> String {
    Utc.timestamp_millis_opt(ms)
        .single()
        .map(|d| {
            d.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| format!("ts={ms}"))
}

/// RFC3339 UTC for CSV (machine-readable column).
pub fn fmt_ts_utc_iso(ms: i64) -> String {
    Utc.timestamp_millis_opt(ms)
        .single()
        .map(|d| d.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
        .unwrap_or_else(|| format!("ts={ms}"))
}

/// Human duration: "3d 11h", "1h 30m", "70m 0s", "42s".
pub fn fmt_duration(ms: i64) -> String {
    let s = (ms / 1000).max(0);
    let d = s / 86_400;
    let h = (s % 86_400) / 3_600;
    let m = (s % 3_600) / 60;
    let sec = s % 60;
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m {sec}s")
    } else {
        format!("{sec}s")
    }
}

/// Host identification for the report header: "myhost, macos aarch64 [kernel]".
pub fn host_string() -> String {
    let name = std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into());
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_default();
    if kernel.is_empty() {
        format!("{name}, {os} {arch}")
    } else {
        format!("{name}, {os} {arch} {kernel}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_duration_units() {
        assert_eq!(fmt_duration(42_000), "42s");
        assert_eq!(fmt_duration(4_200_000), "1h 10m");
        assert_eq!(fmt_duration(5_400_000), "1h 30m");
        assert_eq!(fmt_duration(300_000_000), "3d 11h");
    }

    #[test]
    fn iso_roundtrip() {
        let s = now_iso();
        assert!(parse_iso(&s).is_some());
    }
}
