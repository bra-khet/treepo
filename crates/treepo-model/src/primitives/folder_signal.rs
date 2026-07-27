//! Folder-signal records — `F-EXT-5`, `design/feature-system.md` §3.1.
//!
//! > These are never simple booleans. A folder named `public`, `src`, `lib`, `internal`,
//! > `vendor`, `assets`, `docs`, `scripts`, `test(s)`, `examples`, `pkg`, `cmd`, `crates`,
//! > etc. receives a structured signal.
//!
//! The reason is in the same section: a `public` folder full of static assets means
//! something different from a `public` folder full of binaries, or from a `public` that is
//! really the root of a separate package. An `is_public: bool` throws away the distinction
//! before anything downstream can use it, so the record carries the name, the conventional
//! weight, what the contents did to that weight, and where in the hierarchy it sits.
//!
//! # Why the name is a string
//!
//! The dictionary lives in `assets/params/folder-signals.ron` and is editable without a
//! recompile, the same way `F-SKEL-4` treats the L-system table. An enum would make every
//! new convention a code change and a release, which is precisely the coupling the RON
//! parameter files exist to avoid.

use alloc::boxed::Box;
use treepo_det::Fx;

/// What the folder's contents did to its conventional weight.
///
/// Every field is a proportion of the subtree in `0..=1`, so a signal can be reweighted
/// without re-reading the folder. These are the inputs to
/// [`FolderSignal::effective_weight`], kept alongside the result so a surprising weight can
/// be explained rather than merely observed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContentModulation {
    /// How concentrated the subtree is in one language, in `0..=1`.
    ///
    /// 1 is a single-language folder. A `src` that is all one language reads as a real
    /// source root; one holding six languages is more likely a container.
    pub language_concentration: Fx,
    /// The subtree's bytes as a proportion of the repository's, in `0..=1`.
    pub size_ratio: Fx,
    /// Proportion of bytes in binary content, in `0..=1`.
    pub binary_ratio: Fx,
    /// Proportion of files whose names or paths read as tests, in `0..=1`.
    pub test_like_ratio: Fx,
    /// Proportion of bytes marked or detected as generated, in `0..=1`.
    pub generated_ratio: Fx,
}

/// Where a signalled folder sits, and what it sits inside.
///
/// A `docs` at the repository root is the documentation. A `docs` inside `vendor` is
/// somebody else's documentation, and `F-MAT-5`'s enrichment should treat it differently —
/// which it can only do if the enclosing signals travelled with the record.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HierarchyPosition {
    /// Distance from the repository root.
    pub depth: u16,
    /// Signal names on the ancestors of this folder, root-first.
    ///
    /// Empty for a top-level folder. Only *signalled* ancestors appear, so the list is
    /// short even in a deep tree.
    pub ancestor_signals: Box<[Box<str>]>,
}

impl HierarchyPosition {
    /// Whether any ancestor carries the named signal.
    #[must_use]
    pub fn is_within(&self, signal: &str) -> bool {
        self.ancestor_signals.iter().any(|name| &**name == signal)
    }
}

/// A structured folder signal — never a boolean (`F-EXT-5`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderSignal {
    /// The dictionary entry that matched — `src`, `vendor`, `docs`, and so on.
    ///
    /// Matched case-insensitively against the folder name by `treepo-vcs`, but stored as
    /// the dictionary spells it so two spellings of one convention are one signal.
    pub signal_name: Box<str>,
    /// The conventional weight from `assets/params/folder-signals.ron`, in `0..=1`.
    pub default_semantic_weight: Fx,
    /// What the contents did to that weight.
    pub content_modulation: ContentModulation,
    /// The weight after modulation, in `0..=1`. What `F-SKEL-1` actually reads.
    pub effective_weight: Fx,
    /// Where this folder sits and what encloses it.
    pub position_in_hierarchy: HierarchyPosition,
}

impl FolderSignal {
    /// Builds a signal whose contents told us nothing — the conventional weight, unmodified.
    ///
    /// The honest starting point for a folder whose subtree has not been measured yet, and
    /// the value `treepo-vcs` refines as the walk rolls up.
    #[must_use]
    pub fn unmodulated(signal_name: Box<str>, default_semantic_weight: Fx, depth: u16) -> Self {
        Self {
            signal_name,
            default_semantic_weight,
            content_modulation: ContentModulation::default(),
            effective_weight: default_semantic_weight,
            position_in_hierarchy: HierarchyPosition {
                depth,
                ancestor_signals: Box::default(),
            },
        }
    }

    /// How far modulation moved the weight from its conventional value.
    ///
    /// Signed: negative means the contents argued against the name. Useful for the debug
    /// surface, where "this `src` is weighted 0.3" is a question and this is the answer.
    #[must_use]
    pub fn modulation_delta(&self) -> Fx {
        self.effective_weight.sub(self.default_semantic_weight)
    }

    /// Whether this signal is nested inside another signalled folder.
    #[must_use]
    pub fn is_nested(&self) -> bool {
        !self.position_in_hierarchy.ancestor_signals.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn signal(name: &str, weight: Fx) -> FolderSignal {
        FolderSignal::unmodulated(name.into(), weight, 1)
    }

    #[test]
    fn an_unmodulated_signal_keeps_its_conventional_weight() {
        let s = signal("src", Fx::from_ratio(9, 10));
        assert_eq!(s.effective_weight, s.default_semantic_weight);
        assert_eq!(s.modulation_delta(), Fx::ZERO);
        assert!(!s.is_nested());
    }

    /// The distinction `F-EXT-5` exists to preserve: same name, different meaning.
    #[test]
    fn one_name_carries_different_meanings() {
        let base = Fx::from_ratio(8, 10);

        let mut assets = signal("public", base);
        assets.content_modulation.binary_ratio = Fx::from_ratio(1, 20);
        assets.effective_weight = base;

        let mut binaries = signal("public", base);
        binaries.content_modulation.binary_ratio = Fx::from_ratio(19, 20);
        binaries.effective_weight = Fx::from_ratio(3, 10);

        assert_eq!(assets.signal_name, binaries.signal_name);
        assert_ne!(assets.effective_weight, binaries.effective_weight);
        assert_eq!(assets.modulation_delta(), Fx::ZERO);
        assert!(binaries.modulation_delta() < Fx::ZERO);
    }

    #[test]
    fn nesting_is_visible_from_the_record() {
        let mut nested = signal("docs", Fx::ONE);
        nested.position_in_hierarchy.ancestor_signals = vec!["vendor".into()].into();
        assert!(nested.is_nested());
        assert!(nested.position_in_hierarchy.is_within("vendor"));
        assert!(!nested.position_in_hierarchy.is_within("src"));
    }
}
