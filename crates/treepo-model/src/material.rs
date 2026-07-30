//! What a limb is made of and who is drawn on it — `F-MAT-1`, `F-MAT-2`, `F-MAT-3`,
//! `design/feature-system.md` §8.4–§8.5.
//!
//! These types live here for the reason [`segment`](crate::segment)'s do: they are a
//! *handoff*. `treepo-gen` decides them, `treepo-grow` interpolates between two of them
//! during a transition, and `treepo-render` binds them to a texture and a palette. A type
//! three crates exchange belongs in the crate they all already depend on.
//!
//! # The slices this module arrived in
//!
//! The crate header said material types would "arrive with the phases that produce them",
//! and Phase 4 produced them in slices: [`MaterialFamily`], the primary material of `F-MAT-1`;
//! [`Composition`], what the rest of a node is made of or holds; [`Material::budget`], the
//! normalized representation of `F-MAT-3`; [`Mosaic`], the ownership partition of `F-MAT-2`;
//! [`AgeGradient`], where along a node its material sits (`F-MAT-4`).
//!
//! [`Stress`] is the last of them — `F-MAT-6`'s surface treatment, which an earlier revision of
//! this header deferred on the grounds that "a field nothing writes is a field a renderer will
//! read anyway". Something writes it now.
//!
//! # Made of, against owned by
//!
//! [`Composition`] and [`Mosaic`] are separate fields rather than two arms of one enum,
//! because a limb that is *made of* heartwood and *owned by* three people is two facts about
//! it and not one. `F-MAT-2` says so in its own wording — ownership is "accent, vein, and
//! mosaic treatment **over** the primary material" — and a type that made them alternatives
//! would have no way to say both.
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

use crate::identity::AuthorKey;
use crate::segment::NodeId;
use alloc::vec::Vec;
use treepo_det::{Digest, Fx, OrderedMap, Sha256};

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

/// Who is drawn on one node, and over how much of it — `F-MAT-2`.
///
/// > Ownership drives accent, vein, and mosaic treatment over the primary material —
/// > proportional partitioning only, never a figure or ranking (`N4`).
///
/// # What a cell is
///
/// One indivisible unit of a node's surface, and the mosaic is [`cells`](Self::cells) of them
/// **running base to tip along the limb**. A holder occupies a contiguous run, and the runs
/// follow [`AuthorKey`] order — so the arrangement is not stored, it *is* [`holders`] read in
/// sequence, and there is no second structure that could disagree with the first.
///
/// The length axis rather than the width, for three reasons that all point the same way. A
/// limb is long and thin, so a partition across the width turns the 2% contributor of
/// `AC-MAT-2` into a sliver and loses `AC-MAT-4`'s legibility first. `F-EXT-3`'s blame
/// segments are line ranges, which are sequential within a file, so when they land they refine
/// this arrangement instead of replacing its geometry. And `design/feature-system.md` §8.3's
/// Grow migration moves material *along* a limb — a mosaic on the width axis would be
/// scrambled by the animation that is meant to carry it.
///
/// A cell is not a pixel. How many pixels one covers depends on zoom and on
/// [`Material::budget`], for the same reason the budget is a proportion rather than a count.
///
/// # `N4`, and what this type will not answer
///
/// A cell count is a contribution share wearing different units, which
/// `design/feature-system.md` §3.4 permits explicitly: share "may size a mosaic, allocate
/// material, or seed an accent". What `N4` forbids is *surfacing* it, and `AC-MAT-3` binds the
/// UI rather than this arithmetic.
///
/// Two properties keep the type itself clean. Iteration is in key order, which is hash order
/// and carries no information about contribution. And there is no accessor for the largest
/// holder, the ordering, or the remainder-by-rank — supplying one would put a leaderboard a
/// call away.
///
/// Unlike [`AuthorShare`](crate::primitives::AuthorShare), which closes the route at the type
/// level by implementing neither [`Ord`] nor [`PartialOrd`], the protection here is the shape
/// of the API and not something the compiler holds: a cell count is a `u32` and a caller who
/// collects [`holders`] can sort it. That is accepted rather than overlooked. Cells are a
/// geometric quantity — a renderer has to count them, compare them to a quota, and lay them
/// out — and a count that could not be compared would be obstructive to every legitimate use
/// in order to inconvenience one illegitimate one. The gate that matters sits upstream, where
/// the shares are: there is no way to reach a ranking without passing through
/// [`allocate`](../../treepo_gen/normalize/struct.Normalize.html#method.allocate), and by then
/// the numbers are a drawing instruction. `AC-MAT-3` binds the surface that would display one.
///
/// # Unclaimed cells are the normal case
///
/// `F-MAT-2` makes ownership an accent *over* the primary material, so a cell no contributor
/// holds already has something to be: the node's own [`MaterialFamily`]. Handing the remainder
/// to the largest holder would be both a ranking and a small lie about who wrote what.
///
/// [`holders`]: Self::holders
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Mosaic {
    held: OrderedMap<AuthorKey, u32>,
    cells: u32,
    claimed: u32,
}

