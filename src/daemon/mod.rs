//! Daemon: lock, start/stop/status, the foreground loop, gateway detection.

pub mod lock;
pub mod log;
pub mod signals;

#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;

use crate::config::{self, Config};
use crate::detect::{Detector, apply_cmd};
use crate::probe::{self, Probe};
use crate::store::{CloseWindow, Store};
use crate::util::{fmt_duration, fmt_ts, now_iso, now_ms, parse_iso};

/// Detect the default gateway: `ip route` (Linux) / `route -n get default` (macOS).
pub fn detect_gateway() -> Option<std::net::IpAddr> {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("route")
            .args(["-n", "get", "default"])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if let Some(rest) = line.trim().strip_prefix("gateway:") {
                return rest.trim().parse().ok();
            }
        }
        None
    }
    #[cfg(target_os = "linux")]
    {
        let out = std::process::Command::new("ip")
            .args(["-4", "route", "show", "default"])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let mut it = line.split_whitespace();
            while let Some(tok) = it.next() {
                if tok == "via" {
                    return it.next()?.parse().ok();
                }
            }
        }
        None
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

/// Best-effort NTP status note (roadmap N8).
fn clock_note() -> String {
    #[cfg(target_os = "linux")]
    {
        if let Ok(out) = std::process::Command::new("timedatectl").output() {
            let text = String::from_utf8_lossy(&out.stdout);
            if let Some(line) = text.lines().find(|l| l.starts_with("NTP service:")) {
                let state = line
                    .split_once(':')
                    .map(|(_, v)| v.trim().to_string())
                    .unwrap_or_default();
                return format!("NTP service: {state} (timedatectl)");
            }
        }
        "NTP: unknown".into()
    }
    #[cfg(not(target_os = "linux"))]
    {
        "NTP: unknown (best-effort)".into()
    }
}

/// Spawn the detached daemon and verify it started (pidfile after 500 ms).
pub fn start(cfg: &Config, config_arg: Option<&Path>) -> i32 {
    let lock_path = config::lock_path(cfg);
    if let Some(pid) = lock::peek(&lock_path) {
        println!("urania is already running (pid {pid})");
        return 2;
    }
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("urania: cannot resolve current exe: {e}");
            return 1;
        }
    };
    let mut cmd = std::process::Command::new(&exe);
    cmd.arg("run").arg("--foreground");
    if let Some(p) = config_arg {
        cmd.arg("--config").arg(p);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    // Detach into a new session (async-signal-safe, both platforms): the
    // daemon survives the launcher exiting and SIGHUP from terminal logout.
    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| {
            let _ = libc::setsid();
            Ok(())
        });
    }
    match cmd.spawn() {
        Ok(_child) => {}
        Err(e) => {
            eprintln!("urania: spawn failed: {e}");
            return 1;
        }
    }
    // Verify: the daemon writes the pidfile after acquiring the lock.
    std::thread::sleep(Duration::from_millis(500));
    match lock::peek(&lock_path) {
        Some(pid) => {
            println!("urania started (pid {pid})");
            0
        }
        None => {
            println!(
                "urania failed to start; check the log in {}",
                config::log_dir(cfg).display()
            );
            1
        }
    }
}

/// Stop: SIGTERM + up to 10 s. Never SIGKILL in v1.
pub fn stop(cfg: &Config) -> i32 {
    let lock_path = config::lock_path(cfg);
    let Some(pid) = lock::peek(&lock_path) else {
        println!("urania is not running");
        return 0;
    };
    println!("stopping urania (pid {pid})…");
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }
    for _ in 0..100 {
        std::thread::sleep(Duration::from_millis(100));
        if !lock::is_alive(pid) {
            println!("urania stopped");
            return 0;
        }
    }
    println!(
        "urania still running after 10s; no SIGKILL in v1 — check the log in {}",
        config::log_dir(cfg).display()
    );
    1
}

