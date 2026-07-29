//! What is placed *on* a limb, and how several of them grow together — `F-MAT-5`,
//! `design/feature-system.md` §8.7.
//!
//! > Semantic enrichment structures placed during Grow: docs → bookshelves/archive platforms;
//! > assets/binaries → stockpiles and crates; tests → distinct secondary growth or
//! > proving-ground platforms; high-churn clusters → work sites.
//!
//! [`material`](crate::material) says what a limb is *made of*. This says what has been *built
//! on* it, which is a different fact about the same node: a limb of parchment is documentation,
//! and a bookshelf on a limb of heartwood is documentation kept beside code. §8.7 is explicit
//! that these are "placed and parameterized during Grow; Thrive only animates them", so they
//! are a handoff exactly as [`Material`](crate::material::Material) is, and they live here for
//! the same reason.
//!
//! # Things grow together rather than stacking
//!
//! The rule that shapes every type here: two placements of one kind at one place on a limb are
//! **one larger thing**, never two things overlapping. Two shelves become a taller run of
//! shelving; two crates become a braced pile; several work sites densify into a yard. Nothing
//! is discarded to make that happen — [`Placement::fuse`] adds the weights and keeps the
//! source count, so what compounded is still legible in what it compounded into.
//!
//! This is why [`EnrichmentForm`] is a ladder rather than a size. A size would say "this shelf
//! is 2.4 units tall", which is a number a renderer would have to invent an appearance for at
//! every value. A ladder says "this is a cluster", which is a sprite. `P6` — legibility bounds
//! detail — and §8.4 says the same thing in the enrichment's own words: at detail scale, size
//! "modulates *quality*, *fanciness*, *shelf placement*, or *material richness* rather than
//! pure scale".
//!
//! # Position is the axis the rest of the material layer already uses
//!
//! [`Placement::position`] runs base to tip, the same axis [`Mosaic`](crate::material::Mosaic)
//! lays its cells along and [`AgeGradient`](crate::material::AgeGradient) shades. Three
//! readings of one limb on one axis is a coherent picture; three axes would be three pictures
//! sharing a shape.
//!
//! # What is not here
//!
//! No sprite, no colour, no pixel size. A placement says *what*, *where*, *how much* and *how
//! many grew together*; what a `Cluster` of `Bookshelf` looks like is `treepo-render`'s, and
//! putting an appearance here would make the parameter table a rendering decision.

use crate::segment::NodeId;
use alloc::vec::Vec;
use treepo_det::{Digest, Fx, Sha256};

/// The four structures `F-MAT-5` names — `design/feature-system.md` §8.7.
///
/// Four rather than one per material family, because enrichment answers a different question
/// from [`MaterialFamily`](crate::material::MaterialFamily). A family is what a surface is; a
/// kind is what somebody built. `Resin` and `Machined` have no entry here and that is not an
/// omission — §8.7 lists four structures and a fifth invented to fill the grid would be a
/// shape with no requirement behind it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EnrichmentKind {
    /// Docs and library folders — shelves, cases, archive platforms.
    ///
    /// §8.7: "small pixel bookshelves or archive platforms built into or hanging from the
    /// limb. Individual files become books (or scrolls)."
    Bookshelf,
    /// Assets, binaries and media — crates, piles, loading platforms.
    ///
    /// §8.7 places these "near the base of the relevant limb", which is the only positional
    /// statement the design makes about enrichment and therefore the one row of the table that
    /// is transcribed rather than chosen.
    Stockpile,
    /// Tests — markers, secondary growth, proving grounds.
    ///
    /// §8.7: "distinct secondary growth, hanging markers, or small 'proving ground'
    /// platforms." Distal, because secondary growth happens where a thing is growing.
    ProvingGround,
    /// Recently churning content — scaffolds, work sites, yards.
    ///
    /// §8.7: "elevated particle systems, more restless workers, or temporary 'work sites'."
    /// The word is *temporary*, and it is the reason this kind reads a recent window rather
    /// than a lifetime total: a directory that was rewritten three years ago is not a building
    /// site now.
    WorkSite,
}

