//! Temporal primitives — `F-EXT-2`, `design/feature-system.md` §3.3.
//!
//! # Ages are not stored. This is the load-bearing decision in this module.
//!
//! `design/feature-system.md` names the primitives `first_commit_age` and `last_commit_age`,
//! and an age is a duration from *now*. Storing one would make the manifest a function of
//! the repository **and the clock**, which loses two acceptance criteria at once:
//!
//! * `AC-MAN-1` — delete the store, regenerate, and the bytes must be identical. They would
//!   not be; every age would have moved by the elapsed time.
//! * `AC-DET-1`/`N3` — the same repository must produce the same tree at any time. A tree
//!   whose limb thicknesses come from ages would drift a little every day, on a machine
//!   doing nothing.
//!
//! So this module stores **absolute epoch seconds** and nothing else. Age is derived, at the
//! point of use, against [`reference_time`](crate::Manifest::reference_time) — the timestamp
//! of the newest commit in the repository, which is a property *of the repository*. See
//! [`TemporalPrimitives::age_at`].
//!
//! The visible consequence is worth stating plainly, because it will look like a bug to
//! someone: a repository that has not been committed to for a year does not age. Its tree is
//! identical to the tree it produced a year ago. That is the correct behaviour — the tree
//! depicts the repository, and nothing about the repository changed.
//!
//! # Clock skew
//!
//! Git commit timestamps are supplied by the committer's machine and are not monotonic. A
//! commit from a machine with a wrong clock can sit in the future, making
//! `reference_time - first_commit_time` negative. Ages are therefore [`i64`] and clamp at
//! zero on the way out rather than wrapping — a skewed commit reads as brand new, which is
//! the least surprising rendering of "committed tomorrow".

use treepo_det::Fx;

/// Seconds in a day, the unit every window below is defined in.
const DAY: i64 = 86_400;

/// Churn measured over the windows `F-EXT-2` names: 30, 90, and 365 days, plus lifetime.
///
/// Lines rather than commits: a one-line typo fix and a rewrite are not the same event, and
/// `F-MAT-4`'s age/recency gradient reads better against volume than against frequency.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChurnWindows {
    /// Lines added plus removed in the 30 days before the reference time.
    pub days_30: u64,
    /// Lines added plus removed in the 90 days before the reference time.
    pub days_90: u64,
    /// Lines added plus removed in the 365 days before the reference time.
    pub days_365: u64,
    /// Lines added plus removed over the path's whole history.
    pub lifetime: u64,
}

impl ChurnWindows {
    /// Churn per day within a window, as a fixed-point rate.
    ///
    /// `days` of zero yields zero rather than panicking; a caller asking for a zero-length
    /// window wants "no activity", not a division trap.
    #[must_use]
    pub const fn rate_per_day(lines: u64, days: i64) -> Fx {
        if days <= 0 {
            return Fx::ZERO;
        }
        Fx::from_ratio(lines as i64, days)
    }
}

/// When a path was touched, and how often — `F-EXT-2`.
///
/// Every timestamp is absolute epoch seconds. Nothing here is relative to now.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TemporalPrimitives {
    /// The commit that introduced this path, as epoch seconds.
    ///
    /// `None` for a path with no history — untracked, or a repository with no commits.
    pub first_commit_time: Option<i64>,
    /// The most recent commit touching this path, as epoch seconds.
    pub last_commit_time: Option<i64>,
    /// How many commits touched this path over its whole history.
    pub commit_count: u32,
    /// Lines changed per window (`churn_rate`).
    pub churn: ChurnWindows,
    /// `recency_heat` — exponentially weighted recent activity, in `0..=1`.
    ///
    /// Computed during extraction with an integer decay so it does not need `exp`, and
    /// stored rather than derived because recomputing it means replaying the commit stream.
    pub recency_heat: Fx,
    /// `modification_burstiness` — how concentrated the commits are in time, in `0..=1`.
    ///
    /// 0 is perfectly regular, 1 is every commit in one moment. A path with fewer than two
    /// commits has no intervals to measure and is 0.
    pub burstiness: Fx,
    /// `stability_score` — inverse of recent churn relative to size, in `0..=1`.
    ///
    /// 1 is untouched, 0 is churning at or above its own line count in the recent window.
    pub stability: Fx,
}

impl TemporalPrimitives {
    /// Age in seconds at a reference time, clamped at zero.
    ///
    /// `reference` is [`Manifest::reference_time`](crate::Manifest::reference_time), not a
    /// clock reading — see this module's header for why that distinction is the whole point.
    #[must_use]
    pub const fn age_at(reference: i64, timestamp: Option<i64>) -> Option<i64> {
        match timestamp {
            // Clock skew can place a commit after the reference time; such a path is new,
            // not negatively aged.
            Some(t) if reference >= t => Some(reference - t),
            Some(_) => Some(0),
            None => None,
        }
    }

