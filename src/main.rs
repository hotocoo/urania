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
        cli::Cmd::Status { check } => daemon::status(&cfg, *check),
        cli::Cmd::Report {
            since,
            from,
            end,
            to,
            csv,
            manifest,
        } => {
            // Absolute incident window (--from [--end]) wins over --since.
            if from.is_none() && end.is_some() {
                eprintln!("urania: --end requires --from");
                return 1;
            }
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
            let period = match report::Period::parse(since) {
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
                manifest: *manifest,
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
                    println!(
                        "report: {} ({} probes, {} outages)",
                        summary.html.display(),
                        summary.probes,
                        summary.outages
                    );
                    if let Some(c) = &summary.csv {
                        println!("csv:      {}", c.display());
                    }
                    if let Some(m) = &summary.manifest {
                        println!("manifest: {}", m.display());
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
        cli::Cmd::Verify { report, manifest } => {
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
                        println!("{}  {name}", if *ok { "ok" } else { "FAIL" });
                    }
                    if all_ok {
                        println!("verify: {} — all hashes match", manifest_path.display());
                        0
                    } else {
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
        cli::Cmd::Config => {
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
        } => once(&cfg, target.as_deref(), probe_type.as_deref(), *json).await,
        cli::Cmd::Selftest => match selftest::run().await {
            Ok(code) => code,
            Err(e) => {
                eprintln!("urania: {e}");
                1
            }
        },
    }
}

/// One probe round, table output, exit 0 = all ok / 1 = any failed.
async fn once(
    cfg: &config::Config,
    target: Option<&str>,
    probe_type: Option<&str>,
    json: bool,
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
    let mut any_fail = false;
    for r in &results {
        if !r.ok {
            any_fail = true;
        }
    }
    if json {
        // Flat JSON for scripting. Hand-rolled (no serde_json dep): the
        // only free-form fields are target names and detail strings,
        // both controlled by urania.
        let ts = results
            .first()
            .map(|r| r.ts_wall_ms)
            .unwrap_or_else(crate::util::now_ms);
        let items: Vec<String> = results
            .iter()
            .map(|r| {
                let mut s = format!(
                    "{{\"target\":\"{}\",\"type\":\"{}\",\"ok\":{}",
                    json_escape(&r.target),
                    r.probe_type,
                    r.ok
                );
                if let Some(l) = r.latency_ms {
                    s.push_str(&format!(",\"latency_ms\":{l:.1}"));
                }
                if !r.detail.is_empty() {
                    s.push_str(&format!(",\"detail\":\"{}\"", json_escape(&r.detail)));
                }
                s.push('}');
                s
            })
            .collect();
        println!(
            "{{\"ts\":{ts},\"all_ok\":{},\"results\":[{}]}}",
            !any_fail,
            items.join(",")
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
                if r.ok { "ok" } else { "FAIL" },
                lat,
                r.detail
            );
        }
    }
    if any_fail { 1 } else { 0 }
}

/// Minimal JSON string escaping for the controlled free-form fields
/// (target names, probe detail strings).
fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
