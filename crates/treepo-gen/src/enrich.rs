//! `F-MAT-5` — what gets built on a limb, where it sits, and how several of them grow
//! together.
//!
//! > Semantic enrichment structures placed during Grow: docs → bookshelves/archive platforms;
//! > assets/binaries → stockpiles and crates; tests → distinct secondary growth or
//! > proving-ground platforms; high-churn clusters → work sites.
//!
//! [`material`](crate::material) decides what a node is made of. This decides what is built on
//! it, from the same records and with no new extraction — `design/feature-system.md` §8.7 is
//! explicit that enrichment is "still driven by the same primitives".
//!
//! # One candidate per source, then they grow together
//!
//! The shape of the pass, and the reason it is a pass rather than a lookup:
//!
//! 1. **Candidates.** Every path a node stands for is examined for all four kinds. A
//!    documentation file offers a bookshelf, an image offers a stockpile, a `tests/` directory
//!    offers a proving ground, and anything churning offers a work site. A node standing for
//!    forty paths can therefore offer a great many.
//! 2. **Fusion.** Candidates of one kind sitting within [`Rules::merge_window`] of each other
//!    become one larger structure ([`Placement::fuse`]). This is the whole visual idea: two
//!    shelves are a taller run of shelving, two crates are a braced pile. Nothing is dropped
//!    to make it happen — the weights add and the source count is kept.
//! 3. **Densification.** If a kind still has more structures than [`Rules::max_per_kind`], the
//!    closest remaining pair fuses, repeatedly, until it does not. `P6` bounds how much detail
//!    is drawn; it does not permit discarding what was compressed, so the excess *grows into*
//!    what stays rather than being thrown away.
//! 4. **Form.** Each surviving structure is graded on the ladder — [`Single`], [`Run`],
//!    [`Cluster`], [`Platform`] — by weight *and* by how many sources fused into it, whichever
//!    reads larger. Both ladders are needed: summed weight alone would draw two small shelves
//!    as one small shelf, which is exactly what "compound rather than stack" forbids.
//!
//! The result is that the picture reports the shape of the data. Documentation written in one
//! burst becomes one dense archive; documentation that accreted over years spreads into
//! several shelves along the limb. Neither is special-cased.
//!
//! # Why position comes off the age gradient
//!
//! A structure sits at its kind's [`anchor`](KindRules::anchor), offset by the vintage of the
//! content it stands for — [`AgeGradient::position_of`](treepo_model::AgeGradient::position_of),
//! the inverse of the shading `F-MAT-4` already applies. So a bookshelf holding three-year-old
//! documentation is drawn on the three-year-old wood, and a work site is out at the vital tip
//! because that is where the recent material is.
//!
//! That is one axis carrying three readings rather than a second axis invented for this
//! feature, which is the same argument [`Mosaic`](treepo_model::Mosaic) makes for running its
//! cells along the limb's length.
//!
//! The anchor is what keeps it honest where the design has an opinion. §8.7's only positional
//! statement is that stockpiles sit "near the base of the relevant limb", so
//! [`Stockpile`](EnrichmentKind::Stockpile) is given a basal anchor and a narrow spread — the
//! requirement is transcribed into a row rather than left to whatever the ages happen to do.
//!
//! # Where the four signals come from, and one deliberate non-choice
//!
//! | Kind | Signal | Primitive |
//! |---|---|---|
//! | [`Bookshelf`](EnrichmentKind::Bookshelf) | share of parchment | `size.category_bytes` (`F-EXT-1`) |
//! | [`Stockpile`](EnrichmentKind::Stockpile) | share of ore | `size.category_bytes` (`F-EXT-1`) |
//! | [`ProvingGround`](EnrichmentKind::ProvingGround) | `test_like_ratio` | [`FolderSignal`](treepo_model::FolderSignal) (`F-EXT-5`) |
//! | [`WorkSite`](EnrichmentKind::WorkSite) | lines changed in thirty days, log-scaled | [`ChurnWindows`](treepo_model::primitives::temporal::ChurnWindows) (`F-EXT-2`) |
//!
//! The first two are the [`Subordinate`](treepo_model::Composition::Subordinate) inventory read
//! one member at a time, which is what that arm was built to be asked. It keeps its whole tail
//! rather than its largest two precisely so a 3% documentation corner of a container can still
//! put a shelf up.
//!
//! [`recency_heat`](treepo_model::TemporalPrimitives::recency_heat) is deliberately *not* the
//! work-site signal, though it is already a `0..=1` activity score and would have been one
//! field access. It is §8.8's driver — glow, sway, particle rate, all of it Thrive — and
//! spending it here would make one number responsible for both a structure Grow places and an
//! animation Thrive runs, so tuning either would move the other. The thirty-day window asks the
//! question this feature actually has, and §8.7's word for the answer is "temporary": a path
//! being rewritten now is a building site, one rewritten three years ago is not.
//!
//! The window is read as an **absolute line count** against
//! [`Normalize::churn_full_scale_lines`](crate::normalize::Normalize::churn_full_scale_lines),
//! not as a share of the path's own lifetime churn. The share was built first and measured, and
//! it does not work: in any young or actively developed repository most of every path's history
//! *is* recent, so the ratio sits near one everywhere. Over this repository it placed a work
//! site on 151 of 151 nodes. `Normalize::heat` has the rest of that argument.
//!
//! [`Single`]: EnrichmentForm::Single
//! [`Run`]: EnrichmentForm::Run
//! [`Cluster`]: EnrichmentForm::Cluster
//! [`Platform`]: EnrichmentForm::Platform

use crate::material::Table;
use crate::params::per_mille;
use alloc::vec::Vec;
use core::fmt;
use serde::Deserialize;
use treepo_det::Fx;
use treepo_model::enrichment::{
    Enrichment, EnrichmentForm, EnrichmentKind, EnrichmentMap, Placement,
};
use treepo_model::material::{FamilyMix, Material, MaterialFamily};
use treepo_model::segment::NodeRole;
use treepo_model::{Manifest, PathRecord, Skeleton};

