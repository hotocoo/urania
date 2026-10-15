//! Self-contained HTML report: inline SVG sparklines, zero JavaScript,
//! zero external requests (roadmap §11.6).

use crate::config::ProbeType;
use crate::detect::attr;
use crate::store::{OutageRow, ProbeRow};
use crate::util::{fmt_duration, fmt_ts};

pub struct TargetInfo {
    pub name: String,
    pub probe_type: ProbeType,
    pub endpoint: String,
    pub interval: u64,
    /// Gateway that never responded in the full history: rendered without an
    /// availability or outage claim (silence is its baseline state).
    pub reference_only: bool,
}

pub struct TargetData {
    pub name: String,
    pub rows: Vec<ProbeRow>,
}

pub struct HtmlCtx {
    pub version: String,
    pub from_ms: i64,
    pub to_ms: i64,
    pub period_label: String,
    pub generated_ms: i64,
    pub host: String,
    pub targets: Vec<TargetInfo>,
    pub data: Vec<TargetData>,
    pub outages: Vec<OutageRow>,
    pub clock_note: Option<String>,
    pub retention_days: u32,
    /// Manifest file name (shown in the footer), if requested.
    pub manifest_name: Option<String>,
    /// Build commit (shown in header + footer) for evidence provenance.
    pub build_hash: Option<String>,
    /// Build that wrote the data (db `tool_version` meta). Shown when it
    /// differs from the rendering build — or always, since it is the
    /// provenance of the evidence itself.
    pub collected_by: Option<String>,
}

/// Availability percentage over the rows (100.0 for empty input).
pub fn availability_pct(rows: &[ProbeRow]) -> f64 {
    if rows.is_empty() {
        return 100.0;
    }
    let ok = rows.iter().filter(|r| r.ok).count() as f64;
    ok / rows.len() as f64 * 100.0
}

/// (avg, min, max, jitter) latency in ms over successful rows; zeros if none.
pub fn latency_stats(rows: &[ProbeRow]) -> (f64, f64, f64, f64) {
    let lats: Vec<f64> = rows.iter().filter_map(|r| r.latency_ms).collect();
    if lats.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let n = lats.len() as f64;
    let avg = lats.iter().sum::<f64>() / n;
    let min = lats.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = lats.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    // jitter: mean absolute deviation
    let jitter = lats.iter().map(|l| (l - avg).abs()).sum::<f64>() / n;
    (avg, min, max, jitter)
}

/// Count rows whose monotonic delta deviates >10% from the expected interval
/// (possible clock adjustment while running).
pub fn drift_count(rows: &[ProbeRow], expected_ms: i64) -> usize {
    if expected_ms <= 0 {
        return 0;
    }
    rows.iter()
        .filter(|r| {
            r.ts_mono_delta_ms
                .map(|d| d > 0 && (d as f64 - expected_ms as f64).abs() / expected_ms as f64 > 0.10)
                .unwrap_or(false)
        })
        .count()
}

