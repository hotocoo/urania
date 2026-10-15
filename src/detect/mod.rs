//! Detection: outage windows, attribution, flap, location class.

pub mod attr;
pub mod flap;
pub mod window;

use std::collections::HashMap;

use crate::config::{ProbeType, Target};
use crate::probe::ProbeResult;
use crate::store::{CloseWindow, Store, WindowRow};
use window::{TargetState, WindowBuilder, seed_open_window};

/// Commands the detector asks the caller to apply to the store.
#[derive(Debug)]
pub enum StoreCmd {
    CloseWindow {
        id: i64,
        end_ts: i64,
        layers: String,
        cause: Option<String>,
        location_class: String,
        flap: bool,
        probes_failed: u32,
    },
}

/// Apply a detector command to the store.
pub fn apply_cmd(store: &Store, cmd: &StoreCmd) -> anyhow::Result<()> {
    match cmd {
        StoreCmd::CloseWindow {
            id,
            end_ts,
            layers,
            cause,
            location_class,
            flap,
            probes_failed,
        } => store.close_window(&CloseWindow {
            id: *id,
            end_ts: *end_ts,
            layers: layers.clone(),
            cause: cause.clone(),
            location_class: location_class.clone(),
            flap: *flap,
            probes_failed: *probes_failed,
        }),
    }
}

/// Per-target state machine over probe results (roadmap §5.1).
pub struct Detector {
    fail_threshold: u32,
    states: HashMap<String, TargetState>,
    intervals: HashMap<String, u64>,
    gateway_target: Option<String>,
}

/// One step of the state machine (pure; no store access).
enum Event {
    None,
    Open {
        start_ts: i64,
        probe_type: ProbeType,
        detail: String,
    },
    Close {
        wb: WindowBuilder,
    },
}

fn step(st: &mut TargetState, res: &ProbeResult, threshold: u32) -> Event {
    if res.ok {
        st.consecutive_failures = 0;
        st.first_fail_ts = None;
        match st.open.take() {
            Some(wb) => Event::Close { wb },
            None => Event::None,
        }
    } else {
        st.consecutive_failures = st.consecutive_failures.saturating_add(1);
        if st.first_fail_ts.is_none() {
            st.first_fail_ts = Some(res.ts_wall_ms);
        }
        if let Some(wb) = st.open.as_mut() {
            wb.failing_types.insert(res.probe_type);
            wb.detail_samples.push((res.probe_type, res.detail.clone()));
            wb.probes_failed = wb.probes_failed.saturating_add(1);
            Event::None
        } else if st.consecutive_failures >= threshold {
            Event::Open {
                start_ts: st.first_fail_ts.unwrap_or(res.ts_wall_ms),
                probe_type: res.probe_type,
                detail: res.detail.clone(),
            }
        } else {
            Event::None
        }
    }
}

fn close_window_cmd(
    wb: WindowBuilder,
    end_ts: i64,
    recent_closes: &std::collections::VecDeque<i64>,
    interval_secs: u64,
) -> StoreCmd {
    let layers = attr::layers_str(&wb.failing_types);
    let cause = attr::finest_cause(&wb.detail_samples);
    let location_class = attr::location_class(wb.gateway_ok, wb.gateway_fail);
    let cutoff = end_ts - 3_600_000;
    let closes = recent_closes.iter().filter(|ts| **ts >= cutoff).count();
    let flap = flap::is_flap(
        end_ts.saturating_sub(wb.start_ts),
        interval_secs,
        closes + 1,
    );
    StoreCmd::CloseWindow {
        id: wb.id,
        end_ts,
        layers,
        cause,
        location_class,
        flap,
        probes_failed: wb.probes_failed,
    }
}

impl Detector {
    pub fn new(
        fail_threshold: u32,
        targets: &[Target],
        gateway_target: Option<String>,
        open_windows: &[WindowRow],
    ) -> Self {
        let mut states = HashMap::new();
        let mut intervals = HashMap::new();
        for t in targets {
            intervals.insert(t.name.clone(), t.interval);
            states.entry(t.name.clone()).or_default();
        }
        for w in open_windows {
            seed_open_window(&mut states, w);
        }
        Detector {
            fail_threshold,
            states,
            intervals,
            gateway_target,
        }
    }

