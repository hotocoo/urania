//! Deterministic CSV export.

use std::path::Path;

use crate::store::ProbeRow;
use crate::util::fmt_ts_utc_iso;

/// Columns: ts_iso (UTC), target, type, ok, latency_ms, detail.
pub fn write_csv(path: &Path, rows: &[ProbeRow]) -> anyhow::Result<()> {
    let mut w = csv::Writer::from_path(path)?;
    w.write_record(["ts_iso", "target", "type", "ok", "latency_ms", "detail"])?;
    for r in rows {
        let ts = fmt_ts_utc_iso(r.ts);
        let lat = r.latency_ms.map(|l| format!("{l:.3}"));
        let ok = if r.ok { "1" } else { "0" };
        w.write_record([
            ts.as_str(),
            r.target.as_str(),
            r.probe_type.as_str(),
            ok,
            lat.as_deref().unwrap_or(""),
            r.detail.as_str(),
        ])?;
    }
    w.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_columns_and_rows() {
        let rows = vec![ProbeRow {
            ts: 1_700_000_000_000,
            ts_mono_delta_ms: Some(5000),
            target: "t".into(),
            probe_type: "dns".into(),
            ok: true,
            latency_ms: Some(10.5),
            detail: String::new(),
        }];
        let dir = std::env::temp_dir().join(format!("urania-csv-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("out.csv");
        write_csv(&p, &rows).unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("ts_iso,target,type,ok,latency_ms,detail"));
        assert!(lines[1].contains("10.500"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
