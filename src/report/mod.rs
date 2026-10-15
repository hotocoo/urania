//! Report rendering: self-contained HTML, deterministic CSV, SHA-256 manifest.

pub mod csv;
pub mod html;
pub mod manifest;

use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::store::Store;
use crate::util::now_ms;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    H24,
    D7,
    D30,
    D90,
}

impl Period {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "24h" => Some(Self::H24),
            "7d" => Some(Self::D7),
            "30d" => Some(Self::D30),
            "90d" => Some(Self::D90),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::H24 => "24h",
            Self::D7 => "7d",
            Self::D30 => "30d",
            Self::D90 => "90d",
        }
    }

    pub fn ms(self) -> i64 {
        match self {
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
    let mut targets = Vec::new();
    let mut data = Vec::new();
    for t in &cfg.targets {
        let trows: Vec<crate::store::ProbeRow> = rows
            .iter()
            .filter(|r| r.target == t.name)
            .cloned()
            .collect();
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

    let ctx = build_ctx(cfg, store, from, to, opts.since.label(), manifest_name)?;
    let html_text = html::render_html(&ctx);
    write_parent(&opts.out_html)?;
    std::fs::write(&opts.out_html, &html_text)?;

    let mut csv_path = None;
    if let Some(p) = &opts.out_csv {
        write_parent(p)?;
        csv::write_csv(p, &rows)?;
        csv_path = Some(p.clone());
    }

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
        probes: rows.len(),
        outages: outages.len(),
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
