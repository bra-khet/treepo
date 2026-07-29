//! What a limb is made of — `F-MAT-1`, `F-MAT-3`, `design/feature-system.md` §8.4–§8.5.
//!
//! These types live here for the reason [`segment`](crate::segment)'s do: they are a
//! *handoff*. `treepo-gen` decides them, `treepo-grow` interpolates between two of them
//! during a transition, and `treepo-render` binds them to a texture and a palette. A type
//! three crates exchange belongs in the crate they all already depend on.
//!
//! # This module arrives incomplete, on purpose
//!
//! The crate header said material types would "arrive with the phases that produce them",
//! and Phase 4 produces them in slices. What is here is what the first slice decides:
//! [`MaterialFamily`], the primary material of `F-MAT-1`; [`Composition`], what the rest of a
//! node is made of or holds; and [`Material::budget`], the normalized representation of
//! `F-MAT-3`.
//!
//! What is deliberately *not* here yet is the ownership mosaic (`F-MAT-2`), the age/recency
//! gradient (`F-MAT-4`), and the stress signals (`F-MAT-6`). Each is a field on [`Material`]
//! when the slice that computes it lands. Declaring them now would be the guess the crate
//! header warned about — a field nothing writes is a field a renderer will read anyway.
//!
//! # Family is not category
//!
//! [`ContentCategory`](crate::primitives::size::ContentCategory) is a measurement: what kind
//! of file this is. [`MaterialFamily`] is a reading: what the thing should look like. They
//! are close enough that collapsing them is tempting and different enough that it would be
//! wrong — `Asset` and `Binary` are two measurements and one material, because
//! `design/feature-system.md` §8.5 treats "binary / asset-heavy regions" as a single visual
//! idea, and a later slice may split `Generated` from vendored content without any new
//! measurement existing.
//!
//! # Made of, against holds
//!
//! A node with mixed content can mean two different things, and [`Composition`] is the
//! distinction. A limb whose bytes are part code and part image *is* a mixture — one material
//! veined with another, which the renderer interpolates. An
//! [`AggregateNode`](crate::AggregateNode) standing for a directory it does not draw *holds*
//! materials without being made of them; the variety inside it is inventory, not surface.
//!
//! This is the same line [`NodeRole`](crate::segment::NodeRole) already draws between
//! [`Group`](crate::segment::NodeRole::Group) — several paths, each still drawn — and
//! [`Aggregate`](crate::segment::NodeRole::Aggregate) — several paths, and this node *is*
//! their representation. That table exists because collapsing the two would make `F2`'s
//! "fewer, thicker limbs" indistinguishable from `F-SKEL-7`'s "this directory and all its
//! contents"; the same collapse here would make a limb of mixed content indistinguishable
//! from a container of assorted content, which are different pictures of different facts.

use crate::segment::NodeId;
use alloc::vec::Vec;
use treepo_det::{Digest, Fx, Sha256};

/// The primary material of one limb — `F-MAT-1`.
///
/// > Primary material family is driven by language, binary-vs-text, and asset class. Binary
/// > and asset-heavy regions render as resource-like material rather than living wood.
///
/// Six families, drawn from `design/feature-system.md` §8.5's "wood-like, crystalline,
/// metallic, leafy, dusty". Every one of them is a material a tree or the ground under it
/// could plausibly be made of, which is the constraint that keeps the set from becoming a
/// legend the user has to memorize: a limb of [`Ore`](Self::Ore) reads as *heavy* before it
/// reads as *binary*, and that is the right order for a thing you look at before you
/// interrogate it.
///
/// The mapping from a measured category is [`MaterialFamily::of_category`]; how a *mixture*
/// of categories resolves to one family is a tuning decision and lives in
/// `assets/params/materials.ron`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MaterialFamily {
    /// Living wood. Source in a recognized language — the tree itself.
    ///
    /// The default reading, and the one every other family is a departure from. A repository
    /// that is mostly code is mostly tree, which is the metaphor working rather than an
    /// accident of the mapping.
    Heartwood,
    /// Dense, resource-like matter. Assets and binaries.
    ///
    /// `F-MAT-1`'s named requirement: "binary and asset-heavy regions render as
    /// resource-like material rather than living wood". §8.5 offers bullion, ore, stacked
    /// crates and raw stockpiles; this is the material, and `F-MAT-5`'s enrichment is where
    /// the crates themselves get placed.
    Ore,
    /// Uniform, tooled, machine-cut. Generated output and vendored content.
    ///
    /// §8.5: "a slightly different, more uniform or 'machined' material treatment". The
    /// visual claim is *regularity* — generated code has no hand in it, and a surface with
    /// no grain says so without a label.
    Machined,
    /// Fibrous, pale, layered. Documentation and prose.
    ///
    /// Distinct from the docs bookshelves of §8.7, which are enrichment placed *on* a limb.
    /// This is what the limb is made of when it carries documentation and nothing has been
    /// placed on it yet.
    Parchment,
    /// Hardened sap. Manifests, lockfiles, CI definitions, dotfiles.
    ///
    /// Config is the joinery of a project — it holds the parts in relation to one another
    /// without being one of them. Resin is the tree-native material that does the same job,
    /// which keeps a `Cargo.toml` from having to read as either wood or metal.
    Resin,
    /// Inert, uncarved. Content treepo could not identify.
    ///
    /// `assets/languages/languages.ron` is explicit that an unrecognized extension yields
    /// `Unknown` rather than a guess, and that a repository full of it should render as a
    /// repository full of files treepo could not name — "honest, and visibly fixable by
    /// adding an entry here". A distinct family is what makes that visible. Reading as
    /// *un-grown* rather than as *dead* is the distinction that matters: `N4`'s refusal to
    /// judge people extends to not editorializing about their files.
    Stone,
}

