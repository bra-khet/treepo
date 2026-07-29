//! The skeleton parameter table — `F-SKEL-5`, and the mapping half of `F-SKEL-4`.
//!
//! `design/l-system-parameterization.md` §5 is a menu of choices, and §6 says the way to
//! settle them is to look at silhouettes and adjust one family at a time. That only works if
//! adjusting one costs an edit rather than a rebuild, which is what `F-SKEL-5` requires and
//! what this module provides: [`Table`] is parsed from `assets/params/lsystem.ron`, and
//! [`Table::limb_params`] is a pure function from one path's primitives to the knobs the
//! L-system turns.
//!
//! # The two-stage split, and why the seam is where it is
//!
//! Mapping runs in two stages, and only the second is data.
//!
//! 1. **Primitives → [`SkeletonInputs`]** — nine normalized drivers. This is *what the
//!    numbers mean*, and it is code because it is the Feature System's reading of itself,
//!    not a knob. "Hierarchy skew is how chain-like against fan-like a subtree is" does not
//!    become a different sentence because a silhouette looked wrong.
//! 2. **Drivers → [`LimbParams`]** — a base value plus a weighted sum of drivers, clamped to
//!    a stated range, once per output parameter. This is *how strongly each meaning shows*,
//!    which is exactly what §6 says gets tuned, so it is the part that lives in the file.
//!
//! Putting the seam anywhere else makes one of the two halves untunable or unreadable. A
//! table that mapped raw byte counts to angles would need the reader to know what a typical
//! byte count is; a table that only chose between named presets would not survive §6's
//! "adjust one parameter family at a time".
//!
//! # Normalization is against absolute scales, never against the repository
//!
//! [`Scales`] holds the constants that turn a raw count into a `0..=1` driver — the subtree
//! depth that reads as maximally deep, the language count that reads as maximally mixed.
//! They are constants in the table rather than maxima taken over the manifest, and that is a
//! correctness requirement rather than a convenience.
//!
//! If `depth` were normalized against the deepest subtree in the repository, then adding one
//! deeply-nested file anywhere would renormalize every limb and reshape the entire tree.
//! `AC-GROW-4` requires a one-file change to produce a *visible, confined* Grow, and
//! `AC-DET-1` requires the same repository to produce the same tree. Repository-relative
//! normalization loses both, in a way that would look like nondeterminism rather than like
//! the design decision it was. The cost is that the scales are a tuning liability — set
//! `depth_full_scale` too low and every repository looks equally deep — which is why they
//! are in the file where they can be seen and changed.
//!
//! # `G1`: the skeleton cannot read the clock, or anything derived from it
//!
//! The v0.1 row picks `G1` — age and churn influence the skeleton not at all; they live in
//! the material and Thrive layers. [`SkeletonInputs::from_record`] is the one place that
//! could violate it, and it destructures [`PathRecord`] so that `temporal` is named and
//! discarded in view of the reader rather than merely unused.
//!
//! There is a sharper reason than "the row says so", and it is what settles the conflict
//! between `G1` and `D1` recorded below. [`ChurnWindows`](treepo_model::primitives::temporal::ChurnWindows)
//! are measured against
//! [`Manifest::reference_time`](treepo_model::Manifest::reference_time), which is the newest
//! commit in the **repository**. So a commit to any one file moves the window for every
//! path at once. Had churn fed branch angles or noise, committing to `src/` would have
//! re-shaped `docs/` — every limb in the tree would move on every commit, which is
//! `AC-GROW-4` lost outright and Grow's cinematic transitions made unreadable. Age and churn
//! belong to materials precisely because a material can change without the tree moving.
//!
//! # The v0.1 row, and the one revision it needed
//!
//! `A3 + B2/B3 + C1 + D1 + E3 + F2 + G1`, from §5. `F2` (primary limb grouping) belongs to
//! `trunk.rs` and is not read here; the other six are.
//!
//! **`D1` is revised to `D1 + D3`, and `AC-SKEL-1` is the evidence.** `D1` alone is "noise
//! rises sharply with churn + skew", and `G1` removes churn from that pair — leaving
//! hierarchy skew as the single route to the alien, overgrown silhouette the design wants.
//! `AC-SKEL-1` asks for a "high-skew, **mixed-language**, **unconventional**" repository to
//! look visibly wilder than a clean one, and mixed-language and unconventional are precisely
//! the two signals `D3` adds and `D1` does not carry. `D1` as written cannot satisfy the
//! acceptance criterion that tests it. §4's "Mess / Chaos Signals" paragraph already lists
//! all four signals together, so this is a correction to §5's shorthand rather than a
//! departure from the design.
//!
//! What survives of `D1` is the half that matters and that `D3` does not state: the response
//! is *near-zero for clean repositories and rises sharply*, rather than `D2`'s always-on
//! shimmer. That is a property of the weights, and [`Table::validate`] does not enforce it —
//! it is what the silhouette review in a later sprint is for.

use alloc::string::String;
use core::fmt;
use serde::Deserialize;
use treepo_det::{Angle, Fx};
use treepo_model::PathRecord;
use treepo_model::primitives::structural::BranchingHistogram;

/// The compiled-in table. See `treepo-vcs::filter` for why `include_str!` rather than a
/// runtime read: this crate has no business resolving asset paths, and `no_std` means it
/// could not open the file if it wanted to.
///
/// `AC-SKEL-4` — edit the file, reload, see a different silhouette — is served by
/// [`Table::from_ron`], which the application calls with the contents of a file on disk.
/// Compiling a copy in is what makes the crate testable and what guarantees a usable table
/// exists when the user has not supplied one.
const BUILT_IN_RON: &str = include_str!("../../../assets/params/lsystem.ron");

/// The table format this crate understands.
///
/// Separate from [`treepo_model::SCHEMA_VERSION`]: a manifest and a parameter table version
/// independently, and a table edit must not invalidate a stored manifest.
///
/// Version 2 adds `branch_capacity`, the composition threshold. Version 3 adds the [`trunk`]
/// section. Version 4 adds `tropism` and the [`ground`] band, and replaces the trunk's
/// `basal_length` row with an aspect rule. Version 5 turns the trunk into a pipe column:
/// `flare`, `internode_aspect`, `internode_min`, `support_knee` and `support_beyond` join the
/// [`trunk`] section. Bumped rather than accepted silently each time, because an older table
/// parses perfectly while lacking a row that changes what the tree looks like.
///
/// [`trunk`]: TrunkTable
/// [`ground`]: GroundTable
pub const TABLE_VERSION: u32 = 5;

/// Children-per-node used as each [`BranchingHistogram`] bucket's representative value.
///
/// The histogram's buckets are `0, 1, 2, 3, 4, 5-8, 9-16, 17-32, 33+`; the last four are
/// ranges, so turning the distribution into a mean needs a value to stand for each. The
/// low end of a range would understate every bushy repository and the high end would
/// overstate every one, so these sit inside: the midpoint for the bounded ranges, and a
/// figure for `33+` that a directory of a few hundred files does not immediately saturate.
///
/// This duplicates knowledge that belongs to `treepo-model`, which is why the assertion
/// below ties the two together — a change to the number of buckets stops this crate
/// compiling rather than silently misweighting every repository.
const CHILDREN_PER_BUCKET: [u32; BranchingHistogram::BUCKETS] = [0, 1, 2, 3, 4, 6, 12, 24, 48];

const _: () = assert!(
    CHILDREN_PER_BUCKET.len() == BranchingHistogram::BUCKETS,
    "one representative child count per branching bucket"
);

/// Per mille — the unit every ratio in the table is written in. `1000` means `1.0`.
///
/// The same convention `assets/params/folder-signals.ron` established, for the same reason:
/// `N3` keeps floats out of anything reaching generated output, and in a file that exists to
/// be hand-tuned `750` reads as "0.75" where `750000` does not.
const PER_MILLE: i64 = 1000;

/// The knobs the L-system turns for one limb — the output of the whole module.
///
/// Every field is an exact integer quantity: [`Fx`] ratios and [`Angle`] rotations, never a
/// float. `design/l-system-parameterization.md` §3 is the list this is drawn from, minus the
/// axiom (which `trunk.rs` sizes from the whole repository rather than from one path) and
/// minus the termination threshold (which is one number for the tree, and lives on
/// [`Table`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimbParams {
    /// How many successive branch generations this limb runs — `A3`, hard-capped.
    ///
    /// Content below the cap is not truncated: `F-SKEL-7` aggregates it into a proportional
    /// container, so the cap costs detail rather than data (`P6`).
    pub recursion_depth: u8,
    /// The base branching angle δ, applied either side of the parent's direction.
    pub branch_angle: Angle,
    /// Maximum stochastic deviation from [`branch_angle`](Self::branch_angle), either way.
    ///
    /// Zero is a perfectly regular limb. The drawn value is a function of the path seed, so
    /// "stochastic" never means "different between two runs" (`N3`).
    pub angle_jitter: Angle,
    /// Length multiplier from one generation to the next — `r` in §2.2.
    pub length_ratio: Fx,
    /// Width multiplier from one generation to the next.
    ///
    /// `C1` requires this to exceed [`length_ratio`](Self::length_ratio) for every limb, so
    /// that thickness persists after length has fallen away and the primary limbs overlap
    /// into visible trunk mass near the origin. [`Table::validate`] enforces it.
    pub width_ratio: Fx,
    /// Maximum stochastic deviation from [`length_ratio`](Self::length_ratio), either way.
    pub length_jitter: Fx,
    /// Everything that bends this limb's heading after a segment — `E3` and its opposite.
    pub tropism: Tropism,
    /// Length of this limb's first segment, in world units.
    pub base_length: Fx,
    /// Width of this limb's first segment, in world units.
    pub base_width: Fx,
    /// How many children this limb may carry as first-class limbs of their own.
    ///
    /// The composition threshold. Below it, children stay hierarchical L-system instances
    /// seeded by their own paths (`F-SKEL-2`); at it, the excess collapses into aggregate
    /// containers (`F-SKEL-7`). It is a property of the *limb* rather than a global constant
    /// because a bushy, conventionally-named directory can carry more before it stops being
    /// readable than a lopsided one can.
    pub branch_capacity: u16,
}