/// Status: daemon, db, lock, per-target last probe, open outages.
/// Degrades gracefully when the db is unreadable (roadmap T4).
pub fn status(cfg: &Config) -> i32 {
    println!("urania v{}", env!("CARGO_PKG_VERSION"));
    let lock_path = config::lock_path(cfg);
    let running_pid = lock::peek(&lock_path);
    let db_path = config::db_path(cfg);

    let uptime = if let Some(pid) = running_pid {
        Store::open(&db_path)
            .ok()
            .and_then(|s| s.get_meta("daemon_started_at").ok().flatten())
            .and_then(|v| parse_iso(&v))
            .map(|started| {
                format!(
                    " (pid {pid}, uptime {})",
                    fmt_duration(now_ms().saturating_sub(started))
                )
            })
            .unwrap_or_else(|| format!(" (pid {pid})"))
    } else {
        "no".into()
    };

    match Store::open(&db_path) {
        Ok(store) => {
            let size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
            let rows = store.probe_count().unwrap_or(0);
            println!("  running:   {uptime}");
            println!(
                "  db:        {} ({:.1} MB, retention {}d, {rows} rows)",
                db_path.display(),
                size as f64 / 1_048_576.0,
                cfg.retention_days
            );
            println!(
                "  lock:      {}",
                match lock::read_pid(&lock_path) {
                    Some(p) => format!("held by pid {p}"),
                    None => "free".into(),
                }
            );
            println!("  targets:");
            for t in &cfg.targets {
                match store.last_probe(&t.name) {
                    Ok(Some(row)) => {
                        let lat = row
                            .latency_ms
                            .map(|l| format!("{l:.1}ms"))
                            .unwrap_or_else(|| "—".into());
                        println!(
                            "    {:<16} last {} {} {} ({})",
                            t.name,
                            fmt_ts(row.ts),
                            if row.ok { "ok" } else { "FAIL" },
                            lat,
                            t.desc()
                        );
                    }
                    Ok(None) => println!("    {:<16} no data yet ({})", t.name, t.desc()),
                    Err(_) => println!("    {:<16} (db error)", t.name),
                }
            }
            match store.open_windows() {
                Ok(open) if open.is_empty() => println!("  open outages: none"),
                Ok(open) => {
                    println!("  open outages:");
                    for w in &open {
                        println!(
                            "    {} since {} ({}m)",
                            w.target,
                            fmt_ts(w.start_ts),
                            (now_ms().saturating_sub(w.start_ts) / 60_000)
                        );
                    }
                }
                Err(_) => println!("  open outages: (db error)"),
            }
        }
        Err(e) => {
            println!("  running:   {uptime}");
            println!("  db:        {} (unreadable: {e})", db_path.display());
            println!(
                "  lock:      {}",
                match lock::read_pid(&lock_path) {
                    Some(p) => format!("held by pid {p}"),
                    None => "free".into(),
                }
            );
            println!("  targets:   (unavailable without db access)");
            println!(
                "  hint:      under a systemd DynamicUser install, use `systemctl status urania`"
            );
        }
    }
    0
}