impl MaterialFamily {
    /// Every family, in declaration order.
    pub const ALL: [Self; 6] = [
        Self::Heartwood,
        Self::Ore,
        Self::Machined,
        Self::Parchment,
        Self::Resin,
        Self::Stone,
    ];

    /// The family one measured category reads as.
    ///
    /// The `Asset`/`Binary` pair is the only place this is not one-to-one, and it is not an
    /// oversight — see the module header.
    ///
    /// This is code rather than a row in `materials.ron` for the reason
    /// [`SkeletonInputs`](../../treepo_gen/params/struct.SkeletonInputs.html) is: it states
    /// what a category *means*, and a meaning does not become a different sentence because a
    /// limb looked wrong. What can be tuned is how much of a category it takes to claim a
    /// mixed directory, and that is in the file.
    #[must_use]
    pub const fn of_category(category: crate::primitives::size::ContentCategory) -> Self {
        use crate::primitives::size::ContentCategory as C;
        match category {
            C::Code => Self::Heartwood,
            C::Asset | C::Binary => Self::Ore,
            C::Config => Self::Resin,
            C::Docs => Self::Parchment,
            C::Generated => Self::Machined,
            C::Unknown => Self::Stone,
        }
    }

    /// This family's index in [`ALL`](Self::ALL) — how a [`FamilyMix`] is addressed.
    ///
    /// A `match` rather than a search, so it is a constant-time lookup and so that adding a
    /// family without giving it a slot does not compile.
    #[must_use]
    pub const fn position(self) -> usize {
        match self {
            Self::Heartwood => 0,
            Self::Ore => 1,
            Self::Machined => 2,
            Self::Parchment => 3,
            Self::Resin => 4,
            Self::Stone => 5,
        }
    }

    /// Whether this family is living tree rather than matter the tree carries.
    ///
    /// The distinction `F-MAT-1` draws in its second sentence, and the one Thrive will want:
    /// §8.8 gives sway, breathing and glow to the living material, and a stockpile of ore
    /// that swayed would read as an error.
    #[must_use]
    pub const fn is_living(self) -> bool {
        matches!(self, Self::Heartwood | Self::Parchment)
    }
}

/// How much of a node each family accounts for, in `0..=1`.
///
/// Indexed by position in [`MaterialFamily::ALL`], so it is `Copy`, allocation-free, and the
/// same size for every node. At T3 there are eighty thousand of these; a map or a `Vec` each
/// would be eighty thousand allocations to describe at most six numbers.
///
/// The shares are proportions of the node's bytes and sum to approximately one — approximately
/// because each is rounded independently, and reconciling them would be arithmetic nobody
/// looks at in service of a total nobody reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FamilyMix([Fx; MaterialFamily::ALL.len()]);

impl FamilyMix {
    /// A mix from per-family shares, in [`MaterialFamily::ALL`] order.
    #[must_use]
    pub const fn new(shares: [Fx; MaterialFamily::ALL.len()]) -> Self {
        Self(shares)
    }

    /// One family's share of this node.
    #[must_use]
    pub fn share_of(&self, family: MaterialFamily) -> Fx {
        self.0[family.position()]
    }