/// What bends a heading after each segment — `E3`'s droop, and the two things that oppose it.
///
/// # Why droop alone was not enough
///
/// `E3` bends a limb *downward* in proportion to `sin(heading)`, which is correct physics for
/// a loaded branch and is the whole of the tropism the v0.1 row asked for. With nothing
/// pulling the other way, a branch angle of 36° over five generations plus the composition
/// fan walks a heading to horizontal and past it, and the tree stops standing up. Real trees
/// do not, because phototropism opposes gravity continuously.
///
/// So [`uplift`](Self::uplift) is the same expression with the opposite sign, and the turtle
/// applies their difference: one subtraction, no second code path.
/// `design/l-system-parameterization.md` §8 already names tropism forces as an addition that
/// does not change the parameterization contract.
///
/// # The ground band, and why `sin` cannot do it
///
/// A `sin`-scaled uplift is zero at straight down — the one heading it most needs to correct
/// is its own fixed point. A limb that reaches vertical-down stays there forever.
///
/// The ground band is the fix and is not scaled by anything: past
/// [`ground_engage`](Self::ground_engage) from vertical, a flat
/// [`ground_lift`](Self::ground_lift) rotates the heading back toward up every segment, and
/// it keeps doing so until the heading is inside
/// [`ground_release`](Self::ground_release). That gap is deliberate. With a single threshold
/// a limb chatters along it — corrected, released, corrected — and draws a visible zigzag on
/// the boundary. With two, it dips, is lifted clear, and then travels freely until it dips
/// again, which is the long shallow arc a real branch makes.
///
/// [`Table::validate`] refuses `release >= engage`, because that is a band with no hysteresis
/// in it and the chatter is what it was written to prevent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tropism {
    /// Downward pitch applied to a limb heavy enough to sag — `E3`.
    pub droop: Angle,
    /// Upward pitch opposing it, per segment.
    pub uplift: Angle,
    /// How far from vertical a heading must stray before the ground band engages.
    pub ground_engage: Angle,
    /// How far back toward vertical it must come before the band lets go.
    pub ground_release: Angle,
    /// The flat rotation applied toward vertical while the band is engaged.
    pub ground_lift: Angle,
}

impl Tropism {
    /// No bending at all — what a perfectly straight limb is grown with.
    ///
    /// The band is placed at straight down rather than at zero: a heading reaches exactly
    /// half a turn only by arriving there, so the band is off because nothing can cross it
    /// rather than because a lift of zero makes it harmless.
    pub const NONE: Self = Self {
        droop: Angle::ZERO,
        uplift: Angle::ZERO,
        ground_engage: Angle::HALF,
        ground_release: Angle::ZERO,
        ground_lift: Angle::ZERO,
    };
}

impl LimbParams {
    /// This limb as it grows from a branch that has already tapered to `carried`.
    ///
    /// `C1`'s width falloff applies from one generation to the next *inside* a derivation, and
    /// for a while nothing applied it between one limb and the next: every limb started from
    /// the width its own mass justified, so a limb four levels out drew as thick as one leaving
    /// the trunk and the whole tree rendered at a single weight. The falloff has to cross the
    /// composition boundary or it is not a falloff, it is a per-limb constant.
    ///
    /// The rule is the narrower of two claims, and both are needed:
    ///
    /// * **what it carries** — one more step of `width_ratio` past the branch it grafts onto,
    ///   which is exactly what the next generation *inside* that branch would have got, so a
    ///   joint between two limbs is indistinguishable from a joint within one;
    /// * **what it is** — its own mass-driven [`base_width`](Self::base_width), so a small
    ///   directory hanging off the trunk is drawn small rather than inheriting the trunk.
    ///
    /// Taking the minimum means a heavy subtree on a thin twig stays thin, which is right: the
    /// parent is thick *because* of what it carries, so a thin parent is already the claim that
    /// there is not much below it. Under equal drivers the inherited term always wins and the
    /// chain narrows strictly, one `width_ratio` per level.
    #[must_use]
    pub fn grafted_onto(self, carried: Fx) -> Self {
        Self {
            base_width: self.base_width.min(carried.mul(self.width_ratio)),
            ..self
        }
    }
}

/// The trunk column's knobs for one repository — the output of [`Table::trunk_params`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrunkParams {
    /// The collar's length as a multiple of the column's width — see
    /// [`TrunkTable::basal_aspect`]. Read through [`basal_length`](Self::basal_length).
    pub basal_aspect: Fx,
    /// The shortest a collar may be, whatever the aspect rule works out to.
    pub basal_min: Fx,
    /// How much wider the foot of the collar is than the pipe above it — see
    /// [`TrunkTable::flare`]. Read through [`flared`](Self::flared).
    pub flare: Fx,
    /// An internode's length as a multiple of the support that leaves at its top — see
    /// [`TrunkTable::internode_aspect`]. Read through
    /// [`internode_length`](Self::internode_length).
    pub internode_aspect: Fx,
    /// The shortest an internode may be, whatever the aspect rule works out to.
    pub internode_min: Fx,
    /// Total angular spread of the limbs leaving a column.
    pub fan: Angle,
    /// How many root-mass nodes sit at the base.
    pub root_cluster: u16,
    /// What share of its limbs' combined width a column occupies, in `0..=1`.
    pub packing: Fx,
    /// Combined limb width above which further mass counts only in part — see
    /// [`TrunkTable::support_knee`]. Read through [`support`](Self::support).
    pub support_knee: Fx,
    /// How much of the excess above the knee still counts, in `0..=1`.
    pub support_beyond: Fx,
    /// The share of the repository below which a top-level entry is grouped (`F2`).
    pub group_below: Fx,
}

impl TrunkParams {
    /// How wide a column carrying `combined` raw limb width is.
    ///
    /// Packed, so the limbs it supports overlap into it rather than sitting beside it, and
    /// soft-capped, so a monorepo with forty top-level entries does not draw a telephone pole.
    /// Above [`support_knee`](Self::support_knee) only
    /// [`support_beyond`](Self::support_beyond) of the excess counts — piecewise linear, and
    /// still strictly increasing, because a repository that grows must still read as having
    /// grown. `P6`: legibility bounds how much of the mass is *shown*, and nothing about the
    /// mass itself changes.
    #[must_use]
    pub fn support(&self, combined: Fx) -> Fx {
        let counted = if combined > self.support_knee {
            self.support_knee
                .add(combined.sub(self.support_knee).mul(self.support_beyond))
        } else {
            combined
        };
        counted.mul(self.packing)
    }

    /// How long the collar below the first departure should be, for a column `width` wide.
    ///
    /// The rule rather than a number, so `grow` and `place_group` cannot disagree about it —
    /// an `F2` group stem is the trunk column one level down, and the two being the same
    /// function is what keeps that true.
    #[must_use]
    pub fn basal_length(&self, width: Fx) -> Fx {
        width.mul(self.basal_aspect).max(self.basal_min)
    }

    /// How much vertical room a primary taking `drop` of the column's width needs to leave.
    ///
    /// Measured against the support that departs rather than against a length of its own, so
    /// the internodes sum to an aspect of the column's own width: a column keeps its
    /// proportions whether it carries three primaries or thirty, and the count decides only
    /// how the height is divided up.
    #[must_use]
    pub fn internode_length(&self, drop: Fx) -> Fx {
        drop.mul(self.internode_aspect).max(self.internode_min)
    }

    /// The column's width at the ground, where the buttress is widest.
    #[must_use]
    pub fn flared(&self, width: Fx) -> Fx {
        width.mul(self.flare)
    }
}

/// Every structural signal the skeleton is permitted to read, normalized.
///
/// Nine drivers, each a [`Fx`] in `0..=1` except [`skew`](Self::skew), which is signed. The
/// type exists to be a wall: [`Table`] evaluates against this and never sees a
/// [`PathRecord`], so a later sprint cannot reach for a timestamp without first widening
/// this struct — which is a visible edit to a documented boundary rather than one more field
/// access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SkeletonInputs {
    mass: Fx,
    depth: Fx,
    bushiness: Fx,
    skew: Fx,
    skew_abs: Fx,
    balance: Fx,
    convention: Fx,
    diversity: Fx,
    fragmentation: Fx,
}

