//! `F-MAT-3` — logarithmic, soft-clamped, floored, and quota'd.
//!
//! > Size normalization is logarithmic with a soft clamp, a **minimum representation floor**
//! > guaranteeing every surviving path a visible pixel budget, and a minimum visible quota
//! > per significant contributor (`P7`).
//!
//! One requirement, two mechanisms, and they are separated here because they normalize
//! different things. [`Normalize::budget`] turns a byte count into how much of the picture a
//! *path* may occupy; [`Normalize::allocate`] turns contribution shares into how much of one
//! limb's [`Mosaic`] each *contributor* holds. Both exist to stop a large thing erasing a small
//! one, which is why `F-MAT-3` states them in one sentence.
//!
//! [`Normalize::mosaic`] is the two joined: a node's budget decides how finely its surface
//! subdivides, and the shares decide who holds which part of it.
//!
//! Nothing here decides what anything looks like. Family is [`material`](crate::material) and
//! the arrangement of the cells is [`Mosaic`]'s own documented reading; this module is the
//! arithmetic underneath, and it is separate so that `AC-MAT-1` and `AC-MAT-2` can be tested
//! against numbers rather than against pictures.
//!
//! # Why the scale is absolute, again
//!
//! [`Normalize::full_scale_bytes`] is a constant in the table rather than the largest path in
//! the repository, for exactly the argument [`params`](crate::params) makes about
//! [`Scales`](crate::params::Scales): a repository-relative maximum means adding one large
//! file renormalizes every other path, so `AC-GROW-4`'s confined Grow becomes a whole-tree
//! reflow and `AC-DET-1`'s stability becomes a coincidence of content. The cost is the same
//! too — the scale is a tuning liability, which is why it is in the file where it can be
//! seen.
//!
//! # `N4`, and what a cell count is
//!
//! [`Mosaic`] holds a cell count per contributor, which is a contribution share wearing
//! different units. That is permitted and intended: `design/feature-system.md` §3.4 says
//! share "may size a mosaic, allocate material, or seed an accent", and this is the sizing.
//! What `N4` forbids is *surfacing* it — `AC-MAT-3` binds the UI, not this arithmetic. The two
//! properties that keep the output itself clean — key-order iteration and the absence of any
//! largest-holder accessor — travel with the type rather than living here.

use crate::params::per_mille;
use serde::Deserialize;
use treepo_det::{Fx, OrderedMap};
use treepo_model::Mosaic;
use treepo_model::identity::AuthorKey;
use treepo_model::primitives::ownership::OwnershipPrimitives;

/// Parts per million — the unit [`AuthorShare`](treepo_model::primitives::AuthorShare) is
/// carried in, and therefore the unit a significance threshold has to be written in.
const PER_MILLION: u32 = 1_000_000;