/// The real daemon loop (systemd / Docker / `start --foreground`).
pub async fn run_foreground(cfg: &Config) -> anyhow::Result<i32> {
    let data_dir = config::log_dir(cfg);
    std::fs::create_dir_all(&data_dir)?;
    let lock_path = config::lock_path(cfg);
    let _lock = lock::PidLock::acquire(&lock_path)?;
    let _log_guard = log::init_file(&data_dir)?;
    tracing::info!(
        "urania v{} starting (pid {})",
        env!("CARGO_PKG_VERSION"),
        std::process::id()
    );

    let mut store = Store::open(&config::db_path(cfg))?;
    store.set_meta("tool_version", env!("CARGO_PKG_VERSION"))?;
    store.set_meta("daemon_started_at", &now_iso())?;
    store.set_meta("clock_note", &clock_note())?;
    if let Ok(n) = store.prune(cfg.retention_days)
        && n > 0
    {
        tracing::info!("retention: pruned {n} probe rows");
    }

    // Restart recovery (roadmap §7.3): close windows whose last probe was ok;
    // re-seed the detector with the rest.
    let open = store.open_windows()?;
    let now = now_ms();
    let mut kept = Vec::new();
    for w in &open {
        match store.last_probe(&w.target)? {
            Some(row) if !row.ok => kept.push(w.clone()),
            Some(row) => {
                store.close_window(&CloseWindow {
                    id: w.id,
                    end_ts: row.ts,
                    layers: "unknown".into(),
                    cause: Some("recovered while daemon was stopped".into()),
                    location_class: "unknown".into(),
                    flap: false,
                    probes_failed: w.probes_failed as u32,
                })?;
                tracing::info!(
                    "recovery: closed stale window {} for {} at last ok probe",
                    w.id,
                    w.target
                );
            }
            None => {
                store.close_window(&CloseWindow {
                    id: w.id,
                    end_ts: now,
                    layers: "unknown".into(),
                    cause: Some("daemon stopped; no probe data".into()),
                    location_class: "unknown".into(),
                    flap: false,
                    probes_failed: w.probes_failed as u32,
                })?;
                tracing::info!(
                    "recovery: closed window {} for {} (no probe data)",
                    w.id,
                    w.target
                );
            }
        }
    }

    // Gateway auto-detect (T3: the port-list fallback lives in the probe).
    let gateway_target = cfg
        .targets
        .iter()
        .find(|t| t.is_gateway())
        .map(|t| t.name.clone());
    let gateway_ip = if gateway_target.is_some() {
        match detect_gateway() {
            Some(ip) => {
                store.set_meta("gateway_ip", &ip.to_string())?;
                Some(ip)
            }
            None => {
                store.set_meta(
                    "gateway_note",
                    "auto-detect failed; location_class will be unknown",
                )?;
                None
            }
        }
    } else {
        None
    };

    // ICMP capability check (feature-gated).
    #[cfg(feature = "icmp")]
    let icmp_ok = if cfg.icmp
        && cfg
            .targets
            .iter()
            .any(|t| t.probe_type == crate::config::ProbeType::Icmp)
    {
        probe::icmp::available("127.0.0.1", 2)
    } else {
        true
    };

    let mut probes: Vec<Arc<dyn Probe>> = Vec::new();
    for t in &cfg.targets {
        if t.is_gateway() && gateway_ip.is_none() {
            tracing::warn!("gateway target {} skipped: auto-detect failed", t.name);
            continue;
        }
        #[cfg(feature = "icmp")]
        if t.probe_type == crate::config::ProbeType::Icmp && !icmp_ok {
            tracing::warn!(
                "icmp target {} skipped: no permission (run as root or grant CAP_NET_RAW)",
                t.name
            );
            continue;
        }
        match probe::build_probe(t, &gateway_ip) {
            Some(p) => probes.push(p),
            None => tracing::warn!("target {} skipped: bad endpoint", t.name),
        }
    }
    if probes.is_empty() {
        anyhow::bail!("no valid targets; nothing to do");
    }

    let (tx, mut rx) = mpsc::channel::<probe::ProbeResult>(256);
    let scheduler_tx = tx.clone();
    let handles = probe::spawn_scheduler(probes, scheduler_tx);
    let mut detector = Detector::new(cfg.fail_threshold, &cfg.targets, gateway_target, &kept);
    let mut batch: Vec<probe::ProbeResult> = Vec::new();
    let mut prune_iv = tokio::time::interval(Duration::from_secs(6 * 3_600));
    tracing::info!("urania running: {} target(s)", handles.len());

    let mut shutdown = false;
    while !shutdown {
        tokio::select! {
            maybe_res = rx.recv() => {
                let Some(res) = maybe_res else { break };
                let cmds = detector.feed(res.clone(), &store);
                batch.push(res);
                if !cmds.is_empty() || batch.len() >= 100 {
                    if let Err(e) = store.insert_probes(&batch) {
                        tracing::error!("probe insert failed: {e}");
                    }
                    batch.clear();
                    for cmd in cmds {
                        if let Err(e) = apply_cmd(&store, &cmd) {
                            tracing::error!("window close failed: {e}");
                        }
                    }
                }
            }
            _ = signals::wait_terminate() => {
                shutdown = true;
            }
            _ = prune_iv.tick() => {
                if let Ok(n) = store.prune(cfg.retention_days) && n > 0 {
                    tracing::info!("retention: pruned {n} probe rows");
                }
            }
        }
    }

    // Graceful shutdown: close open windows, flush the batch, mark stopped.
    tracing::info!("shutdown: closing open windows and flushing");
    let now = now_ms();
    for cmd in detector.close_all(now) {
        let _ = apply_cmd(&store, &cmd);
    }
    if !batch.is_empty() {
        let _ = store.insert_probes(&batch);
    }
    store.set_meta("daemon_stopped_at", &now_iso())?;
    drop(tx); // close the channel → scheduler tasks exit
    drop(handles);
    _lock.release();
    tracing::info!("urania stopped");
    Ok(0)
}