impl Mosaic {
    /// A mosaic from per-contributor cell counts and the cell count the node was sized for.
    ///
    /// Contributors holding nothing are dropped rather than recorded as zero, so every key in
    /// the map is someone who is actually drawn. Recording a zero would make this a
    /// contributor list rather than a description of a surface, and every caller would have to
    /// filter it before every use.
    ///
    /// The mosaic grows past `budgeted` where the guaranteed quotas of `F-MAT-3` ask for more
    /// cells than were offered — see
    /// [`Normalize::allocate`](../../treepo_gen/normalize/struct.Normalize.html#method.allocate)
    /// for why growing is the answer rather than a failure.
    #[must_use]
    pub fn new(mut held: OrderedMap<AuthorKey, u32>, budgeted: u32) -> Self {
        held.retain(|_, cells| *cells > 0);
        let claimed = held
            .values()
            .fold(0u32, |sum, &cells| sum.saturating_add(cells));
        Self {
            cells: budgeted.max(claimed),
            claimed,
            held,
        }
    }

    /// Every contributor drawn here and how many cells they hold, in key order.
    pub fn holders(&self) -> impl Iterator<Item = (&AuthorKey, &u32)> {
        self.held.iter()
    }

    /// How many cells one contributor holds. Zero if they are not drawn here.
    #[must_use]
    pub fn cells_for(&self, author: &AuthorKey) -> u32 {
        self.held.get(author).copied().unwrap_or(0)
    }

    /// Whether this contributor appears at all.
    ///
    /// The `AC-MAT-2` predicate, and the one a caller should reach for: presence is what `N4`
    /// permits asking about, magnitude is what it does not.
    #[must_use]
    pub fn is_present(&self, author: &AuthorKey) -> bool {
        self.held.contains_key(author)
    }

    /// How many contributors are drawn.
    #[must_use]
    pub fn holder_count(&self) -> usize {
        self.held.len()
    }

    /// Whether nobody is drawn — an unattributed path, which is an ordinary case (PRD §6).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// How many cells this node's surface is divided into.
    #[must_use]
    pub const fn cells(&self) -> u32 {
        self.cells
    }

    /// How many of them contributors hold.
    #[must_use]
    pub const fn claimed(&self) -> u32 {
        self.claimed
    }

    /// How many the primary material shows through — see the type header.
    #[must_use]
    pub const fn unclaimed(&self) -> u32 {
        self.cells.saturating_sub(self.claimed)
    }
}

/// How old a node's material is at its base and at its tip — `F-MAT-4`.
///
/// > Age/recency gradient: older material sits basal/inward, recent material distal/tip-ward.
///
/// `design/feature-system.md` §8.3 states the rule and names what it buys: "a natural growth
/// rings + tip vitality reading without requiring explicit ring geometry".
///
/// # The two numbers are the node's own commit span
///
/// [`base`](Self::base) is the normalized age of the *first* commit to anything the node stands
/// for; [`tip`](Self::tip) is the age of the *last*. Nothing is invented — both come from
/// [`TemporalPrimitives`](crate::primitives::temporal::TemporalPrimitives), both roll up the
/// tree during extraction, and a file created three years ago and touched yesterday therefore
/// reads old at its base and vital at its tip without anyone deciding it should.
///
/// A path with one commit has one moment, so `base == tip` and the limb is uniform. That is
/// the honest rendering rather than a degenerate one: there is no span, so there is no
/// gradient.
///
/// # `base >= tip`, always
///
/// A first commit cannot be newer than a last one, so the requirement's direction is an
/// invariant of the type rather than a convention callers follow. [`new`](Self::new) orders its
/// arguments, so it holds even for a caller who passes them the other way round.
///
/// # Zero is new, one is old
///
/// Both values are normalized ages in `0..=1`, against the absolute scale in
/// `assets/params/materials.ron` — never against the repository's own oldest path, for the
/// reason [`normalize`](../../treepo_gen/normalize/index.html) gives about
/// `full_scale_bytes`: a repository-relative scale means one ancient vendored file
/// renormalizes every limb in the tree.
///
/// The direction is worth stating because it inverts against everything else here — a large
/// [`budget`](Material::budget) is more, a large age is *older*, and `F-MAT-4`'s sentence is
/// about age rather than about vitality. A renderer wanting the vitality reading of §8.3's
/// Thrive manifestation should take `ONE - age`, or read
/// [`recency_heat`](crate::primitives::temporal::TemporalPrimitives::recency_heat) directly,
/// which is what that manifestation is actually about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgeGradient {
    base: Fx,
    tip: Fx,
}