/// `F-MAT-3`'s rules, as data.
///
/// A section of `assets/params/materials.ron`. Every field is an integer in a stated unit,
/// the convention `lsystem.ron` and `author-palette.ron` already use, and for the same
/// reason: `N3` keeps floats out of anything reaching generated output, and in a file that
/// exists to be hand-tuned `800` reads as "0.8" where `800000` does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Normalize {
    /// The byte count that reads as a full budget.
    ///
    /// Absolute, never the repository's maximum — see the module header. Its logarithm is
    /// the divisor that turns `log2(bytes)` into a proportion, so the number that matters is
    /// its *order of magnitude* rather than its exact value: doubling it moves every budget
    /// by the same small factor rather than reshuffling the ordering.
    pub full_scale_bytes: u64,
    /// The budget above which further size counts only in part, per mille.
    ///
    /// `P7`'s "maximum soft clamp: prevents a single enormous file or directory from
    /// consuming the entire visual budget of its parent". The same piecewise-linear shape
    /// [`TrunkParams::support`](crate::params::TrunkParams::support) uses on carried limb
    /// width, applied here to a normalized size — because it is the same problem twice, and
    /// two differently-shaped clamps would be two different opinions about what "too big"
    /// means.
    pub clamp_knee: i32,
    /// How much of the budget above [`clamp_knee`](Self::clamp_knee) still counts, per mille.
    ///
    /// Not zero, and [`validate`](Self::validate) refuses zero for the reason
    /// `support_beyond` does: a hard ceiling makes every large path draw at the same size,
    /// which loses the ordering `P6` keeps ("legibility bounds how much of the mass is
    /// shown", not whether a bigger thing is bigger).
    pub clamp_beyond: i32,
    /// The smallest budget any surviving path may receive, per mille.
    ///
    /// `F-MAT-3`'s minimum representation floor, and the whole of `AC-MAT-1`: a 3-line file
    /// beside a 50k-line file is not competing with it, because the floor is not taken from
    /// anyone. It is a floor on the *proportion*, so it survives the camera moving.
    ///
    /// Per `F-MAT-3` as amended by PRD §11 Q1, this is owed to paths surviving filtering
    /// **and aggregation** — an [`AggregateNode`](treepo_model::AggregateNode) discharges it
    /// on behalf of everything inside it. Nothing here needs to know that: the caller asks
    /// about the nodes that exist, and a container is one of them.
    pub floor: i32,
    /// The share at or above which a contributor is guaranteed a quota, in parts per million.
    ///
    /// `F-MAT-3` says "a minimum visible quota per **significant** contributor" and leaves
    /// significance undefined; `AC-MAT-2` pins the ceiling on it at 2%, and this sits below
    /// that so the acceptance criterion clears the bar rather than balancing on it.
    ///
    /// It is also what keeps PRD §6's thousand-author repository from fragmenting a limb into
    /// noise, and the bound is arithmetic rather than hopeful: at most
    /// `1_000_000 / significant_ppm` contributors can clear it, so the guaranteed cells are
    /// capped no matter how many people appear.
    pub significant_ppm: u32,
    /// Cells guaranteed to a significant contributor, whatever their share works out to.
    ///
    /// One is enough to satisfy `AC-MAT-2` and `design/feature-system.md` §3.4's "even a
    /// single pixel of a contributor's colour is considered high-value". Above one it buys
    /// legibility at the cost of proportionality, which is a tuning question and therefore
    /// lives here.
    pub quota_cells: u32,
    /// Cells in the mosaic of a node drawn at no budget at all.
    ///
    /// The coarse end of `P6`'s "legibility bounds how much of the mass is shown". A node drawn
    /// small subdivided finely produces cells nobody can see, so the smallest node gets the
    /// fewest, largest cells.
    ///
    /// At *no* budget rather than at the representation floor, because the count is linear over
    /// the whole `0..=1` range and the floor is not zero — a floored node lands just above this
    /// and nothing lands on it. Anchoring the range at the floor instead would tie the mosaic's
    /// granularity to a `F-MAT-3` row it has no other business reading, to move every real
    /// node's count by one.
    ///
    /// At least one, which [`validate`](Self::validate) enforces: a node whose mosaic had no
    /// cells could not show its sole contributor, and `P7` does not distinguish between a path
    /// with no pixels and a path whose pixels say nothing.
    pub mosaic_min_cells: u32,
    /// Cells in the mosaic of a node drawn at a full budget.
    ///
    /// The fine end. Between the two the count is linear in [`budget`](Self::budget), so a
    /// node drawn twice as large is subdivided twice as finely and each cell covers about the
    /// same area — which is what keeps `AC-MAT-4`'s "distinguishable at medium zoom" a
    /// property of the palette rather than of how big the limb happened to be.
    pub mosaic_max_cells: u32,
}

impl Normalize {
    /// How much of the picture a path holding `bytes` may occupy, in `0..=1`.
    ///
    /// Log, then soft clamp, then floor — `F-MAT-3` in the order it states them, and the
    /// order matters. Clamping before the floor means the clamp shapes the top of the range
    /// without ever being able to push something below the floor; flooring first would let
    /// the clamp take back the guarantee.
    ///
    /// An empty path receives the floor rather than nothing. A zero-byte file is an ordinary
    /// path — `__init__.py`, a `.gitkeep`, a placeholder — and `P1` requires every visible
    /// element to resolve to a real path, which cuts both ways: a real path with no pixels is
    /// as much a break as pixels with no path.
    #[must_use]
    pub fn budget(&self, bytes: u64) -> Fx {
        let floor = per_mille(self.floor);
        let Some(magnitude) = Fx::log2_u64(bytes) else {
            return floor;
        };
        // Validated non-zero, so the logarithm exists and is positive.
        let Some(full_scale) = Fx::log2_u64(self.full_scale_bytes) else {
            return floor;
        };
        if full_scale.is_zero() {
            return floor;
        }

        let raw = magnitude.div(full_scale);
        self.soft_clamp(raw).max(floor).min(Fx::ONE)
    }

