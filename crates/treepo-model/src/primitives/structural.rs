//! Structural primitives — `F-EXT-1`, `design/feature-system.md` §3.1.
//!
//! Topology and organization: how deep, how wide, how evenly the mass is spread. These are
//! what `F-SKEL-1` turns into branching, and they come from the filesystem walk alone —
//! nothing here needs a commit.
//!
//! `design/feature-system.md` §3.1 is explicit that profiles beat scalars here: a mean
//! branching factor of 3 describes both a tree where every node has three children and one
//! where a single node has three hundred. [`BranchingHistogram`] keeps the difference,
//! because it is exactly the difference between a bushy silhouette and a spindly one.

use treepo_det::Fx;

/// Distribution of children-per-node across a subtree — bushiness against spindliness.
///
/// Fixed buckets rather than a map: the shape of this distribution is what matters, the
/// buckets are stable across every repository, and a `[u32; 9]` costs 36 bytes against a
/// `BTreeMap`'s allocation per path. At T3 that is 80,000 allocations avoided.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BranchingHistogram {
    /// Counts for 0, 1, 2, 3, 4, 5–8, 9–16, 17–32, and 33+ children.
    buckets: [u32; Self::BUCKETS],
}

impl BranchingHistogram {
    /// How many buckets the distribution is quantized into.
    pub const BUCKETS: usize = 9;

    /// Human-readable bucket bounds, for debug surfaces and tests.
    pub const LABELS: [&'static str; Self::BUCKETS] =
        ["0", "1", "2", "3", "4", "5-8", "9-16", "17-32", "33+"];

    /// Records one node with `children` immediate children.
    pub const fn observe(&mut self, children: u32) {
        let bucket = match children {
            0..=4 => children as usize,
            5..=8 => 5,
            9..=16 => 6,
            17..=32 => 7,
            _ => 8,
        };
        self.buckets[bucket] = self.buckets[bucket].saturating_add(1);
    }

    /// Merges a child subtree's distribution into this one.
    pub const fn merge(&mut self, other: &Self) {
        let mut i = 0;
        while i < Self::BUCKETS {
            self.buckets[i] = self.buckets[i].saturating_add(other.buckets[i]);
            i += 1;
        }
    }

    /// The raw counts, lowest bucket first.
    #[must_use]
    pub const fn buckets(&self) -> &[u32; Self::BUCKETS] {
        &self.buckets
    }

    /// How many nodes were observed in total.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.buckets.iter().map(|&count| u64::from(count)).sum()
    }

    /// Proportion of observed nodes that are leaves — bucket 0.
    #[must_use]
    pub fn leaf_ratio(&self) -> Fx {
        let total = self.total();
        if total == 0 {
            return Fx::ZERO;
        }
        Fx::from_ratio(i64::from(self.buckets[0]), total as i64)
    }
}

/// How mass is distributed across depth levels — top-heavy, bottom-heavy, or even.
///
/// Indexed by depth relative to the node this profile belongs to, so a directory's profile
/// is independent of where in the repository it sits and two identical subtrees produce
/// identical profiles.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DepthProfile {
    /// File count at each relative depth, index 0 being this node's own level.
    levels: alloc::vec::Vec<u32>,
}

impl DepthProfile {
    /// A profile for a single file: one item at relative depth 0.
    #[must_use]
    pub fn leaf() -> Self {
        Self {
            levels: alloc::vec![1],
        }
    }

    /// Folds a child's profile in, shifted one level deeper.
    pub fn absorb_child(&mut self, child: &Self) {
        if self.levels.is_empty() {
            self.levels.push(0);
        }
        for (offset, &count) in child.levels.iter().enumerate() {
            let level = offset + 1;
            if self.levels.len() <= level {
                self.levels.resize(level + 1, 0);
            }
            self.levels[level] = self.levels[level].saturating_add(count);
        }
    }

    /// File counts by relative depth.
    #[must_use]
    pub fn levels(&self) -> &[u32] {
        &self.levels
    }

    /// The centre of mass, in relative depth levels.
    ///
    /// Below half the profile's depth is top-heavy, above it is bottom-heavy. Zero for an
    /// empty profile.
    #[must_use]
    pub fn centre_of_mass(&self) -> Fx {
        let mut weighted = 0i64;
        let mut total = 0i64;
        for (level, &count) in self.levels.iter().enumerate() {
            weighted += (level as i64) * i64::from(count);
            total += i64::from(count);
        }
        if total == 0 {
            return Fx::ZERO;
        }
        Fx::from_ratio(weighted, total)
    }
}

/// How evenly a subtree is balanced, on the three axes `design/feature-system.md` §3.1 names.
///
/// Each is in `0..=1`, where 1 is perfectly even. A separate vector rather than one number
/// because they disagree usefully: a directory holding ten equal-sized files and one holding
/// ten files of wildly different sizes have the same size-balance only by coincidence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BalanceScore {
    /// How evenly bytes are spread across immediate children.
    pub size: Fx,
    /// How evenly subtree depth is spread across immediate children.
    pub depth: Fx,
    /// How evenly content categories (code, asset, config, docs) are spread.
    ///
    /// `None` until content classification has run (`F-EXT-4`), following the same
    /// convention as [`DerivedSignals`](super::DerivedSignals): a missing measurement is
    /// `None`, never a defaulted zero that reads as real imbalance.
    pub kind: Option<Fx>,
}

