//! Per-target state machine (roadmap §5.1).

use std::collections::{BTreeSet, HashMap, VecDeque};

use crate::config::ProbeType;
use crate::store::WindowRow;

/// An in-progress outage window being accumulated by the detector.
pub struct WindowBuilder {
    pub id: i64,
    pub start_ts: i64,
    pub failing_types: BTreeSet<ProbeType>,
    pub detail_samples: Vec<(ProbeType, String)>,
    pub gateway_ok: u32,
    pub gateway_fail: u32,
    pub probes_failed: u32,
}

/// Per-target detection state.
pub struct TargetState {
    pub consecutive_failures: u32,
    pub first_fail_ts: Option<i64>,
    pub open: Option<WindowBuilder>,
    /// Close timestamps (for flap detection); trimmed to the last hour.
    pub recent_closes: VecDeque<i64>,
}

impl TargetState {
    pub fn new() -> Self {
        TargetState {
            consecutive_failures: 0,
            first_fail_ts: None,
            open: None,
            recent_closes: VecDeque::new(),
        }
    }
}

impl Default for TargetState {
    fn default() -> Self {
        Self::new()
    }
}

/// Seed a detector state map from persisted open windows (restart recovery).
pub fn seed_open_window(states: &mut HashMap<String, TargetState>, w: &WindowRow) {
    states.entry(w.target.clone()).or_default().open = Some(WindowBuilder {
        id: w.id,
        start_ts: w.start_ts,
        failing_types: BTreeSet::new(),
        detail_samples: Vec::new(),
        gateway_ok: 0,
        gateway_fail: 0,
        probes_failed: w.probes_failed as u32,
    });
}
