//! Report rendering: self-contained HTML, deterministic CSV, SHA-256 manifest.

pub mod csv;
pub mod html;
pub mod manifest;

use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::store::{OutageRow, ProbeRow, Store};
use crate::util::now_ms;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    H1,
    H24,
    D7,
    D30,
    D90,
}

impl Period {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "1h" => Some(Self::H1),
            "24h" => Some(Self::H24),
            "7d" => Some(Self::D7),
            "30d" => Some(Self::D30),
            "90d" => Some(Self::D90),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::H1 => "1h",
            Self::H24 => "24h",
            Self::D7 => "7d",
            Self::D30 => "30d",
            Self::D90 => "90d",
        }
    }

    pub fn ms(self) -> i64 {
        match self {
            Self::H1 => 3_600_000,
            Self::H24 => 86_400_000,
            Self::D7 => 7 * 86_400_000,
            Self::D30 => 30 * 86_400_000,
            Self::D90 => 90 * 86_400_000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReportOpts {
    pub since: Period,
    pub out_html: PathBuf,
    pub out_csv: Option<PathBuf>,
    pub manifest: bool,
}

#[derive(Debug, Clone)]
pub struct RenderSummary {
    pub html: PathBuf,
    pub csv: Option<PathBuf>,
    pub manifest: Option<PathBuf>,
    pub probes: usize,
    pub outages: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    #[error("no probe data in the selected period")]
    NoData,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("store: {0}")]
    Store(#[from] anyhow::Error),
}

/// Period + labeling parameters for HTML context construction.
struct CtxMeta<'a> {
    from_ms: i64,
    to_ms: i64,
    period_label: &'a str,
    manifest_name: Option<String>,
}

/// Build the full HTML context from the store (shared by `report` and `selftest`).
pub fn build_ctx(
    cfg: &Config,
    store: &Store,
    from_ms: i64,
    to_ms: i64,
    period_label: &str,
    manifest_name: Option<String>,
) -> Result<html::HtmlCtx, ReportError> {
    let rows = store.probes_in(from_ms, to_ms)?;
    let outages = store.outages_in(from_ms, to_ms)?;
    build_ctx_fetched(
        cfg,
        store,
        rows,
        outages,
        CtxMeta {
            from_ms,
            to_ms,
            period_label,
            manifest_name,
        },
    )
}

/// Build the HTML context from already-fetched rows: one table scan per
/// render, and the rows are partitioned into targets in a single pass
/// (no per-target filter+clone over the full set).
fn build_ctx_fetched(
    cfg: &Config,
    store: &Store,
    rows: Vec<ProbeRow>,
    outages: Vec<OutageRow>,
    meta: CtxMeta<'_>,
) -> Result<html::HtmlCtx, ReportError> {
    let CtxMeta {
        from_ms,
        to_ms,
        period_label,
        manifest_name,
    } = meta;
    let names: Vec<&str> = cfg.targets.iter().map(|t| t.name.as_str()).collect();
    let mut per_target: Vec<Vec<ProbeRow>> = vec![Vec::new(); cfg.targets.len()];
    for r in rows {
        if let Some(i) = names.iter().position(|n| *n == r.target) {
            per_target[i].push(r);
        }
    }
    let mut targets = Vec::new();
    let mut data = Vec::new();
    for (t, trows) in cfg.targets.iter().zip(per_target) {
        let reference_only = t.is_gateway() && !store.ever_ok(&t.name)?;
        targets.push(html::TargetInfo {
            name: t.name.clone(),
            probe_type: t.probe_type,
            endpoint: t.endpoint.clone(),
            interval: t.interval,
            reference_only,
        });
        data.push(html::TargetData {
            name: t.name.clone(),
            rows: trows,
        });
    }
    let clock_note = store.get_meta("clock_note")?;
    Ok(html::HtmlCtx {
        version: env!("CARGO_PKG_VERSION").to_string(),
        from_ms,
        to_ms,
        period_label: period_label.to_string(),
        generated_ms: now_ms(),
        host: crate::util::host_string(),
        targets,
        data,
        outages,
        clock_note,
        retention_days: cfg.retention_days,
        manifest_name,
    })
}

/// Render the report files. The HTML bytes are final before the manifest is
/// computed (the footer references the manifest file name, not its hash).
pub fn render(
    cfg: &Config,
    store: &Store,
    opts: &ReportOpts,
) -> Result<RenderSummary, ReportError> {
    let to = now_ms();
    let from = to - opts.since.ms();
    let rows = store.probes_in(from, to)?;
    if rows.is_empty() {
        return Err(ReportError::NoData);
    }
    let outages = store.outages_in(from, to)?;

    let manifest_name = if opts.manifest {
        let file = opts
            .out_html
            .file_name()
            .ok_or_else(|| {
                ReportError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "no filename",
                ))
            })?
            .to_string_lossy()
            .to_string();
        Some(format!("{file}.sha256"))
    } else {
        None
    };

    // One scan feeds everything: record the counts, write the CSV, then
    // hand the rows to the HTML context builder (which takes ownership).
    let probe_count = rows.len();
    let outage_count = outages.len();

    let mut csv_path = None;
    if let Some(p) = &opts.out_csv {
        write_parent(p)?;
        csv::write_csv(p, &rows)?;
        csv_path = Some(p.clone());
    }

    let ctx = build_ctx_fetched(
        cfg,
        store,
        rows,
        outages,
        CtxMeta {
            from_ms: from,
            to_ms: to,
            period_label: opts.since.label(),
            manifest_name,
        },
    )?;
    let html_text = html::render_html(&ctx);
    write_parent(&opts.out_html)?;
    std::fs::write(&opts.out_html, &html_text)?;

    let mut manifest_path = None;
    if opts.manifest {
        let mut files = vec![opts.out_html.clone()];
        if let Some(c) = &csv_path {
            files.push(c.clone());
        }
        let mp = manifest::write_manifest(&files)?;
        manifest_path = Some(mp);
    }

    Ok(RenderSummary {
        html: opts.out_html.clone(),
        csv: csv_path,
        manifest: manifest_path,
        probes: probe_count,
        outages: outage_count,
    })
}