/// `F-MAT-5`'s rules, as data — a section of `assets/params/materials.ron`.
///
/// It sits in that file rather than one of its own for the reason
/// [`Normalize`](crate::normalize::Normalize) does: enrichment is placed *on* material and
/// reads the same age scale to place it, and two files would be two copies of that scale with
/// two chances to disagree about what "old" means.
///
/// Every field is an integer in a stated unit — the convention `lsystem.ron`,
/// `author-palette.ron` and the rest of `materials.ron` already use, because `N3` keeps floats
/// out of anything reaching generated output and `520` reads as "0.52" where `520000` does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    /// How close two structures of one kind must sit before they grow together, per mille of
    /// the limb's length.
    ///
    /// The knob the compounding vocabulary lives or dies on. Wide, and every kind collapses to
    /// one structure per limb wherever it appears — legible, and blind to whether the content
    /// arrived all at once or over a decade. Narrow, and a directory of forty documents becomes
    /// forty shelves, which [`max_per_kind`](Self::max_per_kind) then has to rescue.
    ///
    /// Refused at or above 500: a window covering half the limb fuses anything the anchors and
    /// spreads could ever separate, so the positional tuning below would be configured and
    /// inert — the failure `clamp_beyond == 0` is refused for, arriving through a legal value.
    pub merge_window: i32,
    /// The most structures of one kind a single node may show — `P6`.
    ///
    /// The excess is not discarded. The closest remaining pair fuses until the count is met,
    /// so a container standing for two hundred documents shows this many dense archives rather
    /// than two hundred shelves nobody can see or none at all.
    ///
    /// At least one, which [`validate`](Self::validate) enforces: zero would place nothing
    /// while the table looked configured.
    pub max_per_kind: u32,
    /// Where the ladder's rungs sit.
    pub forms: Forms,
    /// Per-kind placement rules.
    pub kinds: Kinds,
}

/// The thresholds that grade a structure onto the [`EnrichmentForm`] ladder.
///
/// Two ladders, and a structure takes the higher rung of the two. Weight alone is not enough:
/// two small shelves that fused have twice the weight of one, which may still be a small
/// number, and drawing them as one small shelf is the "stacking rather than compounding"
/// failure this whole feature is defined against. Count alone is not enough either — one
/// enormous documentation directory is a single source and unmistakably an archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Forms {
    /// Weight at which a structure reads as a run rather than a unit, per mille.
    pub run_at: i32,
    /// Weight at which it reads as a dense cluster, per mille.
    pub cluster_at: i32,
    /// Weight at which it becomes its own built surface, per mille.
    pub platform_at: i32,
    /// Fused sources at which a structure reads as a run. At least two.
    pub run_from: u32,
    /// Fused sources at which it reads as a dense cluster.
    pub cluster_from: u32,
    /// Fused sources at which it becomes its own built surface.
    pub platform_from: u32,
}

/// One rule row per kind.
///
/// Named fields rather than an array indexed by [`EnrichmentKind::position`], because this file
/// is read and edited by a person: `stockpile:` says what the row is for and `[1]:` does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kinds {
    /// Docs and library content.
    pub bookshelf: KindRules,
    /// Assets, binaries and media.
    pub stockpile: KindRules,
    /// Tests.
    pub proving_ground: KindRules,
    /// Recently churning content.
    pub work_site: KindRules,
}

impl Kinds {
    /// The row for one kind.
    ///
    /// A `match` rather than a lookup, so a kind added without a row does not compile.
    #[must_use]
    pub const fn of(&self, kind: EnrichmentKind) -> &KindRules {
        match kind {
            EnrichmentKind::Bookshelf => &self.bookshelf,
            EnrichmentKind::Stockpile => &self.stockpile,
            EnrichmentKind::ProvingGround => &self.proving_ground,
            EnrichmentKind::WorkSite => &self.work_site,
        }
    }
}

/// How one kind of structure is placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KindRules {
    /// Where along the limb this kind sits by default, per mille from base to tip.
    ///
    /// What a structure gets when the content it stands for has no vintage to place it by —
    /// and, through [`spread`](Self::spread), the point everything of this kind is arranged
    /// around.
    pub anchor: i32,
    /// How far vintage may move a structure from the anchor, per mille of the limb.
    ///
    /// The full width of the band, so a spread of 400 puts the oldest content 200 per mille
    /// below the anchor and the newest 200 above it. Zero pins every structure of this kind to
    /// the anchor, which is a defensible tuning position and therefore permitted: it says the
    /// kind's conventional place matters and its content's age does not.
    ///
    /// May exceed 1000. The result is clamped to the limb, so a wide spread means "spend the
    /// whole limb and pile up at whichever end runs out first" rather than an error.
    pub spread: i32,
    /// The weight below which nothing of this kind is built at all, per mille.
    ///
    /// `P6` at the small end: a trace of documentation in a source directory is not a shelf,
    /// it is noise with furniture on it. Set per kind rather than globally because the signals
    /// are not equally noisy — a byte count is exact, and a churn ratio on a young path is not.
    pub present_at: i32,
}

/// Builds enrichment for every node of a skeleton — `F-MAT-5` over a whole tree.
///
/// The third pass, after [`grow`](crate::grow) and [`materialize`](crate::materialize), and it
/// takes the [`MaterialMap`](treepo_model::MaterialMap) rather than recomputing anything: a
/// placement's weight is a share of the node's `F-MAT-3` budget and its position is read off
/// the node's `F-MAT-4` gradient, both of which the material pass already settled. Enrichment
/// is built *on* material, and the signature says so.
///
/// Paired by [`NodeId`](treepo_model::NodeId) because the walk visits nodes in skeleton order,
/// exactly as [`materialize`](crate::materialize) does — [`EnrichmentMap::covers`] is the
/// invariant and it holds by construction here rather than by care.
///
/// # A node with no material
///
/// Carries nothing. It cannot be sized or placed without a budget and a gradient, and inventing
/// either would put a structure on a limb that has no surface to build on. Unreachable while
/// [`MaterialMap::covers`](treepo_model::MaterialMap::covers) holds, which
/// `every_node_gets_a_material` is what would notice.
#[must_use]
pub fn enrich(
    manifest: &Manifest,
    skeleton: &Skeleton,
    materials: &treepo_model::MaterialMap,
    table: &Table,
) -> EnrichmentMap {
    let mut map = EnrichmentMap::new();
    for (index, node) in skeleton.nodes().iter().enumerate() {
        let material = materials.get(treepo_model::NodeId::new(index as u32));
        map.push(match material {
            Some(material) => table.enrichment_of(manifest, &node.role, material),
            None => Enrichment::none(),
        });
    }
    map
}

impl Table {
    /// Everything built on one node — the four steps this module's header describes.
    #[must_use]
    pub fn enrichment_of(
        &self,
        manifest: &Manifest,
        role: &NodeRole,
        material: &Material,
    ) -> Enrichment {
        let mut placed: Vec<Placement> = Vec::new();

        for kind in EnrichmentKind::ALL {
            let mut of_kind = self.candidates(manifest, role, material, kind);
            if of_kind.is_empty() {
                continue;
            }

            // Base to tip, so fusion is accretion along the limb rather than an arbitrary
            // pairing. Stable, so equal positions keep the order the paths arrived in, which is
            // the manifest's path order (`N3`).
            of_kind.sort_by_key(|placement| placement.position.to_bits());
            self.fuse_neighbours(&mut of_kind);
            self.densify_to_fit(&mut of_kind);

            for mut placement in of_kind {
                placement.form = self.form_of(&placement);
                placed.push(placement);
            }
        }

        // One order for the whole node, base to tip, with the kind breaking a tie — so a
        // renderer walking a limb meets structures in the order it draws them, and two nodes
        // whose structures coincide cannot encode differently for having been gathered in a
        // different kind order.
        placed.sort_by_key(|placement| (placement.position.to_bits(), placement.kind.position()));
        Enrichment::new(placed)
    }

