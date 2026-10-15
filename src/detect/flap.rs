//! Flap detection (roadmap §5.4): short windows or close clustering.

/// A window is a flap if it lasted less than 2× the target interval, or if
/// there were ≥3 closes within the last hour (including this one).
pub fn is_flap(duration_ms: i64, interval_secs: u64, closes_in_last_hour: usize) -> bool {
    let min_duration = (interval_secs.saturating_mul(2)) as i64 * 1_000;
    duration_ms < min_duration || closes_in_last_hour >= 3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_window_is_flap() {
        assert!(is_flap(3_000, 5, 1)); // 3s < 10s
    }

    #[test]
    fn long_window_single_close_not_flap() {
        assert!(!is_flap(60_000, 5, 1));
    }

    #[test]
    fn three_closes_in_hour_is_flap() {
        assert!(is_flap(60_000, 5, 3));
    }
}
