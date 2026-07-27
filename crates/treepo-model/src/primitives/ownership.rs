//! Ownership and social primitives — `F-EXT-2`, `design/feature-system.md` §3.4.
//!
//! # `N4` is enforced here, by the type system
//!
//! > treepo never produces ranked lists, leaderboards, contribution-percentage scoreboards,
//! > or any ordering of people by volume or importance. This holds even where the extracted
//! > primitives would trivially support it.
//!
//! "Even where the primitives would trivially support it" is the hard part. This module
//! holds exactly the data a leaderboard needs, so a comment asking people not to build one
//! is worth very little. Three type-level decisions do the work instead:
//!
//! 1. **[`AuthorShare`] implements neither [`Ord`] nor [`PartialOrd`].** `sort_by_key` on a
//!    share does not compile. Neither does `max_by_key`, `min`, `>`, or a `BinaryHeap`.
//!    Every convenient route to an ordering of people is closed at the type level, and
//!    there is a `compile_fail` doctest below that fails CI if someone opens one.
//! 2. **No accessor returns a percentage.** [`AuthorShare::allocate`] converts a share into
//!    a count of mosaic cells or material units, which is the depiction `N4` intends. There
//!    is no `as_percent`, no `Display`, and the [`Debug`] output is deliberately coarse —
//!    a share is not a figure to surface, including in a tooltip.
//! 3. **Iteration is in key order, which is hash order.** [`OwnershipPrimitives::shares`]
//!    yields contributors ordered by [`AuthorKey`], and that order is uncorrelated with
//!    contribution. Iterating and rendering in sequence produces no ranking to read.
//!
//! `dominant_author` and `bus_factor_proxy` are named in `F-EXT-2` and both survive, because
//! neither is an ordering of people: the first picks a limb's base material family, and the
//! second is a count. Both are computed during extraction from raw line counts, before any
//! [`AuthorShare`] exists — which is why nothing here needs to compare two shares.

use crate::identity::AuthorKey;
use core::fmt;
use treepo_det::{Fx, OrderedMap};

/// One contributor's proportion of a path's attributed lines, in parts per million.
///
/// Parts per million rather than [`Fx`] because a share is a proportion of a whole and must
/// sum exactly: integer ppm can be made to total 1,000,000 by construction, where
/// fixed-point division would leave a residue that drifts as contributor counts grow.
///
/// # Deliberately not comparable
///
/// ```compile_fail
/// # use treepo_model::primitives::AuthorShare;
/// let mut shares = [AuthorShare::from_ppm(10), AuthorShare::from_ppm(900_000)];
/// shares.sort(); // N4: AuthorShare implements neither Ord nor PartialOrd.
/// ```
///
/// ```compile_fail
/// # use treepo_model::primitives::AuthorShare;
/// let a = AuthorShare::from_ppm(10);
/// let b = AuthorShare::from_ppm(20);
/// let _bigger = a > b; // N4: no ordering of people by volume.
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct AuthorShare(u32);

impl AuthorShare {
    /// A whole share — the single-author case (PRD §6, "Single author").
    pub const WHOLE: Self = Self(1_000_000);

    /// Builds a share from parts per million, clamped to a whole.
    #[must_use]
    pub const fn from_ppm(ppm: u32) -> Self {
        Self(if ppm > 1_000_000 { 1_000_000 } else { ppm })
    }

    /// Builds a share from a part and a total, rounding to nearest.
    ///
    /// A zero total yields a zero share rather than panicking: a path with no attributed
    /// lines is an ordinary case, not a caller bug.
    #[must_use]
    pub const fn from_ratio(part: u64, total: u64) -> Self {
        if total == 0 {
            return Self(0);
        }
        let scaled = (part as u128 * 1_000_000 + (total as u128) / 2) / total as u128;
        Self::from_ppm(if scaled > 1_000_000 {
            1_000_000
        } else {
            scaled as u32
        })
    }