    /// The budget above the knee, counted in part — see [`clamp_beyond`](Self::clamp_beyond).
    fn soft_clamp(&self, raw: Fx) -> Fx {
        let knee = per_mille(self.clamp_knee);
        if raw > knee {
            knee.add(raw.sub(knee).mul(per_mille(self.clamp_beyond)))
        } else {
            raw
        }
    }

    /// One node's whole mosaic — `F-MAT-2`, over a node already budgeted by `F-MAT-3`.
    ///
    /// The pairing the material walk calls: [`cells_for`](Self::cells_for) decides how finely
    /// the surface subdivides and [`allocate`](Self::allocate) decides who holds which part.
    /// `budget` is this node's own, as [`budget`](Self::budget) returned it.
    #[must_use]
    pub fn mosaic(&self, ownership: &OwnershipPrimitives, budget: Fx) -> Mosaic {
        self.allocate(ownership, self.cells_for(budget))
    }

    /// How many cells a node drawn at `budget` subdivides into.
    ///
    /// Linear between [`mosaic_min_cells`](Self::mosaic_min_cells) at a zero budget and
    /// [`mosaic_max_cells`](Self::mosaic_max_cells) at a full one. No real node carries a zero
    /// budget — [`budget`](Self::budget) floors it — so the minimum is the bound rather than a
    /// value anything is given.
    ///
    /// The budget rather than the byte count, because the budget is already the *drawn* size —
    /// logged, clamped and floored — and it is the drawn size that decides how much room there
    /// is to subdivide. Driving this from bytes instead would give a 50 MB asset a mosaic four
    /// times finer than the source file beside it, which is exactly the disparity `F-MAT-3`
    /// exists to compress.
    #[must_use]
    pub fn cells_for(&self, budget: Fx) -> u32 {
        let span = self.mosaic_max_cells.saturating_sub(self.mosaic_min_cells);
        let earned = Fx::from_int(i32::try_from(span).unwrap_or(i32::MAX))
            .mul(budget)
            .round();
        self.mosaic_min_cells
            .saturating_add(u32::try_from(earned.clamp(0, i64::from(span))).unwrap_or(span))
    }

    /// How many of `cells` each contributor holds in one limb's mosaic.
    ///
    /// Two tiers, and the split is what makes `AC-MAT-2` a property rather than a hope:
    ///
    /// * a contributor at or above [`significant_ppm`](Self::significant_ppm) receives their
    ///   proportional share **or** [`quota_cells`](Self::quota_cells), whichever is larger;
    /// * everyone else receives their proportional share, which rounds down and may be zero.
    ///
    /// The proportional part is
    /// [`AuthorShare::allocate`](treepo_model::primitives::AuthorShare::allocate), which
    /// rounds down and documents that the floor is "a material-policy decision, not an
    /// arithmetic one". This is that policy.
    ///
    /// # The total may exceed `cells`, and that is the answer rather than a failure
    ///
    /// Guaranteeing a quota to several contributors can ask for more cells than the caller
    /// offered. Something has to give, and every alternative is worse: capping the guarantee
    /// breaks `AC-MAT-2`, and dropping contributors to fit requires choosing *which*, which
    /// is the ordering of people `N4` forbids. So the mosaic subdivides further instead, and
    /// [`Mosaic::cells`] reports what it actually came to.
    ///
    /// This cannot run away. At most `1_000_000 / significant_ppm` contributors can be
    /// significant — a hundred of them at one percent — so the overshoot is bounded by that
    /// count times the quota, independently of how many people touched the path.
    ///
    /// # Cells left over are not a gap
    ///
    /// Where the total falls short of `cells`, the remainder is *not* redistributed. `F-MAT-2`
    /// makes ownership "accent, vein, and mosaic treatment **over** the primary material", so
    /// an unclaimed cell already has something to be: the limb's own
    /// [`MaterialFamily`](treepo_model::MaterialFamily). Handing the remainder to the largest
    /// holder would be both a ranking and a small lie about who wrote what.
    #[must_use]
    pub fn allocate(&self, ownership: &OwnershipPrimitives, cells: u32) -> Mosaic {
        let mut held: OrderedMap<AuthorKey, u32> = OrderedMap::new();

        // Key order — hash order — so this loop cannot become a ranking however it is read.
        for (&key, share) in ownership.shares() {
            let earned = share.allocate(cells);
            let granted = if share.to_ppm() >= self.significant_ppm {
                earned.max(self.quota_cells)
            } else {
                earned
            };
            held.insert(key, granted);
        }

        // `Mosaic::new` drops the contributors who earned nothing and totals the rest, so the
        // count and the map cannot be made to disagree by an arm of this loop.
        Mosaic::new(held, cells)
    }