impl EnrichmentKind {
    /// Every kind, in declaration order.
    pub const ALL: [Self; 4] = [
        Self::Bookshelf,
        Self::Stockpile,
        Self::ProvingGround,
        Self::WorkSite,
    ];

    /// This kind's index in [`ALL`](Self::ALL) — how the parameter table is addressed.
    ///
    /// A `match` rather than a search, so adding a kind without giving it a slot does not
    /// compile. Same discipline as [`MaterialFamily::position`](crate::material::MaterialFamily::position).
    #[must_use]
    pub const fn position(self) -> usize {
        match self {
            Self::Bookshelf => 0,
            Self::Stockpile => 1,
            Self::ProvingGround => 2,
            Self::WorkSite => 3,
        }
    }

    /// A short stable name, for the debug surface and for parameter-table errors.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Bookshelf => "bookshelf",
            Self::Stockpile => "stockpile",
            Self::ProvingGround => "proving_ground",
            Self::WorkSite => "work_site",
        }
    }
}

/// How much of a thing grew here — the compounding ladder.
///
/// Four rungs, and the user-facing requirement is that each one **grows out of** the one below
/// rather than replacing it. A `Run` is more of what a `Single` was; a `Platform` is what a
/// `Cluster` becomes when it can no longer be read as separate pieces. That is what makes
/// enrichment scale gracefully from a repository with one README to one with a documentation
/// site, without a special case at either end.
///
/// The reading per kind, which is `treepo-render`'s to draw and is written down here so that
/// four sprite sets are designed against one vocabulary:
///
/// | | [`Bookshelf`](EnrichmentKind::Bookshelf) | [`Stockpile`](EnrichmentKind::Stockpile) | [`ProvingGround`](EnrichmentKind::ProvingGround) | [`WorkSite`](EnrichmentKind::WorkSite) |
/// |---|---|---|---|---|
/// | [`Single`](Self::Single) | one shelf | one crate | a hanging marker | a scaffold pole |
/// | [`Run`](Self::Run) | a run of shelving | a stack of crates | a row of markers | scaffolding |
/// | [`Cluster`](Self::Cluster) | a stacked case | a braced pile | secondary growth | a work site |
/// | [`Platform`](Self::Platform) | an archive platform | a loading platform | a proving ground | a yard |
///
/// Ordered, and the order is the whole point — [`fuse`](Placement::fuse) takes the greater of
/// two forms so that compounding can never quietly demote something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EnrichmentForm {
    /// One unit. The floor, and the form a lone file earns.
    Single,
    /// A short run of units — several of one thing, still countable.
    Run,
    /// Dense enough to read as a mass rather than as its parts.
    Cluster,
    /// Its own built surface, hanging from or built into the limb.
    Platform,
}

impl EnrichmentForm {
    /// Every form, coarsest first.
    pub const ALL: [Self; 4] = [Self::Single, Self::Run, Self::Cluster, Self::Platform];

    /// This form's rung, counted from zero — how it reaches a digest.
    #[must_use]
    pub const fn position(self) -> usize {
        match self {
            Self::Single => 0,
            Self::Run => 1,
            Self::Cluster => 2,
            Self::Platform => 3,
        }
    }

    /// The larger of two forms.
    ///
    /// `const`, and by rung rather than by [`Ord`], so it stays usable where a comparison
    /// cannot be.
    #[must_use]
    pub const fn grown(self, other: Self) -> Self {
        if other.position() > self.position() {
            other
        } else {
            self
        }
    }
}