    /// How many of `units` this share is entitled to — the `N4`-sanctioned use.
    ///
    /// Sizing a mosaic, allocating material, seeding an accent: all of them are this
    /// function. Rounds down, so `F-MAT-3`'s minimum quota is applied on top by the caller
    /// rather than silently here; a 2% contributor keeping visible presence (`AC-MAT-2`) is
    /// a material-policy decision, not an arithmetic one.
    #[must_use]
    pub const fn allocate(self, units: u32) -> u32 {
        ((self.0 as u64 * units as u64) / 1_000_000) as u32
    }

    /// Whether this contributor touched the path at all.
    ///
    /// The intended predicate for "does this author get a mosaic cell" — a question about
    /// presence, which `N4` permits, rather than about magnitude, which it does not.
    #[must_use]
    pub const fn is_present(self) -> bool {
        self.0 > 0
    }

    /// The raw parts-per-million value.
    ///
    /// Deliberately awkward to reach for and deliberately not `as_percent`. Present because
    /// `treepo-store` must serialize the value and `treepo-gen`'s normalization
    /// (`F-MAT-3` — log, clamp, floor) genuinely needs the magnitude. **Not for display**:
    /// `N4` draws the line at surfacing a share as a figure, and this is that figure.
    #[must_use]
    pub const fn to_ppm(self) -> u32 {
        self.0
    }

    /// The share as a fixed-point proportion in `0..=1`, for material weighting.
    #[must_use]
    pub const fn to_fx(self) -> Fx {
        Fx::from_ratio(self.0 as i64, 1_000_000)
    }
}

/// Coarse by design. A debug dump of a manifest should not be a scoreboard either, so this
/// renders a bucket rather than the value.
impl fmt::Debug for AuthorShare {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bucket = match self.0 {
            0 => "none",
            1..=9_999 => "trace",
            10_000..=99_999 => "minor",
            100_000..=499_999 => "partial",
            _ => "major",
        };
        write!(f, "AuthorShare({bucket})")
    }
}

/// Who contributed to a path, and how recently — `F-EXT-2`.
///
/// Built by [`OwnershipPrimitives::from_line_counts`] rather than field by field, so the
/// shares are proportions of one whole by construction and `dominant` is consistent with
/// them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OwnershipPrimitives {
    /// Contribution share per contributor, keyed by hash.
    ///
    /// Iteration order is key order — uncorrelated with share, per `N4`.
    shares: OrderedMap<AuthorKey, AuthorShare>,
    /// `contribution_recency_per_author`: each contributor's most recent commit to this
    /// path, as absolute epoch seconds. See [`temporal`](super::temporal) for why absolute.
    recency: OrderedMap<AuthorKey, i64>,
    /// The contributor with the most attributed lines, for the base material family.
    ///
    /// `None` when the path has no attributed lines, or when two contributors tie exactly —
    /// a tie has no dominant author, and picking one by key order would be arbitrary in a
    /// way that shows up as a colour flip between two runs of a growing repository.
    dominant: Option<AuthorKey>,
    /// `bus_factor_proxy`: how many contributors account for ~80% of the lines.
    ///
    /// A count, not an ordering — it says "this path depends on two people" without saying
    /// which two, or in what order.
    bus_factor: u16,
}

impl OwnershipPrimitives {
    /// The threshold `bus_factor_proxy` is defined against: 80% of attributed lines.
    const BUS_FACTOR_NUMERATOR: u64 = 4;
    const BUS_FACTOR_DENOMINATOR: u64 = 5;