impl SkeletonInputs {
    /// The drivers for one path.
    ///
    /// # `G1` lives in this function
    ///
    /// The destructure below names every field of [`PathRecord`], so `temporal` is visibly
    /// discarded rather than quietly unread, and a new primitive category cannot be added to
    /// the model without someone deciding here whether the skeleton may see it. The module
    /// header has the argument for why `temporal` is the one that must not be.
    ///
    /// `derived` is discarded too, and that one is a v0.1 scoping call rather than a
    /// constraint. §4 gives the skeleton exactly one of its signals — TODO density, "slight
    /// increase in stochasticity", with "enrichment doing the heavier lifting" — and every
    /// field of [`DerivedSignals`](treepo_model::DerivedSignals) is an `Option` that is
    /// `None` on most of the corpus. A driver that is absent wherever it could be tested is
    /// a driver tuned by guesswork.
    #[must_use]
    pub fn from_record(record: &PathRecord, scales: &Scales) -> Self {
        let PathRecord {
            path: _,
            kind: _,
            structural,
            size,
            // G1 — see above. This is the whole of the constraint.
            temporal: _,
            ownership,
            derived: _,
            folder_signal,
        } = record;

        // Relative to the parent, which is what §4 asks length and thickness to follow:
        // "larger subtrees produce longer and/or thicker primary limbs". Already `0..=1`.
        let mass = size.relative_bytes;

        let depth = ratio(
            i64::from(structural.max_subtree_depth),
            i64::from(scales.depth_full_scale),
        );

        // `mean_children_per_node` answers in milli-children so the mean keeps a fractional
        // part; the scale is written in whole children, because that is what a person
        // tuning the file is thinking in.
        let bushiness = ratio(
            mean_children_per_node(&structural.branching),
            i64::from(scales.bushiness_full_scale) * PER_MILLE,
        );

        // Already `-1..=1` and signed on purpose: the model records chain-like as negative
        // and fan-like as positive precisely so the two produce different silhouettes rather
        // than one shared "irregular" reading. The table gets both this and its magnitude,
        // because the two answer different questions — which way the structure leans, and
        // how far from ordinary it is in either direction.
        let skew = structural.hierarchy_skew.clamp(Fx::NEG_ONE, Fx::ONE);

        // `balance.kind` is deliberately left out: it is an `Option` that is `None` until
        // content classification has run, and its own documentation records that real values
        // cluster low. Two axes that are always present beat three where one is usually
        // absent and the third reads as imbalance when it is really silence.
        let balance = structural
            .balance
            .size
            .add(structural.balance.depth)
            .mul(Fx::HALF)
            .clamp(Fx::ZERO, Fx::ONE);

        // No recognized folder name is zero convention, not missing data. That is the
        // reading §4 asks for — "missing conventional folders" is itself the mess signal.
        let convention = folder_signal
            .as_ref()
            .map_or(Fx::ZERO, |signal| signal.effective_weight)
            .clamp(Fx::ZERO, Fx::ONE);

        let diversity = ratio(
            i64::from(size.language_count()),
            i64::from(scales.diversity_full_scale),
        );

        // A bus factor of 0 means nothing was attributed — no history, or an untracked path
        // — and 1 means one person carries the subtree. Both are zero fragmentation, and
        // conflating them is correct rather than convenient: with `G1` in force a repository
        // without history must produce the same skeleton as one owned by a single author,
        // which is what PRD §6 requires when it asks for a tree from filesystem primitives
        // alone. `N4` is untouched — a bus factor is a count of people, never an order.
        let fragmentation = ratio(
            i64::from(ownership.bus_factor_proxy().saturating_sub(1)),
            i64::from(scales.fragmentation_full_scale),
        );

        Self {
            mass: mass.clamp(Fx::ZERO, Fx::ONE),
            depth,
            bushiness,
            skew,
            skew_abs: skew.abs(),
            balance,
            convention,
            diversity,
            fragmentation,
        }
    }

    /// This subtree's size as a proportion of its parent's.
    #[must_use]
    pub const fn mass(&self) -> Fx {
        self.mass
    }

    /// How deep this subtree runs, against [`Scales::depth_full_scale`].
    #[must_use]
    pub const fn depth(&self) -> Fx {
        self.depth
    }

    /// Typical children per node, against [`Scales::bushiness_full_scale`].
    #[must_use]
    pub const fn bushiness(&self) -> Fx {
        self.bushiness
    }

    /// Chain-like (negative) against fan-like (positive), in `-1..=1`.
    #[must_use]
    pub const fn skew(&self) -> Fx {
        self.skew
    }

    /// How far the structure sits from ordinary, in either direction.
    #[must_use]
    pub const fn skew_abs(&self) -> Fx {
        self.skew_abs
    }

    /// How evenly size and depth are spread across immediate children.
    #[must_use]
    pub const fn balance(&self) -> Fx {
        self.balance
    }

    /// The folder-convention weight, or zero where the name signalled nothing.
    #[must_use]
    pub const fn convention(&self) -> Fx {
        self.convention
    }

    /// Language mix, against [`Scales::diversity_full_scale`].
    #[must_use]
    pub const fn diversity(&self) -> Fx {
        self.diversity
    }

    /// Ownership spread, against [`Scales::fragmentation_full_scale`]. Zero without history.
    #[must_use]
    pub const fn fragmentation(&self) -> Fx {
        self.fragmentation
    }
}

/// What each driver is measured against — see the module header on why these are constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scales {
    /// The subtree depth that reads as maximally deep.
    pub depth_full_scale: u16,
    /// The mean children-per-node that reads as maximally bushy.
    pub bushiness_full_scale: u16,
    /// The language count that reads as maximally mixed.
    pub diversity_full_scale: u16,
    /// The bus factor above one that reads as maximally fragmented.
    pub fragmentation_full_scale: u16,
}

/// How much each driver moves one output parameter, in that parameter's own unit.
///
/// A weight is the full-scale contribution: a driver at `1.0` contributes the whole weight,
/// one at `0.25` contributes a quarter of it, and a signed driver contributes negatively at
/// negative values. Signed weights are meaningful and used — `balance` reduces angle spread
/// where `skew` raises it.
///
/// `deny_unknown_fields` is load-bearing rather than tidy. Without it a typo'd driver name
/// parses as an unremarkable success and contributes nothing, so the user edits the file,
/// reloads, sees no change, and concludes that `AC-SKEL-4` does not work. Refusing the file
/// makes a misspelling a legible error instead of a silent one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Weights {
    /// Response to subtree size relative to the parent.
    pub mass: i32,
    /// Response to subtree depth.
    pub depth: i32,
    /// Response to children per node.
    pub bushiness: i32,
    /// Response to signed hierarchy skew — chain-like negative, fan-like positive.
    pub skew: i32,
    /// Response to the magnitude of hierarchy skew, irregular in either direction.
    pub skew_abs: i32,
    /// Response to how evenly the subtree is balanced.
    pub balance: i32,
    /// Response to the folder-convention weight.
    pub convention: i32,
    /// Response to language mix.
    pub diversity: i32,
    /// Response to ownership spread.
    pub fragmentation: i32,
}

impl Weights {
    /// The weighted sum of the drivers, in the row's unit.
    fn apply(&self, inputs: &SkeletonInputs) -> Fx {
        // Destructured so that adding a weight without wiring it up does not compile. A
        // silently-ignored weight is the same failure `deny_unknown_fields` closes from the
        // other side.
        let Self {
            mass,
            depth,
            bushiness,
            skew,
            skew_abs,
            balance,
            convention,
            diversity,
            fragmentation,
        } = *self;

        let terms = [
            (mass, inputs.mass),
            (depth, inputs.depth),
            (bushiness, inputs.bushiness),
            (skew, inputs.skew),
            (skew_abs, inputs.skew_abs),
            (balance, inputs.balance),
            (convention, inputs.convention),
            (diversity, inputs.diversity),
            (fragmentation, inputs.fragmentation),
        ];

        let mut sum = Fx::ZERO;
        for (weight, driver) in terms {
            sum = sum.add(Fx::from_int(weight).mul(driver));
        }
        sum
    }
}

/// One output parameter: where it starts, how far it may travel, and what moves it.
///
/// Every value is an integer in the parameter's own unit — per mille for a ratio,
/// millidegrees for an angle, milli-levels for the recursion depth. The clamp is not a
/// safety net bolted on afterwards; it is how the table states the range a parameter is
/// allowed to occupy, which is what makes an edited file survivable (`AC-SKEL-4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    /// The value for a path every driver reads zero on.
    pub base: i32,
    /// The lowest value any path may produce.
    pub min: i32,
    /// The highest value any path may produce.
    pub max: i32,
    /// Per-driver full-scale contributions.
    #[serde(default)]
    pub per: Weights,
}

impl Row {
    /// This row's value for one path, clamped to the row's range.
    fn evaluate(&self, inputs: &SkeletonInputs) -> i32 {
        let raw = Fx::from_int(self.base).add(self.per.apply(inputs)).round();
        // `Fx` saturates rather than wrapping, so an absurd weight yields a huge value here
        // rather than a wrapped one, and the clamp brings it back into the stated range.
        raw.clamp(i64::from(self.min), i64::from(self.max)) as i32
    }
}