    /// Every family present, with its share, in [`MaterialFamily::ALL`] order.
    ///
    /// Families accounting for nothing are skipped: an inventory that lists what is *not*
    /// inside is a longer answer to a question nobody asked.
    pub fn present(&self) -> impl Iterator<Item = (MaterialFamily, Fx)> + '_ {
        MaterialFamily::ALL
            .into_iter()
            .zip(self.0)
            .filter(|(_, share)| !share.is_zero())
    }

    /// How many distinct families this node accounts for.
    #[must_use]
    pub fn count(&self) -> usize {
        self.present().count()
    }
}

/// What a node is made of beyond its primary family, and how to read it.
///
/// The made-of / holds distinction the module header describes. Which arm applies is decided
/// by the node's [`NodeRole`](crate::segment::NodeRole), not by the content — a container of
/// pure documentation is still holding rather than being.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Composition {
    /// One material throughout.
    ///
    /// A file, or a directory whose content is uniform enough that a second material would be
    /// a stripe nobody could see.
    Pure,
    /// Made of a second material as well — the renderer interpolates between the two.
    ///
    /// `design/feature-system.md` §8.5's picture: "a limb whose primary material is
    /// 'TypeScript wood' can still carry author-coloured veins, scars, or surface markings".
    /// Here the vein is a second *material* rather than a contributor, which is the same
    /// mechanism serving `F-MAT-1` instead of `F-MAT-2`.
    ///
    /// # Only two, and the third is not lost
    ///
    /// A node holding three or more families keeps its largest two. Three interleaved
    /// materials on one limb read as mud rather than as information, and the picture the
    /// design asks for is "a limb of X veined with Y". Nothing is destroyed by the omission:
    /// the full category breakdown stays in the
    /// [`SizePrimitives`](crate::primitives::SizePrimitives) this was derived from, which is
    /// what `F-INSP-5`'s why-panel reads.
    Blended {
        /// The second-largest family.
        secondary: MaterialFamily,
        /// Its share of the node, in `0..=1`.
        weight: Fx,
    },
    /// Holds materials it is not made of — `F-SKEL-7`'s container.
    ///
    /// An [`AggregateNode`](crate::AggregateNode) stands for content it does not draw, so the
    /// variety inside it is an inventory rather than a surface. `F-INSP-3` requires the
    /// container to "report what it represents", and `F-MAT-5` places enrichment from it — a
    /// container holding mostly [`Parchment`](MaterialFamily::Parchment) becomes a
    /// bookshelf, one holding mostly [`Ore`](MaterialFamily::Ore) becomes a stockpile.
    ///
    /// The full mix rather than the top two, because an inventory that dropped its tail would
    /// be answering `F-INSP-3` with a summary of itself.
    Subordinate(FamilyMix),
}

impl Composition {
    /// The second material this node is made of, if it is made of two.
    ///
    /// `None` for a container: a container is not made of what it holds, which is the whole
    /// distinction. A caller wanting the contents should match on
    /// [`Subordinate`](Self::Subordinate) and mean it.
    #[must_use]
    pub const fn secondary(&self) -> Option<MaterialFamily> {
        match self {
            Self::Blended { secondary, .. } => Some(*secondary),
            Self::Pure | Self::Subordinate(_) => None,
        }
    }

    /// What this node holds without being, if it holds anything.
    #[must_use]
    pub const fn contents(&self) -> Option<&FamilyMix> {
        match self {
            Self::Subordinate(mix) => Some(mix),
            Self::Pure | Self::Blended { .. } => None,
        }
    }
}

/// What one skeleton node is made of.
///
/// Keyed to a [`NodeId`](crate::segment::NodeId) by [`MaterialMap`]. One node, one material:
/// the mosaic that lets several contributors share a limb is an accent *over* this, not a
/// replacement for it (`F-MAT-2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Material {
    /// The primary material — `F-MAT-1`.
    pub family: MaterialFamily,
    /// What the rest of it is, and whether the node is made of that or holds it.
    pub composition: Composition,
    /// This node's share of the visual budget, in `0..=1` — `F-MAT-3`.
    ///
    /// Logarithmic, soft-clamped, and floored, so that the 3-line file of `AC-MAT-1` keeps a
    /// budget the 50k-line file cannot take from it. It is a *proportion of what a node may
    /// occupy*, not a pixel count: pixels depend on zoom, and `F-MAT-3`'s guarantee has to
    /// survive the user moving the camera.
    ///
    /// Zero is not a legal value. A path with a budget of zero is a path with no pixels,
    /// which is `P7` broken — [`normalize`](../../treepo_gen/normalize/index.html) applies
    /// the floor precisely so that nothing here can carry one.
    pub budget: Fx,
}

