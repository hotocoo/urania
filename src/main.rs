//! urania: headless, zero-permission network outage evidence logger.

mod cli;
mod config;
mod daemon;
mod detect;
mod probe;
mod report;
mod selftest;
mod store;
mod util;
mod version;

use std::collections::HashSet;
use std::sync::Arc;

use clap::Parser;

#[tokio::main]
async fn main() {
    let cli = cli::Cli::parse();
    let code = run(cli).await;
    std::process::exit(code);
}

async fn run(cli: cli::Cli) -> i32 {
    let foreground = matches!(
        &cli.cmd,
        cli::Cmd::Start { foreground: true } | cli::Cmd::Run { foreground: true }
    );
    if !foreground {
        daemon::log::init_stderr();
    }
    let cfg = match config::load(cli.config.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("urania: {e}");
            return 1;
        }
    };
    match &cli.cmd {
        cli::Cmd::Start { foreground } => {
            if *foreground {
                match daemon::run_foreground(&cfg).await {
                    Ok(code) => code,
                    Err(e) => {
                        eprintln!("urania: {e}");
                        1
                    }
                }
            } else {
                daemon::start(&cfg, cli.config.as_deref())
            }
        }
        cli::Cmd::Run { foreground } => {
            if !*foreground {
                eprintln!("urania run requires --foreground");
                return 1;
            }
            match daemon::run_foreground(&cfg).await {
                Ok(code) => code,
                Err(e) => {
                    eprintln!("urania: {e}");
                    1
                }
            }
        }
        cli::Cmd::Stop => daemon::stop(&cfg),
        cli::Cmd::Status { check, json, since } => {
            // Availability window for the at-a-glance per-target numbers.
            // Default 1h keeps the existing output (and the JSON key
            // avail_1h_pct) exactly.
            let (label, window_ms) = match since.as_deref() {
                None => ("1h".to_string(), 3_600_000),
                Some(s) => match report::Period::parse(s) {
                    Some(p) => (p.label().to_string(), p.ms()),
                    None => {
                        eprintln!("urania: --since must be one of 1h 24h 7d 30d 90d");
                        return 1;
                    }
                },
            };
            daemon::status(&cfg, *check, *json, &label, window_ms)
        }
        cli::Cmd::Report {
            since,
            from,
            end,
            to,
            csv,
            outages_csv,
            manifest,
            stdout,
            title,
        } => {
            // --since (trailing period) and --from (absolute window) are
            // mutually exclusive: guessing which the user meant would
            // silently change the evidence window.
            if from.is_some() && since.is_some() {
                eprintln!("urania: --since and --from are mutually exclusive");
                return 1;
            }
            if from.is_none() && end.is_some() {
                eprintln!("urania: --end requires --from");
                return 1;
            }
            let since = since.clone().unwrap_or_else(|| "24h".to_string());
            let window = match from.as_deref() {
                Some(f) => {
                    let f = util::parse_iso(f).unwrap_or_else(|| {
                        eprintln!("urania: --from must be RFC3339, e.g. 2026-10-15T08:00:00+08:00");
                        std::process::exit(1);
                    });
                    let t = match end.as_deref() {
                        Some(e) => util::parse_iso(e).unwrap_or_else(|| {
                            eprintln!("urania: --end must be RFC3339");
                            std::process::exit(1);
                        }),
                        None => util::now_ms(),
                    };
                    if t <= f {
                        eprintln!("urania: --end must be after --from");
                        return 1;
                    }
                    Some((f, t))
                }
                None => None,
            };
            let period = match report::Period::parse(&since) {
                Some(p) => p,
                None => {
                    eprintln!("urania: --since must be 1h | 24h | 7d | 30d | 90d");
                    return 1;
                }
            };
            let out_html = to
                .clone()
                .unwrap_or_else(|| std::path::PathBuf::from("urania-report.html"));
            let opts = report::ReportOpts {
                since: period,
                window,
                out_html,
                out_csv: csv.clone(),
                outages_csv: outages_csv.clone(),
                manifest: *manifest,
                stdout: *stdout,
                title: title.clone(),
            };
            let store = match store::Store::open(&config::db_path(&cfg)) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("urania: {e}");
                    return 1;
                }
            };
            match report::render(&cfg, &store, &opts) {
                Ok(summary) => {
                    let label = match opts.window {
                        Some(_) => "absolute window",
                        None => opts.since.label(),
                    };
                    if opts.stdout {
                        // The HTML owns stdout; the status line goes to
                        // stderr so `report --stdout > r.html` is clean.
                        eprintln!(
                            "report: - ({} probes, {} outages, {label})",
                            summary.probes, summary.outages
                        );
                    } else {
                        println!(
                            "report: {} ({} probes, {} outages, {label})",
                            summary.html.display(),
                            summary.probes,
                            summary.outages
                        );
                        if let Some(c) = &summary.csv {
                            println!("csv:      {}", c.display());
                        }
                        if let Some(c) = &summary.outages_csv {
                            println!("outages:  {}", c.display());
                        }
                        if let Some(m) = &summary.manifest {
                            println!("manifest: {}", m.display());
                        }
                    }
                    0
                }
                Err(report::ReportError::NoData) => {
                    eprintln!("urania: no probe data in the selected period");
                    3
                }
                Err(e) => {
                    eprintln!("urania: {e}");
                    1
                }
            }
        }
        cli::Cmd::Verify {
            report,
            manifest,
            quiet,
        } => {
            let manifest_path = match manifest {
                Some(p) => p.clone(),
                None => {
                    let name = report
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    report.with_file_name(format!("{name}.sha256"))
                }
            };
            match report::manifest::verify_manifest(&manifest_path) {
                Ok(results) => {
                    let mut all_ok = true;
                    for (name, ok) in &results {
                        if !ok {
                            all_ok = false;
                        }
                        if !quiet {
                            println!("{}  {name}", if *ok { "ok" } else { "FAIL" });
                        }
                    }
                    if all_ok {
                        if !quiet {
                            println!("verify: {} — all hashes match", manifest_path.display());
                        }
                        0
                    } else {
                        // Failures always print, quiet or not: the exit
                        // code says *what*, the message says *which*.
                        eprintln!(
                            "verify: {} — hash mismatch or missing file",
                            manifest_path.display()
                        );
                        1
                    }
                }
                Err(e) => {
                    eprintln!("urania: {e}");
                    2
                }
            }
        }
        cli::Cmd::Config { json } => {
            // Where the config came from: explicit --config, the default
            // path if present, else built-in defaults.
            let source = match &cli.config {
                Some(p) => p.display().to_string(),
                None => {
                    let d = config::config_path();
                    if d.exists() {
                        d.display().to_string()
                    } else {
                        "built-in defaults (no config file)".to_string()
                    }
                }
            };
            if *json {
                let targets: Vec<serde_json::Value> = cfg
                    .targets
                    .iter()
                    .map(|t| {
                        serde_json::json!({
                            "name": t.name,
                            "type": t.probe_type.to_string(),
                            "endpoint": t.endpoint,
                            "interval_s": t.interval,
                            "timeout_s": t.timeout,
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::json!({
                        "version": version::full(),
                        "config": source,
                        "db": config::db_path(&cfg).to_string_lossy().to_string(),
                        "log_dir": config::log_dir(&cfg).to_string_lossy().to_string(),
                        "lock": config::lock_path(&cfg).to_string_lossy().to_string(),
                        "fail_threshold": cfg.fail_threshold,
                        "retention_days": cfg.retention_days,
                        "icmp": cfg.icmp,
                        "targets": targets,
                    })
                );
                return 0;
            }
            println!("urania v{}", version::full());
            println!("  config:    {source}");
            println!("  db:        {}", config::db_path(&cfg).display());
            println!("  log:       {}/", config::log_dir(&cfg).display());
            println!("  lock:      {}", config::lock_path(&cfg).display());
            println!("  fail_threshold: {}", cfg.fail_threshold);
            println!("  retention_days:   {}", cfg.retention_days);
            println!("  icmp:      {}", cfg.icmp);
            println!("  targets ({}):", cfg.targets.len());
            for t in &cfg.targets {
                println!(
                    "    {:<16} {:<5} {:<44} {}s  timeout {}s",
                    t.name, t.probe_type, t.endpoint, t.interval, t.timeout
                );
            }
            0
        }
        cli::Cmd::Once {
            target,
            probe_type,
            json,
            quiet,
        } => {
            once(
                &cfg,
                target.as_deref(),
                probe_type.as_deref(),
                *json,
                *quiet,
            )
            .await
        }
        cli::Cmd::Selftest { offline } => match selftest::run(*offline).await {
            Ok(code) => code,
            Err(e) => {
                eprintln!("urania: {e}");
                1
            }
        },
    }
}

/// One probe round, table output, exit 0 = all ok / 1 = any failed.
/// A gateway that has never responded in history is a reference probe: its
/// baseline silence is marked `ref` and does not fail the round (same rule
/// as the daemon detector, status, and restart recovery).
/// `quiet`: silent on success (exit code only); failures still print.
async fn once(
    cfg: &config::Config,
    target: Option<&str>,
    probe_type: Option<&str>,
    json: bool,
    quiet: bool,
) -> i32 {
    let pt = match probe_type {
        Some(s) => match s {
            "dns" => Some(config::ProbeType::Dns),
            "tcp" => Some(config::ProbeType::Tcp),
            "http" => Some(config::ProbeType::Http),
            "icmp" => Some(config::ProbeType::Icmp),
            _ => {
                eprintln!("urania: --type must be dns | tcp | http | icmp");
                return 1;
            }
        },
        None => None,
    };
    let selected: Vec<&config::Target> = cfg
        .targets
        .iter()
        .filter(|t| target.is_none_or(|n| t.name == n) && pt.is_none_or(|p| t.probe_type == p))
        .collect();
    if selected.is_empty() {
        eprintln!("urania: no targets match the filter");
        return 1;
    }
    let gateway_ip = if selected.iter().any(|t| t.is_gateway()) {
        daemon::detect_gateway()
    } else {
        None
    };
    let probes: Vec<Arc<dyn probe::Probe>> = selected
        .iter()
        .filter_map(|t| probe::build_probe(t, &gateway_ip))
        .collect();
    if probes.is_empty() {
        eprintln!("urania: no valid targets (gateway auto-detect failed?)");
        return 1;
    }
    let results = probe::run_round(&probes).await;
    let ref_only = ref_only_gateways(cfg, &selected);
    let mut any_fail = false;
    for r in &results {
        if !r.ok && !ref_only.contains(&r.target) {
            any_fail = true;
        }
    }
    // quiet: silent on success (exit code only). Failures still print —
    // the exit code says *what*, the output says *which*.
    if !quiet || any_fail {
        if json {
            // Flat JSON for scripting (serde_json: already in the tree via
            // reqwest, now a direct dep for the machine-readable outputs).
            let ts = results
                .first()
                .map(|r| r.ts_wall_ms)
                .unwrap_or_else(crate::util::now_ms);
            let items: Vec<serde_json::Value> = results
                .iter()
                .map(|r| {
                    let mut o = serde_json::json!({
                        "target": r.target,
                        "type": r.probe_type.to_string(),
                        "ok": r.ok,
                    });
                    if let Some(l) = r.latency_ms {
                        o["latency_ms"] = serde_json::json!(l);
                    }
                    if !r.detail.is_empty() {
                        o["detail"] = serde_json::json!(r.detail);
                    }
                    if ref_only.contains(&r.target) {
                        o["ref_only"] = serde_json::json!(true);
                    }
                    o
                })
                .collect();
            println!(
                "{}",
                serde_json::json!({ "ts": ts, "all_ok": !any_fail, "results": items })
            );
        } else {
            println!(
                "{:<18} {:<6} {:<7} {:>9}  DETAIL",
                "TARGET", "TYPE", "RESULT", "LATENCY"
            );
            for r in &results {
                let lat = r
                    .latency_ms
                    .map(|l| format!("{l:.1}ms"))
                    .unwrap_or_else(|| "-".into());
                println!(
                    "{:<18} {:<6} {:<7} {:>9}  {}",
                    r.target,
                    r.probe_type,
                    if r.ok {
                        "ok"
                    } else if ref_only.contains(&r.target) {
                        "ref"
                    } else {
                        "FAIL"
                    },
                    lat,
                    r.detail
                );
            }
        }
    }
    if any_fail { 1 } else { 0 }
}

/// Honest silence (same rule as the daemon detector, status, and restart
/// recovery): gateway targets that have never responded in the full db
/// history are reference probes — their silence is the baseline state, not
/// a failure, so their probe failures must not fail the round. Fail-open:
/// db missing or unreadable → empty set (every failure counts).
fn ref_only_gateways(cfg: &config::Config, selected: &[&config::Target]) -> HashSet<String> {
    let db = config::db_path(cfg);
    if !db.exists() {
        return HashSet::new();
    }
    let Ok(store) = store::Store::open(&db) else {
        return HashSet::new();
    };
    selected
        .iter()
        .filter(|t| t.is_gateway() && !store.ever_ok(&t.name).unwrap_or(true))
        .map(|t| t.name.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gateway(name: &str) -> config::Target {
        config::Target {
            name: name.into(),
            probe_type: config::ProbeType::Tcp,
            endpoint: "auto:443,80".into(),
            interval: 5,
            timeout: 2,
        }
    }

    fn gw_probe(ok: bool) -> probe::ProbeResult {
        probe::ProbeResult {
            ts_wall_ms: util::now_ms(),
            ts_mono_delta_ms: 0,
            target: "gateway".into(),
            probe_type: config::ProbeType::Tcp,
            ok,
            latency_ms: None,
            detail: if ok {
                String::new()
            } else {
                "unreachable@192.0.2.1:80".into()
            },
        }
    }

    #[test]
    fn ref_only_gateways_follows_history() {
        let dir = std::env::temp_dir().join(format!("urania-once-refonly-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = config::Config {
            fail_threshold: 3,
            retention_days: 90,
            db_path: dir.join("t.db").to_string_lossy().into_owned(),
            log_path: String::new(),
            icmp: false,
            targets: vec![],
        };
        let gw = gateway("gateway");
        let sel = [&gw];

        // No db → fail-open: silence counts as a failure.
        assert!(ref_only_gateways(&cfg, &sel).is_empty());

        // History of only failures → reference probe: silence is baseline.
        {
            let mut s = store::Store::open(&config::db_path(&cfg)).unwrap();
            s.insert_probes(&[gw_probe(false)]).unwrap();
        }
        assert!(ref_only_gateways(&cfg, &sel).contains("gateway"));

        // One historical ok → responsive device: silence is a real signal.
        {
            let mut s = store::Store::open(&config::db_path(&cfg)).unwrap();
            s.insert_probes(&[gw_probe(true)]).unwrap();
        }
        assert!(ref_only_gateways(&cfg, &sel).is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