fn write_parent(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ProbeType, Target};
    use crate::store::Store;

    fn prow(target: &str, ts: i64, ok: bool) -> ProbeRow {
        ProbeRow {
            ts,
            ts_mono_delta_ms: None,
            target: target.into(),
            probe_type: "dns".into(),
            ok,
            latency_ms: ok.then_some(10.0),
            detail: String::new(),
        }
    }

    #[test]
    fn period_parse_roundtrip() {
        for (s, ms) in [
            ("1h", 3_600_000i64),
            ("24h", 86_400_000),
            ("7d", 7 * 86_400_000),
            ("30d", 30 * 86_400_000),
            ("90d", 90 * 86_400_000),
        ] {
            let p = Period::parse(s).unwrap_or_else(|| panic!("parse {s}"));
            assert_eq!(p.label(), s);
            assert_eq!(p.ms(), ms);
        }
        assert_eq!(Period::parse("2h"), None);
        assert_eq!(Period::parse("91d"), None);
    }

    #[test]
    fn build_ctx_partitions_rows_single_pass() {
        let store = Store::open_in_memory().unwrap();
        let cfg = Config {
            targets: vec![
                Target {
                    name: "a".into(),
                    probe_type: ProbeType::Dns,
                    endpoint: "1.1.1.1".into(),
                    interval: 5,
                    timeout: 5,
                },
                Target {
                    name: "b".into(),
                    probe_type: ProbeType::Tcp,
                    endpoint: "1.1.1.1:443".into(),
                    interval: 5,
                    timeout: 5,
                },
            ],
            ..Default::default()
        };
        // Interleaved targets, an unknown (renamed) target, unsorted ts:
        // per-target input order must be preserved and unknown rows dropped.
        let rows = vec![
            prow("a", 3000, true),
            prow("b", 2000, false),
            prow("legacy", 1500, true),
            prow("a", 1000, true),
            prow("b", 4000, true),
        ];
        let ctx = build_ctx_fetched(
            &cfg,
            &store,
            rows,
            vec![],
            CtxMeta {
                from_ms: 0,
                to_ms: 10_000,
                period_label: "test",
                manifest_name: None,
            },
        )
        .unwrap();
        assert_eq!(ctx.data.len(), 2);
        let a_ts: Vec<i64> = ctx.data[0].rows.iter().map(|r| r.ts).collect();
        let b_ts: Vec<i64> = ctx.data[1].rows.iter().map(|r| r.ts).collect();
        assert_eq!(a_ts, vec![3000, 1000]);
        assert_eq!(b_ts, vec![2000, 4000]);
        assert!(
            ctx.data
                .iter()
                .all(|d| d.rows.iter().all(|r| r.target != "legacy"))
        );
    }
}