/// Every node's material, indexed by [`NodeId`](crate::segment::NodeId).
///
/// A `Vec` rather than a map, for the reason [`Skeleton`](crate::Skeleton) stores its nodes in
/// one: node ids *are* dense indices into creation order, so a map would pay a comparison per
/// lookup to reproduce an offset, and the architecture's `WorldSnapshot` holds this alongside
/// the segments it parallels.
///
/// Built by `treepo-gen::material::materialize` and consumed by `treepo-grow` (which diffs
/// two) and `treepo-render` (which binds them). It is the material half of what
/// [`Skeleton`](crate::Skeleton) is the geometric half of.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MaterialMap {
    materials: Vec<Material>,
}

impl MaterialMap {
    /// An empty map.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            materials: Vec::new(),
        }
    }

    /// Appends the material for the next node, returning the id it was given.
    ///
    /// The only way to add one, so an id cannot be made to disagree with the position it
    /// indexes — the same guarantee [`Skeleton::push_node`](crate::Skeleton::push_node) gives.
    /// A caller walking a skeleton in node order therefore gets a map whose ids match by
    /// construction rather than by care.
    pub fn push(&mut self, material: Material) -> NodeId {
        let id = NodeId::new(u32::try_from(self.materials.len()).unwrap_or(u32::MAX));
        self.materials.push(material);
        id
    }

    /// One node's material.
    #[must_use]
    pub fn get(&self, id: NodeId) -> Option<&Material> {
        self.materials.get(id.index())
    }

    /// Every material, in node order.
    #[must_use]
    pub fn materials(&self) -> &[Material] {
        &self.materials
    }

    /// Every material with the node it belongs to.
    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &Material)> {
        self.materials
            .iter()
            .enumerate()
            .map(|(index, material)| (NodeId::new(index as u32), material))
    }

    /// How many nodes have a material.
    #[must_use]
    pub fn len(&self) -> usize {
        self.materials.len()
    }

    /// Whether nothing has a material.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.materials.is_empty()
    }

    /// Whether this map covers exactly the nodes of a skeleton.
    ///
    /// The invariant the pairing rests on, and cheap enough to assert at a phase boundary. A
    /// map one entry short does not fail loudly on its own — it fails as a node rendering
    /// with whatever the renderer does for `None`, several crates away from the walk that
    /// dropped it.
    #[must_use]
    pub fn covers(&self, skeleton: &crate::Skeleton) -> bool {
        self.materials.len() == skeleton.nodes().len()
    }

    /// The whole map as one number — `AC-DET-1`'s "byte-identical serialized … materials".
    ///
    /// > **AC-DET-1** — Two Grow runs on identical repository state produce byte-identical
    /// > serialized skeletons, **materials**, and enrichment placements.
    ///
    /// The criterion names materials beside skeletons, so this is
    /// [`Skeleton::digest`](crate::Skeleton::digest)'s counterpart and it lives here for the
    /// same stated reason: there must be *one* encoding. `xtask determinism` gates on it and
    /// `xtask budget` will report it, and two copies of a hash are two chances for the gate
    /// and the report to disagree about what changed.
    ///
    /// Discriminants precede their payloads and the count precedes everything, so a limb that
    /// became a container cannot encode to the same bytes as one that did not — the same rule
    /// [`Skeleton::digest`] follows, and for the same reason.
    #[must_use]
    pub fn digest(&self) -> Digest {
        let mut hasher = Sha256::new();
        hasher.update(MATERIAL_DIGEST_TAG);
        hasher.update(&(self.materials.len() as u64).to_le_bytes());

        for material in &self.materials {
            hasher.update(&[material.family.position() as u8]);
            hasher.update(&material.budget.to_bits().to_le_bytes());
            match &material.composition {
                Composition::Pure => hasher.update(&[0]),
                Composition::Blended { secondary, weight } => {
                    hasher.update(&[1]);
                    hasher.update(&[secondary.position() as u8]);
                    hasher.update(&weight.to_bits().to_le_bytes());
                }
                Composition::Subordinate(mix) => {
                    hasher.update(&[2]);
                    // Every slot, present or not: a fixed-width encoding needs no count, and
                    // the absent families are as much a statement about a container as the
                    // present ones.
                    for family in MaterialFamily::ALL {
                        hasher.update(&mix.share_of(family).to_bits().to_le_bytes());
                    }
                }
            }
        }

        hasher.finalize()
    }
}

