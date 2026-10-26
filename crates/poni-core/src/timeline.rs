//! Playback position extrapolation.
//!
//! Media sessions (Windows GSMTC) report a position snapshot that is only
//! "current as of" its update time, and many players update it rarely. To drive
//! a progress bar we extrapolate from that snapshot.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Timeline {
    /// Position in seconds at `sampled_at`.
    pub position_s: f64,
    /// Track length in seconds (0 when unknown).
    pub duration_s: f64,
    /// Monotonic clock (seconds) when `position_s` was valid.
    pub sampled_at: f64,
    pub playing: bool,
    /// Playback rate (1.0 normal).
    pub rate: f64,
}

impl Timeline {
    pub fn new(position_s: f64, duration_s: f64, sampled_at: f64, playing: bool) -> Self {
        Self { position_s, duration_s, sampled_at, playing, rate: 1.0 }
    }

    /// Estimated position at monotonic time `now`, clamped to the track.
    pub fn position_at(&self, now: f64) -> f64 {
        let mut p = self.position_s;
        if self.playing {
            let rate = if self.rate > 0.0 { self.rate } else { 1.0 };
            p += (now - self.sampled_at).max(0.0) * rate;
        }
        if self.duration_s > 0.0 {
            p = p.min(self.duration_s);
        }
        p.max(0.0)
    }

    /// 0..=1 progress, or 0 when duration is unknown.
    pub fn progress_at(&self, now: f64) -> f32 {
        if self.duration_s <= 0.0 {
            0.0
        } else {
            (self.position_at(now) / self.duration_s).clamp(0.0, 1.0) as f32
        }
    }

    pub fn has_duration(&self) -> bool {
        self.duration_s > 0.5
    }
}

/// A seek the player hasn't confirmed yet.
///
/// Players report their position on their own schedule, so the first reports
/// after a seek can still be from before it; passed on, they would pull the
/// bar back to where it was until a fresh report pushed them
/// forward again. Reports that disagree with the seek are held back until one
/// agrees or [`PendingSeek::CONFIRM_S`] passes (the seek didn't take: then the
/// player's word stands).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PendingSeek {
    target_s: f64,
    at: f64,
}

impl PendingSeek {
    /// How long a seek may go unconfirmed.
    pub const CONFIRM_S: f64 = 3.0;
    /// A report this close to where the seek should be by now confirms it
    /// (Apple Music reports about half a second behind, and unevenly).
    pub const TOLERANCE_S: f64 = 1.5;

    /// A seek to `target_s`, asked for at monotonic time `now`.
    pub fn new(target_s: f64, now: f64) -> Self {
        Self { target_s, at: now }
    }

    pub fn target_s(&self) -> f64 {
        self.target_s
    }

    /// The confirmation window is over.
    pub fn expired(&self, now: f64) -> bool {
        now - self.at >= Self::CONFIRM_S
    }

    /// Whether `report` (read at `now`) still predates the seek and should be
    /// held back.
    pub fn holds(&self, report: &Timeline, now: f64) -> bool {
        if self.expired(now) {
            return false;
        }
        let mut expected = self.target_s;
        if report.playing {
            let rate = if report.rate > 0.0 { report.rate } else { 1.0 };
            expected += (now - self.at).max(0.0) * rate;
        }
        if report.duration_s > 0.0 {
            expected = expected.min(report.duration_s);
        }
        (report.position_at(now) - expected).abs() > Self::TOLERANCE_S
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extrapolates_while_playing() {
        let t = Timeline::new(10.0, 200.0, 100.0, true);
        assert!((t.position_at(105.0) - 15.0).abs() < 1e-9);
        assert!((t.progress_at(105.0) - 0.075).abs() < 1e-6);
    }

    #[test]
    fn frozen_while_paused() {
        let t = Timeline::new(10.0, 200.0, 100.0, false);
        assert_eq!(t.position_at(150.0), 10.0);
    }

    #[test]
    fn clamps_to_duration_and_zero() {
        let t = Timeline::new(190.0, 200.0, 0.0, true);
        assert_eq!(t.position_at(100.0), 200.0);
        let t = Timeline::new(-3.0, 0.0, 0.0, false);
        assert_eq!(t.position_at(0.0), 0.0);
        assert_eq!(t.progress_at(0.0), 0.0);
    }

    #[test]
    fn clock_going_backwards_does_not_rewind() {
        let t = Timeline::new(10.0, 200.0, 100.0, true);
        assert_eq!(t.position_at(90.0), 10.0);
    }

    #[test]
    fn pending_seek_holds_stale_reports_only() {
        // At 100 s on the clock, a song at 30 s is dragged to 200 s.
        let seek = PendingSeek::new(200.0, 100.0);
        // The player's next report still says ~31 s: held.
        assert!(seek.holds(&Timeline::new(30.0, 260.0, 99.0, true), 100.5));
        // Its report from after the seek, half a second behind (Apple Music): passed.
        assert!(!seek.holds(&Timeline::new(200.7, 260.0, 101.2, true), 101.2));
        // Paused: the target doesn't move.
        assert!(!seek.holds(&Timeline::new(200.0, 260.0, 100.0, false), 102.0));
        assert!(seek.holds(&Timeline::new(30.0, 260.0, 100.0, false), 102.0));
        // A seek past the end lands on the end.
        let end = PendingSeek::new(300.0, 100.0);
        assert!(!end.holds(&Timeline::new(260.0, 260.0, 100.5, true), 100.5));
        // Unconfirmed for CONFIRM_S: the player's word stands again.
        assert!(seek.expired(100.0 + PendingSeek::CONFIRM_S));
        assert!(!seek.holds(&Timeline::new(33.0, 260.0, 103.0, true), 100.0 + PendingSeek::CONFIRM_S));
    }



}
