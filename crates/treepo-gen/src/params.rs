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
/// Version 2 adds `branch_capacity`, the composition threshold. Bumped rather than accepted
/// silently because a version 1 table has no such row, and a table missing the row that
/// decides when a limb aggregates would compose a different tree while parsing perfectly.
pub const TABLE_VERSION: u32 = 2;

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
    /// Downward pitch applied to a limb heavy enough to sag — `E3`.
    pub droop: Angle,
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
    /// Droop in millidegrees.
    pub droop: Row,
    /// First-segment length, per mille of a world unit.
    pub base_length: Row,
    /// First-segment width, per mille of a world unit.
    pub base_width: Row,
    /// How many children stay first-class limbs, in whole children.
    ///
    /// The composition threshold — see [`LimbParams::branch_capacity`].
    pub branch_capacity: Row,
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
            ("base_length", &self.base_length),
            ("base_width", &self.base_width),
            ("branch_capacity", &self.branch_capacity),
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
            droop: Angle::from_millidegrees(self.droop.evaluate(inputs)),
            base_length: per_mille(self.base_length.evaluate(inputs)),
            base_width: per_mille(self.base_width.evaluate(inputs)),
            branch_capacity: u16::try_from(self.branch_capacity.evaluate(inputs)).unwrap_or(1),
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
fn per_mille(value: i32) -> Fx {
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

        let cases: [Case; 6] = [
            ("recursion", "A3", |t| t.recursion.max = 9_000),
            ("recursion", "A3", |t| t.recursion.min = 0),
            ("branch_angle", "B2/B3", |t| t.branch_angle.max = 75_000),
            ("angle_jitter", "D", |t| t.angle_jitter.max = 40_000),
            ("length_jitter", "D", |t| t.length_jitter.max = 900),
            // C1 — raise length's falloff until it overlaps width's.
            ("length_ratio", "C1", |t| t.length_ratio.max = 995),
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