    /// One unfused candidate per source that offers this kind anything.
    ///
    /// The weight is `(this source's share of the node) × (this kind's signal in it) × (the
    /// node's budget)`, which reads as "how much of the node's drawn surface is this kind of
    /// content". Multiplying by the budget rather than by a byte count is what makes the
    /// thresholds comparable across repositories: the budget is already logged, clamped and
    /// floored, so enrichment inherits `F-MAT-3`'s compression instead of applying a second one
    /// and disagreeing with it about what "large" means.
    fn candidates(
        &self,
        manifest: &Manifest,
        role: &NodeRole,
        material: &Material,
        kind: EnrichmentKind,
    ) -> Vec<Placement> {
        let sources = || {
            role.stands_for()
                .iter()
                .filter_map(|path| manifest.path(path))
        };
        let total = sources().fold(0u64, |sum, record| sum.saturating_add(record.size.bytes));
        if total == 0 {
            // Nothing to apportion. A node whose sources hold no bytes — an empty directory, or
            // a role whose paths are not in the manifest — has no surface to build on, and a
            // share of zero bytes is not a number.
            return Vec::new();
        }

        let floor = per_mille(self.enrich.kinds.of(kind).present_at);
        let mut candidates = Vec::new();

        for record in sources() {
            let signal = self.signal_of(kind, record);
            if signal.is_zero() {
                continue;
            }
            let share = Fx::from_ratio(record.size.bytes as i64, total as i64);
            let weight = share.mul(signal).mul(material.budget);
            if weight < floor {
                continue;
            }
            candidates.push(Placement::single(
                kind,
                self.position_of(kind, record, material, manifest.reference_time),
                weight,
            ));
        }

        candidates
    }

    /// Where one source's structure sits — the anchor, offset by the source's own vintage.
    ///
    /// See the module header. The fallback is the anchor exactly, and it is reached in two
    /// ordinary cases rather than in a failure: a node whose gradient is uniform (a single
    /// commit, so there is no span to place anything within) and a source with no history at
    /// all (PRD §6). Both mean the same thing — age says nothing here — and the kind's
    /// conventional place is the honest answer to that.
    fn position_of(
        &self,
        kind: EnrichmentKind,
        record: &PathRecord,
        material: &Material,
        reference: i64,
    ) -> Fx {
        let rules = self.enrich.kinds.of(kind);
        let vintage = material
            .gradient
            .and_then(|gradient| {
                let days = record.temporal.last_commit_age_days(reference)?;
                gradient.position_of(self.normalize.age(days))
            })
            .unwrap_or(Fx::HALF);

        per_mille(rules.anchor)
            .add(vintage.sub(Fx::HALF).mul(per_mille(rules.spread)))
            .clamp(Fx::ZERO, Fx::ONE)
    }

    /// Grows neighbouring structures into one another — step 2.
    ///
    /// A single forward sweep, comparing each candidate against the *running fused position*
    /// rather than against the last thing absorbed. `placements` must already be in position
    /// order.
    ///
    /// # A fused mass absorbs what is near the mass
    ///
    /// The consequence is worth stating because it looks like a limitation and is the reason
    /// the rule was written this way. As a run grows, its centre of mass lags behind its
    /// leading edge, so a long line of evenly spaced candidates does **not** all collapse into
    /// one structure — it forms a bounded cluster, then starts another.
    ///
    /// That is the behaviour the picture wants. Unbounded chaining would fuse anything
    /// connected by a trail of near neighbours, and a limb whose documentation accreted across
    /// a decade would draw identically to one whose documentation arrived in a week. Bounded
    /// clustering keeps the distinction that `F-MAT-4`'s gradient is already drawing
    /// underneath: several dense archives spread along the limb, rather than one archive
    /// covering it.
    fn fuse_neighbours(&self, placements: &mut Vec<Placement>) {
        let window = per_mille(self.enrich.merge_window);
        let mut fused: Vec<Placement> = Vec::with_capacity(placements.len());

        for candidate in placements.drain(..) {
            match fused.last_mut() {
                Some(previous) if previous.distance_to(&candidate) <= window => {
                    *previous = previous.fuse(candidate);
                }
                _ => fused.push(candidate),
            }
        }

        *placements = fused;
    }

    /// Densifies until a node shows no more than [`Rules::max_per_kind`] of a kind — step 3.
    ///
    /// The closest adjacent pair grows together, repeatedly. Closest rather than smallest,
    /// because the question `P6` poses is which two things a viewer could not have told apart
    /// anyway — and merging by size would move a structure across the limb to join one it has
    /// nothing to do with.
    ///
    /// Fusing two adjacent structures puts the result between them, so the ordering survives
    /// and the loop needs no re-sort. A tie keeps the basal pair, which is arbitrary but total
    /// (`N3`).
    fn densify_to_fit(&self, placements: &mut Vec<Placement>) {
        let limit = self.enrich.max_per_kind.max(1) as usize;

        while placements.len() > limit {
            let mut closest = 0;
            let mut gap = placements[0].distance_to(&placements[1]);
            for index in 1..placements.len() - 1 {
                let candidate = placements[index].distance_to(&placements[index + 1]);
                if candidate < gap {
                    gap = candidate;
                    closest = index;
                }
            }

            let absorbed = placements.remove(closest + 1);
            placements[closest] = placements[closest].fuse(absorbed);
        }
    }

    /// Grades one structure onto the ladder — step 4.
    fn form_of(&self, placement: &Placement) -> EnrichmentForm {
        let forms = &self.enrich.forms;

        let by_weight = if placement.weight >= per_mille(forms.platform_at) {
            EnrichmentForm::Platform
        } else if placement.weight >= per_mille(forms.cluster_at) {
            EnrichmentForm::Cluster
        } else if placement.weight >= per_mille(forms.run_at) {
            EnrichmentForm::Run
        } else {
            EnrichmentForm::Single
        };

        let by_sources = if placement.sources >= forms.platform_from {
            EnrichmentForm::Platform
        } else if placement.sources >= forms.cluster_from {
            EnrichmentForm::Cluster
        } else if placement.sources >= forms.run_from {
            EnrichmentForm::Run
        } else {
            EnrichmentForm::Single
        };

        by_weight.grown(by_sources)
    }
}