/// One structure on one limb — `F-MAT-5`.
///
/// # Why a weight *and* a source count
///
/// They answer different questions and the compounding rules need both. [`weight`](Self::weight)
/// is how much of the limb's drawn surface this content accounts for — a proportion, so a
/// documentation directory in a small repository and one in a large repository are comparable.
/// [`sources`](Self::sources) is how many separate pieces of content grew together here, which
/// is what makes "two small bookshelves become a denser section of shelf" possible at all: two
/// small things are still two things, and a rule reading only their summed weight would draw
/// them as one small thing.
///
/// Neither is a count of anything a person did, so `N4` has nothing to say about them. Sources
/// are *paths*, and `design/feature-system.md`'s aggregation ranks paths freely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    /// What was built.
    pub kind: EnrichmentKind,
    /// How much of it — the compounding rung.
    pub form: EnrichmentForm,
    /// Where along the limb, base to tip, in `0..=1`.
    ///
    /// The same axis [`Mosaic`](crate::material::Mosaic) runs its cells along and
    /// [`AgeGradient`](crate::material::AgeGradient) shades, so a renderer resolves all three
    /// against one parameterization of the limb.
    pub position: Fx,
    /// How much of the node's drawn surface this content accounts for, in `0..=1`.
    ///
    /// A proportion of the node's own `F-MAT-3` budget, so it is already logged, clamped and
    /// floored — enrichment inherits the size compression rather than applying a second one.
    pub weight: Fx,
    /// How many separate pieces of content grew together here. Never zero.
    pub sources: u32,
}

impl Placement {
    /// One piece of content, before anything has grown into it.
    #[must_use]
    pub const fn single(kind: EnrichmentKind, position: Fx, weight: Fx) -> Self {
        Self {
            kind,
            form: EnrichmentForm::Single,
            position,
            weight,
            sources: 1,
        }
    }

    /// Grows `other` into this placement — the compounding rule.
    ///
    /// Weights add and source counts add, because nothing is discarded: the point of fusing is
    /// that two shelves become *more shelf*, not that one of them was dropped for tidiness.
    ///
    /// The position moves to the mass-weighted mean of the two, which is what "they grew
    /// together" means positionally — a large stockpile absorbing a small one barely moves,
    /// and two equal ones meet in the middle. Computed as a
    /// [`lerp`](treepo_det::Fx::lerp) toward the newcomer rather than as a sum of products
    /// divided by a total, so that fusing a long chain never accumulates a large intermediate
    /// and the arithmetic stays in the same well-conditioned range at every step.
    ///
    /// The form is only ever the greater of the two. The caller re-derives it from its own
    /// table afterwards — this is a floor, so a fused placement is never briefly in a state
    /// that says less than either of its parts did.
    ///
    /// # Order matters, and that is intended
    ///
    /// Fusing A then B is not identical to fusing B then A in the last bit of the position,
    /// because fixed-point rounding is not associative. The caller fuses in position order,
    /// base to tip, which is both deterministic (`N3`) and the order in which things actually
    /// accrete along a limb.
    #[must_use]
    pub fn fuse(self, other: Self) -> Self {
        let total = self.weight.add(other.weight);
        let toward = if total.is_zero() {
            // Two weightless placements have no mass to be centred on; the midpoint is the
            // only answer that does not privilege one of them. Unreachable through
            // `treepo-gen`, which drops anything under its presence floor.
            Fx::HALF
        } else {
            other.weight.div(total)
        };

        Self {
            kind: self.kind,
            form: self.form.grown(other.form),
            position: self.position.lerp(other.position, toward),
            weight: total,
            sources: self.sources.saturating_add(other.sources),
        }
    }

    /// How far apart two placements sit along the limb.
    #[must_use]
    pub fn distance_to(&self, other: &Self) -> Fx {
        self.position.sub(other.position).abs()
    }
}

/// Everything built on one node, base to tip.
///
/// Empty for most nodes, and that is the ordinary case rather than a gap: a limb of plain
/// source code has nothing built on it, which is what a limb of plain source code should look
/// like. `P6` again — enrichment that appeared everywhere would be texture, not information.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Enrichment {
    placements: Vec<Placement>,
}

