//! Skeleton geometry — what `F-SKEL-1` produces and every later phase consumes.
//!
//! > `(subtree primitives, path seed, parameter table) → oriented, thickened segments`
//!
//! These types live here rather than in `treepo-gen` because they are a *handoff*, in the
//! same sense [`Manifest`](crate::Manifest) is: `treepo-gen` writes them, `treepo-grow` diffs
//! them, and `treepo-render` draws them. A type three crates exchange belongs in the crate
//! they all already depend on.
//!
//! # The skeleton is a tree of nodes, not a bag of lines
//!
//! [`Segment`] is geometry and nothing else. What makes the skeleton inspectable, diffable,
//! and pickable is [`SkeletonNode`] — every segment names the node that drew it, and every
//! node names what it stands for: a repository path, or an
//! [`AggregateNode`](crate::AggregateNode) standing for content past the composition
//! threshold.
//!
//! That matters for three separate requirements, which is why it is worth the indirection.
//! `F-INSP-3` needs a click on any pixel to answer "what is this"; `N7`/`P1` need every baked
//! pixel to carry an element ID; and `AC-GROW-4` needs a diff to say "this limb changed" and
//! not "these four thousand lines moved". All three are questions about nodes.
//!
//! # Two dimensions, and thickness
//!
//! `design/l-system-parameterization.md` §2.1: "In treepo we primarily need the 2-D subset
//! plus thickness (`!`) for the initial skeleton. 3-D extensions remain available for later
//! canopy or camera work." The turtle's convention is documented on [`SkeletonNode::heading`]
//! — it is the one thing here a reader cannot guess.

use crate::aggregate::AggregateNode;
use crate::path::RepoPath;
use alloc::vec::Vec;
use treepo_det::{Angle, Fx, Seed};

/// A point in world space.
///
/// [`Fx`] rather than a float, for the reason every coordinate in treepo is: `AC-DET-2`
/// requires identical output hashes on three platforms, and a float coordinate computed from
/// a platform `libm` is not identical on three platforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point {
    /// Rightward.
    pub x: Fx,
    /// Upward.
    pub y: Fx,
}

impl Point {
    /// The origin.
    pub const ORIGIN: Self = Self {
        x: Fx::ZERO,
        y: Fx::ZERO,
    };

    /// A point from its coordinates.
    #[must_use]
    pub const fn new(x: Fx, y: Fx) -> Self {
        Self { x, y }
    }
}

/// Which node in a [`Skeleton`] something belongs to.
///
/// An index rather than a reference: the skeleton is built in one pass, held immutably, and
/// serialized. A `u32` also survives the trip into a render-side ID buffer (`N7`), where a
/// pointer would not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(u32);

impl NodeId {
    /// The node this id refers to, as an index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// An id from an index.
    #[must_use]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }
}

/// One drawn length of limb.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment {
    /// The end nearer the root.
    pub start: Point,
    /// The end nearer the tip.
    pub end: Point,
    /// Width at [`start`](Self::start).
    pub base_width: Fx,
    /// Width at [`end`](Self::end).
    pub tip_width: Fx,
    /// The node whose L-system instance drew this.
    pub node: NodeId,
    /// Which branch generation within that instance, zero being the instance's own base.
    pub generation: u8,
}

/// What a [`SkeletonNode`] stands for.
///
/// Four kinds, and the distinction that matters is *what each one draws*:
///
/// | Role | Stands for | Draws its contents |
/// |---|---|---|
/// | [`Limb`](Self::Limb) | one path | — |
/// | [`Group`](Self::Group) | several paths | yes, each as its own limb (`F2`) |
/// | [`Aggregate`](Self::Aggregate) | several paths | no — it *is* their representation (`F-SKEL-7`) |
/// | [`RootMass`](Self::RootMass) | the repository's base | — |
///
/// [`Group`](Self::Group) and [`Aggregate`](Self::Aggregate) are the pair worth keeping
/// apart. Both gather several paths under one node; only the aggregate replaces them.
/// Collapsing the two would make `F2`'s "fewer, thicker limbs" indistinguishable from
/// `F-SKEL-7`'s "this directory and all its contents", and every path inside a group would
/// read as compressed when it is in fact drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeRole {
    /// One repository path, rendered as its own limb.
    Limb {
        /// The path this limb is.
        path: RepoPath,
    },
    /// Several small siblings sharing one thicker stem — `F2`.
    ///
    /// The members are still drawn individually, as limbs hanging from this stem. What the
    /// group buys is a base region carrying more mass in fewer limbs, which is where the
    /// hybrid trunk's overlap comes from.
    Group {
        /// The limb whose children were grouped.
        anchor: RepoPath,
        /// The paths sharing this stem, in path order.
        members: alloc::vec::Vec<RepoPath>,
    },
    /// A proportional container standing for several paths at once (`F-SKEL-7`).
    Aggregate(AggregateNode),
    /// One node of the root-boulder cluster at the base (`AC-SKEL-2`).
    ///
    /// It stands for the repository rather than for any path in it, which is why
    /// `design/visual-construction.md` gives it the global signals to carry. It is also what
    /// an empty repository consists of: a seed and a root cluster, never a lonely trunk.
    RootMass {
        /// The repository root, so every node answers `F-INSP-4` the same way.
        anchor: RepoPath,
        /// Which node of the cluster this is, counted from zero.
        index: u16,
    },
}