impl AgeGradient {
    /// A gradient from two normalized ages, ordered so the older one is the base.
    #[must_use]
    pub fn new(base: Fx, tip: Fx) -> Self {
        Self {
            base: base.max(tip),
            tip: base.min(tip),
        }
    }

    /// A node whose material is one age throughout — a path with a single commit.
    #[must_use]
    pub const fn uniform(age: Fx) -> Self {
        Self {
            base: age,
            tip: age,
        }
    }

    /// The normalized age at the base, nearer the trunk. The older end.
    #[must_use]
    pub const fn base(&self) -> Fx {
        self.base
    }

    /// The normalized age at the tip. The newer end.
    #[must_use]
    pub const fn tip(&self) -> Fx {
        self.tip
    }

    /// The normalized age a fraction `along` of the way from base to tip.
    ///
    /// Linear, because the normalization it is interpolating is already logarithmic and
    /// compressing twice would flatten the recent end into nothing — which is the end the
    /// picture is about.
    #[must_use]
    pub const fn at(&self, along: Fx) -> Fx {
        self.base.lerp(self.tip, along)
    }

    /// Where along the limb material of a given age sits — the inverse of [`at`](Self::at).
    ///
    /// [`at`](Self::at) answers "at this fraction along, how old is the material"; this answers
    /// "for material of this age, how far along". `F-MAT-5` is what needs it: enrichment placed
    /// where its own content's vintage sits lands *on* the material of that vintage, rather than
    /// at a position chosen independently of the shading it will be drawn over.
    ///
    /// `None` for a uniform gradient. A single-commit path has one age everywhere, so there is
    /// no position that age picks out — every point is equally right and the caller has to
    /// decide on some other ground. Returning a midpoint instead would be inventing an answer
    /// to a question with none, which is the same mistake `Option<AgeGradient>` refuses to make
    /// about a path with no history.
    ///
    /// Clamped to `0..=1`: an age outside the node's own span belongs to content the node does
    /// not stand for, and the honest placement is the nearer end rather than off the limb.
    #[must_use]
    pub fn position_of(&self, age: Fx) -> Option<Fx> {
        let span = self.span();
        if span.is_zero() {
            return None;
        }
        Some(self.base.sub(age).div(span).clamp(Fx::ZERO, Fx::ONE))
    }

    /// How much of the age range this node covers — zero for a single-commit path.
    #[must_use]
    pub const fn span(&self) -> Fx {
        self.base.sub(self.tip)
    }

    /// Whether the node's material is one age throughout, so there is no gradient to draw.
    #[must_use]
    pub fn is_uniform(&self) -> bool {
        self.base == self.tip
    }
}

/// One of the three ways a surface can read as stressed — `F-MAT-6`.
///
/// > Quality/debt signals introduce subtle stress materials (cracks, sparse density) coexisting
/// > with the primary material.
///
/// `design/feature-system.md` §8.5 names the appearances: "high TODO / debt signals can introduce
/// subtle stress materials (**cracks**, **sparse density**, **restless micro-particles**) that
/// coexist with the primary material". Three appearances, so three variants, and no fourth
/// invented to fill a grid — the same discipline [`EnrichmentKind`](crate::EnrichmentKind)
/// applies to §8.7's four names.
///
/// # These are appearances, not signals
///
/// A variant says what the surface *looks* like; which primitive produces it is
/// `treepo-gen::stress`'s decision and is documented there. The split is deliberate and matches
/// [`MaterialFamily`]: a renderer binding a texture needs the appearance and must not have to
/// know that a crack came from a `FIXME`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StressKind {
    /// Fissures in the surface. The author's own unfinished-work markers.
    Cracked,
    /// Coarse, thin, few-grained material. Mass concentrated in a handful of large files.
    Sparse,
    /// Unsettled — restless micro-particles, §8.8's "slight visual unease". Churning content.
    Restless,
}