/// Namespaces [`MaterialMap::digest`], and dates its encoding.
///
/// Bumped whenever the encoding changes, so a digest from an older build cannot be mistaken
/// for a disagreement about materials. Same discipline as `treepo-skeleton-v2`.
const MATERIAL_DIGEST_TAG: &[u8] = b"treepo-material-v1";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::size::ContentCategory;

    /// Every category must map somewhere, or a repository containing one renders as nothing.
    /// The `match` is exhaustive so the compiler holds this — the test holds the *shape*:
    /// six families out of seven categories, with exactly one pair collapsed.
    #[test]
    fn every_category_has_a_family_and_only_one_pair_shares_one() {
        let families: alloc::vec::Vec<MaterialFamily> = ContentCategory::ALL
            .iter()
            .map(|&c| MaterialFamily::of_category(c))
            .collect();

        assert_eq!(families.len(), ContentCategory::ALL.len());

        let mut distinct = families.clone();
        distinct.sort();
        distinct.dedup();
        assert_eq!(distinct.len(), MaterialFamily::ALL.len());
        assert_eq!(
            MaterialFamily::of_category(ContentCategory::Asset),
            MaterialFamily::of_category(ContentCategory::Binary),
            "the one collapsed pair — F-MAT-1 treats binary and asset-heavy as one reading"
        );
    }

    /// `ALL` is what a renderer iterates to build its texture set, so an added family that
    /// never reached it would be a family with no appearance.
    #[test]
    fn all_lists_every_family_once() {
        let mut sorted = MaterialFamily::ALL;
        sorted.sort();
        let mut deduped = alloc::vec::Vec::from(sorted);
        deduped.dedup();
        assert_eq!(deduped.len(), MaterialFamily::ALL.len());
    }

    #[test]
    fn only_the_grown_materials_are_living() {
        assert!(MaterialFamily::Heartwood.is_living());
        assert!(!MaterialFamily::Ore.is_living());
        assert!(!MaterialFamily::Stone.is_living());
    }

    /// `position` indexes `ALL`, and a mismatch would silently address the wrong family's
    /// share — a container reporting ore when it holds parchment.
    #[test]
    fn position_indexes_all() {
        for (index, family) in MaterialFamily::ALL.into_iter().enumerate() {
            assert_eq!(family.position(), index, "{family:?}");
        }
    }

    #[test]
    fn a_mix_reports_only_what_is_present() {
        let mut shares = [Fx::ZERO; MaterialFamily::ALL.len()];
        shares[MaterialFamily::Parchment.position()] = Fx::from_ratio(3, 4);
        shares[MaterialFamily::Ore.position()] = Fx::from_ratio(1, 4);
        let mix = FamilyMix::new(shares);

        assert_eq!(mix.count(), 2);
        assert_eq!(
            mix.share_of(MaterialFamily::Parchment),
            Fx::from_ratio(3, 4)
        );
        assert_eq!(mix.share_of(MaterialFamily::Heartwood), Fx::ZERO);

        // ALL order, so iterating it cannot be read as a ranking of anything either.
        let present: alloc::vec::Vec<MaterialFamily> = mix.present().map(|(f, _)| f).collect();
        assert_eq!(present, [MaterialFamily::Ore, MaterialFamily::Parchment]);
    }

    /// The distinction the whole enum exists for: a container is not made of what it holds.
    #[test]
    fn made_of_and_holds_are_not_the_same_question() {
        let blended = Composition::Blended {
            secondary: MaterialFamily::Ore,
            weight: Fx::from_ratio(1, 3),
        };
        assert_eq!(blended.secondary(), Some(MaterialFamily::Ore));
        assert!(blended.contents().is_none());

        let holding = Composition::Subordinate(FamilyMix::default());
        assert_eq!(
            holding.secondary(),
            None,
            "a container is not made of its contents"
        );
        assert!(holding.contents().is_some());

        assert_eq!(Composition::Pure.secondary(), None);
        assert!(Composition::Pure.contents().is_none());
    }

    fn material(family: MaterialFamily, composition: Composition) -> Material {
        Material {
            family,
            composition,
            budget: Fx::from_ratio(1, 3),
        }
    }

    /// A map with one of each composition, so the digest tests have every arm in them.
    fn sample() -> MaterialMap {
        let mut map = MaterialMap::new();
        map.push(material(MaterialFamily::Heartwood, Composition::Pure));
        map.push(material(
            MaterialFamily::Heartwood,
            Composition::Blended {
                secondary: MaterialFamily::Ore,
                weight: Fx::from_ratio(1, 4),
            },
        ));
        let mut shares = [Fx::ZERO; MaterialFamily::ALL.len()];
        shares[MaterialFamily::Parchment.position()] = Fx::ONE;
        map.push(material(
            MaterialFamily::Parchment,
            Composition::Subordinate(FamilyMix::new(shares)),
        ));
        map
    }

    #[test]
    fn ids_index_the_order_materials_were_pushed_in() {
        let map = sample();
        assert_eq!(map.len(), 3);
        assert_eq!(
            map.get(NodeId::new(1)).unwrap().composition.secondary(),
            Some(MaterialFamily::Ore)
        );
        assert!(map.get(NodeId::new(3)).is_none());

        let ids: alloc::vec::Vec<NodeId> = map.iter().map(|(id, _)| id).collect();
        assert_eq!(ids, [NodeId::new(0), NodeId::new(1), NodeId::new(2)]);
    }

    #[test]
    fn the_same_materials_hash_the_same_way_every_time() {
        assert_eq!(sample().digest(), sample().digest());
    }

    /// The three things a material can differ by, each on its own. A digest blind to any of
    /// them would call two different trees identical (`AC-DET-1`).
    #[test]
    fn every_part_of_a_material_reaches_the_digest() {
        let baseline = sample().digest();

        let mut recoloured = sample();
        recoloured.materials[0].family = MaterialFamily::Stone;
        assert_ne!(recoloured.digest(), baseline, "family");

        let mut resized = sample();
        resized.materials[0].budget = Fx::HALF;
        assert_ne!(resized.digest(), baseline, "budget");

        let mut reveined = sample();
        reveined.materials[1].composition = Composition::Blended {
            secondary: MaterialFamily::Stone,
            weight: Fx::from_ratio(1, 4),
        };
        assert_ne!(reveined.digest(), baseline, "secondary family");

        let mut reweighted = sample();
        reweighted.materials[1].composition = Composition::Blended {
            secondary: MaterialFamily::Ore,
            weight: Fx::from_ratio(1, 5),
        };
        assert_ne!(reweighted.digest(), baseline, "blend weight");
    }

    /// The discriminant-first rule, on the case it exists for: a node that was made of one
    /// material and now holds it is a different tree, however similar the numbers look.
    #[test]
    fn being_and_holding_do_not_collide_in_the_digest() {
        let mut shares = [Fx::ZERO; MaterialFamily::ALL.len()];
        shares[MaterialFamily::Heartwood.position()] = Fx::ONE;

        let mut made_of = MaterialMap::new();
        made_of.push(material(MaterialFamily::Heartwood, Composition::Pure));

        let mut holds = MaterialMap::new();
        holds.push(material(
            MaterialFamily::Heartwood,
            Composition::Subordinate(FamilyMix::new(shares)),
        ));

        assert_ne!(made_of.digest(), holds.digest());
    }

    /// A container that absorbed something else stands for something else, and the tail is
    /// exactly what `Subordinate` keeps that `Blended` does not.
    #[test]
    fn a_containers_whole_inventory_reaches_the_digest() {
        let baseline = sample().digest();
        let mut restocked = sample();
        let Composition::Subordinate(mix) = &mut restocked.materials[2].composition else {
            panic!("the sample's third material is the container");
        };
        let mut shares = [Fx::ZERO; MaterialFamily::ALL.len()];
        shares[MaterialFamily::Parchment.position()] = Fx::from_ratio(9, 10);
        // A tenth of ore that a top-two reading would have dropped entirely.
        shares[MaterialFamily::Ore.position()] = Fx::from_ratio(1, 10);
        *mix = FamilyMix::new(shares);

        assert_ne!(restocked.digest(), baseline);
    }

    #[test]
    fn an_empty_map_still_hashes_and_covers_an_empty_skeleton() {
        let empty = MaterialMap::new();
        assert!(empty.is_empty());
        assert_eq!(empty.digest(), MaterialMap::new().digest());
        assert!(empty.covers(&crate::Skeleton::new()));
        assert!(!sample().covers(&crate::Skeleton::new()));
    }
}