    /// Builds ownership from raw per-contributor line counts.
    ///
    /// This is the only constructor. Extraction hands over counts and timestamps; shares,
    /// dominance, and the bus-factor proxy are derived here, once, so they cannot disagree.
    ///
    /// Sorting happens on `(count, key)` pairs inside this function and nowhere else. That
    /// is the single place in treepo where contributors are ordered by volume, it exists
    /// only to answer "who is dominant" and "how many make 80%", and neither answer leaves
    /// as an ordering.
    #[must_use]
    pub fn from_line_counts(
        counts: &OrderedMap<AuthorKey, u64>,
        recency: OrderedMap<AuthorKey, i64>,
    ) -> Self {
        let total: u64 = counts.values().copied().sum();
        if total == 0 {
            return Self {
                shares: counts
                    .keys()
                    .map(|&k| (k, AuthorShare::default()))
                    .collect(),
                recency,
                dominant: None,
                bus_factor: 0,
            };
        }

        let shares = counts
            .iter()
            .map(|(&key, &count)| (key, AuthorShare::from_ratio(count, total)))
            .collect();

        // Descending by count, then ascending by key so equal counts have a fixed order.
        // `sort_by` rather than `sort_unstable_by`: the unstable sort's treatment of equal
        // elements is unspecified and may change between Rust versions (`treepo-det` §"What
        // this crate cannot protect you from").
        let mut ranked: alloc::vec::Vec<(u64, AuthorKey)> =
            counts.iter().map(|(&key, &count)| (count, key)).collect();
        ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

        let dominant = match ranked.as_slice() {
            [] => None,
            [(_, only)] => Some(*only),
            [(first, key), (second, _), ..] => {
                // An exact tie has no dominant author. Choosing by key would be stable but
                // arbitrary, and the choice would flip the moment one more commit lands.
                if first == second { None } else { Some(*key) }
            }
        };

        let threshold = total * Self::BUS_FACTOR_NUMERATOR / Self::BUS_FACTOR_DENOMINATOR;
        let mut running = 0u64;
        let mut bus_factor = 0u16;
        for (count, _) in &ranked {
            if running >= threshold {
                break;
            }
            running += count;
            bus_factor = bus_factor.saturating_add(1);
        }

        Self {
            shares,
            recency,
            dominant,
            bus_factor,
        }
    }

    /// `author_count` — how many contributors touched this path.
    #[must_use]
    pub fn author_count(&self) -> u32 {
        u32::try_from(self.shares.len()).unwrap_or(u32::MAX)
    }

    /// `author_distribution`, in key order.
    ///
    /// Key order is hash order, so consuming this in sequence cannot produce a ranking.
    pub fn shares(&self) -> impl Iterator<Item = (&AuthorKey, &AuthorShare)> {
        self.shares.iter()
    }

    /// One contributor's share, or a zero share if they never touched this path.
    #[must_use]
    pub fn share_of(&self, author: &AuthorKey) -> AuthorShare {
        self.shares.get(author).copied().unwrap_or_default()
    }

    /// `contribution_recency_per_author`, in key order, as absolute epoch seconds.
    pub fn recency(&self) -> impl Iterator<Item = (&AuthorKey, &i64)> {
        self.recency.iter()
    }

    /// `dominant_author` — the base material family for this path (`F-MAT-1`).
    ///
    /// `None` for an unattributed path or an exact tie.
    #[must_use]
    pub fn dominant_author(&self) -> Option<AuthorKey> {
        self.dominant
    }

    /// `bus_factor_proxy` — how many contributors account for ~80% of the lines.
    #[must_use]
    pub fn bus_factor_proxy(&self) -> u16 {
        self.bus_factor
    }