impl Enrichment {
    /// Nothing built here.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            placements: Vec::new(),
        }
    }

    /// From placements already ordered base to tip.
    ///
    /// The order is the caller's to establish and this does not re-sort: `treepo-gen` produces
    /// them in position order by construction, and a second sort here would be a second
    /// opinion about ties.
    #[must_use]
    pub const fn new(placements: Vec<Placement>) -> Self {
        Self { placements }
    }

    /// Everything built here, base to tip.
    #[must_use]
    pub fn placements(&self) -> &[Placement] {
        &self.placements
    }

    /// Everything of one kind, base to tip.
    pub fn of_kind(&self, kind: EnrichmentKind) -> impl Iterator<Item = &Placement> {
        self.placements
            .iter()
            .filter(move |placement| placement.kind == kind)
    }

    /// Whether anything of this kind was built here.
    #[must_use]
    pub fn carries(&self, kind: EnrichmentKind) -> bool {
        self.of_kind(kind).next().is_some()
    }

    /// How many separate structures are on this node.
    #[must_use]
    pub fn len(&self) -> usize {
        self.placements.len()
    }

    /// Whether nothing was built here — the common case.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.placements.is_empty()
    }

    /// How many pieces of content are represented, counting everything that fused.
    ///
    /// The number `P6`'s bound is actually about: a node showing four structures may be
    /// standing for forty files, and this is what says so.
    #[must_use]
    pub fn sources(&self) -> u64 {
        self.placements
            .iter()
            .map(|placement| u64::from(placement.sources))
            .sum()
    }
}

/// Every node's enrichment, indexed by [`NodeId`](crate::segment::NodeId).
///
/// The third map of a generated tree, beside [`Skeleton`](crate::Skeleton) and
/// [`MaterialMap`](crate::material::MaterialMap) — which is exactly how `AC-DET-1` names them:
/// "byte-identical serialized skeletons, materials, **and enrichment placements**". Three
/// things in the criterion, three maps, three digests.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EnrichmentMap {
    nodes: Vec<Enrichment>,
}

impl EnrichmentMap {
    /// An empty map.
    #[must_use]
    pub const fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    /// Appends the enrichment for the next node, returning the id it was given.
    ///
    /// The only way to add one, so an id cannot disagree with the position it indexes — the
    /// guarantee [`Skeleton::push_node`](crate::Skeleton::push_node) and
    /// [`MaterialMap::push`](crate::material::MaterialMap::push) both give.
    pub fn push(&mut self, enrichment: Enrichment) -> NodeId {
        let id = NodeId::new(u32::try_from(self.nodes.len()).unwrap_or(u32::MAX));
        self.nodes.push(enrichment);
        id
    }

    /// One node's enrichment.
    #[must_use]
    pub fn get(&self, id: NodeId) -> Option<&Enrichment> {
        self.nodes.get(id.index())
    }

    /// Every node's enrichment, in node order.
    #[must_use]
    pub fn nodes(&self) -> &[Enrichment] {
        &self.nodes
    }

