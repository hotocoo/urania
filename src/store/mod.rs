//! SQLite store: schema, inserts, window operations, queries.

pub mod retention;

use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, params};

use crate::probe::ProbeResult;

#[derive(Debug, Clone)]
pub struct ProbeRow {
    pub ts: i64,
    pub ts_mono_delta_ms: Option<i64>,
    pub target: String,
    pub probe_type: String,
    pub ok: bool,
    pub latency_ms: Option<f64>,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct OutageRow {
    pub start_ts: i64,
    pub end_ts: Option<i64>,
    pub target: String,
    pub layers: String,
    pub cause: Option<String>,
    pub location_class: Option<String>,
    pub flap: bool,
}

/// Fields for closing an outage window (roadmap §5.3).
#[derive(Debug, Clone)]
pub struct CloseWindow {
    pub id: i64,
    pub end_ts: i64,
    pub layers: String,
    pub cause: Option<String>,
    pub location_class: String,
    pub flap: bool,
    pub probes_failed: u32,
}

#[derive(Debug, Clone)]
pub struct WindowRow {
    pub id: i64,
    pub start_ts: i64,
    pub target: String,
    pub probes_failed: i64,
}

const PROBE_COLS: &str =
    "ts, ts_mono_delta_ms, target, probe_type, ok, latency_ms, COALESCE(detail, '')";

fn map_probe_row(r: &rusqlite::Row) -> rusqlite::Result<ProbeRow> {
    Ok(ProbeRow {
        ts: r.get(0)?,
        ts_mono_delta_ms: r.get(1)?,
        target: r.get(2)?,
        probe_type: r.get(3)?,
        ok: r.get::<_, i64>(4)? != 0,
        latency_ms: r.get(5)?,
        detail: r.get(6)?,
    })
}

fn map_outage_row(r: &rusqlite::Row) -> rusqlite::Result<OutageRow> {
    Ok(OutageRow {
        start_ts: r.get(0)?,
        end_ts: r.get(1)?,
        target: r.get(2)?,
        layers: r.get(3)?,
        cause: r.get(4)?,
        location_class: r.get(5)?,
        flap: r.get::<_, i64>(6)? != 0,
    })
}

pub struct Store {
    pub(crate) conn: Connection,
}

impl Store {
    /// Open (creating if needed) the database at `path`, applying migrations.
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        Self::init(&conn)?;
        Ok(Store { conn })
    }