impl StressKind {
    /// Every kind, in declaration order.
    pub const ALL: [Self; 3] = [Self::Cracked, Self::Sparse, Self::Restless];

    /// This kind's index in [`ALL`](Self::ALL) — how a [`Stress`] is addressed.
    ///
    /// A `match` rather than a search, so a kind added without a slot does not compile. Same
    /// discipline as [`MaterialFamily::position`].
    #[must_use]
    pub const fn position(self) -> usize {
        match self {
            Self::Cracked => 0,
            Self::Sparse => 1,
            Self::Restless => 2,
        }
    }

    /// The name used in `assets/params/materials.ron` and in error messages.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Cracked => "cracked",
            Self::Sparse => "sparse",
            Self::Restless => "restless",
        }
    }
}

/// How stressed one node's surface is, per kind — `F-MAT-6`.
///
/// Indexed by position in [`StressKind::ALL`], so it is `Copy` and the same size for every node,
/// for the reason [`FamilyMix`] is: at T3 there are eighty thousand of these.
///
/// # Coexisting is the whole requirement
///
/// `F-MAT-6` says stress "coexists with the primary material", and this type is what makes that
/// structural rather than a promise. It sits *beside* [`Material::family`],
/// [`Material::composition`], [`Material::budget`], [`Material::mosaic`] and
/// [`Material::gradient`] and cannot alter any of them — a stressed limb is the same limb with an
/// extra reading, never a limb made of something else. `treepo-gen`'s
/// `stress_coexists_with_the_primary_material` is the test, and the ceiling in
/// `assets/params/materials.ron` is what keeps "subtle" from being a matter of taste.
///
/// # Not measured is not clean
///
/// Each intensity is `Option<Fx>`, and `None` means the signal behind it was never measured —
/// a binary blob nothing read, a path with no line count to divide churn by. Zero means it *was*
/// measured and there is nothing to draw. Both render as an unmarked surface, so the distinction
/// buys nothing for a renderer and everything for `F-INSP-5`'s why-panel and for `P1`: a
/// why-panel that said "no debt here" about a file treepo never opened would be inventing a
/// finding. It is the same refusal [`DerivedSignals`](crate::DerivedSignals) makes field by
/// field, carried through instead of defaulted away.
///
/// [`new`](Self::new) returns `None` when nothing at all was measured, which is why
/// [`Material::stress`] is an `Option` — a `Some` always carries at least one real measurement,
/// so the two nullable layers say different things rather than the same thing twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stress([Option<Fx>; StressKind::ALL.len()]);

impl Stress {
    /// A reading from per-kind intensities, in [`StressKind::ALL`] order.
    ///
    /// `None` where nothing was measured — see the type header. The one place that decision is
    /// made, so no caller can produce a `Stress` that claims a clean surface it never looked at.
    #[must_use]
    pub fn new(intensities: [Option<Fx>; StressKind::ALL.len()]) -> Option<Self> {
        intensities
            .iter()
            .any(Option::is_some)
            .then_some(Self(intensities))
    }

    /// How strongly one kind shows, or `None` where its signal was not measured.
    #[must_use]
    pub fn intensity_of(&self, kind: StressKind) -> Option<Fx> {
        self.0[kind.position()]
    }

    /// Every kind with something to draw, in [`StressKind::ALL`] order.
    ///
    /// Skips both the unmeasured and the measured-clean, because a renderer asking what to draw
    /// wants the same answer for either — the distinction is [`intensity_of`](Self::intensity_of)'s
    /// to report.
    pub fn present(&self) -> impl Iterator<Item = (StressKind, Fx)> + '_ {
        StressKind::ALL
            .into_iter()
            .zip(self.0)
            .filter_map(|(kind, intensity)| {
                intensity
                    .filter(|value| !value.is_zero())
                    .map(|value| (kind, value))
            })
    }

    /// Whether something was measured and there is nothing to draw.
    ///
    /// The healthy surface, and an ordinary answer rather than a gap.
    #[must_use]
    pub fn is_clear(&self) -> bool {
        self.present().next().is_none()
    }

    /// How many kinds have something to draw.
    #[must_use]
    pub fn count(&self) -> usize {
        self.present().count()
    }
}