    /// Every node's enrichment with the node it belongs to.
    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &Enrichment)> {
        self.nodes
            .iter()
            .enumerate()
            .map(|(index, enrichment)| (NodeId::new(index as u32), enrichment))
    }

    /// How many nodes are covered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether no node is covered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// How many structures were placed across the whole tree.
    ///
    /// What `xtask budget` will report, and what a tuner watches while moving the thresholds:
    /// a table that placed one structure on every node has stopped saying anything.
    #[must_use]
    pub fn placement_count(&self) -> usize {
        self.nodes.iter().map(Enrichment::len).sum()
    }

    /// Whether this map covers exactly the nodes of a skeleton.
    #[must_use]
    pub fn covers(&self, skeleton: &crate::Skeleton) -> bool {
        self.nodes.len() == skeleton.nodes().len()
    }

    /// The whole map as one number — `AC-DET-1`'s "enrichment placements".
    ///
    /// Counts precede their contents and discriminants precede their payloads, so a node that
    /// gained a structure cannot encode as one whose structure merely moved — the rule
    /// [`Skeleton::digest`](crate::Skeleton::digest) and
    /// [`MaterialMap::digest`](crate::material::MaterialMap::digest) both follow.
    #[must_use]
    pub fn digest(&self) -> Digest {
        let mut hasher = Sha256::new();
        hasher.update(ENRICHMENT_DIGEST_TAG);
        hasher.update(&(self.nodes.len() as u64).to_le_bytes());

        for enrichment in &self.nodes {
            hasher.update(&(enrichment.len() as u64).to_le_bytes());
            for placement in enrichment.placements() {
                hasher.update(&[
                    placement.kind.position() as u8,
                    placement.form.position() as u8,
                ]);
                hasher.update(&placement.position.to_bits().to_le_bytes());
                hasher.update(&placement.weight.to_bits().to_le_bytes());
                hasher.update(&placement.sources.to_le_bytes());
            }
        }

        hasher.finalize()
    }
}