    /// In-memory store for tests.
    #[cfg(test)]
    pub fn open_in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init(&conn)?;
        Ok(Store { conn })
    }

    fn init(conn: &Connection) -> anyhow::Result<()> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.busy_timeout(Duration::from_millis(5_000))?;
        let has_meta: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='meta'",
            [],
            |r| r.get(0),
        )?;
        if has_meta == 0 {
            conn.execute_batch(include_str!("schema.sql"))?;
            conn.execute(
                "INSERT INTO meta(key, value) VALUES ('schema_version', '1')",
                [],
            )?;
        }
        Ok(())
    }

    pub fn insert_probes(&mut self, rows: &[ProbeResult]) -> anyhow::Result<()> {
        if rows.is_empty() {
            return Ok(());
        }
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO probes(ts, ts_mono_delta_ms, target, probe_type, ok, latency_ms, detail)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for r in rows {
                stmt.execute(params![
                    r.ts_wall_ms,
                    r.ts_mono_delta_ms,
                    r.target,
                    r.probe_type.to_string(),
                    r.ok,
                    r.latency_ms,
                    r.detail,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn open_window(&self, target: &str, start_ts: i64) -> anyhow::Result<i64> {
        self.conn.execute(
            "INSERT INTO outages(start_ts, target, layers, probes_failed) VALUES (?1, ?2, '', 0)",
            params![start_ts, target],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn close_window(&self, w: &CloseWindow) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE outages SET end_ts=?1, layers=?2, cause=?3, location_class=?4, flap=?5, probes_failed=?6 WHERE id=?7",
            params![w.end_ts, w.layers, w.cause, w.location_class, w.flap, w.probes_failed, w.id],
        )?;
        Ok(())
    }

    pub fn set_meta(&self, key: &str, value: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO meta(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_meta(&self, key: &str) -> anyhow::Result<Option<String>> {
        let v = self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", params![key], |r| {
                r.get::<_, String>(0)
            })
            .optional()?;
        Ok(v)
    }

    pub fn last_probe(&self, target: &str) -> anyhow::Result<Option<ProbeRow>> {
        let row = self
            .conn
            .query_row(
                &format!("SELECT {PROBE_COLS} FROM probes WHERE target = ?1 ORDER BY ts DESC, id DESC LIMIT 1"),
                params![target],
                map_probe_row,
            )
            .optional()?;
        Ok(row)
    }

    /// True if the target ever recorded an ok probe (full history, not just
    /// the retention window). Used for gateway reference-only detection.
    pub fn ever_ok(&self, target: &str) -> anyhow::Result<bool> {
        let found: Option<i64> = self
            .conn
            .query_row(
                "SELECT 1 FROM probes WHERE target = ?1 AND ok = 1 LIMIT 1",
                params![target],
                |r| r.get(0),
            )
            .optional()?;
        Ok(found.is_some())
    }

    pub fn open_windows(&self) -> anyhow::Result<Vec<WindowRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, start_ts, target, probes_failed FROM outages WHERE end_ts IS NULL ORDER BY start_ts",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(WindowRow {
                id: r.get(0)?,
                start_ts: r.get(1)?,
                target: r.get(2)?,
                probes_failed: r.get(3)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn probes_in(&self, from_ms: i64, to_ms: i64) -> anyhow::Result<Vec<ProbeRow>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {PROBE_COLS} FROM probes WHERE ts >= ?1 AND ts <= ?2 ORDER BY ts, id"
        ))?;
        let rows = stmt.query_map(params![from_ms, to_ms], map_probe_row)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Windows overlapping [from_ms, to_ms]: a window counts if it started
    /// before the window's end and is either still open or ended inside it.
    /// (Start-in-period would miss an outage that began before the period
    /// and is still running — the one that matters most.)
    pub fn outages_in(&self, from_ms: i64, to_ms: i64) -> anyhow::Result<Vec<OutageRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT start_ts, end_ts, target, layers, cause, location_class, flap
             FROM outages
             WHERE start_ts <= ?2 AND (end_ts IS NULL OR end_ts >= ?1)
             ORDER BY start_ts",
        )?;
        let rows = stmt.query_map(params![from_ms, to_ms], map_outage_row)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn probe_count(&self) -> anyhow::Result<i64> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM probes", [], |r| r.get(0))?;
        Ok(n)
    }

    /// (ok, total) probe counts for a target since `from_ms`; None if no rows
    /// (SQLite aggregate queries return a (0,0) row for an empty set).
    pub fn probe_counts_since(
        &self,
        target: &str,
        from_ms: i64,
    ) -> anyhow::Result<Option<(i64, i64)>> {
        let r: (i64, i64) = self.conn.query_row(
            "SELECT COALESCE(SUM(ok), 0), COUNT(*) FROM probes
             WHERE target = ?1 AND ts >= ?2",
            params![target, from_ms],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok(if r.1 > 0 { Some(r) } else { None })
    }

    /// Delete data older than the retention window. Returns pruned probe rows.
    pub fn prune(&self, retention_days: u32) -> anyhow::Result<u32> {
        retention::prune(&self.conn, retention_days)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProbeType;
    use crate::probe::ProbeResult;

    fn res(target: &str, ok: bool, ts: i64) -> ProbeResult {
        ProbeResult {
            ts_wall_ms: ts,
            ts_mono_delta_ms: 0,
            target: target.into(),
            probe_type: ProbeType::Tcp,
            ok,
            latency_ms: ok.then_some(10.0),
            detail: String::new(),
        }
    }

    #[test]
    fn ever_ok_tracks_full_history() {
        let mut store = Store::open_in_memory().unwrap();
        assert!(!store.ever_ok("gw").unwrap(), "no data yet");
        store.insert_probes(&[res("gw", false, 1_000)]).unwrap();
        assert!(!store.ever_ok("gw").unwrap(), "failures only");
        store.insert_probes(&[res("gw", true, 2_000)]).unwrap();
        assert!(store.ever_ok("gw").unwrap(), "one ok is enough");
        assert!(!store.ever_ok("other").unwrap());
    }

    #[test]
    fn probe_counts_since_respects_target_and_window() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .insert_probes(&[
                res("a", true, 1_000),
                res("a", false, 2_000),
                res("a", true, 5_000),
                res("b", false, 5_000),
            ])
            .unwrap();
        assert_eq!(store.probe_counts_since("a", 0).unwrap(), Some((2, 3)));
        assert_eq!(store.probe_counts_since("a", 4_000).unwrap(), Some((1, 1)));
        assert_eq!(store.probe_counts_since("b", 4_000).unwrap(), Some((0, 1)));
        assert_eq!(store.probe_counts_since("a", 10_000).unwrap(), None);
        assert_eq!(store.probe_counts_since("missing", 0).unwrap(), None);
    }

    #[test]
    fn outages_in_uses_overlap_semantics() {
        let store = Store::open_in_memory().unwrap();
        // Three windows: closed before the period, overlapping (started
        // before, closed inside), still open.
        store
            .conn
            .execute(
                "INSERT INTO outages(start_ts, end_ts, target, layers, probes_failed) VALUES
                 (100, 200, 'old', '', 0), (800, 5000, 'span', '', 0), (7000, NULL, 'open', '', 0)",
                [],
            )
            .unwrap();
        // Period [4000, 6000]: only 'span' overlaps ('old' ended before,
        // 'open' starts after).
        let o = store.outages_in(4_000, 6_000).unwrap();
        let names: Vec<&str> = o.iter().map(|w| w.target.as_str()).collect();
        assert_eq!(names, vec!["span"]);
        // Period [4000, 8000]: 'span' and the still-open 'open'.
        let o = store.outages_in(4_000, 8_000).unwrap();
        let names: Vec<&str> = o.iter().map(|w| w.target.as_str()).collect();
        assert_eq!(names, vec!["span", "open"]);
        // Period [0, i64::MAX]: everything.
        assert_eq!(store.outages_in(0, i64::MAX).unwrap().len(), 3);
        // Period after all starts: only the open window.
        let o = store.outages_in(8_000, 9_000).unwrap();
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].target, "open");
    }
}
