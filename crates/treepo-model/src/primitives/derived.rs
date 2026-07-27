//! Derived quality signals — `F-EXT-6`, `design/feature-system.md` §3.5.
//!
//! > Kept intentionally lightweight in early versions.
//!
//! Every signal here is computed from primitives already extracted, or from a text scan
//! cheap enough to fold into the walk. Nothing evaluates repository content, which is `N1`:
//! a TODO count comes from searching for a literal, never from running a linter that loads
//! project configuration and plugins.
//!
//! These are `P1` priority in the PRD, so an extractor that cannot compute one leaves it
//! `None` rather than guessing. A missing signal is a signal a later phase must not weight;
//! a defaulted one is indistinguishable from a real zero, and would quietly claim a file has
//! no documentation when nobody looked.

use treepo_det::Fx;

/// Lightweight quality signals for one path — `F-EXT-6`.
///
/// Every field is optional, and `None` means "not measured", not "zero".
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DerivedSignals {
    /// Comment lines over code plus comment lines, in `0..=1`.
    ///
    /// Mirrors [`LineCounts::comment_density`](super::size::LineCounts::comment_density) at
    /// the subtree level, where the per-file value has been rolled up.
    pub comment_density: Option<Fx>,
    /// Test bytes over source bytes, unbounded above.
    ///
    /// Not clamped to `0..=1`: a repository with more test code than source is a real and
    /// meaningful shape, and clamping would erase it.
    pub test_to_source: Option<Fx>,
    /// TODO and FIXME markers per thousand code lines.
    ///
    /// A literal text scan over textual files. Case-insensitive on ASCII, which is what the
    /// convention is written in.
    pub todo_density: Option<Fx>,
    /// How stale documentation is relative to the code it accompanies, in days.
    ///
    /// Positive means the docs are older than the code — the direction that matters.
    /// Negative means docs were touched more recently, which is not a problem to surface.
    /// Derived from stored timestamps against
    /// [`reference_time`](crate::Manifest::reference_time), never from a clock.
    pub doc_staleness_days: Option<i64>,
    /// Bytes in generated content over total bytes, in `0..=1`.
    pub generated_debt: Option<Fx>,
    /// Bytes in files over the large-file threshold, over total bytes, in `0..=1`.
    pub large_file_debt: Option<Fx>,
}

impl DerivedSignals {
    /// Whether any signal was measured at all.
    ///
    /// False for a path nothing could be computed for — a binary file, or an extraction
    /// that skipped the text scan. The debug surface uses this to say "not measured"
    /// instead of drawing six empty bars.
    #[must_use]
    pub const fn is_measured(&self) -> bool {
        self.comment_density.is_some()
            || self.test_to_source.is_some()
            || self.todo_density.is_some()
            || self.doc_staleness_days.is_some()
            || self.generated_debt.is_some()
            || self.large_file_debt.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_measured_is_distinguishable_from_measured_zero() {
        let unmeasured = DerivedSignals::default();
        assert!(!unmeasured.is_measured());

        let measured_zero = DerivedSignals {
            todo_density: Some(Fx::ZERO),
            ..DerivedSignals::default()
        };
        assert!(measured_zero.is_measured());
        assert_ne!(unmeasured, measured_zero);
    }

    /// A test-heavy repository is a real shape, not an out-of-range value.
    #[test]
    fn test_to_source_is_not_clamped_to_one() {
        let signals = DerivedSignals {
            test_to_source: Some(Fx::from_ratio(5, 2)),
            ..DerivedSignals::default()
        };
        assert!(signals.test_to_source.unwrap() > Fx::ONE);
    }

    #[test]
    fn doc_staleness_is_signed_in_both_directions() {
        let stale = DerivedSignals {
            doc_staleness_days: Some(400),
            ..DerivedSignals::default()
        };
        let fresh = DerivedSignals {
            doc_staleness_days: Some(-3),
            ..DerivedSignals::default()
        };
        assert!(stale.doc_staleness_days.unwrap() > 0);
        assert!(fresh.doc_staleness_days.unwrap() < 0);
    }
}
