//! `urania selftest`: round-trip against known-good targets (roadmap §11.7).
//! Exit 0 = all checks pass, 1 = any failure.

use std::sync::Arc;

use crate::config::{Config, ProbeType, Target};
use crate::detect::{Detector, apply_cmd};
use crate::probe::{self, Probe};
use crate::report::{build_ctx, csv, html, manifest};
use crate::store::Store;

pub async fn run(offline: bool) -> anyhow::Result<i32> {
    let temp = std::env::temp_dir().join(format!("urania-selftest-{}", std::process::id()));
    std::fs::create_dir_all(&temp)?;
    let db_path = temp.join("selftest.db");
    let _ = std::fs::remove_file(&db_path);

    let targets: Vec<Target> = if offline {
        // Air-gapped: a local TCP dummy on an ephemeral port. The
        // accept loop keeps the listener alive for the probe round
        // (connections are dropped immediately).
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        std::thread::spawn(move || for _ in listener.incoming() {});
        vec![Target {
            name: "st-tcp-local".into(),
            probe_type: ProbeType::Tcp,
            endpoint: format!("127.0.0.1:{port}"),
            interval: 5,
            timeout: 5,
        }]
    } else {
        vec![
            Target {
                name: "st-dns".into(),
                probe_type: ProbeType::Dns,
                endpoint: "1.1.1.1".into(),
                interval: 5,
                timeout: 5,
            },
            Target {
                name: "st-tcp".into(),
                probe_type: ProbeType::Tcp,
                endpoint: "1.1.1.1:443".into(),
                interval: 5,
                timeout: 5,
            },
            Target {
                name: "st-http".into(),
                probe_type: ProbeType::Http,
                endpoint: "https://connectivitycheck.gstatic.com/generate_204".into(),
                interval: 10,
                timeout: 10,
            },
        ]
    };

    println!(
        "urania selftest: probing {}…",
        if offline {
            "local dummy (offline)"
        } else {
            "known-good targets"
        }
    );
    let probes: Vec<Arc<dyn Probe>> = targets
        .iter()
        .filter_map(|t| probe::build_probe(t, &None))
        .collect();
    if probes.is_empty() {
        println!("selftest: FAIL — no probes could be built");
        return Ok(1);
    }
    let results = probe::run_round(&probes).await;
    for r in &results {
        let lat = r
            .latency_ms
            .map(|l| format!("{l:.1}ms"))
            .unwrap_or_else(|| "-".into());
        println!(
            "  {:<10} {:<5} ok={} latency={} detail={}",
            r.target, r.probe_type, r.ok, lat, r.detail
        );
    }
    let all_ok = results.iter().all(|r| r.ok);

    // Store round-trip + detector (should produce zero windows).
    let mut store = Store::open(&db_path)?;
    store.insert_probes(&results)?;
    let mut detector = Detector::new(3, &targets, None, &[]);
    let mut cmds = Vec::new();
    for r in &results {
        cmds.extend(detector.feed(r.clone(), &store));
    }
    for cmd in &cmds {
        apply_cmd(&store, cmd)?;
    }
    let zero_windows = store.outages_in(0, i64::MAX)?.is_empty();

    // Render the report from the store.
    let cfg = Config {
        targets: targets.clone(),
        ..Default::default()
    };
    let ctx = build_ctx(&cfg, &store, 0, i64::MAX, "selftest", None)?;
    let html_text = html::render_html(&ctx);
    let html_path = temp.join("selftest.html");
    std::fs::write(&html_path, &html_text)?;
    let csv_path = temp.join("selftest.csv");
    csv::write_csv(&csv_path, &store.probes_in(0, i64::MAX)?)?;

    // Manifest round-trip: write + verify the evidence integrity manifest.
    let manifest_ok = match manifest::write_manifest(&[html_path.clone(), csv_path.clone()]) {
        Ok(mp) => manifest::verify_manifest(&mp)
            .map(|res| res.iter().all(|(_, ok)| *ok))
            .unwrap_or(false),
        Err(_) => false,
    };

    let checks = vec![
        ("all probes ok", all_ok),
        ("0 outage windows", zero_windows),
        (
            "html header",
            html_text.contains("urania") && html_text.contains("network evidence report"),
        ),
        (
            "sparkline present",
            html_text.contains("<svg") && html_text.contains("polyline"),
        ),
        (
            "target rows",
            targets.iter().all(|t| html_text.contains(&t.name)),
        ),
        (
            "no external urls",
            !html_text.contains("src=") && !html_text.contains("href=http"),
        ),
        ("manifest verify", manifest_ok),
    ];
    let mut pass = true;
    for (name, ok) in &checks {
        println!("  [{}] {name}", if *ok { "PASS" } else { "FAIL" });
        if !ok {
            pass = false;
        }
    }
    println!(
        "{}",
        if pass {
            "selftest: PASS"
        } else {
            "selftest: FAIL"
        }
    );
    Ok(if pass { 0 } else { 1 })
}