    /// Checks the section against the rules `F-MAT-3` and `P7` state.
    ///
    /// # Errors
    ///
    /// The first violated rule, naming the row and the requirement it came from.
    pub fn validate(&self) -> Result<(), NormalizeError> {
        if self.full_scale_bytes == 0 {
            return Err(NormalizeError {
                row: "full_scale_bytes",
                detail: "the full scale is a positive byte count — it is the divisor every \
                         budget is measured against",
            });
        }

        for (row, value) in [
            ("clamp_knee", self.clamp_knee),
            ("clamp_beyond", self.clamp_beyond),
            ("floor", self.floor),
        ] {
            if !(0..=1000).contains(&value) {
                return Err(NormalizeError {
                    row,
                    detail: "a proportion belongs in 0..=1000 per mille",
                });
            }
        }

        // P6, as `support_beyond` states it: a hard ceiling makes every large path draw at
        // the same size, which is the ordering lost rather than the picture tidied.
        if self.clamp_beyond == 0 {
            return Err(NormalizeError {
                row: "clamp_beyond",
                detail: "size past the knee must still count for something, or a bigger path \
                         stops drawing bigger",
            });
        }

        // The floor is a guarantee, and a guarantee that reaches the knee has stopped being a
        // floor and become the whole scale.
        if self.floor >= self.clamp_knee {
            return Err(NormalizeError {
                row: "floor",
                detail: "the representation floor sits below the clamp knee, or every path \
                         is drawn at one size",
            });
        }

        // The clamp must not be able to reach a full budget for *any* `u64` byte count, or
        // the `min(ONE)` in `budget` becomes a real ceiling and two enormous paths draw
        // identically — the same failure `clamp_beyond == 0` is refused for, arriving by
        // arithmetic instead of by configuration.
        //
        // `log2(u64::MAX) < 64`, so the largest raw value any input can produce is
        // `64 / log2(full_scale_bytes)`.
        let full_scale = Fx::log2_u64(self.full_scale_bytes).unwrap_or(Fx::ZERO);
        if full_scale.is_zero() {
            return Err(NormalizeError {
                row: "full_scale_bytes",
                detail: "the full scale is above one byte — log2(1) is zero, and a zero \
                         divisor is not a scale",
            });
        }
        let widest = Fx::from_int(64).div(full_scale);
        if self.soft_clamp(widest) >= Fx::ONE {
            return Err(NormalizeError {
                row: "clamp_beyond",
                detail: "the soft clamp must stay below a full budget for every byte count a \
                         u64 can hold, or the largest paths all draw the same size",
            });
        }

        if self.significant_ppm == 0 || self.significant_ppm > PER_MILLION {
            return Err(NormalizeError {
                row: "significant_ppm",
                detail: "the significance threshold is a share in 1..=1_000_000 ppm",
            });
        }

        // AC-MAT-2 states the bar: "a contributor responsible for 2% of a limb retains
        // visible presence in its mosaic". A threshold above 2% would exclude the very case
        // the criterion names, and the table is not permitted to tune its way out of an
        // acceptance criterion.
        if self.significant_ppm > 20_000 {
            return Err(NormalizeError {
                row: "significant_ppm",
                detail: "significance must sit at or below 2% — AC-MAT-2 requires a 2% \
                         contributor to keep visible presence",
            });
        }

        if self.quota_cells == 0 {
            return Err(NormalizeError {
                row: "quota_cells",
                detail: "a guaranteed quota of zero cells is no guarantee — AC-MAT-2 needs at \
                         least one",
            });
        }

        // P7 again, in the mosaic's units: a node whose surface has no cells cannot show the
        // one person who wrote it, and a floored budget would be a floor on nothing.
        if self.mosaic_min_cells == 0 {
            return Err(NormalizeError {
                row: "mosaic_min_cells",
                detail: "the smallest mosaic still has a cell in it, or a node at the \
                         representation floor has nowhere to draw its contributors",
            });
        }

        // Equal is permitted — a fixed-size mosaic is a defensible tuning position. Inverted
        // is not: `cells_for` would return the minimum for every node, so the fine end would
        // be configured and unreachable, which is the failure `clamp_beyond == 0` is refused
        // for arriving by arithmetic instead of by configuration.
        if self.mosaic_max_cells < self.mosaic_min_cells {
            return Err(NormalizeError {
                row: "mosaic_max_cells",
                detail: "the fine end of the mosaic sits at or above the coarse end, or a \
                         larger node is subdivided no further than a smaller one",
            });
        }

        Ok(())
    }

