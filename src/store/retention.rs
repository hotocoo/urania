//! 90-day retention pruning (roadmap §6.3).

use rusqlite::{Connection, params};

use crate::util::now_ms;

pub fn prune(conn: &Connection, retention_days: u32) -> anyhow::Result<u32> {
    let cutoff = now_ms() - retention_days as i64 * 86_400_000;
    let n_probes = conn.execute("DELETE FROM probes WHERE ts < ?1", params![cutoff])?;
    // Closed outages fully before the cutoff.
    conn.execute(
        "DELETE FROM outages WHERE end_ts IS NOT NULL AND end_ts < ?1",
        params![cutoff],
    )?;
    // Still-open outages whose target has no probes left in the window.
    conn.execute(
        "DELETE FROM outages WHERE end_ts IS NULL
         AND target NOT IN (SELECT DISTINCT target FROM probes WHERE ts >= ?1)",
        params![cutoff],
    )?;
    let _ = conn.execute_batch("PRAGMA incremental_vacuum;");
    Ok(n_probes as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProbeType;
    use crate::probe::ProbeResult;
    use crate::store::Store;

    fn row(target: &str, ts: i64, ok: bool) -> ProbeResult {
        ProbeResult {
            ts_wall_ms: ts,
            ts_mono_delta_ms: 0,
            target: target.into(),
            probe_type: ProbeType::Dns,
            ok,
            latency_ms: ok.then_some(10.0),
            detail: String::new(),
        }
    }

    #[test]
    fn prune_keeps_recent_drops_old() {
        let mut store = Store::open_in_memory().unwrap();
        let now = now_ms();
        let mut rows = Vec::new();
        for i in 0..100 {
            rows.push(row("t", now - (99 - i) * 86_400_000, true)); // 99 days ago → today
        }
        store.insert_probes(&rows).unwrap();
        let n = store.prune(90).unwrap();
        assert_eq!(n, 9);
        assert_eq!(store.probe_count().unwrap(), 91);
    }
}