impl Table {
    /// How much of one kind of content one record holds, in `0..=1`.
    ///
    /// Zero means "offers nothing of this kind", which is the overwhelmingly common answer and
    /// short-circuits before any arithmetic. See the module header for where each signal comes
    /// from and why.
    fn signal_of(&self, kind: EnrichmentKind, record: &PathRecord) -> Fx {
        match kind {
            EnrichmentKind::Bookshelf => family_share(record, MaterialFamily::Parchment),
            EnrichmentKind::Stockpile => family_share(record, MaterialFamily::Ore),
            // `F-EXT-5` only signals folders the dictionary knows, so a test file scattered
            // through a source directory offers nothing here. That is the honest reading of
            // §8.7's word "clusters" — a proving ground is a place, and one file is not one.
            EnrichmentKind::ProvingGround => record
                .folder_signal
                .as_ref()
                .map_or(Fx::ZERO, |signal| signal.content_modulation.test_like_ratio),
            // Lines changed in the last thirty days, against the absolute scale in
            // `materials.ron` — not against the path's own lifetime churn. See
            // [`Normalize::heat`](crate::normalize::Normalize::heat) for the measurement that
            // settled that, and the module header for why this is not `recency_heat`.
            EnrichmentKind::WorkSite => self.normalize.heat(record.temporal.churn.days_30),
        }
    }
}

/// One record's share of a single material family.
fn family_share(record: &PathRecord, family: MaterialFamily) -> Fx {
    let mut bytes = [0u64; MaterialFamily::ALL.len()];
    for (&category, &count) in &record.size.category_bytes {
        let slot = MaterialFamily::of_category(category).position();
        bytes[slot] = bytes[slot].saturating_add(count);
    }

    let total = bytes.iter().fold(0u64, |sum, &n| sum.saturating_add(n));
    if total == 0 {
        return Fx::ZERO;
    }

    let mut shares = [Fx::ZERO; MaterialFamily::ALL.len()];
    for (share, count) in shares.iter_mut().zip(bytes) {
        *share = Fx::from_ratio(count as i64, total as i64);
    }
    FamilyMix::new(shares).share_of(family)
}

impl Rules {
    /// Checks the section against the rules `F-MAT-5` and `P6` state.
    ///
    /// # Errors
    ///
    /// The first violated rule, naming the row and the requirement it came from.
    pub fn validate(&self) -> Result<(), EnrichError> {
        // Half the limb fuses anything the anchors and spreads below could separate, so the
        // positional tuning would be configured and inert.
        if !(0..500).contains(&self.merge_window) {
            return Err(EnrichError {
                row: "merge_window",
                kind: None,
                detail: "the merge window is a distance in 0..500 per mille — at half the limb \
                         every structure of a kind fuses and the anchors stop meaning anything",
            });
        }

        if self.max_per_kind == 0 {
            return Err(EnrichError {
                row: "max_per_kind",
                kind: None,
                detail: "a node may show at least one structure of a kind, or enrichment is \
                         off while the table looks configured",
            });
        }

        self.forms.validate()?;

        for kind in EnrichmentKind::ALL {
            self.kinds.of(kind).validate(kind)?;
        }

        Ok(())
    }
}

impl Forms {
    /// Checks that both ladders climb.
    fn validate(&self) -> Result<(), EnrichError> {
        for (row, value) in [
            ("forms.run_at", self.run_at),
            ("forms.cluster_at", self.cluster_at),
            ("forms.platform_at", self.platform_at),
        ] {
            if !(1..=1000).contains(&value) {
                return Err(EnrichError {
                    row,
                    kind: None,
                    detail: "a rung of the weight ladder is a share in 1..=1000 per mille",
                });
            }
        }

        // A rung at or below the one under it is a rung nothing can land on, so the ladder
        // would have a step configured and unreachable.
        if self.run_at >= self.cluster_at || self.cluster_at >= self.platform_at {
            return Err(EnrichError {
                row: "forms.cluster_at",
                kind: None,
                detail: "the weight ladder climbs: run_at < cluster_at < platform_at, or a \
                         rung is configured and nothing can ever land on it",
            });
        }

        // One source is a `Single` by definition — that is what the rung means — so a count
        // ladder starting at one would grade every lone structure as a run.
        if self.run_from < 2 {
            return Err(EnrichError {
                row: "forms.run_from",
                kind: None,
                detail: "a run is at least two sources grown together; one source is a Single",
            });
        }

        if self.run_from >= self.cluster_from || self.cluster_from >= self.platform_from {
            return Err(EnrichError {
                row: "forms.cluster_from",
                kind: None,
                detail: "the source ladder climbs: run_from < cluster_from < platform_from, or \
                         a rung is configured and nothing can ever land on it",
            });
        }

        Ok(())
    }
}

impl KindRules {
    /// Checks one kind's row.
    fn validate(&self, kind: EnrichmentKind) -> Result<(), EnrichError> {
        if !(0..=1000).contains(&self.anchor) {
            return Err(EnrichError {
                row: "anchor",
                kind: Some(kind),
                detail: "an anchor is a position on the limb in 0..=1000 per mille",
            });
        }

        // Above the limb's own length the spread is still meaningful — it clamps, which reads
        // as "pile up at the ends" — but a negative one would invert vintage and draw the
        // oldest content at the tip, which is `F-MAT-4` backwards.
        if !(0..=2000).contains(&self.spread) {
            return Err(EnrichError {
                row: "spread",
                kind: Some(kind),
                detail: "a spread is a width in 0..=2000 per mille — negative would place old \
                         content tip-ward, which is F-MAT-4 inverted",
            });
        }

        // Zero would build a structure for a rounding artefact; a full budget would mean only a
        // node made of nothing else could ever have one.
        if !(1..=1000).contains(&self.present_at) {
            return Err(EnrichError {
                row: "present_at",
                kind: Some(kind),
                detail: "a presence floor is a weight in 1..=1000 per mille",
            });
        }

        Ok(())
    }
}

/// Why an enrichment section was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnrichError {
    /// The row that failed.
    pub row: &'static str,
    /// The kind whose row it was, for the per-kind rows.
    pub kind: Option<EnrichmentKind>,
    /// The rule it broke, and where the rule comes from.
    pub detail: &'static str,
}