    /// Feed one probe result; returns store commands (window closes).
    pub fn feed(&mut self, res: ProbeResult, store: &Store) -> Vec<StoreCmd> {
        let mut cmds = Vec::new();
        let target = res.target.clone();
        let interval = self.intervals.get(&target).copied().unwrap_or(5);
        let threshold = self.fail_threshold;
        let is_gateway = self.gateway_target.as_deref() == Some(target.as_str());

        let st = self.states.entry(target.clone()).or_default();
        match step(st, &res, threshold) {
            Event::Open {
                start_ts,
                probe_type,
                detail,
            } => {
                if let Ok(id) = store.open_window(&target, start_ts) {
                    st.open = Some(WindowBuilder {
                        id,
                        start_ts,
                        failing_types: [probe_type].into_iter().collect(),
                        detail_samples: vec![(probe_type, detail)],
                        gateway_ok: 0,
                        gateway_fail: 0,
                        // Includes the pre-threshold failures in the streak.
                        probes_failed: threshold,
                    });
                }
            }
            Event::Close { wb } => {
                let cmd = close_window_cmd(wb, res.ts_wall_ms, &st.recent_closes, interval);
                st.recent_closes.push_back(res.ts_wall_ms);
                let cutoff = res.ts_wall_ms - 3_600_000;
                while let Some(front) = st.recent_closes.front() {
                    if *front >= cutoff {
                        break;
                    }
                    st.recent_closes.pop_front();
                }
                cmds.push(cmd);
            }
            Event::None => {}
        }

        // Gateway probe accounting for all open windows.
        if is_gateway {
            for st2 in self.states.values_mut() {
                if let Some(wb) = st2.open.as_mut() {
                    if res.ok {
                        wb.gateway_ok = wb.gateway_ok.saturating_add(1);
                    } else {
                        wb.gateway_fail = wb.gateway_fail.saturating_add(1);
                    }
                }
            }
        }
        cmds
    }