/// Where the ground is, and how hard a limb works to stay off it — see [`Tropism`].
///
/// Three constants rather than three [`Row`]s: this is a statement about the *tree's* frame
/// rather than about any one limb, in the same way [`Table::min_length`] is. A limb permitted
/// its own idea of where the ground was could grant itself a lower one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroundTable {
    /// Degrees from vertical, in millidegrees, past which the band engages.
    pub engage: i32,
    /// Degrees from vertical, in millidegrees, inside which it lets go. Strictly less than
    /// [`engage`](Self::engage) — the gap is the hysteresis.
    pub release: i32,
    /// The flat rotation toward vertical applied per segment while engaged, in millidegrees.
    pub lift: i32,
}

/// The whole parameter table — `F-SKEL-5`.
///
/// Constructed from RON by [`Table::from_ron`] or [`Table::built_in`], and validated on the
/// way in: a table that reaches a caller has already been checked against the ranges
/// `design/l-system-parameterization.md` §3 states and the decision row §5 selects.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Table {
    /// Format version — must equal [`TABLE_VERSION`].
    pub version: u32,
    /// What the drivers are normalized against.
    pub scales: Scales,
    /// Segment length below which a limb stops branching, in per mille of a world unit.
    ///
    /// §3's termination threshold. One number for the tree rather than one per limb: it
    /// exists to stop microscopic branching, and a limb permitted its own threshold could
    /// grant itself an unbounded one.
    pub min_length: i32,
    /// Recursion depth in milli-levels — `1000` is one generation.
    pub recursion: Row,
    /// Base branching angle in millidegrees.
    pub branch_angle: Row,
    /// Stochastic angle deviation in millidegrees.
    pub angle_jitter: Row,
    /// Length falloff per generation, per mille.
    pub length_ratio: Row,
    /// Width falloff per generation, per mille.
    pub width_ratio: Row,
    /// Stochastic length deviation, per mille.
    pub length_jitter: Row,
    /// Droop in millidegrees — `E3`, downward.
    pub droop: Row,
    /// Upward tropism in millidegrees, opposing [`droop`](Self::droop) — see [`Tropism`].
    pub tropism: Row,
    /// The hysteretic band that keeps a tree off the ground — see [`Tropism`].
    pub ground: GroundTable,
    /// First-segment length, per mille of a world unit.
    pub base_length: Row,
    /// First-segment width, per mille of a world unit.
    pub base_width: Row,
    /// How many children stay first-class limbs, in whole children.
    ///
    /// The composition threshold — see [`LimbParams::branch_capacity`].
    pub branch_capacity: Row,
    /// The hybrid trunk — `F-SKEL-3`, `F2`.
    pub trunk: TrunkTable,
}

/// The trunk column's parameters — `F-SKEL-3`, `F2`, and `AC-SKEL-2`.
///
/// # The hybrid, revised
///
/// `design/visual-construction.md` settles the trunk on a hybrid rather than on a dedicated
/// column every repository shares, and that decision stands. What changed is *how* the mass
/// becomes a column. The first construction was co-origin: one short basal segment, every
/// primary leaving its tip, and the trunk was purely their overlap. Its arithmetic worked and
/// its silhouette did not — the overlap a fan of `F` radians leaves is `1/(packing × F)` of a
/// stem-width regardless of how many limbs there are, so a wide fan left the base half a
/// stem-width of trunk and the result read as an oversized seed rather than as something
/// planted.
///
/// The column is grown instead. Each primary claims an **internode** — the vertical room it
/// needs to leave — and the axis is the chain of them. Below the first departure the column
/// carries every primary; each departure drops that primary's share; and the whole thing sits
/// on a flared collar. Nothing draws an *arbitrary* trunk: the column exists only because
/// primaries need somewhere to leave from, so an empty repository still has none.
///
/// So the numbers divide three ways. [`basal_aspect`](Self::basal_aspect) and
/// [`flare`](Self::flare) shape the collar; [`internode_aspect`](Self::internode_aspect) sets
/// how much room a departure claims, and therefore how tall the column is;
/// [`packing`](Self::packing), [`support_knee`](Self::support_knee) and
/// [`support_beyond`](Self::support_beyond) turn carried limb width into a drawn one.
/// [`fan`](Self::fan) is left doing lateral work only.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrunkTable {
    /// The collar's length as a per-mille of the column's width — an aspect ratio.
    ///
    /// This was a [`Row`] with an absolute range, and it fought [`packing`](Self::packing) and
    /// lost. A column is as wide as its limbs' combined base widths, so a repository with
    /// eight top-level entries produced one near eight limb-widths across while the row held
    /// its length under one limb-length. The two numbers were each internally consistent and
    /// jointly described a disc.
    ///
    /// An aspect ratio cannot have that argument with anything, and it is closer to what
    /// `F-SKEL-3` actually asks for — "length/radius driven by total root mass / primary limb
    /// count" already says the length is mass-driven, and the column's width is where that
    /// mass is already summed up.
    ///
    /// The collar is *below the first departure*, so it stays stubby on purpose:
    /// [`Table::validate`] caps it at twice the width. Height comes from the internodes above
    /// it, which is the part that scales with what the repository actually carries.
    pub basal_aspect: i32,
    /// The shortest the collar may be, per mille of a world unit.
    ///
    /// A floor rather than a target: it is what an almost-empty repository's seed is drawn at,
    /// where the aspect rule has almost no width to work from.
    pub basal_min: i32,
    /// How much wider the foot of the collar is than the pipe above it, per mille.
    ///
    /// A pure pipe is a stack of cylinders and reads as machined. The flare is the buttress —
    /// the widening where a real trunk meets its roots — and it is drawn as an honest taper on
    /// the collar segment rather than as a render-time trick, because the skeleton is what
    /// Grow, picking and the digests all read.
    ///
    /// `1000` is no flare at all, and [`Table::validate`] refuses less: a trunk that narrows
    /// toward the ground is one that has already fallen over.
    pub flare: i32,
    /// How much vertical room a departing primary claims, per mille of the support it takes
    /// with it.
    ///
    /// This is the row that decides whether a tree has a trunk. A primary needs somewhere to
    /// *be* as it leaves — a point fan gives it nowhere, which is what made the old base read
    /// as a seed with rays coming out of it.
    ///
    /// Measured against the support that departs rather than in world units, so the internodes
    /// sum to an aspect of the column's own width and a column keeps its proportions at every
    /// scale. Thirty primaries divide the same height into more, shorter internodes rather than
    /// stacking thirty full-length blocks.
    pub internode_aspect: i32,
    /// The shortest an internode may be, per mille of a world unit.
    ///
    /// Held below the shortest limb the table can produce, for the reason
    /// [`basal_min`](Self::basal_min) is: an insertion zone that out-reaches the limb it makes
    /// room for has stopped being a joint and become a branch of its own.
    pub internode_min: i32,
    /// How much of the combined limb width a column occupies, per mille.
    ///
    /// A column carries limbs whose base widths sum to some total. At `1000` it is as wide as
    /// all of them laid side by side — limbs that touch but do not overlap. Below that they
    /// overlap, and that overlap is still where the trunk's *surface* comes from (`F-SKEL-3`).
    /// Above 1000 the column would be wider than what it carries, which is a trunk with the
    /// limbs stuck on rather than one made of them.
    pub packing: i32,
    /// Combined limb width above which further mass counts only in part, per mille of a world
    /// unit.
    ///
    /// Even with a grown column, a width strictly proportional to the sum of what it carries
    /// makes a forty-directory monorepo draw a telephone pole. `F2` already reduces the count;
    /// this bounds the width of what survives it.
    ///
    /// Not a ceiling — see [`support_beyond`](Self::support_beyond). `P6`: legibility bounds
    /// how much of the mass is shown, and honesty keeps the ordering, so a bigger repository
    /// still draws a wider base.
    pub support_knee: i32,
    /// How much of the width above [`support_knee`](Self::support_knee) still counts, per
    /// mille.
    ///
    /// [`Table::validate`] refuses zero. A hard cap would make every large repository draw the
    /// same base, which is the constant trunk `design/visual-construction.md` rejected arriving
    /// by another route.
    pub support_beyond: i32,
    /// Total angular spread of the limbs leaving a column, millidegrees.
    ///
    /// Lateral character: how far the crown reaches sideways, and the trunk-scale companion to
    /// `B2/B3`'s per-fork branching angle.
    ///
    /// It used to be the trunk's height budget as well — under the co-origin construction the
    /// overlap that *was* the trunk ended where the fan pulled two limbs apart, so widening the
    /// fan shortened the trunk. The column removed that coupling, and this row now does one
    /// job. `AC-SKEL-1`'s wilder repository can spread as far as it likes without losing its
    /// base.
    pub fan: Row,
    /// How many root-mass nodes sit at the base, in whole nodes.
    ///
    /// The root-boulder cluster `design/visual-construction.md` puts at the base to carry
    /// global signals, and the half of `AC-SKEL-2` that makes an empty repository read as a
    /// seed rather than as nothing.
    pub root_cluster: Row,
    /// A top-level entry below this share of the repository is grouped, per mille.
    ///
    /// `F2`: "small top-level directories grouped into fewer, thicker limbs". Zero disables
    /// grouping, which is `F1` — one primary limb per top-level directory.
    pub group_below: i32,
}

impl Table {
    /// The compiled-in v0.1 table.
    ///
    /// # Panics
    ///
    /// If the compiled-in table is malformed, which a unit test in this module rules out.
    #[must_use]
    pub fn built_in() -> Self {
        Self::from_ron(BUILT_IN_RON).expect("built-in parameter table must parse and validate")
    }

