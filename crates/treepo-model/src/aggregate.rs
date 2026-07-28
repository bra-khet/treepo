//! Aggregate containers — `F-SKEL-7`, `P6`, `F-INSP-3`.
//!
//! > "Aggregation past the recursion cap produces a proportional container object rather than
//! > truncating content (`P6`). The container records the full descendant set for inspection
//! > (`F-INSP-3`)."
//!
//! `design/design-outline.md` §6 states the same rule from the user's side: branching and
//! nesting are not infinite, and past a practical threshold "an object can simply represent
//! *this directory and all its contents*".
//!
//! # A container is a phase transition, not a truncation
//!
//! The distinction is the whole of `P6`: *legibility bounds detail; honesty bounds data*.
//! Aggregation is a presentation decision, and the manifest beneath it is untouched — so an
//! [`AggregateNode`] is not a stub marking where the tree gave up. It is a node with a
//! position, a heading, a seed, and a place in the parent chain, exactly as a limb is, and a
//! later phase is free to give it a form and hang enrichment off it.
//!
//! `F-MAT-3` makes that explicit from the other direction: a container "discharges the
//! [minimum representation] floor on behalf of everything it represents", which is what makes
//! `P7` — nothing important is erased — compatible with a T3 repository being legible at all.
//!
//! # Members are roots, not the transitive closure
//!
//! [`members`](AggregateNode::members) holds the child paths the container absorbed, and the
//! counts hold everything beneath them. It does not hold the flattened descendant set.
//!
//! `F-INSP-3` asks that the container "report what it represents and allow drilling into
//! their contents", and it is satisfied either way — the manifest is right there, and walking
//! it from these roots is a binary search per level. Storing the closure instead would copy a
//! large part of the manifest into the skeleton, and at T3 that is tens of thousands of paths
//! duplicated into a structure that is rebuilt on every Grow. The roots are the smallest
//! thing that answers the question.

use crate::path::RepoPath;
use alloc::vec::Vec;

/// A proportional container standing for content past the composition threshold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateNode {
    /// The limb whose children this container stands for.
    pub anchor: RepoPath,
    /// Which container this is among that limb's, counted from zero.
    ///
    /// Part of the container's identity, and the reason it survives a Grow: the seed is
    /// derived from the anchor and this index rather than from the membership, so a file
    /// arriving in a container does not reroll the container's appearance.
    pub index: u16,
    /// The child paths absorbed — the roots of what this represents (`F-INSP-3`).
    pub members: Vec<RepoPath>,
    /// Bytes across everything beneath the members, inclusive.
    ///
    /// What "proportional container" is proportional *to*: a container standing for half a
    /// repository should not render at the size of one standing for four small files.
    pub bytes: u64,
    /// Files beneath the members, inclusive.
    pub file_count: u32,
    /// Directories beneath the members, inclusive.
    pub dir_count: u32,
}

impl AggregateNode {
    /// How many paths this container absorbed directly.
    #[must_use]
    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    /// Everything this container stands for, files and directories together.
    #[must_use]
    pub const fn represented_count(&self) -> u32 {
        self.file_count.saturating_add(self.dir_count)
    }

    /// Whether this container stands for anything at all.
    ///
    /// An empty container is a defect rather than an edge case — composition creates one only
    /// when it has residue to put in it — so callers assert on this rather than handling it.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn path(text: &str) -> RepoPath {
        RepoPath::new(text.as_bytes()).unwrap()
    }

    #[test]
    fn a_container_reports_what_it_represents() {
        let aggregate = AggregateNode {
            anchor: path("vendor"),
            index: 1,
            members: vec![path("vendor/a"), path("vendor/b")],
            bytes: 900_000,
            file_count: 412,
            dir_count: 37,
        };

        // Two roots, but it speaks for 449 paths — the distinction F-INSP-3 turns on.
        assert_eq!(aggregate.member_count(), 2);
        assert_eq!(aggregate.represented_count(), 449);
        assert!(!aggregate.is_empty());
    }

    #[test]
    fn represented_counts_saturate_rather_than_wrapping() {
        let huge = AggregateNode {
            anchor: path("x"),
            index: 0,
            members: vec![path("x/y")],
            bytes: u64::MAX,
            file_count: u32::MAX,
            dir_count: 10,
        };
        assert_eq!(huge.represented_count(), u32::MAX);
    }
}