/// Namespaces [`EnrichmentMap::digest`], and dates its encoding.
///
/// Bumped whenever the encoding changes, so a digest from an older build cannot be mistaken for
/// a disagreement about enrichment. Same discipline as `treepo-skeleton-v2` and
/// `treepo-material-v3`.
const ENRICHMENT_DIGEST_TAG: &[u8] = b"treepo-enrichment-v1";

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn at(kind: EnrichmentKind, position: i64, weight: i64) -> Placement {
        Placement::single(
            kind,
            Fx::from_ratio(position, 100),
            Fx::from_ratio(weight, 100),
        )
    }

    #[test]
    fn every_kind_has_a_slot_and_a_name() {
        for (index, kind) in EnrichmentKind::ALL.into_iter().enumerate() {
            assert_eq!(kind.position(), index, "{kind:?}");
            assert!(!kind.name().is_empty());
        }
        let mut names: alloc::vec::Vec<&str> =
            EnrichmentKind::ALL.iter().map(|k| k.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(
            names.len(),
            EnrichmentKind::ALL.len(),
            "two kinds share a name"
        );
    }

    /// The ladder is ordered, and the order is what stops compounding from demoting anything.
    #[test]
    fn forms_climb_and_never_step_back_down() {
        for (index, form) in EnrichmentForm::ALL.into_iter().enumerate() {
            assert_eq!(form.position(), index, "{form:?}");
        }
        assert!(EnrichmentForm::Single < EnrichmentForm::Platform);
        assert_eq!(
            EnrichmentForm::Run.grown(EnrichmentForm::Platform),
            EnrichmentForm::Platform
        );
        assert_eq!(
            EnrichmentForm::Platform.grown(EnrichmentForm::Run),
            EnrichmentForm::Platform,
            "growing into something smaller must not shrink it"
        );
        assert_eq!(
            EnrichmentForm::Single.grown(EnrichmentForm::Single),
            EnrichmentForm::Single
        );
    }

    /// Eighths and sixteenths for anything an assertion pins exactly.
    ///
    /// `Fx` is Q32.32, so a tenth and a twentieth are not exact and a test comparing
    /// `0.05 + 0.05` against `0.10` is pinning a rounding artefact rather than the claim it
    /// means to make. Binary fractions are exact through addition, subtraction and the `lerp`
    /// that [`Placement::fuse`] centres on, so the numbers below say what they look like.
    fn exact(kind: EnrichmentKind, position: (i64, i64), weight: (i64, i64)) -> Placement {
        Placement::single(
            kind,
            Fx::from_ratio(position.0, position.1),
            Fx::from_ratio(weight.0, weight.1),
        )
    }

    /// The rule the whole feature turns on: two things at one place are one larger thing, and
    /// nothing is lost in making that true.
    #[test]
    fn two_placements_grow_into_one_larger_one() {
        let first = exact(EnrichmentKind::Bookshelf, (3, 8), (1, 8));
        let second = exact(EnrichmentKind::Bookshelf, (5, 8), (1, 8));

        let grown = first.fuse(second);
        assert_eq!(grown.kind, EnrichmentKind::Bookshelf);
        assert_eq!(grown.sources, 2, "both are still counted");
        assert_eq!(grown.weight, Fx::from_ratio(1, 4), "and both still weigh");
        assert_eq!(grown.position, Fx::HALF, "equal masses meet in the middle");
    }

    /// Mass-weighted, so a large structure absorbing a small one barely moves — which is what
    /// makes a densifying cluster look like it grew rather than like it slid.
    #[test]
    fn a_large_structure_absorbing_a_small_one_barely_moves() {
        let large = exact(EnrichmentKind::Stockpile, (1, 4), (3, 4));
        let small = exact(EnrichmentKind::Stockpile, (1, 1), (1, 4));

        let grown = large.fuse(small);
        assert_eq!(grown.weight, Fx::ONE);
        // A quarter of the way from 1/4 to 1: 1/4 + 3/4 × 1/4 = 7/16.
        assert_eq!(grown.position, Fx::from_ratio(7, 16));
        assert!(
            grown.distance_to(&large) < grown.distance_to(&small),
            "the mass, not the midpoint, is what it centres on"
        );
    }

    /// A chain accretes rather than one step jumping to a mean of everything — and it stays in
    /// the span it started in, which is what keeps a fused run on the limb it grew on.
    #[test]
    fn a_chain_of_placements_accretes_within_its_own_span() {
        let grown = exact(EnrichmentKind::WorkSite, (1, 16), (1, 16))
            .fuse(exact(EnrichmentKind::WorkSite, (2, 16), (1, 16)))
            .fuse(exact(EnrichmentKind::WorkSite, (3, 16), (1, 16)))
            .fuse(exact(EnrichmentKind::WorkSite, (4, 16), (1, 16)));

        assert_eq!(grown.sources, 4);
        assert_eq!(grown.weight, Fx::from_ratio(1, 4));
        assert!(grown.position >= Fx::from_ratio(1, 16));
        assert!(grown.position <= Fx::from_ratio(4, 16));
    }

    #[test]
    fn an_enrichment_reports_what_it_carries() {
        let enrichment = Enrichment::new(vec![
            at(EnrichmentKind::Stockpile, 15, 30),
            at(EnrichmentKind::Bookshelf, 50, 20),
            at(EnrichmentKind::Bookshelf, 70, 10),
        ]);

        assert_eq!(enrichment.len(), 3);
        assert_eq!(enrichment.sources(), 3);
        assert!(enrichment.carries(EnrichmentKind::Bookshelf));
        assert!(!enrichment.carries(EnrichmentKind::ProvingGround));
        assert_eq!(enrichment.of_kind(EnrichmentKind::Bookshelf).count(), 2);

        assert!(Enrichment::none().is_empty());
        assert_eq!(Enrichment::default().sources(), 0);
    }

    /// A fused structure still reports what grew into it, which is what `P6`'s bound is
    /// actually about — four structures may stand for forty files.
    #[test]
    fn a_fused_structure_still_counts_what_it_stands_for() {
        let dense =
            at(EnrichmentKind::Bookshelf, 40, 10).fuse(at(EnrichmentKind::Bookshelf, 45, 10));
        let enrichment = Enrichment::new(vec![dense]);
        assert_eq!(enrichment.len(), 1, "one thing is drawn");
        assert_eq!(enrichment.sources(), 2, "and it stands for two");
    }

    fn sample() -> EnrichmentMap {
        let mut map = EnrichmentMap::new();
        map.push(Enrichment::none());
        map.push(Enrichment::new(vec![at(EnrichmentKind::Bookshelf, 50, 20)]));
        map.push(Enrichment::new(vec![
            at(EnrichmentKind::Stockpile, 10, 40),
            at(EnrichmentKind::WorkSite, 90, 15),
        ]));
        map
    }

    #[test]
    fn ids_index_the_order_enrichment_was_pushed_in() {
        let map = sample();
        assert_eq!(map.len(), 3);
        assert_eq!(map.placement_count(), 3);
        assert!(map.get(NodeId::new(0)).unwrap().is_empty());
        assert!(
            map.get(NodeId::new(2))
                .unwrap()
                .carries(EnrichmentKind::WorkSite)
        );
        assert!(map.get(NodeId::new(3)).is_none());

        let ids: alloc::vec::Vec<NodeId> = map.iter().map(|(id, _)| id).collect();
        assert_eq!(ids, [NodeId::new(0), NodeId::new(1), NodeId::new(2)]);
    }

    #[test]
    fn the_same_placements_hash_the_same_way_every_time() {
        assert_eq!(sample().digest(), sample().digest());
    }

    /// Every way a placement can differ, each on its own. A digest blind to any of them would
    /// call two different trees identical (`AC-DET-1`).
    #[test]
    fn every_part_of_a_placement_reaches_the_digest() {
        let baseline = sample().digest();

        let mut rebuilt = sample();
        rebuilt.nodes[1].placements[0].kind = EnrichmentKind::ProvingGround;
        assert_ne!(rebuilt.digest(), baseline, "kind");

        let mut grown = sample();
        grown.nodes[1].placements[0].form = EnrichmentForm::Platform;
        assert_ne!(grown.digest(), baseline, "form");

        let mut moved = sample();
        moved.nodes[1].placements[0].position = Fx::from_ratio(51, 100);
        assert_ne!(moved.digest(), baseline, "position");

        let mut heavier = sample();
        heavier.nodes[1].placements[0].weight = Fx::from_ratio(21, 100);
        assert_ne!(heavier.digest(), baseline, "weight");

        // The one that would be easiest to leave out, and the one that says a Cluster of two
        // is not a Cluster of twenty.
        let mut denser = sample();
        denser.nodes[1].placements[0].sources = 9;
        assert_ne!(denser.digest(), baseline, "sources");

        let mut bare = sample();
        bare.nodes[2] = Enrichment::none();
        assert_ne!(bare.digest(), baseline, "nothing built at all");
    }

    /// The count-first rule, on the case it exists for: one node with two structures must not
    /// encode like two nodes with one each.
    #[test]
    fn a_moved_structure_does_not_collide_with_a_gained_one() {
        let mut together = EnrichmentMap::new();
        together.push(Enrichment::new(vec![
            at(EnrichmentKind::Bookshelf, 30, 10),
            at(EnrichmentKind::Bookshelf, 60, 10),
        ]));
        together.push(Enrichment::none());

        let mut apart = EnrichmentMap::new();
        apart.push(Enrichment::new(vec![at(EnrichmentKind::Bookshelf, 30, 10)]));
        apart.push(Enrichment::new(vec![at(EnrichmentKind::Bookshelf, 60, 10)]));

        assert_ne!(together.digest(), apart.digest());
    }

    #[test]
    fn an_empty_map_still_hashes_and_covers_an_empty_skeleton() {
        let empty = EnrichmentMap::new();
        assert!(empty.is_empty());
        assert_eq!(empty.placement_count(), 0);
        assert_eq!(empty.digest(), EnrichmentMap::new().digest());
        assert!(empty.covers(&crate::Skeleton::new()));
        assert!(!sample().covers(&crate::Skeleton::new()));
    }

    /// A tree with nothing built on it is an ordinary tree, and must not hash like a tree that
    /// was never enriched at all.
    #[test]
    fn a_bare_tree_is_not_an_absent_one() {
        let mut bare = EnrichmentMap::new();
        bare.push(Enrichment::none());
        assert_ne!(bare.digest(), EnrichmentMap::new().digest());
    }
}