    /// Parses and validates a table from RON, for a caller supplying its own (`AC-SKEL-4`).
    ///
    /// # Errors
    ///
    /// [`TableError::Parse`] if the text is not a well-formed table, or one of the
    /// validation variants if it is well-formed but describes a tree the design does not
    /// permit.
    pub fn from_ron(text: &str) -> Result<Self, TableError> {
        let table: Self = ron::from_str(text).map_err(|error| TableError::Parse {
            detail: alloc::format!("{error}"),
        })?;
        table.validate()?;
        Ok(table)
    }

    /// Checks a parsed table against the design document's stated ranges.
    ///
    /// # Why a data-driven table validates itself
    ///
    /// `F-SKEL-5` makes the table editable, which makes it editable into nonsense. Every
    /// rule below is a sentence `design/l-system-parameterization.md` already states, turned
    /// from a paragraph someone might read into an error someone cannot miss. The
    /// alternative — clamping quietly to a sane range — would leave a user tuning a
    /// parameter that had stopped responding, which is a worse failure than a refused file.
    ///
    /// # Errors
    ///
    /// The first violated rule, naming the row and the decision it came from.
    pub fn validate(&self) -> Result<(), TableError> {
        if self.version != TABLE_VERSION {
            return Err(TableError::Version {
                found: self.version,
                expected: TABLE_VERSION,
            });
        }

        let rows = [
            ("recursion", &self.recursion),
            ("branch_angle", &self.branch_angle),
            ("angle_jitter", &self.angle_jitter),
            ("length_ratio", &self.length_ratio),
            ("width_ratio", &self.width_ratio),
            ("length_jitter", &self.length_jitter),
            ("droop", &self.droop),
            ("tropism", &self.tropism),
            ("base_length", &self.base_length),
            ("base_width", &self.base_width),
            ("branch_capacity", &self.branch_capacity),
            ("fan", &self.trunk.fan),
            ("root_cluster", &self.trunk.root_cluster),
        ];
        for (name, row) in rows {
            if row.min > row.max || row.base < row.min || row.base > row.max {
                return Err(TableError::Range {
                    row: name,
                    base: row.base,
                    min: row.min,
                    max: row.max,
                });
            }
        }

        // A3 — "hard-capped at 4–5 levels; deeper content is aggregated". The cap is the
        // point of the choice, so a table may not raise it, and a floor below one level
        // would produce a limb with no branches at all rather than a shallow one.
        if self.recursion.max > 5 * 1000 || self.recursion.min < 1000 {
            return Err(TableError::Decision {
                row: "recursion",
                decision: "A3",
                detail: "recursion must stay within 1..=5 levels; deeper content is aggregated",
            });
        }

        // §3 — the branching angle's practical range is 15°–60°. B2/B3 sit inside it.
        if self.branch_angle.min < 15_000 || self.branch_angle.max > 60_000 {
            return Err(TableError::Decision {
                row: "branch_angle",
                decision: "B2/B3",
                detail: "branching angles belong in 15..=60 degrees",
            });
        }

        // C1 — "length falls off faster than thickness". Faster for *every* limb, not on
        // average, so the two ranges may not overlap: an overlap would let some path taper
        // its width faster than its length, and the overlapping trunk mass F-SKEL-3 depends
        // on comes from width outlasting length near the origin.
        if self.length_ratio.max >= self.width_ratio.min {
            return Err(TableError::Decision {
                row: "length_ratio",
                decision: "C1",
                detail: "length_ratio.max must be below width_ratio.min so length always \
                         falls off faster than thickness",
            });
        }

        // §3 — stochasticity runs 0.0 to 0.4. For the length ratio that is per mille
        // directly; for the angle it is 0.4 of the widest permitted branching angle, which
        // is where 24000 millidegrees comes from.
        if self.length_jitter.min < 0 || self.length_jitter.max > 400 {
            return Err(TableError::Decision {
                row: "length_jitter",
                decision: "D",
                detail: "length jitter belongs in 0.0..=0.4",
            });
        }
        if self.angle_jitter.min < 0 || self.angle_jitter.max > 24_000 {
            return Err(TableError::Decision {
                row: "angle_jitter",
                decision: "D",
                detail: "angle jitter belongs in 0..=24 degrees — 0.4 of the widest angle",
            });
        }

        // §8 — tropism opposes droop, per segment. Bounded at 45° for the same reason the
        // branching angle is bounded: a per-segment rotation larger than a branching angle
        // stops being a bias and becomes the shape.
        if self.tropism.min < 0 || self.tropism.max > 45_000 {
            return Err(TableError::Decision {
                row: "tropism",
                decision: "§8",
                detail: "upward tropism belongs in 0..=45 degrees per segment",
            });
        }
        if self.ground.lift < 0 || self.ground.lift > 45_000 {
            return Err(TableError::Decision {
                row: "ground",
                decision: "§8",
                detail: "the ground lift belongs in 0..=45 degrees per segment",
            });
        }

        // The band has to sit past the horizontal — a tree whose limbs may not reach sideways
        // is a column — and short of straight down, where a root cluster already lives.
        if !(90_000..=170_000).contains(&self.ground.engage) {
            return Err(TableError::Decision {
                row: "ground",
                decision: "§8",
                detail: "the ground band engages between 90 and 170 degrees from vertical",
            });
        }

        // The whole point of the band. Equal thresholds are a band with no hysteresis, and a
        // limb then chatters along the boundary instead of arcing clear of it.
        if self.ground.release >= self.ground.engage || self.ground.release < 0 {
            return Err(TableError::Decision {
                row: "ground",
                decision: "§8",
                detail: "ground.release must be below ground.engage — the gap is the \
                         hysteresis, and without it a limb oscillates on the threshold",
            });
        }

        // Ratios are proportions. A falloff at or above 1.0 never terminates.
        for (name, row) in [
            ("length_ratio", &self.length_ratio),
            ("width_ratio", &self.width_ratio),
        ] {
            if row.min <= 0 || row.max >= 1000 {
                return Err(TableError::Decision {
                    row: name,
                    decision: "§3",
                    detail: "a falloff ratio must sit strictly between 0.0 and 1.0",
                });
            }
        }

        // A limb offers `2^generations` attachment sites and `A3` caps generations at 5, so a
        // capacity above 32 is a promise the grammar cannot keep — the excess children would
        // silently have nowhere to attach. A capacity below one leaves a limb unable to carry
        // even a single child, which would aggregate every repository down to its root.
        if self.branch_capacity.min < 1
            || i64::from(self.branch_capacity.max) > i64::from(self.max_sites())
        {
            return Err(TableError::Decision {
                row: "branch_capacity",
                decision: "A3",
                detail: "a limb cannot carry more children than its recursion cap offers \
                         attachment sites, nor fewer than one",
            });
        }

        // F-SKEL-3 — the collar sits below the first departure, and it stays stubby.
        //
        // Stated as an aspect rather than as an absolute length. An absolute cap fought
        // `packing`: the column's width is the sum of its limbs' widths, so a broad repository
        // produced one many limb-widths across while the cap held its length under a single
        // limb-length, and the base drew as a disc. The design document already says the
        // length is driven by root mass, and the column's width is where that mass has been
        // summed.
        //
        // The column's *height* is not this row's business — the internodes above supply it.
        // A collar longer than twice its own width would be a decorative pole under the first
        // branch, which is the dedicated trunk arriving by the back door.
        if self.trunk.basal_aspect < 1 || self.trunk.basal_aspect > 2000 {
            return Err(TableError::Decision {
                row: "basal_aspect",
                decision: "F-SKEL-3",
                detail: "the collar is minimal — it may be at most twice as long as it is \
                         wide, or the column stops being grown by its primaries",
            });
        }

        // The floor is what an almost-empty repository's seed is drawn at, so it is held below
        // the shortest limb the table can produce: at the floor, a collar out-reaching a limb
        // would be the lonely trunk `AC-SKEL-2` rejects.
        if self.trunk.basal_min < 1 || self.trunk.basal_min > self.base_length.min {
            return Err(TableError::Decision {
                row: "basal_min",
                decision: "F-SKEL-3",
                detail: "the collar's floor is positive and below the shortest limb the \
                         table can produce",
            });
        }

        // A trunk that narrows toward the ground is one that has already fallen over, and a
        // foot several times the pipe is a mound rather than a buttress.
        if !(1000..=3000).contains(&self.trunk.flare) {
            return Err(TableError::Decision {
                row: "flare",
                decision: "F-SKEL-3",
                detail: "the collar's foot is between one and three times the pipe above it — \
                         never narrower, or the trunk stands on a point",
            });
        }

        // The insertion zone is measured in the departing limb's own thickness. Past a few
        // multiples the column stops being a trunk with knots in it and becomes a ladder of
        // thin rungs, which is the Lego stack the pipe model exists to avoid.
        if self.trunk.internode_aspect < 1 || self.trunk.internode_aspect > 8000 {
            return Err(TableError::Decision {
                row: "internode_aspect",
                decision: "F-SKEL-3",
                detail: "an internode is the room one primary needs to leave, at most eight \
                         times the support it takes with it",
            });
        }
        if self.trunk.internode_min < 1 || self.trunk.internode_min > self.base_length.min {
            return Err(TableError::Decision {
                row: "internode_min",
                decision: "F-SKEL-3",
                detail: "an internode's floor is positive and below the shortest limb the \
                         table can produce, or the joint out-reaches the branch",
            });
        }

        // A column wider than the limbs it carries is a trunk with limbs attached, which is the
        // construction `design/visual-construction.md` rejected.
        if self.trunk.packing < 1 || self.trunk.packing > 1000 {
            return Err(TableError::Decision {
                row: "packing",
                decision: "F-SKEL-3",
                detail: "a column occupies between a thousandth and all of its limbs' combined \
                         width; the trunk's surface comes from their overlap, not from the \
                         column alone",
            });
        }

        // P6 — the projection that keeps a monorepo off the telephone pole. `beyond` at zero
        // is a hard ceiling, and a hard ceiling makes every large repository draw the same
        // base: the constant trunk, arriving by another route. Honesty keeps the ordering.
        if self.trunk.support_knee < 1 {
            return Err(TableError::Decision {
                row: "support_knee",
                decision: "P6",
                detail: "the knee is a positive width — below it, carried mass counts in full",
            });
        }
        if !(1..=1000).contains(&self.trunk.support_beyond) {
            return Err(TableError::Decision {
                row: "support_beyond",
                decision: "P6",
                detail: "mass past the knee must still count for something, or a bigger \
                         repository stops drawing a bigger base",
            });
        }

        if self.trunk.group_below < 0 || self.trunk.group_below > 1000 {
            return Err(TableError::Decision {
                row: "group_below",
                decision: "F2",
                detail: "the grouping threshold is a share of the repository, in 0..=1000",
            });
        }

        // A cluster of zero is an empty repository with nothing at all to show, which is the
        // half of AC-SKEL-2 that says "a seed", not "nothing".
        if self.trunk.root_cluster.min < 1 {
            return Err(TableError::Decision {
                row: "root_cluster",
                decision: "AC-SKEL-2",
                detail: "every tree has at least one root node — an empty repository shows a \
                         seed and a root cluster, never nothing",
            });
        }

        if self.min_length <= 0 {
            return Err(TableError::Decision {
                row: "min_length",
                decision: "§3",
                detail: "the termination threshold must be positive or branching never stops",
            });
        }

        // Every scale is a divisor.
        let Scales {
            depth_full_scale,
            bushiness_full_scale,
            diversity_full_scale,
            fragmentation_full_scale,
        } = self.scales;
        for (name, scale) in [
            ("depth_full_scale", depth_full_scale),
            ("bushiness_full_scale", bushiness_full_scale),
            ("diversity_full_scale", diversity_full_scale),
            ("fragmentation_full_scale", fragmentation_full_scale),
        ] {
            if scale == 0 {
                return Err(TableError::Scale { scale: name });
            }
        }

        Ok(())
    }

