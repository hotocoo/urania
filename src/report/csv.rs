//! Deterministic CSV export.

use std::path::Path;

use crate::store::{OutageRow, ProbeRow};
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

/// Outage-window summary: start_iso, end_iso, duration_s, target, layers,
/// cause, location_class, flap. The attachable incident summary for an ISP
/// dispute. Ongoing windows leave end_iso and duration_s empty (the empty
/// end_iso is the marker; a final duration would be misleading).
pub fn write_outages_csv(path: &Path, outages: &[OutageRow]) -> anyhow::Result<()> {
    let mut w = csv::Writer::from_path(path)?;
    w.write_record([
        "start_iso",
        "end_iso",
        "duration_s",
        "target",
        "layers",
        "cause",
        "location_class",
        "flap",
    ])?;
    for o in outages {
        let (end_iso, duration_s) = match o.end_ts {
            Some(end) => (fmt_ts_utc_iso(end), Some((end - o.start_ts) / 1000)),
            None => (String::new(), None),
        };
        w.write_record([
            fmt_ts_utc_iso(o.start_ts).as_str(),
            end_iso.as_str(),
            duration_s.map(|d| d.to_string()).as_deref().unwrap_or(""),
            o.target.as_str(),
            o.layers.as_str(),
            o.cause.as_deref().unwrap_or(""),
            o.location_class.as_deref().unwrap_or(""),
            if o.flap { "1" } else { "0" },
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

    #[test]
    fn outages_csv_closed_and_ongoing() {
        let outages = vec![
            OutageRow {
                start_ts: 1_700_000_000_000,
                end_ts: Some(1_700_000_060_000), // 60s later
                target: "t".into(),
                layers: "tcp".into(),
                cause: Some("timeout".into()),
                location_class: Some("isp".into()),
                flap: false,
            },
            OutageRow {
                start_ts: 1_700_000_100_000,
                end_ts: None, // ongoing
                target: "g".into(),
                layers: "tcp".into(),
                cause: None,
                location_class: None,
                flap: true,
            },
        ];
        let dir = std::env::temp_dir().join(format!("urania-outages-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("outages.csv");
        write_outages_csv(&p, &outages).unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(
            lines[0].starts_with(
                "start_iso,end_iso,duration_s,target,layers,cause,location_class,flap"
            )
        );
        // Closed: duration 60s, cause + location present, flap 0.
        assert!(lines[1].contains(",60,t,tcp,timeout,isp,0"));
        // Ongoing: empty end_iso + duration_s, empty cause/location, flap 1.
        assert!(lines[2].contains(",,g,tcp,,,1"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