impl NodeRole {
    /// The path this node is, or the limb it hangs from.
    ///
    /// What a "reveal in file manager" (`F-INSP-4`) resolves to, for every kind.
    #[must_use]
    pub const fn anchor(&self) -> &RepoPath {
        match self {
            Self::Limb { path } => path,
            Self::Group { anchor, .. } | Self::RootMass { anchor, .. } => anchor,
            Self::Aggregate(aggregate) => &aggregate.anchor,
        }
    }

    /// Whether this node stands for content it does not draw individually.
    #[must_use]
    pub const fn is_aggregate(&self) -> bool {
        matches!(self, Self::Aggregate(_))
    }
}

/// A first-class citizen of the skeleton: one limb, or one aggregate container.
///
/// Aggregates are nodes in exactly the sense limbs are — they carry a position, a heading, a
/// seed of their own, and a place in the parent chain. That is what makes `F-SKEL-7`'s
/// container a *phase transition* rather than a truncation: a later phase can give it a
/// visual form, hang enrichment off it, or grow it into something without the skeleton
/// needing a second concept for "the parts we did not draw".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkeletonNode {
    /// This node's own id.
    pub id: NodeId,
    /// The node it hangs from. `None` for the basal node.
    pub parent: Option<NodeId>,
    /// Where its first segment begins.
    pub origin: Point,
    /// The direction its first segment leaves in.
    ///
    /// Measured from straight up, increasing clockwise: `ZERO` is up, `QUARTER` is right,
    /// `HALF` is down. Chosen so that a tree's natural direction is the zero of the
    /// coordinate system and a limb's angle from vertical is read directly — the quantity
    /// `E3`'s droop is proportional to.
    pub heading: Angle,
    /// This node's generator seed (`P2`).
    ///
    /// Derived from the path for a limb, and from the parent's seed and container index for
    /// an aggregate. An aggregate's seed is deliberately independent of *which* paths landed
    /// in it, so that a file arriving in a container does not reroll the container's
    /// appearance — the cluster stays the cluster and gains a member.
    pub seed: Seed,
    /// What this node stands for.
    pub role: NodeRole,
}

/// One repository's structural skeleton.
///
/// Nodes are stored in creation order, and [`NodeId`] indexes into that order — so a node's
/// parent always precedes it, and a single forward pass suffices to walk the tree from the
/// base.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Skeleton {
    nodes: Vec<SkeletonNode>,
    segments: Vec<Segment>,
}

impl Skeleton {
    /// An empty skeleton.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            nodes: Vec::new(),
            segments: Vec::new(),
        }
    }

    /// Adds a node, returning the id it was given.
    ///
    /// The only way to add one, so [`NodeId`] cannot be made to disagree with the position
    /// it indexes.
    pub fn push_node(
        &mut self,
        parent: Option<NodeId>,
        origin: Point,
        heading: Angle,
        seed: Seed,
        role: NodeRole,
    ) -> NodeId {
        let id = NodeId::new(u32::try_from(self.nodes.len()).unwrap_or(u32::MAX));
        self.nodes.push(SkeletonNode {
            id,
            parent,
            origin,
            heading,
            seed,
            role,
        });
        id
    }

    /// Adds drawn geometry.
    pub fn extend_segments(&mut self, segments: impl IntoIterator<Item = Segment>) {
        self.segments.extend(segments);
    }

    /// Every node, parents before children.
    #[must_use]
    pub fn nodes(&self) -> &[SkeletonNode] {
        &self.nodes
    }

    /// Every drawn segment.
    #[must_use]
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// One node by id.
    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<&SkeletonNode> {
        self.nodes.get(id.index())
    }

    /// How many nodes stand for content they do not draw individually.
    #[must_use]
    pub fn aggregate_count(&self) -> usize {
        self.nodes
            .iter()
            .filter(|node| node.role.is_aggregate())
            .count()
    }

    /// Whether the skeleton represents `path` — drawn as a limb, or inside a container.
    ///
    /// `P7`'s question, and the one a caller usually means. A container's members are the
    /// *roots* of what it stands for (see [`AggregateNode`]), so a file five levels beneath
    /// an aggregated directory is represented by that container without appearing in its
    /// member list. Asking only whether a path is a limb or a member would call that file
    /// erased when it is merely compressed — the exact confusion `P6` separates.
    ///
    /// Linear in the node count, so it is a query rather than an inner loop; `F-MAT-3`'s
    /// floor and `F-INSP-3`'s drill-down both ask it once per user action.
    #[must_use]
    pub fn represents(&self, path: &RepoPath) -> bool {
        self.nodes.iter().any(|node| match &node.role {
            NodeRole::Limb { path: limb } => limb == path,
            NodeRole::Aggregate(aggregate) => aggregate
                .members
                .iter()
                .any(|member| path.starts_with(member)),
            // A group draws its members as limbs of their own, so they are represented by
            // those nodes and counting them here would double-count rather than add. A
            // root-mass node stands for the repository, not for any path in it.
            NodeRole::Group { .. } | NodeRole::RootMass { .. } => false,
        })
    }

    /// Every path the skeleton names directly: each limb, and each container's member roots.
    ///
    /// Not the same as everything it *represents* — see [`represents`](Self::represents).
    /// Group members are absent for the same reason they are absent there: each is present
    /// as a limb in its own right.
    pub fn accounted_roots(&self) -> impl Iterator<Item = &RepoPath> {
        self.nodes.iter().flat_map(|node| match &node.role {
            NodeRole::Limb { path } => AccountedPaths::One(core::iter::once(path)),
            NodeRole::Aggregate(aggregate) => AccountedPaths::Many(aggregate.members.iter()),
            NodeRole::Group { .. } | NodeRole::RootMass { .. } => AccountedPaths::None,
        })
    }
}