    /// The L-system parameters for one path — the mapping `F-SKEL-4` describes.
    #[must_use]
    pub fn limb_params(&self, inputs: &SkeletonInputs) -> LimbParams {
        LimbParams {
            // Milli-levels to levels. Rounding rather than truncating: a table asking for
            // 3.6 levels means "nearer four than three", and truncation would make every
            // fractional tuning read as the level below.
            recursion_depth: u8::try_from(
                (i64::from(self.recursion.evaluate(inputs)) + PER_MILLE / 2) / PER_MILLE,
            )
            .unwrap_or(u8::MAX),
            branch_angle: Angle::from_millidegrees(self.branch_angle.evaluate(inputs)),
            angle_jitter: Angle::from_millidegrees(self.angle_jitter.evaluate(inputs)),
            length_ratio: per_mille(self.length_ratio.evaluate(inputs)),
            width_ratio: per_mille(self.width_ratio.evaluate(inputs)),
            length_jitter: per_mille(self.length_jitter.evaluate(inputs)),
            tropism: Tropism {
                droop: Angle::from_millidegrees(self.droop.evaluate(inputs)),
                uplift: Angle::from_millidegrees(self.tropism.evaluate(inputs)),
                ground_engage: Angle::from_millidegrees(self.ground.engage),
                ground_release: Angle::from_millidegrees(self.ground.release),
                ground_lift: Angle::from_millidegrees(self.ground.lift),
            },
            base_length: per_mille(self.base_length.evaluate(inputs)),
            base_width: per_mille(self.base_width.evaluate(inputs)),
            branch_capacity: u16::try_from(self.branch_capacity.evaluate(inputs)).unwrap_or(1),
        }
    }

    /// The trunk parameters for one repository (`F-SKEL-3`, `F2`).
    ///
    /// `inputs` are the *root's* drivers, so the basal axiom and the fan respond to the
    /// repository as a whole — "sized from total root mass and primary limb count" — rather
    /// than to any one directory.
    #[must_use]
    pub fn trunk_params(&self, inputs: &SkeletonInputs) -> TrunkParams {
        TrunkParams {
            basal_aspect: per_mille(self.trunk.basal_aspect),
            basal_min: per_mille(self.trunk.basal_min),
            flare: per_mille(self.trunk.flare),
            internode_aspect: per_mille(self.trunk.internode_aspect),
            internode_min: per_mille(self.trunk.internode_min),
            fan: Angle::from_millidegrees(self.trunk.fan.evaluate(inputs)),
            root_cluster: u16::try_from(self.trunk.root_cluster.evaluate(inputs)).unwrap_or(1),
            packing: per_mille(self.trunk.packing),
            support_knee: per_mille(self.trunk.support_knee),
            support_beyond: per_mille(self.trunk.support_beyond),
            group_below: per_mille(self.trunk.group_below),
        }
    }

    /// The segment length below which branching stops.
    #[must_use]
    pub fn min_length(&self) -> Fx {
        per_mille(self.min_length)
    }

    /// `A3`'s cap, in whole hierarchy levels.
    ///
    /// The recursion row governs two related things and it is worth naming both. Per limb,
    /// its evaluated value is how many generations that limb's own L-system runs — how many
    /// attachment sites it offers. Table-wide, its *maximum* is how many levels of repository
    /// hierarchy get limbs at all, after which a subtree becomes one container. Both are
    /// "hard-capped at 4–5 levels; deeper content is aggregated"; they are the same cap read
    /// at the two scales `F-SKEL-2`'s hierarchical composition operates on.
    #[must_use]
    pub fn max_levels(&self) -> u8 {
        u8::try_from((i64::from(self.recursion.max) + PER_MILLE / 2) / PER_MILLE).unwrap_or(u8::MAX)
    }

    /// How many attachment sites a limb at the recursion cap can offer.
    #[must_use]
    pub fn max_sites(&self) -> u16 {
        crate::lsystem::grammar::sites_for(self.max_levels())
    }
}

/// Why a parameter table was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableError {
    /// The text is not a well-formed table.
    Parse {
        /// The RON parser's message, including its position.
        detail: String,
    },
    /// The table was written for a different format version.
    Version {
        /// The version the file declares.
        found: u32,
        /// The version this build understands.
        expected: u32,
    },
    /// A row's base sits outside its own range, or the range is inverted.
    Range {
        /// The row that failed.
        row: &'static str,
        /// Its declared base.
        base: i32,
        /// Its declared minimum.
        min: i32,
        /// Its declared maximum.
        max: i32,
    },
    /// A row contradicts a choice the design document has already made.
    Decision {
        /// The row that failed.
        row: &'static str,
        /// The decision-menu letter or section it contradicts.
        decision: &'static str,
        /// What the design requires.
        detail: &'static str,
    },
    /// A normalization scale is zero, which no driver can be divided by.
    Scale {
        /// The scale that is zero.
        scale: &'static str,
    },
}

impl fmt::Display for TableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse { detail } => write!(f, "parameter table is malformed: {detail}"),
            Self::Version { found, expected } => write!(
                f,
                "parameter table is version {found}, this build reads version {expected}"
            ),
            Self::Range {
                row,
                base,
                min,
                max,
            } => write!(f, "`{row}` has base {base} outside its range {min}..={max}"),
            Self::Decision {
                row,
                decision,
                detail,
            } => write!(f, "`{row}` contradicts decision {decision}: {detail}"),
            Self::Scale { scale } => write!(f, "`{scale}` is zero and every driver divides by it"),
        }
    }
}

impl core::error::Error for TableError {}

/// A per-mille integer as a ratio.
///
/// `pub(crate)` because [`PER_MILLE`] is the whole crate's unit convention rather than this
/// module's — `materials.ron` writes its ratios the same way `lsystem.ron` does, and two
/// conversions that rounded differently would be two subtly different trees.
pub(crate) fn per_mille(value: i32) -> Fx {
    Fx::from_ratio(i64::from(value), PER_MILLE)
}

/// `part / whole`, clamped to `0..=1`. Zero for a zero denominator.
fn ratio(part: i64, whole: i64) -> Fx {
    if whole <= 0 {
        return Fx::ZERO;
    }
    Fx::from_ratio(part, whole).clamp(Fx::ZERO, Fx::ONE)
}