impl fmt::Display for EnrichError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            Some(kind) => write!(
                f,
                "materials.ron: `enrich.kinds.{}.{}` — {}",
                kind.name(),
                self.row,
                self.detail
            ),
            None => write!(f, "materials.ron: `enrich.{}` — {}", self.row, self.detail),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsystem::compose::tests::manifest_of;
    use alloc::vec;
    use treepo_model::primitives::size::ContentCategory;
    use treepo_model::{AgeGradient, Composition, RepoPath, SizePrimitives};

    fn table() -> Table {
        Table::built_in()
    }

    fn path(text: &str) -> RepoPath {
        RepoPath::new(text.as_bytes()).unwrap()
    }

    /// A material with a full-ish budget and a real span, so weights are not squeezed to
    /// nothing and `position_of` has somewhere to place things.
    fn surface(budget: Fx, gradient: Option<AgeGradient>) -> Material {
        Material {
            family: MaterialFamily::Heartwood,
            composition: Composition::Pure,
            budget,
            mosaic: treepo_model::Mosaic::default(),
            gradient,
        }
    }

    fn placed(kind: EnrichmentKind, position: i64, weight: i64) -> Placement {
        Placement::single(
            kind,
            Fx::from_ratio(position, 100),
            Fx::from_ratio(weight, 100),
        )
    }

    #[test]
    fn the_built_in_section_validates() {
        assert_eq!(table().enrich.validate(), Ok(()));
    }

    /// Every kind must have a row, or a kind that reached the walk would place structures with
    /// no rules behind them.
    #[test]
    fn every_kind_has_a_row_and_they_are_not_all_the_same_row() {
        let kinds = table().enrich.kinds;
        let rows: Vec<KindRules> = EnrichmentKind::ALL.iter().map(|&k| *kinds.of(k)).collect();
        assert_eq!(rows.len(), EnrichmentKind::ALL.len());
        assert!(
            rows.iter().any(|row| *row != rows[0]),
            "every kind is tuned identically, so the per-kind table buys nothing"
        );
    }

    /// §8.7's one positional statement, transcribed: "stockpiles... near the base of the
    /// relevant limb". The whole band a stockpile can occupy has to stay basal, not merely its
    /// anchor — a wide spread would put crates at the tip while the anchor still read as low.
    #[test]
    fn stockpiles_stay_near_the_base_whatever_their_vintage() {
        let t = table();
        let rules = t.enrich.kinds.of(EnrichmentKind::Stockpile);
        let reach = per_mille(rules.anchor).add(per_mille(rules.spread).div(Fx::from_int(2)));
        assert!(
            reach < Fx::HALF,
            "a stockpile can reach {reach} up the limb — §8.7 puts them near the base"
        );
    }

    /// The rule the visual idea rests on: two structures at one place are one larger structure,
    /// and the form climbs because of it.
    #[test]
    fn two_neighbours_grow_into_one_larger_structure() {
        let t = table();
        let window = per_mille(t.enrich.merge_window);
        let mut pair = vec![
            placed(EnrichmentKind::Bookshelf, 40, 5),
            Placement::single(
                EnrichmentKind::Bookshelf,
                Fx::from_ratio(40, 100).add(window),
                Fx::from_ratio(5, 100),
            ),
        ];

        t.fuse_neighbours(&mut pair);
        assert_eq!(pair.len(), 1, "they are one thing now");
        assert_eq!(pair[0].sources, 2);
        // Stated as the sum rather than as `0.10`, because a twentieth is not exact in binary
        // and pinning a decimal across a fixed-point addition pins a rounding artefact rather
        // than the claim — which is only that neither weight was thrown away.
        assert_eq!(
            pair[0].weight,
            Fx::from_ratio(5, 100).add(Fx::from_ratio(5, 100))
        );
        assert_eq!(
            t.form_of(&pair[0]),
            EnrichmentForm::Run,
            "two of a thing is a run, whatever the summed weight came to"
        );
    }

    /// And the counterpart, or fusion would be unconditional: two structures far apart stay
    /// two structures.
    #[test]
    fn structures_far_apart_stay_apart() {
        let t = table();
        let mut spread = vec![
            placed(EnrichmentKind::Bookshelf, 10, 5),
            placed(EnrichmentKind::Bookshelf, 90, 5),
        ];
        t.fuse_neighbours(&mut spread);
        assert_eq!(spread.len(), 2);
        assert!(spread.iter().all(|p| p.sources == 1));
    }

    /// A line of near things densifies into a few dense things rather than a chain of pairs —
    /// which is what comparing against the running fused position buys.
    ///
    /// Not into *one*: the fused mass absorbs what is near the mass, and its centre lags behind
    /// its leading edge, so a long even line forms a bounded cluster and then starts another.
    /// That is the property being pinned, and the reason is in [`Table::fuse_neighbours`] —
    /// unbounded chaining would draw a decade of accreted documentation exactly like a week of
    /// it.
    #[test]
    fn a_line_of_near_structures_densifies_into_a_few_masses() {
        let t = table();
        let step = per_mille(t.enrich.merge_window).div(Fx::from_int(2));
        let mut line: Vec<Placement> = (0..8)
            .map(|i| {
                Placement::single(
                    EnrichmentKind::WorkSite,
                    Fx::from_ratio(10, 100).add(step.mul(Fx::from_int(i))),
                    Fx::from_ratio(3, 100),
                )
            })
            .collect();

        t.fuse_neighbours(&mut line);
        assert!(
            (2..=3).contains(&line.len()),
            "eight candidates became {} structures",
            line.len()
        );
        assert!(
            line.iter().all(|placement| placement.sources > 1),
            "something on a line of near neighbours stayed alone"
        );
        assert_eq!(line.iter().map(|p| p.sources).sum::<u32>(), 8);
        assert!(
            line.iter()
                .any(|placement| t.form_of(placement) >= EnrichmentForm::Cluster),
            "nothing grew past a run, so densifying bought no visual change"
        );
    }

    /// `P6` bounds the count, and the excess *grows into* what stays — the user-facing rule
    /// that nothing is discarded to simplify the picture.
    #[test]
    fn an_overcrowded_kind_densifies_rather_than_dropping_anything() {
        let t = table();
        let limit = t.enrich.max_per_kind as usize;
        // Deliberately far apart, so the window sweep cannot do this work and the cap has to.
        // A sixty-fourth apiece, which is exact in binary — the total is then the claim rather
        // than a fixed-point rounding artefact of nine additions.
        let each = Fx::from_ratio(1, 64);
        let mut many: Vec<Placement> = (0..limit + 5)
            .map(|i| {
                Placement::single(
                    EnrichmentKind::Bookshelf,
                    Fx::from_ratio(i as i64 * 7, 100),
                    each,
                )
            })
            .collect();
        let sources_before = many.len() as u32;

        t.densify_to_fit(&mut many);
        assert_eq!(many.len(), limit);
        assert_eq!(
            many.iter().map(|p| p.sources).sum::<u32>(),
            sources_before,
            "a source was discarded — the picture was simplified by throwing data away"
        );
        assert_eq!(
            many.iter().fold(Fx::ZERO, |sum, p| sum.add(p.weight)),
            Fx::from_ratio(i64::from(sources_before), 64),
            "and the weight it carried went with it"
        );
    }

    /// The two ladders, and the reason there are two. Weight alone would draw a fused pair of
    /// small shelves as one small shelf, which is the stacking failure the feature is defined
    /// against; sources alone would call one enormous archive a `Single`.
    #[test]
    fn form_climbs_by_weight_or_by_how_many_grew_together() {
        let t = table();
        let forms = t.enrich.forms;

        let lone_but_large = Placement {
            weight: per_mille(forms.platform_at),
            ..placed(EnrichmentKind::Bookshelf, 50, 0)
        };
        assert_eq!(t.form_of(&lone_but_large), EnrichmentForm::Platform);
        assert_eq!(
            lone_but_large.sources, 1,
            "one source, and still a platform"
        );

        let many_but_slight = Placement {
            sources: forms.cluster_from,
            weight: Fx::from_ratio(1, 1000),
            ..placed(EnrichmentKind::Bookshelf, 50, 0)
        };
        assert_eq!(
            t.form_of(&many_but_slight),
            EnrichmentForm::Cluster,
            "several small things that grew together are not one small thing"
        );

        let slight = placed(EnrichmentKind::Bookshelf, 50, 0);
        assert_eq!(t.form_of(&slight), EnrichmentForm::Single);
    }

    /// Position comes off the age gradient, so a structure lands on material of its own
    /// vintage — old content basal, recent content tip-ward, exactly as `F-MAT-4` shades it.
    #[test]
    fn vintage_places_a_structure_along_the_limb() {
        let t = table();
        let reference = crate::lsystem::compose::tests::REFERENCE;
        const DAY: i64 = 86_400;

        let gradient = AgeGradient::new(t.normalize.age(1000), t.normalize.age(1));
        let material = surface(Fx::HALF, Some(gradient));

        let dated = |days: i64| {
            let mut record =
                treepo_model::PathRecord::new(path("docs"), treepo_model::NodeKind::Directory);
            record.temporal.first_commit_time = Some(reference - 1000 * DAY);
            record.temporal.last_commit_time = Some(reference - days * DAY);
            record
        };

        let old = t.position_of(
            EnrichmentKind::Bookshelf,
            &dated(1000),
            &material,
            reference,
        );
        let recent = t.position_of(EnrichmentKind::Bookshelf, &dated(1), &material, reference);

        assert!(
            old < recent,
            "old content sits basal and recent content tip-ward: old {old}, recent {recent}"
        );
        // And the anchor is what they are arranged around, not one of the two ends.
        let anchor = per_mille(t.enrich.kinds.of(EnrichmentKind::Bookshelf).anchor);
        assert!(old < anchor && recent > anchor);
    }

    /// A uniform gradient has no span to place anything within, and a source with no history
    /// has no vintage at all. Both are ordinary (PRD §6), and both fall back to the kind's
    /// conventional place rather than to an invented one.
    #[test]
    fn a_source_with_no_vintage_sits_at_its_anchor() {
        let t = table();
        let anchor = per_mille(t.enrich.kinds.of(EnrichmentKind::Stockpile).anchor);
        let undated =
            treepo_model::PathRecord::new(path("assets"), treepo_model::NodeKind::Directory);

        for gradient in [None, Some(AgeGradient::uniform(Fx::HALF))] {
            let at = t.position_of(
                EnrichmentKind::Stockpile,
                &undated,
                &surface(Fx::HALF, gradient),
                1_800_000_000,
            );
            assert_eq!(at, anchor, "gradient {gradient:?}");
        }
    }

    /// The four signals, each on a record that offers exactly one of them — so a mapping that
    /// crossed two kinds would fail rather than pass by coincidence.
    #[test]
    fn each_kind_reads_its_own_signal() {
        let t = table();
        let mut docs =
            treepo_model::PathRecord::new(path("docs/guide.md"), treepo_model::NodeKind::File);
        docs.size.category_bytes = core::iter::once((ContentCategory::Docs, 1000)).collect();

        let mut asset =
            treepo_model::PathRecord::new(path("assets/logo.png"), treepo_model::NodeKind::File);
        asset.size.category_bytes = core::iter::once((ContentCategory::Asset, 1000)).collect();

        let mut tests =
            treepo_model::PathRecord::new(path("tests"), treepo_model::NodeKind::Directory);
        let mut signal = treepo_model::FolderSignal::unmodulated("tests".into(), Fx::ONE, 1);
        signal.content_modulation.test_like_ratio = Fx::from_ratio(4, 5);
        tests.folder_signal = Some(signal);

        let mut churning =
            treepo_model::PathRecord::new(path("src/hot.rs"), treepo_model::NodeKind::File);
        churning.temporal.churn.lifetime = 400;
        churning.temporal.churn.days_30 = 300;

        let cases = [
            (EnrichmentKind::Bookshelf, &docs),
            (EnrichmentKind::Stockpile, &asset),
            (EnrichmentKind::ProvingGround, &tests),
            (EnrichmentKind::WorkSite, &churning),
        ];

        for (kind, record) in cases {
            assert!(
                !t.signal_of(kind, record).is_zero(),
                "{kind:?} reads nothing from {:?}",
                record.path
            );
            for other in EnrichmentKind::ALL {
                if other == kind {
                    continue;
                }
                assert!(
                    t.signal_of(other, record).is_zero(),
                    "{other:?} also fires on {:?}, so the two kinds cannot be told apart",
                    record.path
                );
            }
        }
    }

    /// A ratio rather than a rate, so a project's tempo cannot make everything a building
    /// The work-site signal asks about the *work*, not about the proportion — the correction a
    /// measurement forced. A path with its whole short life inside the window is not a building
    /// site if that life was four lines; one being rewritten at five hundred lines a month is,
    /// however long it has existed.
    ///
    /// The rejected formulation would have failed this: as a share of lifetime churn the young
    /// four-line path reads 1.0 and the heavily-rewritten decade-old one reads near zero.
    #[test]
    fn a_work_site_is_the_work_rather_than_the_proportion() {
        let n = table().normalize;

        let mut young =
            treepo_model::PathRecord::new(path("src/new.rs"), treepo_model::NodeKind::File);
        young.temporal.churn.lifetime = 4;
        young.temporal.churn.days_30 = 4;

        let mut busy =
            treepo_model::PathRecord::new(path("src/hot.rs"), treepo_model::NodeKind::File);
        busy.temporal.churn.lifetime = 100_000;
        busy.temporal.churn.days_30 = 4_000;

        let t = table();
        let barely = t.signal_of(EnrichmentKind::WorkSite, &young);
        let hammered = t.signal_of(EnrichmentKind::WorkSite, &busy);

        assert!(
            hammered > barely,
            "the wholly-recent four-line path outranks the one being rewritten: {barely} vs \
             {hammered} — this is the formulation that placed a work site on every node"
        );

        // Untouched this month is not a building site, whatever its history.
        let mut dormant =
            treepo_model::PathRecord::new(path("vendor/old.rs"), treepo_model::NodeKind::File);
        dormant.temporal.churn.lifetime = 900_000;
        assert_eq!(t.signal_of(EnrichmentKind::WorkSite, &dormant), Fx::ZERO);

        // And the scale saturates rather than passing one.
        assert_eq!(n.heat(0), Fx::ZERO);
        assert_eq!(n.heat(n.churn_full_scale_lines), Fx::ONE);
        assert_eq!(n.heat(u64::MAX), Fx::ONE);
    }

    /// `P6` at the small end: a trace of documentation in a source directory is noise, not
    /// furniture.
    #[test]
    fn a_trace_of_something_builds_nothing() {
        let t = table();
        let manifest = manifest_of(&[("src/main.rs", 400_000), ("src/notes.md", 40)]);
        let role = NodeRole::Limb { path: path("src") };
        let material = surface(t.normalize.budget(400_040), None);

        let enrichment = t.enrichment_of(&manifest, &role, &material);
        assert!(
            !enrichment.carries(EnrichmentKind::Bookshelf),
            "a hundredth of a percent of docs put a shelf up"
        );
    }

    // ---------------------------------------------------------------------------------
    // The walk
    // ---------------------------------------------------------------------------------

    /// A repository with something of every kind in it, shaped so that composition produces
    /// multi-path nodes as well as limbs.
    ///
    /// The shape is not decoration. A [`Limb`](NodeRole::Limb) stands for exactly one path, so
    /// it offers at most one structure per kind and nothing on it can ever compound — which is
    /// correct rather than a limitation: one thing is one thing. Compounding is a property of
    /// the roles that stand for *several* paths, [`Group`](NodeRole::Group) and
    /// [`Aggregate`](NodeRole::Aggregate), because several things at one place is the situation
    /// the rules exist for. A fixture without either would make every fusion assertion
    /// vacuously true, which is exactly what the first draft of this one did.
    ///
    /// So: twelve tiny top-level entries for `F2` to group, and a deep wide documentation
    /// subtree for `F-SKEL-7` to aggregate.
    fn furnished_repository() -> Manifest {
        let mut files: Vec<(alloc::string::String, u64)> = Vec::new();

        for i in 0..12 {
            files.push((alloc::format!("tiny{i}/mod.rs"), 60));
        }
        files.push(("src/main.rs".into(), 40_000));
        files.push(("src/lib.rs".into(), 30_000));
        for i in 0..30 {
            files.push((alloc::format!("docs/manual/part/chapter{i}.md"), 12_000));
        }
        files.push(("docs/README.md".into(), 4_000));
        for i in 0..5 {
            files.push((alloc::format!("assets/sprite{i}.png"), 90_000));
        }
        for i in 0..6 {
            files.push((alloc::format!("tests/case{i}.rs"), 8_000));
        }
        files.push(("Cargo.toml".into(), 900));

        let borrowed: Vec<(&str, u64)> = files.iter().map(|(p, b)| (p.as_str(), *b)).collect();
        manifest_of(&borrowed)
    }

    fn furnished_tree() -> (Manifest, Skeleton, treepo_model::MaterialMap, EnrichmentMap) {
        let manifest = furnished_repository();
        let table = table();
        let skeleton = crate::grow(&manifest, &crate::Table::built_in());
        let materials = crate::materialize(&manifest, &skeleton, &table);
        let enrichment = enrich(&manifest, &skeleton, &materials, &table);
        (manifest, skeleton, materials, enrichment)
    }

    /// The pairing invariant, and it must hold for every node rather than for a convenient one.
    #[test]
    fn every_node_is_covered_and_nothing_is_placed_off_the_limb() {
        let (_, skeleton, _, enrichment) = furnished_tree();

        assert!(enrichment.covers(&skeleton));
        assert!(
            enrichment.placement_count() > 0,
            "nothing was built anywhere — the fixture or the thresholds are wrong"
        );

        for (id, node) in enrichment.iter() {
            for placement in node.placements() {
                assert!(
                    placement.position >= Fx::ZERO && placement.position <= Fx::ONE,
                    "node {id:?} has a structure at {} — off the limb",
                    placement.position
                );
                assert!(placement.sources > 0);
                assert!(placement.weight > Fx::ZERO);
            }
        }
    }

    /// All four kinds have to be reachable over a real walk, or the table rows for the ones
    /// that never fire are untested and the feature ships three-quarters built.
    #[test]
    fn every_kind_is_reachable_over_a_real_repository() {
        let (_, _, _, enrichment) = furnished_tree();

        for kind in EnrichmentKind::ALL {
            let count = enrichment
                .nodes()
                .iter()
                .filter(|node| node.carries(kind))
                .count();
            assert!(count > 0, "{kind:?} was never placed anywhere");
        }
    }

    /// The user-facing rule over the whole walk: `P6` bounds what is drawn, and everything it
    /// bounded is still accounted for inside what stays.
    #[test]
    fn no_node_shows_more_than_the_table_permits_and_none_of_the_excess_is_lost() {
        let (manifest, skeleton, materials, enrichment) = furnished_tree();
        let t = table();

        let mut compounded = 0;
        for (index, node) in skeleton.nodes().iter().enumerate() {
            let id = treepo_model::NodeId::new(index as u32);
            let placed = enrichment.get(id).unwrap();

            for kind in EnrichmentKind::ALL {
                let shown = placed.of_kind(kind).count() as u32;
                assert!(
                    shown <= t.enrich.max_per_kind,
                    "{:?} shows {shown} of {kind:?}",
                    node.role
                );
            }

            // Every candidate the pass generated is inside something that survived.
            let material = materials.get(id).unwrap();
            let offered: u32 = EnrichmentKind::ALL
                .into_iter()
                .map(|kind| t.candidates(&manifest, &node.role, material, kind).len() as u32)
                .sum();
            assert_eq!(
                u64::from(offered),
                placed.sources(),
                "{:?} offered {offered} sources and accounts for {}",
                node.role,
                placed.sources()
            );
            if placed.len() < offered as usize {
                compounded += 1;
            }
        }

        assert!(
            compounded > 0,
            "nothing ever compounded, so the fusion rules are untested by this fixture"
        );
    }

    /// The distinction the whole design turns on: a node standing for many documents shows a
    /// *bigger* structure, not more of them.
    ///
    /// An [`Aggregate`](NodeRole::Aggregate) over the chapters rather than the `docs` limb,
    /// because a limb stands for one path and would show one structure whatever the rules did —
    /// the assertion would hold under a pass that never fused anything. A container standing
    /// for a dozen separate documents is the case where "fewer, larger" is a claim.
    #[test]
    fn many_documents_read_as_one_larger_archive_rather_than_many_shelves() {
        let t = table();
        let manifest = furnished_repository();

        let chapters: Vec<treepo_model::RepoPath> = (0..12)
            .map(|i| path(&alloc::format!("docs/manual/part/chapter{i}.md")))
            .collect();
        let bytes = chapters
            .iter()
            .filter_map(|p| manifest.path(p))
            .fold(0u64, |sum, record| sum + record.size.bytes);

        let container = NodeRole::Aggregate(treepo_model::AggregateNode {
            anchor: path("docs/manual/part"),
            index: 0,
            members: chapters.clone(),
            bytes,
            file_count: chapters.len() as u32,
            dir_count: 0,
        });

        let record = manifest.path(&path("docs/manual/part")).unwrap();
        let material = t.material_of(
            &record.size,
            bytes,
            &record.ownership,
            Some(crate::material::AgeSpan {
                oldest_days: record
                    .temporal
                    .first_commit_age_days(manifest.reference_time)
                    .unwrap(),
                newest_days: record
                    .temporal
                    .last_commit_age_days(manifest.reference_time)
                    .unwrap(),
            }),
            &container,
        );

        // The container really does offer one candidate per document, or "fewer" means nothing.
        let offered = t.candidates(&manifest, &container, &material, EnrichmentKind::Bookshelf);
        assert_eq!(
            offered.len(),
            chapters.len(),
            "every chapter offers a shelf"
        );

        let enrichment = t.enrichment_of(&manifest, &container, &material);
        let shelves: Vec<&Placement> = enrichment.of_kind(EnrichmentKind::Bookshelf).collect();

        assert!(
            shelves.len() < offered.len(),
            "{} documents stayed {} separate shelves",
            offered.len(),
            shelves.len()
        );
        assert!(
            shelves.iter().map(|s| s.sources).sum::<u32>() == offered.len() as u32,
            "and not by dropping any of them"
        );
        assert!(
            shelves
                .iter()
                .any(|shelf| shelf.form >= EnrichmentForm::Cluster),
            "nothing grew past a run — compounding made it fewer but not larger: {:?}",
            shelves.iter().map(|s| s.form).collect::<Vec<_>>()
        );
    }

    /// A limb of plain source has nothing built on it, and that is what plain source should
    /// look like. Without this, enrichment is texture rather than information.
    #[test]
    fn a_plain_source_limb_carries_nothing() {
        let t = table();
        let manifest = manifest_of(&[("src/main.rs", 40_000), ("src/lib.rs", 30_000)]);
        let role = NodeRole::Limb {
            path: path("src/main.rs"),
        };
        let material = surface(t.normalize.budget(40_000), None);
        assert!(t.enrichment_of(&manifest, &role, &material).is_empty());
    }

    /// PRD §6, "Empty repository": nothing to build on, and nothing built.
    #[test]
    fn an_empty_repository_builds_nothing() {
        let t = table();
        let mut manifest = Manifest::new(
            alloc::string::String::from("test"),
            treepo_det::Seed::root(b"empty"),
        );
        manifest.set_paths(vec![treepo_model::PathRecord::new(
            RepoPath::root(),
            treepo_model::NodeKind::Directory,
        )]);

        let skeleton = crate::grow(&manifest, &crate::Table::built_in());
        let materials = crate::materialize(&manifest, &skeleton, &t);
        let enrichment = enrich(&manifest, &skeleton, &materials, &t);

        assert!(enrichment.covers(&skeleton));
        assert_eq!(enrichment.placement_count(), 0);
    }

    /// A node whose sources hold no bytes has no surface to apportion, and a share of zero
    /// bytes is not a number.
    #[test]
    fn a_node_of_no_bytes_is_not_a_division_by_zero() {
        let t = table();
        let manifest = manifest_of(&[("src/main.rs", 4000)]);
        let missing = NodeRole::Limb {
            path: path("nowhere/at/all"),
        };
        assert!(
            t.enrichment_of(&manifest, &missing, &surface(Fx::HALF, None))
                .is_empty()
        );
    }

    /// `AC-DET-1`, over a whole tree: "byte-identical serialized skeletons, materials, and
    /// **enrichment placements**".
    #[test]
    fn the_same_repository_enriches_the_same_way_twice() {
        let (manifest, skeleton, materials, first) = furnished_tree();
        let t = table();

        assert_eq!(
            enrich(&manifest, &skeleton, &materials, &t).digest(),
            first.digest()
        );

        // And a freshly built identical manifest lands in the same place — nothing in the pass
        // reads a pointer, an allocation address, or an iteration order.
        let (_, _, _, rebuilt) = furnished_tree();
        assert_eq!(rebuilt.digest(), first.digest());
    }

    /// A table that breaks a stated rule is refused, naming the row.
    #[test]
    fn a_section_that_breaks_a_stated_rule_is_refused() {
        let base = table().enrich;

        let cases: [(&str, Rules); 8] = [
            (
                "merge_window",
                Rules {
                    merge_window: 500,
                    ..base
                },
            ),
            (
                "max_per_kind",
                Rules {
                    max_per_kind: 0,
                    ..base
                },
            ),
            (
                "forms.run_at",
                Rules {
                    forms: Forms {
                        run_at: 0,
                        ..base.forms
                    },
                    ..base
                },
            ),
            // A rung nothing can land on: the ladder must climb.
            (
                "forms.cluster_at",
                Rules {
                    forms: Forms {
                        cluster_at: base.forms.run_at,
                        ..base.forms
                    },
                    ..base
                },
            ),
            (
                "forms.run_from",
                Rules {
                    forms: Forms {
                        run_from: 1,
                        ..base.forms
                    },
                    ..base
                },
            ),
            (
                "forms.cluster_from",
                Rules {
                    forms: Forms {
                        cluster_from: base.forms.platform_from,
                        ..base.forms
                    },
                    ..base
                },
            ),
            (
                "anchor",
                Rules {
                    kinds: Kinds {
                        bookshelf: KindRules {
                            anchor: 1001,
                            ..base.kinds.bookshelf
                        },
                        ..base.kinds
                    },
                    ..base
                },
            ),
            // Negative would place old content tip-ward — F-MAT-4 inverted.
            (
                "spread",
                Rules {
                    kinds: Kinds {
                        work_site: KindRules {
                            spread: -100,
                            ..base.kinds.work_site
                        },
                        ..base.kinds
                    },
                    ..base
                },
            ),
        ];

        for (row, rules) in cases {
            let error = rules.validate().expect_err("should have been refused");
            assert_eq!(error.row, row, "{rules:?} was refused for the wrong reason");
        }

        // And a per-kind failure says which kind, or a reader has four rows to check.
        let inverted = Rules {
            kinds: Kinds {
                work_site: KindRules {
                    present_at: 0,
                    ..base.kinds.work_site
                },
                ..base.kinds
            },
            ..base
        };
        let error = inverted.validate().expect_err("should have been refused");
        assert_eq!(error.kind, Some(EnrichmentKind::WorkSite));
        assert!(alloc::format!("{error}").contains("work_site"));
    }

    /// Unused-import guard for the size primitive the signal reader depends on, and a check
    /// that a record with no categories offers nothing rather than dividing by zero.
    #[test]
    fn a_record_with_no_measured_content_offers_nothing() {
        let mut bare =
            treepo_model::PathRecord::new(path("src"), treepo_model::NodeKind::Directory);
        bare.size = SizePrimitives::default();
        assert_eq!(family_share(&bare, MaterialFamily::Parchment), Fx::ZERO);
        assert_eq!(family_share(&bare, MaterialFamily::Ore), Fx::ZERO);
    }
}