/// What one skeleton node is made of, who is drawn on it, how old it is, and what is wrong
/// with it.
///
/// Keyed to a [`NodeId`](crate::segment::NodeId) by [`MaterialMap`].
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// Who is drawn on it and over how much of it — `F-MAT-2`.
    ///
    /// Empty for an unattributed path, which is an ordinary case rather than a gap: a
    /// repository with no `.git` renders as a whole tree of primary material (PRD §6).
    pub mosaic: Mosaic,
    /// How old its material is from base to tip — `F-MAT-4`.
    ///
    /// `None` where nothing the node stands for has any history: no `.git`, a repository with
    /// no commits, or a file git has never seen (PRD §6). That is *unknown*, which is not the
    /// same as *old* — a neutral value here would render a brand-new working directory as
    /// ancient, and the user is already being told separately that age is unavailable.
    pub gradient: Option<AgeGradient>,
    /// What is wrong with its surface, subtly and over the top of everything else — `F-MAT-6`.
    ///
    /// `None` where none of the debt signals behind it was measured: a binary blob nothing read,
    /// a repository extracted without the content pass. That is *unknown* rather than *healthy*,
    /// for the reason [`gradient`](Self::gradient) is `None` rather than new — see [`Stress`].
    ///
    /// A `Some` never changes any other field. `F-MAT-6` says stress "coexists with the primary
    /// material", and every other reading of this node is what it coexists with.
    pub stress: Option<Stress>,
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

            // Count first, then the holders in key order — the same rule the roles follow in
            // `Skeleton::digest`, so a limb that gained a contributor cannot run into one that
            // merely redistributed its cells.
            hasher.update(&material.mosaic.cells().to_le_bytes());
            hasher.update(&material.mosaic.claimed().to_le_bytes());
            hasher.update(&(material.mosaic.holder_count() as u64).to_le_bytes());
            for (author, cells) in material.mosaic.holders() {
                hasher.update(author.as_bytes());
                hasher.update(&cells.to_le_bytes());
            }

            // Discriminant first: a node with no history is a different tree from one whose
            // history happens to normalize to zero, and the two must not encode alike.
            match &material.gradient {
                None => hasher.update(&[0]),
                Some(gradient) => {
                    hasher.update(&[1]);
                    hasher.update(&gradient.base().to_bits().to_le_bytes());
                    hasher.update(&gradient.tip().to_bits().to_le_bytes());
                }
            }

            // Discriminants again, and one per kind rather than one for the whole reading: an
            // unmeasured signal and a measured-clean one are different facts (see `Stress`), so
            // they must not encode alike even though they draw alike.
            match &material.stress {
                None => hasher.update(&[0]),
                Some(stress) => {
                    hasher.update(&[1]);
                    for kind in StressKind::ALL {
                        match stress.intensity_of(kind) {
                            None => hasher.update(&[0]),
                            Some(intensity) => {
                                hasher.update(&[1]);
                                hasher.update(&intensity.to_bits().to_le_bytes());
                            }
                        }
                    }
                }
            }

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
/// for a disagreement about materials. Same discipline as `treepo-skeleton-v2`. `v1` predated
/// the ownership mosaic, `v2` the age gradient and `v3` the stress reading, so none of them
/// could tell two limbs apart that differ only in who wrote them, in when, or in what is
/// wrong with them.
const MATERIAL_DIGEST_TAG: &[u8] = b"treepo-material-v4";

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

    fn author(byte: u8) -> AuthorKey {
        AuthorKey::from_email(&[byte])
    }

    fn mosaic(held: &[(AuthorKey, u32)], budgeted: u32) -> Mosaic {
        Mosaic::new(held.iter().copied().collect(), budgeted)
    }

    fn material(family: MaterialFamily, composition: Composition) -> Material {
        Material {
            family,
            composition,
            budget: Fx::from_ratio(1, 3),
            mosaic: mosaic(&[(author(1), 6), (author(2), 2)], 16),
            gradient: Some(AgeGradient::new(
                Fx::from_ratio(9, 10),
                Fx::from_ratio(1, 10),
            )),
            // One of each state, so the digest tests below have a measured intensity, a
            // measured zero and an unmeasured signal in them.
            stress: Stress::new([Some(Fx::from_ratio(1, 8)), Some(Fx::ZERO), None]),
        }
    }

    /// `F-MAT-4`'s direction, held by the type rather than by the caller: a first commit
    /// cannot be newer than a last one, so the older end is the base whichever way round the
    /// arguments arrive.
    #[test]
    fn the_older_end_is_always_the_base() {
        // Quarters, which are exact in binary — a tenth is not, and `span` would then be a
        // rounding artefact away from any constant this could be compared against.
        let old = Fx::from_ratio(3, 4);
        let new = Fx::from_ratio(1, 4);

        for gradient in [AgeGradient::new(old, new), AgeGradient::new(new, old)] {
            assert_eq!(gradient.base(), old, "older material sits basal");
            assert_eq!(gradient.tip(), new, "recent material sits tip-ward");
            assert!(gradient.base() >= gradient.tip());
            assert_eq!(gradient.span(), Fx::HALF);
            assert!(!gradient.is_uniform());
        }
    }

    /// A path with one commit has one moment, so there is no gradient to draw. The honest
    /// rendering rather than a degenerate one.
    #[test]
    fn a_single_commit_path_is_one_age_throughout() {
        let flat = AgeGradient::uniform(Fx::HALF);
        assert!(flat.is_uniform());
        assert_eq!(flat.span(), Fx::ZERO);
        assert_eq!(flat.base(), flat.tip());
        assert_eq!(flat.at(Fx::ZERO), Fx::HALF);
        assert_eq!(flat.at(Fx::ONE), Fx::HALF);
        assert_eq!(AgeGradient::new(Fx::HALF, Fx::HALF), flat);
    }

    /// `F-MAT-5`'s question, and the inverse of the one above: for material of this age, how
    /// far along does it sit? A structure placed there lands on material of its own vintage.
    #[test]
    fn material_of_a_given_age_has_a_place_on_the_limb() {
        let gradient = AgeGradient::new(Fx::ONE, Fx::ZERO);
        assert_eq!(
            gradient.position_of(Fx::ONE),
            Some(Fx::ZERO),
            "oldest is basal"
        );
        assert_eq!(
            gradient.position_of(Fx::ZERO),
            Some(Fx::ONE),
            "newest is at the tip"
        );
        assert_eq!(gradient.position_of(Fx::HALF), Some(Fx::HALF));

        // Round-trips against `at`, which is what makes it the inverse rather than a second
        // opinion about the axis.
        for step in 0..=8i64 {
            let along = Fx::from_ratio(step, 8);
            assert_eq!(gradient.position_of(gradient.at(along)), Some(along));
        }

        // An age outside the node's own span belongs to content the node does not stand for,
        // and the honest placement is the nearer end rather than off the limb.
        let narrow = AgeGradient::new(Fx::from_ratio(3, 4), Fx::from_ratio(1, 4));
        assert_eq!(narrow.position_of(Fx::ONE), Some(Fx::ZERO));
        assert_eq!(narrow.position_of(Fx::ZERO), Some(Fx::ONE));
    }

    /// A single-commit path has one age everywhere, so no age picks out a position. Returning a
    /// midpoint would be inventing an answer to a question that has none — the same refusal
    /// `Option<AgeGradient>` makes about a path with no history at all.
    #[test]
    fn a_uniform_gradient_places_nothing() {
        assert_eq!(AgeGradient::uniform(Fx::HALF).position_of(Fx::HALF), None);
        assert_eq!(AgeGradient::uniform(Fx::ZERO).position_of(Fx::ZERO), None);
    }

    /// The renderer's question: at a fraction of the way up, how old is the material?
    #[test]
    fn reading_along_the_limb_runs_from_old_to_new() {
        let gradient = AgeGradient::new(Fx::ONE, Fx::ZERO);
        assert_eq!(gradient.at(Fx::ZERO), Fx::ONE, "the base is the oldest");
        assert_eq!(gradient.at(Fx::ONE), Fx::ZERO, "the tip is the newest");
        assert_eq!(gradient.at(Fx::HALF), Fx::HALF);

        // Monotonically newer toward the tip, which is the whole of §8.3's primary rule.
        let mut previous = Fx::ONE.add(Fx::from_ratio(1, 1000));
        for step in 0..=100i64 {
            let age = gradient.at(Fx::from_ratio(step, 100));
            assert!(age <= previous, "the material got older toward the tip");
            previous = age;
        }
    }

    /// `position` indexes the array a [`Stress`] is, and a mismatch would report cracks where a
    /// node is restless.
    #[test]
    fn stress_positions_index_all() {
        for (index, kind) in StressKind::ALL.into_iter().enumerate() {
            assert_eq!(kind.position(), index, "{kind:?}");
        }
        let mut sorted = StressKind::ALL;
        sorted.sort();
        let mut deduped = alloc::vec::Vec::from(sorted);
        deduped.dedup();
        assert_eq!(deduped.len(), StressKind::ALL.len());
    }

    /// The distinction the whole `Option` layering exists for: a surface nobody measured is not
    /// a surface with nothing wrong. Both draw as unmarked, and only one of them is a finding.
    #[test]
    fn nothing_measured_is_not_the_same_as_nothing_wrong() {
        assert_eq!(Stress::new([None, None, None]), None, "nothing to say");

        let clear = Stress::new([Some(Fx::ZERO), None, None]).expect("one signal was measured");
        assert!(clear.is_clear());
        assert_eq!(clear.count(), 0);
        assert_eq!(
            clear.intensity_of(StressKind::Cracked),
            Some(Fx::ZERO),
            "measured, and there is nothing there"
        );
        assert_eq!(
            clear.intensity_of(StressKind::Sparse),
            None,
            "not measured, so nothing can be claimed about it"
        );
    }

    /// What a renderer asks for: the kinds with something to draw, and nothing else.
    #[test]
    fn only_the_kinds_with_something_to_draw_are_present() {
        let stress = Stress::new([Some(Fx::from_ratio(1, 4)), Some(Fx::ZERO), Some(Fx::HALF)])
            .expect("measured");

        let present: alloc::vec::Vec<(StressKind, Fx)> = stress.present().collect();
        assert_eq!(
            present,
            [
                (StressKind::Cracked, Fx::from_ratio(1, 4)),
                (StressKind::Restless, Fx::HALF),
            ],
            "ALL order, and the measured zero is not drawn"
        );
        assert_eq!(stress.count(), 2);
        assert!(!stress.is_clear());
    }

    /// The three numbers must agree with the map, or a renderer draws a mosaic whose parts do
    /// not add up to its whole.
    #[test]
    fn a_mosaic_counts_what_it_actually_holds() {
        let m = mosaic(&[(author(1), 6), (author(2), 2)], 16);
        assert_eq!(m.holder_count(), 2);
        assert_eq!(m.claimed(), 8);
        assert_eq!(m.cells(), 16);
        assert_eq!(
            m.unclaimed(),
            8,
            "the primary material shows through the rest"
        );
        assert_eq!(m.cells_for(&author(1)), 6);
        assert!(m.is_present(&author(2)));
        assert!(!m.is_present(&author(9)));
        assert_eq!(m.cells_for(&author(9)), 0);
    }

    /// A contributor holding nothing is not drawn, so recording them would make this a
    /// contributor list rather than a description of a surface.
    #[test]
    fn a_contributor_holding_nothing_is_not_in_the_mosaic() {
        let m = mosaic(&[(author(1), 4), (author(2), 0)], 8);
        assert_eq!(m.holder_count(), 1);
        assert!(!m.is_present(&author(2)));
        assert_eq!(m.claimed(), 4);
    }

    /// `F-MAT-3`'s guaranteed quotas can ask for more cells than the node was sized for, and
    /// the mosaic subdivides further rather than dropping anyone — which would require picking
    /// *which*, and that is the ordering `N4` forbids.
    #[test]
    fn a_mosaic_grows_rather_than_losing_someone() {
        let crowded = mosaic(&[(author(1), 5), (author(2), 5), (author(3), 5)], 8);
        assert_eq!(crowded.cells(), 15, "sized for 8, and 15 are held");
        assert_eq!(crowded.unclaimed(), 0);
        assert_eq!(crowded.holder_count(), 3);
    }

    /// PRD §6, "No `.git`": an unattributed path is ordinary, and the whole surface is
    /// primary material.
    #[test]
    fn an_unattributed_node_has_an_empty_mosaic() {
        let bare = mosaic(&[], 16);
        assert!(bare.is_empty());
        assert_eq!(bare.claimed(), 0);
        assert_eq!(bare.unclaimed(), 16);
        assert!(Mosaic::default().is_empty());
    }

    /// `N4`: holders come out in key order, which is hash order and uncorrelated with what
    /// anyone holds, so consuming this in sequence cannot produce a ranking.
    #[test]
    fn holders_come_out_in_key_order() {
        let m = mosaic(&[(author(3), 1), (author(1), 9), (author(2), 4)], 16);
        let iterated: alloc::vec::Vec<AuthorKey> = m.holders().map(|(&key, _)| key).collect();
        let mut by_key = iterated.clone();
        by_key.sort();
        assert_eq!(iterated, by_key);
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

    /// `F-MAT-2` in `AC-DET-1`: a limb whose contributors changed is a different tree, and
    /// every way that can happen has to reach the digest.
    #[test]
    fn every_part_of_a_mosaic_reaches_the_digest() {
        let baseline = sample().digest();

        let mut rehoused = sample();
        rehoused.materials[0].mosaic = mosaic(&[(author(1), 6), (author(3), 2)], 16);
        assert_ne!(rehoused.digest(), baseline, "a different contributor");

        let mut redistributed = sample();
        redistributed.materials[0].mosaic = mosaic(&[(author(1), 5), (author(2), 3)], 16);
        assert_ne!(redistributed.digest(), baseline, "the same two, shifted");

        let mut resized = sample();
        resized.materials[0].mosaic = mosaic(&[(author(1), 6), (author(2), 2)], 32);
        assert_ne!(resized.digest(), baseline, "a finer subdivision");

        let mut vacated = sample();
        vacated.materials[0].mosaic = mosaic(&[], 16);
        assert_ne!(vacated.digest(), baseline, "nobody at all");
    }

    /// `F-MAT-4` in `AC-DET-1`. The `None` case is the one worth pinning: a node with no
    /// history must not encode like one whose history happens to normalize to zero, or a
    /// repository with no `.git` would hash like an ancient one.
    #[test]
    fn every_part_of_a_gradient_reaches_the_digest() {
        let baseline = sample().digest();

        let mut aged = sample();
        aged.materials[0].gradient = Some(AgeGradient::new(Fx::ONE, Fx::from_ratio(1, 10)));
        assert_ne!(aged.digest(), baseline, "a different base");

        let mut revived = sample();
        revived.materials[0].gradient = Some(AgeGradient::new(Fx::from_ratio(9, 10), Fx::ZERO));
        assert_ne!(revived.digest(), baseline, "a different tip");

        let mut ageless = sample();
        ageless.materials[0].gradient = None;
        assert_ne!(ageless.digest(), baseline, "no history at all");

        // And the case the discriminant exists for.
        let mut brand_new = sample();
        brand_new.materials[0].gradient = Some(AgeGradient::uniform(Fx::ZERO));
        assert_ne!(
            brand_new.digest(),
            ageless.digest(),
            "unknown age and zero age are different facts"
        );
    }

    /// `F-MAT-6` in `AC-DET-1`. The pair that matters is the last one: a surface nobody measured
    /// must not encode like one measured and found clean, or a repository extracted without the
    /// content pass would hash like a healthy one.
    #[test]
    fn every_part_of_a_stress_reaches_the_digest() {
        let baseline = sample().digest();

        let mut worse = sample();
        worse.materials[0].stress = Stress::new([Some(Fx::from_ratio(1, 4)), Some(Fx::ZERO), None]);
        assert_ne!(worse.digest(), baseline, "a different intensity");

        let mut spread = sample();
        spread.materials[0].stress =
            Stress::new([Some(Fx::from_ratio(1, 8)), Some(Fx::ZERO), Some(Fx::ZERO)]);
        assert_ne!(spread.digest(), baseline, "a third signal was measured");

        let mut unknown = sample();
        unknown.materials[0].stress = None;
        assert_ne!(unknown.digest(), baseline, "nothing measured at all");

        let mut clean = sample();
        clean.materials[0].stress = Stress::new([Some(Fx::ZERO), Some(Fx::ZERO), Some(Fx::ZERO)]);
        assert_ne!(
            clean.digest(),
            unknown.digest(),
            "an unexamined surface and a clean one are different facts"
        );
    }

    /// The count-first rule, on the case it exists for. Both mosaics claim eight cells of
    /// sixteen; a length-blind encoding would run the holders together and call them equal.
    #[test]
    fn a_gained_contributor_does_not_collide_with_a_redistribution() {
        let two = material(MaterialFamily::Heartwood, Composition::Pure);
        let mut three = two.clone();
        three.mosaic = mosaic(&[(author(1), 4), (author(2), 2), (author(3), 2)], 16);

        let mut first = MaterialMap::new();
        first.push(two);
        let mut second = MaterialMap::new();
        second.push(three);

        assert_ne!(first.digest(), second.digest());
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