/// The shapes [`Skeleton::accounted_roots`] flattens, without boxing.
enum AccountedPaths<'a> {
    None,
    One(core::iter::Once<&'a RepoPath>),
    Many(core::slice::Iter<'a, RepoPath>),
}

impl<'a> Iterator for AccountedPaths<'a> {
    type Item = &'a RepoPath;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::None => None,
            Self::One(iter) => iter.next(),
            Self::Many(iter) => iter.next(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn seed() -> Seed {
        Seed::root(b"test")
    }

    fn path(text: &str) -> RepoPath {
        RepoPath::new(text.as_bytes()).unwrap()
    }

    #[test]
    fn node_ids_index_the_order_they_were_added_in() {
        let mut skeleton = Skeleton::new();
        let a = skeleton.push_node(
            None,
            Point::ORIGIN,
            Angle::ZERO,
            seed(),
            NodeRole::Limb { path: path("src") },
        );
        let b = skeleton.push_node(
            Some(a),
            Point::ORIGIN,
            Angle::QUARTER,
            seed(),
            NodeRole::Limb {
                path: path("src/lib.rs"),
            },
        );

        assert_eq!(a.index(), 0);
        assert_eq!(b.index(), 1);
        assert_eq!(skeleton.node(b).unwrap().parent, Some(a));
        // A parent always precedes its child, so one forward pass walks the whole tree.
        assert!(skeleton.node(b).unwrap().parent.unwrap().index() < b.index());
    }

    #[test]
    fn every_path_is_accounted_for_as_a_limb_or_as_a_member() {
        let mut skeleton = Skeleton::new();
        skeleton.push_node(
            None,
            Point::ORIGIN,
            Angle::ZERO,
            seed(),
            NodeRole::Limb { path: path("src") },
        );
        skeleton.push_node(
            None,
            Point::ORIGIN,
            Angle::ZERO,
            seed(),
            NodeRole::Aggregate(AggregateNode {
                anchor: path("src"),
                index: 0,
                members: vec![path("src/a.rs"), path("src/b.rs")],
                bytes: 40,
                file_count: 2,
                dir_count: 0,
            }),
        );

        let accounted: Vec<&RepoPath> = skeleton.accounted_roots().collect();
        assert_eq!(accounted.len(), 3);
        assert!(accounted.contains(&&path("src/b.rs")));
        assert_eq!(skeleton.aggregate_count(), 1);
    }

    /// The distinction `represents` exists for: a container names roots, and stands for
    /// everything beneath them.
    #[test]
    fn a_container_represents_what_lies_beneath_its_members() {
        let mut skeleton = Skeleton::new();
        skeleton.push_node(
            None,
            Point::ORIGIN,
            Angle::ZERO,
            seed(),
            NodeRole::Aggregate(AggregateNode {
                anchor: path("v"),
                index: 0,
                members: vec![path("v/lib")],
                bytes: 900,
                file_count: 40,
                dir_count: 6,
            }),
        );

        // Named directly.
        assert!(skeleton.represents(&path("v/lib")));
        // Not named, but compressed rather than erased — the P6 reading.
        assert!(skeleton.represents(&path("v/lib/deep/inner.rs")));
        // A sibling the container does not stand for.
        assert!(!skeleton.represents(&path("v/other.rs")));
    }
}