/// Sparkline: one bucket per ~1/96 of the period (max 96 points); failed
/// buckets render as red bars.
pub fn sparkline_svg(samples: &[(f64, bool)], w: u32, h: u32) -> String {
    const MAX_POINTS: usize = 96;
    if samples.is_empty() {
        return String::new();
    }
    let n = samples.len();
    let bucket = n.div_ceil(MAX_POINTS);
    let mut buckets: Vec<(f64, bool)> = Vec::new();
    for chunk in samples.chunks(bucket) {
        let vals: Vec<f64> = chunk.iter().map(|(v, _)| *v).collect();
        let any_fail = chunk.iter().any(|(_, ok)| !ok);
        let avg = vals.iter().sum::<f64>() / vals.len() as f64;
        buckets.push((avg, any_fail));
    }
    let max = buckets.iter().map(|(v, _)| *v).fold(1.0_f64, f64::max);
    let bw = w as f64 / buckets.len() as f64;
    let mut bars = String::new();
    let mut points = Vec::new();
    for (i, (v, fail)) in buckets.iter().enumerate() {
        let x = i as f64 * bw;
        let bh = (v / max * (h as f64 - 4.0)).max(1.0);
        let y = h as f64 - bh;
        if *fail {
            bars.push_str(&format!(
                "<rect x=\"{:.1}\" y=\"1\" width=\"{:.1}\" height=\"{}\" class=\"fail\"/>",
                x,
                (bw - 0.5).max(0.5),
                h - 1
            ));
        }
        points.push(format!("{:.1},{:.1}", x + bw / 2.0, y));
    }
    format!(
        "<svg width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" xmlns=\"http://www.w3.org/2000/svg\" role=\"img\">{bars}<polyline points=\"{}\" fill=\"none\" stroke=\"#0969da\" stroke-width=\"1.5\"/></svg>",
        points.join(" ")
    )
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// One-line target description for the header (resolver provenance for dns).
pub fn target_line(t: &TargetInfo) -> String {
    match t.probe_type {
        ProbeType::Dns => format!(
            "dns→{} (resolver {}, TCP/53, {}s)",
            t.name, t.endpoint, t.interval
        ),
        ProbeType::Tcp if t.endpoint.starts_with("auto:") => {
            format!("{}→{} (auto, {}s)", t.name, t.endpoint, t.interval)
        }
        ProbeType::Tcp => format!("{}→{} ({}s)", t.name, t.endpoint, t.interval),
        ProbeType::Http => format!("{}→{} ({}s)", t.name, t.endpoint, t.interval),
        ProbeType::Icmp => format!("{}→{} ({}s)", t.name, t.endpoint, t.interval),
    }
}

pub fn render_html(ctx: &HtmlCtx) -> String {
    let mut s = String::with_capacity(64 * 1024);
    s.push_str("<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n");
    s.push_str(&format!(
        "<title>urania report — {}</title>\n",
        esc(&ctx.period_label)
    ));
    s.push_str(
        "<style>\n\
         body { font-family: -apple-system, BlinkMacSystemFont, \"Segoe UI\", Roboto, Helvetica, Arial, sans-serif; margin: 2rem auto; max-width: 900px; color: #1f2328; line-height: 1.5; }\n\
         h1 { font-size: 1.4rem; }\n\
         h2 { font-size: 1.1rem; margin-top: 2rem; }\n\
         dl { display: grid; grid-template-columns: 10rem 1fr; gap: 0.25rem 1rem; margin: 1rem 0; }\n\
         dt { font-weight: 600; color: #57606a; }\n\
         table { border-collapse: collapse; width: 100%; margin: 0.75rem 0; font-size: 0.9rem; }\n\
         th, td { border: 1px solid #d0d7de; padding: 0.35rem 0.6rem; text-align: left; }\n\
         th { background: #f6f8fa; }\n\
         .fail { fill: #f85149; opacity: 0.6; }\n\
         footer { margin-top: 2rem; color: #57606a; font-size: 0.85rem; }\n\
         code { background: #f6f8fa; padding: 0.1rem 0.3rem; border-radius: 4px; }\n\
         </style>\n</head>\n<body>\n",
    );

    // Header
    s.push_str("<header>\n<h1>urania v");
    s.push_str(&esc(&ctx.version));
    s.push_str(" — network evidence report</h1>\n<dl>\n");
    s.push_str(&format!(
        "<dt>Period</dt><dd>{} → {} ({})</dd>\n",
        fmt_ts(ctx.from_ms),
        fmt_ts(ctx.to_ms),
        esc(&ctx.period_label)
    ));
    s.push_str(&format!(
        "<dt>Generated</dt><dd>{}</dd>\n",
        fmt_ts(ctx.generated_ms)
    ));
    if let Some(h) = &ctx.build_hash {
        s.push_str(&format!("<dt>Build</dt><dd>{h}</dd>\n"));
    }
    if let Some(v) = &ctx.collected_by {
        s.push_str(&format!(
            "<dt>Collected by</dt><dd>urania v{}</dd>\n",
            esc(v)
        ));
    }
    s.push_str(&format!("<dt>Host</dt><dd>{}</dd>\n", esc(&ctx.host)));
    let target_lines: Vec<String> = ctx.targets.iter().map(target_line).collect();
    s.push_str(&format!(
        "<dt>Targets</dt><dd>{}</dd>\n",
        esc(&target_lines.join("; "))
    ));
    let clock = ctx
        .clock_note
        .clone()
        .unwrap_or_else(|| "NTP: unknown".into());
    s.push_str(&format!(
        "<dt>Clock</dt><dd>wall clock + monotonic delta per probe; {clock}</dd>\n",
        clock = esc(&clock)
    ));
    s.push_str(&format!(
        "<dt>Retention</dt><dd>{} days</dd>\n",
        ctx.retention_days
    ));
    s.push_str("</dl>\n</header>\n");

    // Period summary — the 10-second read for a ticket reviewer
    s.push_str("<section>\n<h2>Period summary</h2>\n");
    s.push_str("<table>\n<tr><th>Target</th><th>Probes</th><th>OK</th><th>Failed</th><th>Availability</th><th>Outages</th><th>Coverage</th></tr>\n");
    for (info, td) in ctx.targets.iter().zip(ctx.data.iter()) {
        let ok = td.rows.iter().filter(|r| r.ok).count();
        let failed = td.rows.len() - ok;
        let avail = if td.rows.is_empty() {
            "—".to_string()
        } else if info.reference_only {
            "ref-only".to_string()
        } else {
            format!("{:.2}%", availability_pct(&td.rows))
        };
        let outages = ctx.outages.iter().filter(|w| w.target == td.name).count();
        // Coverage: probes recorded vs expected for the period. Gaps mean
        // the logger was not running (sleep, suspend, stop) — stated
        // honestly instead of discovered by the reader.
        let expected = (ctx.to_ms - ctx.from_ms) / (info.interval as i64 * 1000);
        let coverage = if td.rows.is_empty() || expected <= 0 {
            "—".to_string()
        } else {
            let pct = (td.rows.len() as f64 / expected as f64 * 100.0).min(100.0);
            format!("{}/{expected} ({pct:.1}%)", td.rows.len())
        };
        s.push_str(&format!(
            "<tr><td>{} <code>{}</code></td><td>{}</td><td>{ok}</td><td>{failed}</td><td>{avail}</td><td>{outages}</td><td>{coverage}</td></tr>\n",
            esc(&td.name),
            esc(&target_line(info)),
            td.rows.len()
        ));
    }
    s.push_str("</table>\n");
    s.push_str("<p>Coverage = probes recorded ÷ expected for the period (interval × period length). Below 100% means the logger was not running for part of the period (sleep, suspend, stop).</p>\n");
    s.push_str("</section>\n");

    // Per-target sections
    for (info, td) in ctx.targets.iter().zip(ctx.data.iter()) {
        s.push_str("<section>\n<h2>");
        s.push_str(&esc(&td.name));
        if td.rows.is_empty() {
            s.push_str(" — no data in period</h2>\n</section>\n");
            continue;
        }
        if info.reference_only {
            let n = td.rows.len();
            s.push_str(" — reference probe (no outage claims)</h2>\n");
            s.push_str(&format!(
                "<p>No TCP response from the gateway in this period ({n} probes, 0 responses). Many routers do not answer TCP 80/443, so silence is its baseline state: this target only attributes outages to local link vs upstream, and no availability or outage is claimed for it.</p>\n"
            ));
            s.push_str("</section>\n");
            continue;
        }
        let avail = availability_pct(&td.rows);
        s.push_str(&format!(" — availability {avail:.2}%</h2>\n"));
        let samples: Vec<(f64, bool)> = td
            .rows
            .iter()
            .map(|r| (r.latency_ms.unwrap_or(0.0), r.ok))
            .collect();
        let svg = sparkline_svg(&samples, 860, 60);
        if !svg.is_empty() {
            s.push_str(&svg);
            s.push('\n');
        }
        let (avg, min, max, jitter) = latency_stats(&td.rows);
        s.push_str(&format!(
            "<p>latency avg {avg:.1}ms · min {min:.1}ms · max {max:.1}ms · jitter {jitter:.1}ms</p>\n"
        ));
        let drift = drift_count(&td.rows, info.interval as i64 * 1000);
        if drift > 0 {
            s.push_str(&format!(
                "<p>⚠ {drift} probe(s) deviate &gt;10% from the expected interval (possible clock adjustment)</p>\n"
            ));
        }
        s.push_str("</section>\n");
    }

    // Outages
    s.push_str("<section>\n<h2>Outages</h2>\n");
    if ctx.outages.is_empty() {
        s.push_str("<p>none in period</p>\n");
    } else {
        s.push_str("<table>\n<tr><th>Start</th><th>Duration</th><th>Target</th><th>Layers</th><th>Cause</th><th>Location</th><th>Flap</th></tr>\n");
        for o in &ctx.outages {
            let dur = match o.end_ts {
                Some(end) => fmt_duration(end - o.start_ts),
                None => format!("{} (ongoing)", fmt_duration(ctx.generated_ms - o.start_ts)),
            };
            let cause = o.cause.clone().unwrap_or_else(|| "—".into());
            let layers = if o.layers.is_empty() {
                "—".to_string()
            } else {
                format!("{} · {}", attr::attribution_class_str(&o.layers), o.layers)
            };
            s.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td><code>{}</code></td><td>{}</td><td>{}</td></tr>\n",
                fmt_ts(o.start_ts),
                dur,
                esc(&o.target),
                esc(&layers),
                esc(&cause),
                o.location_class.as_deref().map(esc).unwrap_or_else(|| "unknown".into()),
                if o.flap { "yes" } else { "no" }
            ));
        }
        s.push_str("</table>\n");
    }
    s.push_str("</section>\n");

    // Footer
    s.push_str("<footer>\nself-contained artifact · generated by urania v");
    s.push_str(&esc(&ctx.version));
    if let Some(h) = &ctx.build_hash {
        s.push_str(&format!(" ({h})"));
    }
    if let Some(m) = &ctx.manifest_name {
        s.push_str(&format!(" · sha256 manifest: {}", esc(m)));
    }
    s.push_str("\n</footer>\n</body>\n</html>\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::OutageRow as ORow;

    fn row(target: &str, ts: i64, ok: bool, lat: Option<f64>) -> ProbeRow {
        ProbeRow {
            ts,
            ts_mono_delta_ms: None,
            target: target.into(),
            probe_type: "dns".into(),
            ok,
            latency_ms: lat,
            detail: String::new(),
        }
    }

    fn ctx_with(rows: Vec<ProbeRow>, outages: Vec<ORow>) -> HtmlCtx {
        HtmlCtx {
            version: "0.1.0".into(),
            from_ms: 0,
            to_ms: 100_000,
            period_label: "24h".into(),
            generated_ms: 100_000,
            host: "testhost, macos aarch64".into(),
            targets: vec![TargetInfo {
                name: "dns-cloudflare".into(),
                probe_type: ProbeType::Dns,
                endpoint: "1.1.1.1".into(),
                interval: 5,
                reference_only: false,
            }],
            data: vec![TargetData {
                name: "dns-cloudflare".into(),
                rows,
            }],
            outages,
            clock_note: None,
            retention_days: 90,
            manifest_name: None,
            build_hash: None,
            collected_by: None,
        }
    }

    #[test]
    fn availability_and_latency_stats() {
        let rows: Vec<ProbeRow> = (0..100)
            .map(|i| row("t", i * 1000, i % 10 != 0, Some(10.0 + (i % 5) as f64)))
            .collect();
        let avail = availability_pct(&rows);
        assert!((avail - 90.0).abs() < 0.001);
        let (avg, min, max, jitter) = latency_stats(&rows);
        assert!(min > 0.0 && min <= avg && avg <= max);
        assert!(jitter >= 0.0);
        assert_eq!(availability_pct(&[]), 100.0);
    }

    #[test]
    fn sparkline_capped_at_96_points() {
        let samples: Vec<(f64, bool)> = (0..500)
            .map(|i| (10.0 + (i % 7) as f64, i % 50 != 0))
            .collect();
        let svg = sparkline_svg(&samples, 600, 60);
        let points = svg
            .split("points=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .to_string();
        let count = points.split_whitespace().count();
        assert!(count <= 96, "got {count} points");
        assert!(svg.contains("class=\"fail\""));
        assert!(sparkline_svg(&[], 600, 60).is_empty());
    }

    #[test]
    fn full_report_no_external_urls_and_has_sections() {
        let rows: Vec<ProbeRow> = (0..20)
            .map(|i| row("dns-cloudflare", i * 5000, i != 3, Some(10.0)))
            .collect();
        let outages = vec![ORow {
            start_ts: 15_000,
            end_ts: Some(25_000),
            target: "dns-cloudflare".into(),
            layers: "dns".into(),
            cause: Some("dns:query-timeout@1.1.1.1".into()),
            location_class: Some("local".into()),
            flap: false,
        }];
        let html = render_html(&ctx_with(rows, outages));
        assert!(!html.contains("src="), "external src found");
        assert!(!html.contains("href=http"), "external href found");
        assert!(html.contains("urania v0.1.0"));
        // No build hash / collected-by in this synthetic ctx.
        assert!(!html.contains("<dt>Build</dt>"));
        assert!(!html.contains("<dt>Collected by</dt>"));
        assert!(html.contains("network evidence report"));
        assert!(html.contains("resolver 1.1.1.1, TCP/53"));
        assert!(html.contains("Outages"));
        assert!(html.contains("dns:query-timeout@1.1.1.1"));
        assert!(html.contains("local"));
    }

    #[test]
    fn provenance_lines_when_present() {
        let rows: Vec<ProbeRow> = (0..10)
            .map(|i| row("t", i * 1000, true, Some(5.0)))
            .collect();
        let mut ctx = ctx_with(rows, vec![]);
        ctx.build_hash = Some("abc1234".into());
        ctx.collected_by = Some("0.0.9 (deadbee)".into());
        let html = render_html(&ctx);
        assert!(html.contains("<dt>Build</dt><dd>abc1234</dd>"));
        assert!(html.contains("<dt>Collected by</dt><dd>urania v0.0.9 (deadbee)</dd>"));
        // Footer carries the rendering build.
        assert!(html.contains("generated by urania v0.1.0 (abc1234)"));
    }

    #[test]
    fn reference_only_gateway_has_no_availability_claim() {
        let rows: Vec<ProbeRow> = (0..50)
            .map(|i| row("gateway", i * 5000, false, None))
            .collect();
        let mut ctx = ctx_with(rows, vec![]);
        ctx.targets[0].name = "gateway".into();
        ctx.targets[0].probe_type = ProbeType::Tcp;
        ctx.targets[0].endpoint = "auto:443,80".into();
        ctx.targets[0].reference_only = true;
        ctx.data[0].name = "gateway".into();
        let html = render_html(&ctx);
        assert!(html.contains("reference probe (no outage claims)"));
        assert!(html.contains("50 probes, 0 responses"));
        assert!(!html.contains("availability 0.00%"));
        assert!(!html.contains("<svg"), "no sparkline for reference-only");
    }

    #[test]
    fn drift_counted_when_mono_delta_deviates() {
        let mut rows: Vec<ProbeRow> = (0..10)
            .map(|i| row("t", i * 5000, true, Some(10.0)))
            .collect();
        for r in &mut rows {
            r.ts_mono_delta_ms = Some(5000);
        }
        assert_eq!(drift_count(&rows, 5000), 0);
        rows[5].ts_mono_delta_ms = Some(9000); // +80%
        assert_eq!(drift_count(&rows, 5000), 1);
    }

    #[test]
    fn period_summary_table_counts() {
        // 10 probes, 2 failed (i==3, i==7) → 80.00% availability, 1 outage.
        let rows: Vec<ProbeRow> = (0..10)
            .map(|i| row("dns-cloudflare", i * 5000, i != 3 && i != 7, Some(10.0)))
            .collect();
        let outages = vec![ORow {
            start_ts: 15_000,
            end_ts: Some(25_000),
            target: "dns-cloudflare".into(),
            layers: "dns".into(),
            cause: None,
            location_class: None,
            flap: false,
        }];
        let html = render_html(&ctx_with(rows, outages));
        assert!(html.contains("Period summary"));
        let row_line = html
            .lines()
            .find(|l| l.contains("dns-cloudflare") && l.contains("<td>10</td>"))
            .expect("summary row");
        assert!(row_line.contains("80.00%"));
        assert!(row_line.contains("<td>10</td><td>8</td><td>2</td>"));
        // 10 probes over 100 s at a 5 s interval → 20 expected
        assert!(row_line.ends_with("<td>1</td><td>10/20 (50.0%)</td></tr>"));
    }

    #[test]
    fn period_summary_empty_target_and_reference_only() {
        let mut ctx = ctx_with(vec![], vec![]);
        // second target: reference-only gateway, 5 failed probes
        ctx.targets.push(TargetInfo {
            name: "gateway".into(),
            probe_type: ProbeType::Tcp,
            endpoint: "auto:443,80".into(),
            interval: 5,
            reference_only: true,
        });
        ctx.data.push(TargetData {
            name: "gateway".into(),
            rows: (0..5)
                .map(|i| row("gateway", i * 5000, false, None))
                .collect(),
        });
        let html = render_html(&ctx);
        let lines: Vec<&str> = html.lines().collect();
        let empty = lines
            .iter()
            .find(|l| l.contains("dns-cloudflare") && l.contains("<td>0</td>"))
            .expect("empty target row");
        assert!(empty.contains("<td>—</td>"));
        let ref_only = lines
            .iter()
            .find(|l| l.contains("gateway") && l.contains("<td>5</td>"))
            .expect("reference-only row");
        assert!(ref_only.contains("ref-only"));
        assert!(
            !ref_only.contains("%.00%</td>"),
            "no availability % for reference-only"
        );
        // coverage is still shown: 5 probes over 100 s at 5 s → 5/20 (25.0%)
        assert!(ref_only.contains("5/20 (25.0%)"));
    }
}