    /// `first_commit_age` in seconds — how long this path has existed.
    #[must_use]
    pub const fn first_commit_age(&self, reference: i64) -> Option<i64> {
        Self::age_at(reference, self.first_commit_time)
    }

    /// `last_commit_age` in seconds — how long since this path was touched.
    #[must_use]
    pub const fn last_commit_age(&self, reference: i64) -> Option<i64> {
        Self::age_at(reference, self.last_commit_time)
    }

    /// `first_commit_age` in whole days, the unit the parameter tables are written in.
    #[must_use]
    pub const fn first_commit_age_days(&self, reference: i64) -> Option<i64> {
        match self.first_commit_age(reference) {
            Some(seconds) => Some(seconds / DAY),
            None => None,
        }
    }

    /// `last_commit_age` in whole days.
    #[must_use]
    pub const fn last_commit_age_days(&self, reference: i64) -> Option<i64> {
        match self.last_commit_age(reference) {
            Some(seconds) => Some(seconds / DAY),
            None => None,
        }
    }

    /// The span between first and last commit, in seconds. Zero for a single commit.
    #[must_use]
    pub const fn active_span(&self) -> Option<i64> {
        match (self.first_commit_time, self.last_commit_time) {
            (Some(first), Some(last)) if last >= first => Some(last - first),
            (Some(_), Some(_)) => Some(0),
            _ => None,
        }
    }

    /// Whether this path has any history at all.
    ///
    /// False is an ordinary state (PRD §6, "No `.git`" and "Empty repository"), and the
    /// notice the user sees about age and churn being unavailable is driven by this.
    #[must_use]
    pub const fn has_history(&self) -> bool {
        self.first_commit_time.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REFERENCE: i64 = 1_800_000_000;

    fn dated(first: i64, last: i64) -> TemporalPrimitives {
        TemporalPrimitives {
            first_commit_time: Some(first),
            last_commit_time: Some(last),
            commit_count: 2,
            ..TemporalPrimitives::default()
        }
    }

    #[test]
    fn age_is_measured_against_the_reference_not_the_clock() {
        let t = dated(REFERENCE - 30 * DAY, REFERENCE - DAY);
        assert_eq!(t.first_commit_age(REFERENCE), Some(30 * DAY));
        assert_eq!(t.first_commit_age_days(REFERENCE), Some(30));
        assert_eq!(t.last_commit_age_days(REFERENCE), Some(1));
        assert_eq!(t.active_span(), Some(29 * DAY));
    }

    /// The property that protects `AC-MAN-1`: the stored values do not move on their own.
    #[test]
    fn stored_timestamps_do_not_depend_on_when_they_are_read() {
        let t = dated(REFERENCE - 100, REFERENCE);
        let a = t;
        let b = t;
        assert_eq!(a, b);
        // Two different references give different ages from identical stored data — which
        // is exactly why the reference belongs to the repository, not to the machine.
        assert_eq!(a.first_commit_age(REFERENCE), Some(100));
        assert_eq!(b.first_commit_age(REFERENCE + 5_000), Some(5_100));
    }

    /// A committer's clock can be wrong; that must not produce a negative age.
    #[test]
    fn clock_skew_clamps_rather_than_wrapping() {
        let future = dated(REFERENCE + 90 * DAY, REFERENCE + 90 * DAY);
        assert_eq!(future.first_commit_age(REFERENCE), Some(0));
        assert_eq!(future.first_commit_age_days(REFERENCE), Some(0));
        // And a last commit before the first one — also possible with skew — spans zero.
        let inverted = dated(REFERENCE, REFERENCE - DAY);
        assert_eq!(inverted.active_span(), Some(0));
    }

    #[test]
    fn a_path_without_history_reports_none_rather_than_zero() {
        let t = TemporalPrimitives::default();
        assert!(!t.has_history());
        assert_eq!(t.first_commit_age(REFERENCE), None);
        assert_eq!(t.last_commit_age_days(REFERENCE), None);
        assert_eq!(t.active_span(), None);
    }

    #[test]
    fn churn_rate_survives_a_zero_length_window() {
        assert_eq!(ChurnWindows::rate_per_day(300, 30), Fx::from_int(10));
        assert_eq!(ChurnWindows::rate_per_day(300, 0), Fx::ZERO);
        assert_eq!(ChurnWindows::rate_per_day(0, 30), Fx::ZERO);
    }
}
