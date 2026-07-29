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

use treepo_det::Fx;

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
/// Keyed to a [`NodeId`](crate::segment::NodeId) by whatever holds the collection — the
/// architecture's `MaterialMap`, which arrives with the phase that has two of these to
/// compare. One node, one material: the mosaic that lets several contributors share a limb
/// is an accent *over* this, not a replacement for it (`F-MAT-2`).
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
}