/// Mean children per observed node, from the bucketed distribution.
///
/// Scaled by [`PER_MILLE`] so the division keeps a fractional part: a subtree averaging 2.5
/// children reads as 2500, and [`ratio`] divides by a full scale in the same unit.
fn mean_children_per_node(branching: &BranchingHistogram) -> i64 {
    let total = branching.total();
    if total == 0 {
        return 0;
    }
    let weighted: i64 = branching
        .buckets()
        .iter()
        .zip(CHILDREN_PER_BUCKET)
        .map(|(&count, children)| i64::from(count) * i64::from(children) * PER_MILLE)
        .sum();
    weighted / (total as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use treepo_det::OrderedMap;
    use treepo_model::primitives::folder_signal::FolderSignal;
    use treepo_model::primitives::structural::{BalanceScore, DepthProfile, StructuralPrimitives};
    use treepo_model::primitives::temporal::{ChurnWindows, TemporalPrimitives};
    use treepo_model::{LanguageId, NodeKind, RepoPath};

    fn record(path: &str) -> PathRecord {
        PathRecord::new(RepoPath::new(path.as_bytes()).unwrap(), NodeKind::Directory)
    }

    /// A clean, conventional, single-language, balanced, shallow directory.
    fn orderly() -> PathRecord {
        let mut record = record("src");
        record.structural = StructuralPrimitives {
            depth: 1,
            child_count: 4,
            descendant_file_count: 30,
            descendant_dir_count: 4,
            max_subtree_depth: 2,
            branching: histogram(&[3, 3, 3, 3]),
            depth_profile: DepthProfile::leaf(),
            balance: BalanceScore {
                size: Fx::from_ratio(9, 10),
                depth: Fx::from_ratio(9, 10),
                kind: None,
            },
            hierarchy_skew: Fx::ZERO,
        };
        record.size.relative_bytes = Fx::from_ratio(1, 2);
        record.size.language_bytes = languages(1);
        record.folder_signal = Some(FolderSignal::unmodulated(
            "src".into(),
            Fx::from_ratio(95, 100),
            1,
        ));
        record
    }

    /// The same size of directory, deep, lopsided, many-languaged and named nothing.
    fn wild() -> PathRecord {
        let mut record = record("stuff");
        record.structural = StructuralPrimitives {
            depth: 1,
            child_count: 17,
            descendant_file_count: 30,
            descendant_dir_count: 12,
            max_subtree_depth: 9,
            branching: histogram(&[17, 0, 0, 1, 40]),
            depth_profile: DepthProfile::leaf(),
            balance: BalanceScore {
                size: Fx::from_ratio(1, 10),
                depth: Fx::from_ratio(1, 10),
                kind: None,
            },
            hierarchy_skew: Fx::from_ratio(8, 10),
        };
        record.size.relative_bytes = Fx::from_ratio(1, 2);
        // Six languages against the orderly directory's one. `AC-SKEL-1` names
        // "mixed-language" as one of the three signals that must make a repository read as
        // wilder, so a fixture without it would test half the criterion.
        record.size.language_bytes = languages(6);
        record
    }

    /// `count` distinct languages sharing the subtree's bytes evenly.
    fn languages(count: u16) -> OrderedMap<LanguageId, u64> {
        (0..count).map(|id| (LanguageId::new(id), 1_000)).collect()
    }

    fn histogram(children: &[u32]) -> BranchingHistogram {
        let mut histogram = BranchingHistogram::default();
        for &count in children {
            histogram.observe(count);
        }
        histogram
    }

    fn inputs_for(record: &PathRecord) -> SkeletonInputs {
        SkeletonInputs::from_record(record, &Table::built_in().scales)
    }

    #[test]
    fn the_built_in_table_parses_and_validates() {
        let table = Table::built_in();
        assert_eq!(table.version, TABLE_VERSION);
        assert!(table.validate().is_ok());
    }

    /// `AC-SKEL-1`, at the level this sprint can reach: one table, two repositories, visibly
    /// different parameters. The silhouette itself is a later sprint's evidence.
    #[test]
    fn a_clean_directory_and_a_chaotic_one_diverge_from_one_table() {
        let table = Table::built_in();
        let clean = table.limb_params(&inputs_for(&orderly()));
        let messy = table.limb_params(&inputs_for(&wild()));

        assert!(
            messy.angle_jitter > clean.angle_jitter,
            "a high-skew, mixed, unconventional directory must be noisier: \
             clean {:?}, messy {:?}",
            clean.angle_jitter,
            messy.angle_jitter
        );
        assert!(
            messy.branch_angle > clean.branch_angle,
            "and must spread wider: clean {:?}, messy {:?}",
            clean.branch_angle,
            messy.branch_angle
        );
        assert!(
            messy.length_jitter > clean.length_jitter,
            "clean {:?}, messy {:?}",
            clean.length_jitter,
            messy.length_jitter
        );
    }

    /// The recorded evidence for revising the row's `D1` to `D1 + D3`.
    ///
    /// This directory is mixed-language and unconventionally named, and its hierarchy is as
    /// even and unskewed as the orderly one's. `AC-SKEL-1` names all three signals, so it
    /// must still read as wilder. Under `D1` alone — noise from churn and skew, with `G1`
    /// removing churn — the two would be bit-identical, because skew is the only thing `D1`
    /// carries and these two fixtures agree on it exactly. That is the whole argument for
    /// the revision, and it fails as a test rather than resting in a comment.
    #[test]
    fn mixed_and_unconventional_reads_as_wilder_without_any_skew() {
        let table = Table::built_in();

        let mut plain = orderly();
        plain.size.language_bytes = languages(6);
        plain.folder_signal = None;

        let clean = inputs_for(&orderly());
        let mixed = inputs_for(&plain);
        assert_eq!(
            clean.skew_abs(),
            mixed.skew_abs(),
            "the fixtures must agree on skew, or this proves nothing"
        );

        assert!(
            table.limb_params(&mixed).angle_jitter > table.limb_params(&clean).angle_jitter,
            "D3's signals must move the silhouette on their own"
        );
    }

    /// `D1`'s surviving half: a clean repository is near-deterministic, not merely calmer
    /// than a messy one. `D2` — "always a little noise" — is the choice this rules out.
    #[test]
    fn a_clean_directory_is_near_deterministic() {
        let table = Table::built_in();
        let clean = table.limb_params(&inputs_for(&orderly()));
        let millidegrees = i64::from(clean.angle_jitter.to_millidegrees());

        assert!(
            millidegrees * 5 < i64::from(table.angle_jitter.max),
            "a clean directory should sit in the bottom fifth of the jitter range, \
             not merely below the top: {millidegrees} of {}",
            table.angle_jitter.max
        );
        assert!(
            millidegrees > 0,
            "but not at zero — D1 asks for near-deterministic, not sterile"
        );
    }

    /// `G1`, executable. Every temporal field is filled with values a churning, ancient,
    /// bursty path would carry, and the skeleton must not move by one bit.
    #[test]
    fn no_temporal_primitive_reaches_the_skeleton() {
        let table = Table::built_in();
        let quiet = orderly();
        let before = table.limb_params(&inputs_for(&quiet));

        let mut churning = quiet.clone();
        churning.temporal = TemporalPrimitives {
            first_commit_time: Some(1_000_000_000),
            last_commit_time: Some(1_900_000_000),
            commit_count: 50_000,
            churn: ChurnWindows {
                days_30: 900_000,
                days_90: 2_000_000,
                days_365: 8_000_000,
                lifetime: 40_000_000,
            },
            recency_heat: Fx::ONE,
            burstiness: Fx::ONE,
            stability: Some(Fx::ZERO),
        };

        assert_eq!(
            before,
            table.limb_params(&inputs_for(&churning)),
            "G1: age and churn live in the material and Thrive layers, not the skeleton"
        );
    }

    /// The other half of `G1`: a repository with no history at all must produce the same
    /// skeleton as one with a single owner (PRD §6).
    #[test]
    fn a_repository_without_history_grows_the_same_skeleton() {
        let table = Table::built_in();
        let historyless = orderly();
        assert_eq!(historyless.ownership.bus_factor_proxy(), 0);

        let inputs = inputs_for(&historyless);
        assert_eq!(inputs.fragmentation(), Fx::ZERO);
        assert!(table.limb_params(&inputs).recursion_depth >= 1);
    }

    /// `A3`. Nothing a table or a repository can say raises the cap.
    #[test]
    fn recursion_is_hard_capped_whatever_the_repository_looks_like() {
        let table = Table::built_in();
        let mut absurd = wild();
        absurd.structural.max_subtree_depth = u16::MAX;
        absurd.structural.branching = histogram(&[u32::MAX; 4]);

        let depth = table.limb_params(&inputs_for(&absurd)).recursion_depth;
        assert!(
            (1..=5).contains(&depth),
            "A3 caps recursion at 4-5 levels; got {depth}"
        );
    }

    /// `C1`, as a property of every limb rather than of the table's midpoint.
    #[test]
    fn thickness_always_outlasts_length() {
        let table = Table::built_in();
        for record in [orderly(), wild()] {
            let params = table.limb_params(&inputs_for(&record));
            assert!(
                params.width_ratio > params.length_ratio,
                "C1: {:?} must taper slower than {:?}",
                params.width_ratio,
                params.length_ratio
            );
        }
    }

    /// The drivers must not be normalized against the repository — see the module header.
    /// Two identical subtrees produce identical parameters regardless of what else exists.
    #[test]
    fn a_limbs_parameters_depend_on_that_limb_alone() {
        let table = Table::built_in();
        let one = inputs_for(&orderly());
        let two = inputs_for(&orderly());
        assert_eq!(one, two);
        assert_eq!(table.limb_params(&one), table.limb_params(&two));
    }

    #[test]
    fn a_table_from_a_future_version_is_refused() {
        let current = alloc::format!("version: {TABLE_VERSION}");
        assert_eq!(
            BUILT_IN_RON.matches(current.as_str()).count(),
            1,
            "the built-in table must declare `{current}` exactly once"
        );

        let next = TABLE_VERSION + 1;
        let text = BUILT_IN_RON.replace(&current, &alloc::format!("version: {next}"));
        assert!(matches!(
            Table::from_ron(&text),
            Err(TableError::Version { found, expected })
                if found == next && expected == TABLE_VERSION
        ));
    }

    /// `C1`'s falloff has to survive the composition boundary, which is what
    /// [`LimbParams::grafted_onto`] is for. Under equal drivers — the same params grafted onto
    /// themselves, level after level — the chain must narrow strictly, by exactly one
    /// `width_ratio` per step, so a joint between limbs is the same joint as one inside a limb.
    ///
    /// This is the unit half. `compose`'s `no_limb_is_wider_than_the_branch_it_grows_from`
    /// is the same claim over a grown tree, where the drivers are not equal.
    #[test]
    fn a_graft_narrows_a_limb_by_exactly_one_step_of_the_falloff() {
        let table = Table::built_in();
        let params = table.limb_params(&SkeletonInputs::default());
        assert!(
            params.width_ratio < Fx::ONE,
            "a falloff that does not fall is not a falloff"
        );

        let mut level = params;
        let mut widths = alloc::vec![level.base_width];
        for _ in 0..5 {
            level = params.grafted_onto(level.base_width);
            widths.push(level.base_width);
        }

        for pair in widths.windows(2) {
            assert!(
                pair[1] < pair[0],
                "width did not decrease across a graft: {:?}",
                widths
            );
            assert_eq!(
                pair[1],
                pair[0].mul(params.width_ratio),
                "a graft must apply the falloff and nothing else"
            );
        }
    }

    /// The other half of the rule: a graft never *widens* a limb its own mass says is small.
    /// Inheriting the trunk's width would make every leaf directory look like a main branch.
    #[test]
    fn a_graft_onto_something_enormous_leaves_a_small_limb_small() {
        let table = Table::built_in();
        let params = table.limb_params(&SkeletonInputs::default());
        let grafted = params.grafted_onto(Fx::from_int(10_000));

        assert_eq!(
            grafted.base_width, params.base_width,
            "its own mass is the ceiling when the branch it hangs from is wider"
        );
    }

    /// Every validation rule must actually fire. A rule nobody has seen fail is a rule that
    /// might be checking the wrong field, and these are the only thing standing between an
    /// editable table (`F-SKEL-5`) and a tree the design does not permit.
    ///
    /// The table is mutated in Rust rather than by patching the RON text: a text edit would
    /// couple the test to the file's line layout, and reformatting the asset should not
    /// break the tests that guard it.
    #[test]
    fn each_design_rule_refuses_the_edit_that_breaks_it() {
        /// One edit that should be refused: the row it targets, the decision it violates,
        /// and the edit itself.
        type Case = (&'static str, &'static str, fn(&mut Table));

        let cases: [Case; 17] = [
            ("recursion", "A3", |t| t.recursion.max = 9_000),
            ("recursion", "A3", |t| t.recursion.min = 0),
            ("branch_angle", "B2/B3", |t| t.branch_angle.max = 75_000),
            ("angle_jitter", "D", |t| t.angle_jitter.max = 40_000),
            ("length_jitter", "D", |t| t.length_jitter.max = 900),
            // C1 — raise length's falloff until it overlaps width's.
            ("length_ratio", "C1", |t| t.length_ratio.max = 995),
            // §8 — a per-segment bend larger than a branching angle is not a bias.
            ("tropism", "§8", |t| t.tropism.max = 90_000),
            ("ground", "§8", |t| t.ground.lift = 90_000),
            // A band inside the horizontal makes a column; one at straight down never fires.
            ("ground", "§8", |t| t.ground.engage = 40_000),
            ("ground", "§8", |t| t.ground.engage = 179_000),
            // The hysteresis itself: release must sit strictly inside engage.
            ("ground", "§8", |t| t.ground.release = t.ground.engage),
            // F-SKEL-3 — a collar longer than twice its width has become the trunk.
            ("basal_aspect", "F-SKEL-3", |t| t.trunk.basal_aspect = 2_400),
            // A trunk narrower at the ground than above it has already fallen over.
            ("flare", "F-SKEL-3", |t| t.trunk.flare = 900),
            ("flare", "F-SKEL-3", |t| t.trunk.flare = 3_500),
            // An insertion zone many times the branch it makes room for is a ladder rung.
            ("internode_aspect", "F-SKEL-3", |t| {
                t.trunk.internode_aspect = 9_000;
            }),
            ("internode_min", "F-SKEL-3", |t| {
                t.trunk.internode_min = t.base_length.min + 1;
            }),
            // P6 — a hard ceiling makes every large repository draw the same base, which is
            // the constant trunk the construction rejected, arriving by another route.
            ("support_beyond", "P6", |t| t.trunk.support_beyond = 0),
        ];

        for (row, decision, break_it) in cases {
            let mut table = Table::built_in();
            break_it(&mut table);
            match table.validate() {
                Err(TableError::Decision {
                    row: found_row,
                    decision: found_decision,
                    ..
                }) => {
                    assert_eq!(found_row, row);
                    assert_eq!(found_decision, decision);
                }
                other => panic!("editing `{row}` past {decision} was accepted: {other:?}"),
            }
        }
    }

    /// A misspelled driver must be an error, not a weight that silently does nothing —
    /// otherwise `AC-SKEL-4` looks broken to the person tuning the file.
    #[test]
    fn a_misspelled_driver_is_refused_rather_than_ignored() {
        // The needle must be a weight and not one of the file's own worked examples — the
        // first draft of this test misspelled a name inside a comment and asserted against
        // an unmodified table for its trouble.
        let needle = "skew_abs: 14000";
        assert_eq!(
            BUILT_IN_RON.matches(needle).count(),
            1,
            "`{needle}` must name exactly one weight in the table"
        );

        let broken = BUILT_IN_RON.replace(needle, "skewabs: 14000");
        let outcome = Table::from_ron(&broken);
        assert!(
            matches!(outcome, Err(TableError::Parse { .. })),
            "an unknown weight name must refuse the file, got {outcome:?}"
        );
    }

    #[test]
    fn an_inverted_range_is_refused() {
        let mut table = Table::built_in();
        table.droop.min = table.droop.max + 1;
        assert!(matches!(
            table.validate(),
            Err(TableError::Range { row: "droop", .. })
        ));

        // And a base outside its own range, which reads as a typo far more often.
        let mut table = Table::built_in();
        table.base_width.base = table.base_width.max + 1;
        assert!(matches!(
            table.validate(),
            Err(TableError::Range {
                row: "base_width",
                ..
            })
        ));
    }

    #[test]
    fn a_zero_scale_is_refused() {
        let mut table = Table::built_in();
        table.scales.diversity_full_scale = 0;
        assert!(matches!(
            table.validate(),
            Err(TableError::Scale {
                scale: "diversity_full_scale"
            })
        ));
    }

    #[test]
    fn errors_say_what_to_change() {
        let error = TableError::Decision {
            row: "length_ratio",
            decision: "C1",
            detail: "length_ratio.max must be below width_ratio.min",
        };
        let text = error.to_string();
        assert!(text.contains("length_ratio"), "{text}");
        assert!(text.contains("C1"), "{text}");
    }

    /// The bucket representatives must turn a distribution into a mean that tracks it.
    #[test]
    fn mean_children_tracks_the_distribution() {
        assert_eq!(mean_children_per_node(&BranchingHistogram::default()), 0);
        // Three nodes of three children each: exactly 3.000.
        assert_eq!(mean_children_per_node(&histogram(&[3, 3, 3])), 3000);
        // One node of nine and two leaves: the 9-16 bucket stands at 12, so 4.000.
        assert_eq!(mean_children_per_node(&histogram(&[9, 0, 0])), 4000);
        // A bushier distribution must read higher than a spindlier one of equal total.
        assert!(mean_children_per_node(&histogram(&[3, 3, 3])) > 0);
    }

    /// Drivers stay inside their stated ranges however extreme the record.
    #[test]
    fn drivers_are_bounded_by_construction() {
        let table = Table::built_in();
        let mut extreme = wild();
        extreme.structural.max_subtree_depth = u16::MAX;
        extreme.structural.hierarchy_skew = Fx::from_int(9);
        extreme.size.relative_bytes = Fx::from_int(4);

        let inputs = SkeletonInputs::from_record(&extreme, &table.scales);
        for (name, value) in [
            ("mass", inputs.mass()),
            ("depth", inputs.depth()),
            ("bushiness", inputs.bushiness()),
            ("balance", inputs.balance()),
            ("convention", inputs.convention()),
            ("diversity", inputs.diversity()),
            ("fragmentation", inputs.fragmentation()),
            ("skew_abs", inputs.skew_abs()),
        ] {
            assert!(
                (Fx::ZERO..=Fx::ONE).contains(&value),
                "{name} left 0..=1: {value:?}"
            );
        }
        assert!((Fx::NEG_ONE..=Fx::ONE).contains(&inputs.skew()));
    }
}