    /// The most cells a guaranteed quota can add beyond what was asked for.
    ///
    /// The bound [`allocate`](Self::allocate) claims, made checkable. A caller sizing a
    /// mosaic buffer can use it; the tests use it to hold PRD §6's thousand-author repository
    /// to a stated ceiling rather than to whatever it happens to produce.
    #[must_use]
    pub const fn max_guaranteed_cells(&self) -> u32 {
        if self.significant_ppm == 0 {
            return 0;
        }
        (PER_MILLION / self.significant_ppm).saturating_mul(self.quota_cells)
    }
}

/// Why a normalization section was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NormalizeError {
    /// The row that failed.
    pub row: &'static str,
    /// The rule it broke, and where the rule comes from.
    pub detail: &'static str,
}

impl core::fmt::Display for NormalizeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "materials.ron: `{}` — {}", self.row, self.detail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use treepo_model::primitives::AuthorShare;

    /// The shipped section, so the tests measure what the product actually does.
    fn shipped() -> Normalize {
        crate::material::Table::built_in().normalize
    }

    fn author(byte: u8) -> AuthorKey {
        AuthorKey::from_email(&[byte])
    }

    /// A distinct contributor per index.
    ///
    /// Decimal digits rather than `index.to_le_bytes()`, and the difference is not cosmetic:
    /// [`AuthorKey::from_email`] case-folds (`F-EXT-9`, so one human with `Foo@` and `foo@`
    /// is one contributor), and raw little-endian bytes land on `A`–`Z` and `a`–`z`, so
    /// `65` and `97` hash to the same key. A fixture built that way quietly produces fewer
    /// contributors than it asks for, which reads as an allocator that dropped people.
    fn authors(count: u32) -> alloc::vec::Vec<(AuthorKey, u64)> {
        (0..count)
            .map(|i| {
                let email = alloc::format!("{i}@example.test");
                (AuthorKey::from_email(email.as_bytes()), 1)
            })
            .collect()
    }

    fn ownership(counts: &[(AuthorKey, u64)]) -> OwnershipPrimitives {
        let map: OrderedMap<AuthorKey, u64> = counts.iter().copied().collect();
        OwnershipPrimitives::from_line_counts(&map, OrderedMap::new())
    }

    #[test]
    fn the_shipped_section_validates() {
        assert_eq!(shipped().validate(), Ok(()));
    }

    /// `AC-MAT-1`, as arithmetic. The 3-line file and the 50k-line file are four orders of
    /// magnitude apart; what matters is that the small one keeps a budget at all and that the
    /// large one has not taken it.
    #[test]
    fn a_tiny_path_beside_an_enormous_one_keeps_a_budget() {
        let n = shipped();
        let tiny = n.budget(90); // three short lines
        let huge = n.budget(1_500_000); // fifty thousand lines

        assert!(tiny > Fx::ZERO, "a real path with no pixels breaks P1/P7");
        assert!(
            tiny >= per_mille(n.floor),
            "the floor is a guarantee: {tiny}"
        );
        assert!(huge > tiny, "and the ordering still has to be honest");
        // Logarithmic, so a 16,000× difference in bytes is a small multiple in budget rather
        // than a 16,000× one. That is the whole reason F-MAT-3 opens with "logarithmic".
        assert!(
            huge < tiny.mul(Fx::from_int(4)),
            "tiny {tiny}, huge {huge} — the compression is not doing its job"
        );
    }

    #[test]
    fn an_empty_path_receives_the_floor_rather_than_nothing() {
        let n = shipped();
        assert_eq!(n.budget(0), per_mille(n.floor));
        assert_eq!(n.budget(1), per_mille(n.floor));
    }

    /// `P6`: the clamp bounds how much is shown, and never at the cost of the ordering.
    #[test]
    fn the_budget_never_decreases_and_never_fills_the_frame() {
        let n = shipped();
        let mut previous = Fx::ZERO;
        // Every power of two from one byte to sixteen exabytes.
        for exponent in 0..64u32 {
            let budget = n.budget(1u64 << exponent);
            assert!(budget >= previous, "budget fell at 2^{exponent}");
            assert!(
                budget < Fx::ONE,
                "2^{exponent} filled the frame at {budget} — the soft clamp reached its \
                 ceiling, and every larger path now draws identically"
            );
            previous = budget;
        }
        // And strictly increasing well past the knee, which is what `clamp_beyond` buys.
        assert!(n.budget(1 << 40) > n.budget(1 << 32));
    }

    /// `AC-MAT-2` — "a contributor responsible for 2% of a limb retains visible presence in
    /// its mosaic". Sixty-four cells is a mosaic in which 2% earns 1.28 cells, so
    /// `AuthorShare::allocate`'s rounding-down would erase them without the quota.
    #[test]
    fn a_two_percent_contributor_keeps_visible_presence() {
        let n = shipped();
        let minor = author(1);
        let major = author(2);
        let owned = ownership(&[(minor, 2), (major, 98)]);

        assert_eq!(
            owned.share_of(&minor).to_ppm(),
            20_000,
            "2% by construction"
        );

        for cells in [8u32, 16, 32, 64, 256] {
            let allocation = n.allocate(&owned, cells);
            assert!(
                allocation.is_present(&minor),
                "a 2% contributor vanished from a {cells}-cell mosaic"
            );
            assert!(allocation.cells_for(&minor) >= n.quota_cells);
        }

        // And at every mosaic size the shipped table can actually produce, which is the range
        // that matters — a criterion held only at sizes the product never picks is not held.
        for budget in [per_mille(n.floor), Fx::HALF, Fx::ONE] {
            assert!(
                n.mosaic(&owned, budget).is_present(&minor),
                "a 2% contributor vanished from a node budgeted at {budget}"
            );
        }
    }

    /// The rounding-down that makes the quota necessary. Without it this test would pass for
    /// the wrong reason and `a_two_percent_contributor_keeps_visible_presence` would prove
    /// nothing.
    #[test]
    fn the_quota_is_what_saves_them_not_the_arithmetic() {
        assert_eq!(
            AuthorShare::from_ppm(20_000).allocate(16),
            0,
            "2% of sixteen cells rounds to nothing — the quota is load-bearing"
        );
    }

    /// PRD §6, "1000+ authors": the minimum quota "does not fragment limbs into noise".
    ///
    /// The guarantee is bounded by the significance threshold, not by the author count, and
    /// this is the arithmetic that says so.
    #[test]
    fn a_thousand_authors_do_not_fragment_the_mosaic() {
        let n = shipped();
        let owned = ownership(&authors(1000));
        assert_eq!(owned.author_count(), 1000);

        let allocation = n.allocate(&owned, 64);
        // At 0.1% each, nobody is significant, so nobody is guaranteed anything and the
        // mosaic does not grow at all.
        assert_eq!(
            allocation.cells(),
            64,
            "a thousand equal authors grew the mosaic to {}",
            allocation.cells()
        );
        assert!(allocation.claimed() <= 64 + n.max_guaranteed_cells());
    }

    /// The bound `allocate` claims, at the worst case it is claimed for: as many significant
    /// contributors as the threshold physically permits.
    #[test]
    fn the_guarantee_cannot_outgrow_its_stated_bound() {
        let n = shipped();
        // Exactly at the threshold, so every one of them is significant.
        let holders = (PER_MILLION / n.significant_ppm) as usize;
        let owned = ownership(&authors(holders as u32));
        assert_eq!(owned.author_count() as usize, holders);

        let allocation = n.allocate(&owned, 4);
        assert_eq!(allocation.holder_count(), holders);
        assert!(
            allocation.claimed() <= 4 + n.max_guaranteed_cells(),
            "{} cells for {holders} contributors, bound {}",
            allocation.claimed(),
            4 + n.max_guaranteed_cells()
        );
        assert!(allocation.cells() > 4, "the mosaic subdivided further");
        assert_eq!(
            allocation.unclaimed(),
            0,
            "the mosaic grew, so nothing is spare"
        );
    }

    /// PRD §6, "Single author": the mosaic degenerates to one family, not to nothing.
    #[test]
    fn a_single_author_holds_the_whole_mosaic() {
        let n = shipped();
        let only = author(7);
        let allocation = n.allocate(&ownership(&[(only, 42)]), 64);
        assert_eq!(allocation.cells_for(&only), 64);
        assert_eq!(allocation.unclaimed(), 0);
        assert_eq!(allocation.holder_count(), 1);
    }

    /// PRD §6, "No `.git`" and "Empty repository": an unattributed path is ordinary, and the
    /// primary material shows through everywhere.
    #[test]
    fn an_unattributed_path_yields_an_empty_mosaic() {
        let n = shipped();
        let allocation = n.allocate(&ownership(&[]), 64);
        assert!(allocation.is_empty());
        assert_eq!(allocation.claimed(), 0);
        assert_eq!(allocation.unclaimed(), 64);
    }

    /// `F-MAT-2`: ownership is accent *over* the primary material, so a partly-claimed mosaic
    /// is the normal case rather than a rounding bug to be tidied away.
    #[test]
    fn unclaimed_cells_are_left_for_the_primary_material() {
        let n = shipped();
        // Three contributors at a third each: 21 cells apiece of 64, one left over.
        let owned = ownership(&[(author(1), 1), (author(2), 1), (author(3), 1)]);
        let allocation = n.allocate(&owned, 64);
        assert_eq!(allocation.claimed(), 63);
        assert_eq!(allocation.unclaimed(), 1);
        // And it was not quietly handed to anyone — which would have required picking one.
        for (_, &cells) in allocation.holders() {
            assert_eq!(cells, 21);
        }
    }

    /// `P6`: a node drawn small is subdivided coarsely, and one drawn large finely — so a
    /// cell covers about the same area wherever it appears.
    #[test]
    fn a_bigger_node_is_subdivided_more_finely() {
        let n = shipped();
        assert_eq!(n.cells_for(Fx::ZERO), n.mosaic_min_cells);
        assert_eq!(n.cells_for(Fx::ONE), n.mosaic_max_cells);

        // Monotonic across the whole budget range, and never outside the stated bounds — a
        // count that dipped would make a bigger limb coarser than a smaller one.
        let mut previous = 0;
        for step in 0..=1000i64 {
            let cells = n.cells_for(Fx::from_ratio(step, 1000));
            assert!(cells >= previous, "the count fell at {step} per mille");
            assert!((n.mosaic_min_cells..=n.mosaic_max_cells).contains(&cells));
            previous = cells;
        }

        // And a real spread: a fixed-count mosaic would pass every assertion above.
        assert!(n.cells_for(Fx::ONE) > n.cells_for(per_mille(n.floor)));
    }

    /// The floored budget is the smallest a real node can carry, so it is the case `P7` is
    /// actually about — a mosaic there must still be able to show somebody.
    #[test]
    fn the_smallest_node_still_has_a_mosaic() {
        let n = shipped();
        let only = author(7);
        let floored = n.budget(0);
        let mosaic = n.mosaic(&ownership(&[(only, 1)]), floored);

        assert!(mosaic.cells() > 0);
        assert_eq!(mosaic.cells_for(&only), mosaic.cells());
        assert_eq!(mosaic.unclaimed(), 0, "one author holds the whole surface");
    }

    /// `N4`: the output must not be readable as a ranking, however it is iterated.
    #[test]
    fn holders_iterate_in_key_order_not_by_size() {
        let n = shipped();
        // Eight contributors with eight distinct shares, all above the significance
        // threshold so all eight are drawn. Eight rather than three because the guard below
        // is a coincidence check: two orderings of three items agree once in six, which is
        // often enough to be a flaky test, and once in forty thousand is not.
        let counts: alloc::vec::Vec<(AuthorKey, u64)> = authors(8)
            .into_iter()
            .enumerate()
            .map(|(i, (key, _))| (key, 8 - i as u64))
            .collect();
        let owned = ownership(&counts);
        let allocation = n.allocate(&owned, 256);
        assert_eq!(
            allocation.holder_count(),
            8,
            "every contributor here is significant"
        );

        let iterated: alloc::vec::Vec<AuthorKey> = allocation.holders().map(|(&k, _)| k).collect();
        let mut by_key = iterated.clone();
        by_key.sort();
        assert_eq!(iterated, by_key);

        // Guard against the assertion above being vacuous. If hash order happened to agree
        // with size order on this fixture, "it iterates in key order" would prove nothing
        // about rankings. Computed rather than assumed, because which key sorts first is a
        // property of SHA-256 and not something a fixture should be written around.
        let by_size: alloc::vec::Vec<AuthorKey> = {
            let mut pairs: alloc::vec::Vec<(u32, AuthorKey)> = allocation
                .holders()
                .map(|(&k, &cells)| (cells, k))
                .collect();
            pairs.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
            pairs.into_iter().map(|(_, key)| key).collect()
        };
        assert_ne!(
            iterated, by_size,
            "key order coincides with size order here, so this fixture cannot tell the two \
             apart — pick contributors whose hashes disagree with their shares"
        );
    }

    #[test]
    fn a_section_that_breaks_a_stated_rule_is_refused() {
        let base = shipped();

        let cases = [
            (
                "full_scale_bytes",
                Normalize {
                    full_scale_bytes: 0,
                    ..base
                },
            ),
            (
                "clamp_beyond",
                Normalize {
                    clamp_beyond: 0,
                    ..base
                },
            ),
            // A hard ceiling arriving by arithmetic: a small full scale plus a generous
            // `beyond` lets a large repository reach a full budget.
            (
                "clamp_beyond",
                Normalize {
                    full_scale_bytes: 1024,
                    clamp_beyond: 900,
                    ..base
                },
            ),
            (
                "floor",
                Normalize {
                    floor: base.clamp_knee,
                    ..base
                },
            ),
            (
                "significant_ppm",
                Normalize {
                    significant_ppm: 0,
                    ..base
                },
            ),
            // Above 2% excludes the contributor AC-MAT-2 names.
            (
                "significant_ppm",
                Normalize {
                    significant_ppm: 30_000,
                    ..base
                },
            ),
            (
                "quota_cells",
                Normalize {
                    quota_cells: 0,
                    ..base
                },
            ),
            (
                "mosaic_min_cells",
                Normalize {
                    mosaic_min_cells: 0,
                    ..base
                },
            ),
            // Inverted, so the fine end would be configured and unreachable.
            (
                "mosaic_max_cells",
                Normalize {
                    mosaic_max_cells: base.mosaic_min_cells - 1,
                    ..base
                },
            ),
        ];

        for (row, section) in cases {
            let error = section.validate().expect_err("should have been refused");
            assert_eq!(
                error.row, row,
                "{section:?} was refused for the wrong reason"
            );
        }
    }
}