    /// Whether this path has no attributed contributors at all.
    ///
    /// True for a repository with no history and for a file git has never seen — both
    /// ordinary paths under PRD §6, not error states.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.shares.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    fn author(byte: u8) -> AuthorKey {
        AuthorKey::from_email(&[byte])
    }

    fn ownership(counts: &[(AuthorKey, u64)]) -> OwnershipPrimitives {
        let map: OrderedMap<AuthorKey, u64> = counts.iter().copied().collect();
        OwnershipPrimitives::from_line_counts(&map, OrderedMap::new())
    }

    #[test]
    fn shares_are_proportions_of_one_whole() {
        let o = ownership(&[(author(1), 250), (author(2), 750)]);
        assert_eq!(o.author_count(), 2);
        let total: u32 = o.shares().map(|(_, s)| s.to_ppm()).sum();
        assert_eq!(total, 1_000_000);
        assert_eq!(o.share_of(&author(1)).to_ppm(), 250_000);
        assert_eq!(o.share_of(&author(2)).to_ppm(), 750_000);
    }

    /// PRD §6, "Single author": the mosaic degenerates to one family, not to nothing.
    #[test]
    fn a_single_author_holds_the_whole_share() {
        let o = ownership(&[(author(1), 42)]);
        assert_eq!(o.share_of(&author(1)), AuthorShare::WHOLE);
        assert_eq!(o.dominant_author(), Some(author(1)));
        assert_eq!(o.bus_factor_proxy(), 1);
    }

    #[test]
    fn an_unknown_author_has_a_zero_share() {
        let o = ownership(&[(author(1), 10)]);
        let absent = o.share_of(&author(9));
        assert!(!absent.is_present());
        assert_eq!(absent.allocate(1000), 0);
    }

    /// An exact tie has no dominant author — otherwise the base material family flips on
    /// the next commit.
    #[test]
    fn an_exact_tie_has_no_dominant_author() {
        assert_eq!(
            ownership(&[(author(1), 50), (author(2), 50)]).dominant_author(),
            None
        );
        assert_eq!(
            ownership(&[(author(1), 51), (author(2), 50)]).dominant_author(),
            Some(author(1))
        );
    }

    #[test]
    fn bus_factor_counts_contributors_to_eighty_percent() {
        // One author at 90% carries it alone.
        assert_eq!(
            ownership(&[(author(1), 90), (author(2), 10)]).bus_factor_proxy(),
            1
        );
        // Four equal authors need four to pass 80%.
        let even = ownership(&[
            (author(1), 25),
            (author(2), 25),
            (author(3), 25),
            (author(4), 25),
        ]);
        assert_eq!(even.bus_factor_proxy(), 4);
    }

    /// PRD §6, "No `.git`" and "Empty repository": no history is an ordinary path.
    #[test]
    fn an_unattributed_path_is_valid_not_an_error() {
        let o = ownership(&[]);
        assert!(o.is_empty());
        assert_eq!(o.author_count(), 0);
        assert_eq!(o.dominant_author(), None);
        assert_eq!(o.bus_factor_proxy(), 0);
    }

    /// `N4`: iteration order is key order, so consuming it in sequence is not a ranking.
    #[test]
    fn iteration_order_is_uncorrelated_with_share() {
        let o = ownership(&[(author(1), 1), (author(2), 998), (author(3), 1)]);
        let by_iteration: Vec<AuthorKey> = o.shares().map(|(k, _)| *k).collect();
        let mut by_key = by_iteration.clone();
        by_key.sort();
        assert_eq!(by_iteration, by_key);
        // And the dominant author is not first in that order, on this fixture.
        assert_eq!(o.dominant_author(), Some(author(2)));
        assert_ne!(by_iteration.first(), Some(&author(2)));
    }

    #[test]
    fn allocate_divides_units_by_share() {
        assert_eq!(AuthorShare::WHOLE.allocate(64), 64);
        assert_eq!(AuthorShare::from_ppm(500_000).allocate(64), 32);
        assert_eq!(AuthorShare::from_ppm(20_000).allocate(1000), 20);
        // Rounds down: F-MAT-3's minimum quota is applied by material policy, not here.
        assert_eq!(AuthorShare::from_ppm(20_000).allocate(10), 0);
    }

    #[test]
    fn share_construction_saturates_and_survives_a_zero_total() {
        assert_eq!(AuthorShare::from_ppm(2_000_000), AuthorShare::WHOLE);
        assert_eq!(AuthorShare::from_ratio(5, 0).to_ppm(), 0);
        assert_eq!(AuthorShare::from_ratio(10, 3).to_ppm(), 1_000_000);
        assert_eq!(AuthorShare::from_ratio(1, 3).to_ppm(), 333_333);
    }

    /// `N4`: a debug dump of a manifest must not read as a scoreboard.
    #[test]
    fn debug_output_is_a_bucket_not_a_figure() {
        let rendered = alloc::format!("{:?}", AuthorShare::from_ppm(731_902));
        assert_eq!(rendered, "AuthorShare(major)");
        assert!(!rendered.contains("731"));
    }
}