    /// Close all open windows at `now` (shutdown).
    pub fn close_all(&mut self, now: i64) -> Vec<StoreCmd> {
        let intervals: Vec<(String, u64)> = self
            .intervals
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        let mut cmds = Vec::new();
        for (target, st) in self.states.iter_mut() {
            if let Some(wb) = st.open.take() {
                let interval = intervals
                    .iter()
                    .find(|(k, _)| k == target)
                    .map(|(_, v)| *v)
                    .unwrap_or(5);
                cmds.push(close_window_cmd(wb, now, &st.recent_closes, interval));
            }
        }
        cmds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;

    fn target(name: &str, pt: ProbeType, interval: u64) -> Target {
        Target {
            name: name.into(),
            probe_type: pt,
            endpoint: "x".into(),
            interval,
            timeout: 3,
        }
    }

    fn res(t: &str, pt: ProbeType, ok: bool, ts: i64, detail: &str) -> ProbeResult {
        ProbeResult {
            ts_wall_ms: ts,
            ts_mono_delta_ms: 0,
            target: t.into(),
            probe_type: pt,
            ok,
            latency_ms: ok.then_some(10.0),
            detail: detail.into(),
        }
    }

    fn two_targets() -> Vec<Target> {
        vec![
            target("ext", ProbeType::Dns, 5),
            target("gw", ProbeType::Tcp, 5),
        ]
    }

    fn detector() -> Detector {
        Detector::new(3, &two_targets(), Some("gw".into()), &[])
    }

    /// Feed one result and apply the resulting store commands (test-only).
    fn feed(d: &mut Detector, store: &Store, r: ProbeResult) {
        for c in d.feed(r, store) {
            apply_cmd(store, &c).unwrap();
        }
    }

    #[test]
    fn clean_stream_no_windows() {
        let store = Store::open_in_memory().unwrap();
        let mut d = detector();
        for i in 0..10 {
            feed(
                &mut d,
                &store,
                res("ext", ProbeType::Dns, true, 1_000 + i * 5_000, ""),
            );
        }
        assert!(store.open_windows().unwrap().is_empty());
        assert!(store.outages_in(0, i64::MAX).unwrap().is_empty());
    }

    #[test]
    fn three_fails_open_window_backdated() {
        let store = Store::open_in_memory().unwrap();
        let mut d = detector();
        for i in 0..3 {
            feed(
                &mut d,
                &store,
                res(
                    "ext",
                    ProbeType::Dns,
                    false,
                    1_000 + i * 1_000,
                    "query-timeout@1.1.1.1",
                ),
            );
        }
        let open = store.open_windows().unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].start_ts, 1_000); // backdated to the first failure
    }

    #[test]
    fn below_threshold_no_window() {
        let store = Store::open_in_memory().unwrap();
        let mut d = detector();
        feed(
            &mut d,
            &store,
            res("ext", ProbeType::Dns, false, 1_000, "x"),
        );
        feed(
            &mut d,
            &store,
            res("ext", ProbeType::Dns, false, 2_000, "x"),
        );
        feed(&mut d, &store, res("ext", ProbeType::Dns, true, 3_000, ""));
        assert!(store.open_windows().unwrap().is_empty());
    }

    #[test]
    fn recovery_closes_with_attribution() {
        let store = Store::open_in_memory().unwrap();
        let mut d = detector();
        for i in 0..3 {
            feed(
                &mut d,
                &store,
                res(
                    "ext",
                    ProbeType::Dns,
                    false,
                    1_000 + i * 1_000,
                    "query-timeout@1.1.1.1",
                ),
            );
        }
        feed(&mut d, &store, res("ext", ProbeType::Dns, true, 5_000, ""));
        let o = store.outages_in(0, i64::MAX).unwrap();
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].end_ts, Some(5_000));
        assert_eq!(o[0].layers, "dns");
        assert_eq!(o[0].cause.as_deref(), Some("dns:query-timeout@1.1.1.1"));
        assert_eq!(o[0].location_class.as_deref(), Some("unknown")); // no gateway data
    }

    #[test]
    fn gateway_failing_during_window_is_local() {
        let store = Store::open_in_memory().unwrap();
        let mut d = detector();
        for i in 0..3 {
            feed(
                &mut d,
                &store,
                res(
                    "ext",
                    ProbeType::Dns,
                    false,
                    1_000 + i * 1_000,
                    "query-timeout@1.1.1.1",
                ),
            );
        }
        feed(
            &mut d,
            &store,
            res(
                "gw",
                ProbeType::Tcp,
                false,
                4_000,
                "timeout@192.168.1.1:443",
            ),
        );
        feed(&mut d, &store, res("ext", ProbeType::Dns, true, 5_000, ""));
        let o = store.outages_in(0, i64::MAX).unwrap();
        assert_eq!(o[0].location_class.as_deref(), Some("local"));
    }

    #[test]
    fn gateway_ok_during_window_is_upstream() {
        let store = Store::open_in_memory().unwrap();
        let mut d = detector();
        for i in 0..3 {
            feed(
                &mut d,
                &store,
                res(
                    "ext",
                    ProbeType::Dns,
                    false,
                    1_000 + i * 1_000,
                    "query-timeout@1.1.1.1",
                ),
            );
        }
        feed(&mut d, &store, res("gw", ProbeType::Tcp, true, 4_000, ""));
        feed(&mut d, &store, res("ext", ProbeType::Dns, true, 5_000, ""));
        let o = store.outages_in(0, i64::MAX).unwrap();
        assert_eq!(o[0].location_class.as_deref(), Some("upstream"));
    }

    #[test]
    fn short_window_is_flap() {
        let store = Store::open_in_memory().unwrap();
        let mut d = detector(); // interval 5s → flap if < 10s
        for i in 0..3 {
            feed(
                &mut d,
                &store,
                res("ext", ProbeType::Dns, false, 1_000 + i * 1_000, "x"),
            );
        }
        feed(&mut d, &store, res("ext", ProbeType::Dns, true, 4_000, ""));
        let o = store.outages_in(0, i64::MAX).unwrap();
        assert_eq!(o[0].end_ts, Some(4_000));
        assert!(o[0].flap);
    }

    #[test]
    fn three_closes_in_hour_is_flap() {
        let store = Store::open_in_memory().unwrap();
        let mut d = detector();
        // 3 full cycles, each window ~14s (≥ 2×5s interval → duration rule silent).
        let mut t = 1_000i64;
        for _ in 0..3 {
            for i in 0..3 {
                feed(
                    &mut d,
                    &store,
                    res("ext", ProbeType::Dns, false, t + i * 1_000, "x"),
                );
            }
            t += 3_000;
            feed(
                &mut d,
                &store,
                res("ext", ProbeType::Dns, true, t + 11_000, ""),
            );
            t += 12_000;
        }
        let o = store.outages_in(0, i64::MAX).unwrap();
        assert_eq!(o.len(), 3);
        assert!(!o[0].flap, "first close: duration ok, 1 close");
        assert!(!o[1].flap, "second close: duration ok, 2 closes");
        assert!(o[2].flap, "third close: 3 closes in the last hour");
    }

    #[test]
    fn restart_seeds_open_window_and_closes_it() {
        let store = Store::open_in_memory().unwrap();
        store.open_window("ext", 1_000).unwrap();
        let open = store.open_windows().unwrap();
        let mut d = Detector::new(3, &two_targets(), Some("gw".into()), &open);
        feed(&mut d, &store, res("ext", ProbeType::Dns, true, 9_000, ""));
        let o = store.outages_in(0, i64::MAX).unwrap();
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].start_ts, 1_000);
        assert_eq!(o[0].end_ts, Some(9_000));
    }

    #[test]
    fn close_all_at_shutdown() {
        let store = Store::open_in_memory().unwrap();
        let mut d = detector();
        for i in 0..3 {
            feed(
                &mut d,
                &store,
                res("ext", ProbeType::Dns, false, 1_000 + i * 1_000, "x"),
            );
        }
        let cmds = d.close_all(10_000);
        assert_eq!(cmds.len(), 1);
        for c in &cmds {
            apply_cmd(&store, c).unwrap();
        }
        let o = store.outages_in(0, i64::MAX).unwrap();
        assert_eq!(o[0].end_ts, Some(10_000));
    }
}