impl BalanceScore {
    /// A leaf's score: nothing to be unbalanced about, and no categories measured yet.
    pub const EVEN: Self = Self {
        size: Fx::ONE,
        depth: Fx::ONE,
        kind: None,
    };
}

/// Topology and organization for one path — `F-EXT-1`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StructuralPrimitives {
    /// Distance from the repository root. The root itself is 0.
    pub depth: u16,
    /// Immediate children. Always 0 for a file.
    pub child_count: u32,
    /// Files anywhere beneath this path.
    pub descendant_file_count: u32,
    /// Directories anywhere beneath this path.
    pub descendant_dir_count: u32,
    /// Deepest level beneath this path, relative to it. 0 for a leaf.
    pub max_subtree_depth: u16,
    /// Children-per-node across the subtree.
    pub branching: BranchingHistogram,
    /// Mass by relative depth.
    pub depth_profile: DepthProfile,
    /// Balance on three axes.
    pub balance: BalanceScore,
    /// `hierarchy_skew` — deep chains against wide fans, in `-1..=1`.
    ///
    /// Negative is chain-like (each level has one child), positive is fan-like (one level
    /// holds everything), zero is neither. Signed because the two failure modes need
    /// different silhouettes, and collapsing them onto one end of a `0..=1` scale would
    /// make a corridor of single-child directories look like a flat directory of files.
    pub hierarchy_skew: Fx,
}

impl StructuralPrimitives {
    /// Whether this path has no children.
    ///
    /// `is_leaf` in `design/feature-system.md` §3.1. A file is always a leaf; so is an empty
    /// directory, which is why this is not the same question as "is this a file".
    #[must_use]
    pub const fn is_leaf(&self) -> bool {
        self.child_count == 0
    }

    /// Every descendant, files and directories together.
    #[must_use]
    pub const fn descendant_count(&self) -> u32 {
        self.descendant_file_count
            .saturating_add(self.descendant_dir_count)
    }

    /// Structural primitives for a file, which needs no walk to describe.
    #[must_use]
    pub fn leaf(depth: u16) -> Self {
        Self {
            depth,
            depth_profile: DepthProfile::leaf(),
            balance: BalanceScore::EVEN,
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branching_buckets_group_the_long_tail() {
        let mut h = BranchingHistogram::default();
        for children in [0, 1, 2, 3, 4, 7, 12, 20, 500] {
            h.observe(children);
        }
        assert_eq!(h.buckets(), &[1, 1, 1, 1, 1, 1, 1, 1, 1]);
        assert_eq!(h.total(), 9);
    }

    /// The distinction the histogram exists to preserve: same mean, different shape.
    #[test]
    fn equal_means_produce_different_histograms() {
        let mut bushy = BranchingHistogram::default();
        for _ in 0..3 {
            bushy.observe(3);
        }
        let mut spindly = BranchingHistogram::default();
        spindly.observe(9);
        spindly.observe(0);
        spindly.observe(0);

        // Nine children over three nodes, either way.
        assert_eq!(bushy.total(), spindly.total());
        assert_ne!(bushy.buckets(), spindly.buckets());
        assert_eq!(bushy.leaf_ratio(), Fx::ZERO);
        assert_eq!(spindly.leaf_ratio(), Fx::from_ratio(2, 3));
    }

    #[test]
    fn merging_histograms_sums_buckets() {
        let mut a = BranchingHistogram::default();
        a.observe(1);
        let mut b = BranchingHistogram::default();
        b.observe(1);
        b.observe(40);
        a.merge(&b);
        assert_eq!(a.buckets()[1], 2);
        assert_eq!(a.buckets()[8], 1);
    }

    #[test]
    fn depth_profile_shifts_children_one_level_down() {
        let mut dir = DepthProfile::default();
        dir.absorb_child(&DepthProfile::leaf());
        dir.absorb_child(&DepthProfile::leaf());
        assert_eq!(dir.levels(), &[0, 2]);

        let mut parent = DepthProfile::default();
        parent.absorb_child(&dir);
        assert_eq!(parent.levels(), &[0, 0, 2]);
        assert_eq!(parent.centre_of_mass(), Fx::from_int(2));
    }

    #[test]
    fn a_leaf_is_a_leaf_and_so_is_an_empty_directory() {
        let file = StructuralPrimitives::leaf(3);
        assert!(file.is_leaf());
        assert_eq!(file.depth, 3);
        assert_eq!(file.descendant_count(), 0);
        assert_eq!(file.balance, BalanceScore::EVEN);

        let empty_dir = StructuralPrimitives {
            depth: 1,
            ..StructuralPrimitives::default()
        };
        assert!(empty_dir.is_leaf());
    }

    #[test]
    fn descendant_count_saturates_rather_than_wrapping() {
        let huge = StructuralPrimitives {
            descendant_file_count: u32::MAX,
            descendant_dir_count: 10,
            ..StructuralPrimitives::default()
        };
        assert_eq!(huge.descendant_count(), u32::MAX);
    }
}
