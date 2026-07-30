//! The phase handoff type — architecture D4.
//!
//! > Grow publishes `Arc<WorldSnapshot>` into an `ArcSwap` resource. Thrive reads the current
//! > snapshot each frame; on change, `snapshot_sync.rs` reconciles ECS entities to match.
//!
//! Everything Grow produces and Thrive consumes travels in one immutable value, and that is
//! the whole point rather than a convenience: a snapshot under construction is not reachable,
//! so "Thrive observed a half-built tree" is not a state that exists to be defended against.
//! Cancellation is expressed by *not publishing* one (`AC-GROW-3`).
//!
//! # What is here, and what is not yet
//!
//! The architecture's field list names `heat_weights` and an `id_map` alongside these four.
//! Both belong to layers that do not exist: heat is `F-THR-2` (Phase 8) and the element-ID
//! raster is baked by `treepo-render` (later in Phase 5). A field carrying a default nobody
//! computed is worse than an absent one — it reads as measured. They arrive with the passes
//! that fill them, the way [`material`](crate::material) and [`enrichment`](crate::enrichment)
//! did.
//!
//! # Why this is a plain struct
//!
//! It holds the three maps whole rather than borrowing them, because the app hands it across a
//! thread boundary inside an `Arc` and a borrow cannot cross that. The three are already
//! index-parallel by [`NodeId`](crate::NodeId) — [`MaterialMap::covers`](crate::MaterialMap::covers)
//! is the invariant, and it holds by construction because every producer walks
//! [`Skeleton::nodes`](crate::Skeleton::nodes) in order.

use crate::enrichment::EnrichmentMap;
use crate::identity::CommitId;
use crate::material::MaterialMap;
use crate::segment::Skeleton;

/// One committed view of a repository: its geometry, its material, and what is built on it.
///
/// Produced by the generative pipeline, consumed by the render layer, and replaced whole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldSnapshot {
    /// Which snapshot this is, counting from zero within a session.
    ///
    /// Not a hash. `treepo-grow` addresses timelines by the pair they were computed from
    /// (`timeline-<from>-<to>.bin`), so what this needs to be is *ordered and unique*, which a
    /// digest is not. The digest of what a snapshot contains is
    /// [`Skeleton::digest`](crate::Skeleton::digest) and its siblings.
    pub snapshot_id: u64,
    /// The commit the manifest behind it was extracted from (`F-EXT-7`).
    ///
    /// `None` for a repository with no commits or no `.git` — the same absence
    /// [`Manifest::built_from_commit`](crate::Manifest::built_from_commit) carries, passed
    /// through rather than defaulted.
    pub built_from: Option<CommitId>,
    /// The structural skeleton — `F-SKEL-1`…`F-SKEL-7`.
    pub skeleton: Skeleton,
    /// What each node is made of — `F-MAT-1`…`F-MAT-4`, `F-MAT-6`.
    pub materials: MaterialMap,
    /// What is built on each node — `F-MAT-5`.
    pub enrichments: EnrichmentMap,
}

impl WorldSnapshot {
    /// Whether the three maps agree about how many nodes there are.
    ///
    /// True by construction for anything the pipeline built, since each pass walks the
    /// skeleton's nodes in order. It is checkable rather than assumed because the consumer is
    /// a renderer that indexes all three by the same [`NodeId`](crate::NodeId), and an
    /// off-by-one there is a silently wrong picture rather than a crash.
    #[must_use]
    pub fn is_covered(&self) -> bool {
        self.materials.covers(&self.skeleton)
            && self.enrichments.len() == self.skeleton.nodes().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregate::AggregateNode;
    use crate::enrichment::Enrichment;
    use crate::material::{Composition, Material, MaterialFamily, Mosaic};
    use crate::path::RepoPath;
    use crate::segment::{NodeRole, Point};
    use alloc::vec;
    use treepo_det::{Angle, Fx, OrderedMap, Seed};

    fn material() -> Material {
        Material {
            family: MaterialFamily::Heartwood,
            composition: Composition::Pure,
            budget: Fx::from_ratio(1, 2),
            mosaic: Mosaic::new(OrderedMap::new(), 0),
            gradient: None,
            stress: None,
        }
    }

    fn one_node() -> Skeleton {
        let mut skeleton = Skeleton::new();
        skeleton.push_node(
            None,
            Point::ORIGIN,
            Angle::ZERO,
            Seed::root(b"test"),
            NodeRole::Limb {
                path: RepoPath::root(),
            },
        );
        skeleton
    }

    #[test]
    fn a_snapshot_built_from_one_walk_covers_itself() {
        let skeleton = one_node();
        let mut materials = MaterialMap::new();
        materials.push(material());
        let mut enrichments = EnrichmentMap::new();
        enrichments.push(Enrichment::none());

        let snapshot = WorldSnapshot {
            snapshot_id: 0,
            built_from: None,
            skeleton,
            materials,
            enrichments,
        };
        assert!(snapshot.is_covered());
    }

    /// The failure this exists to catch: a pass that skipped a node leaves the renderer
    /// indexing material by an id the material map does not have.
    #[test]
    fn a_snapshot_missing_a_material_does_not_cover_itself() {
        let mut skeleton = one_node();
        skeleton.push_node(
            None,
            Point::ORIGIN,
            Angle::ZERO,
            Seed::root(b"test"),
            NodeRole::Aggregate(AggregateNode {
                anchor: RepoPath::root(),
                index: 0,
                members: vec![],
                bytes: 0,
                file_count: 0,
                dir_count: 0,
            }),
        );

        let mut materials = MaterialMap::new();
        materials.push(material());
        let mut enrichments = EnrichmentMap::new();
        enrichments.push(Enrichment::none());
        enrichments.push(Enrichment::none());

        let snapshot = WorldSnapshot {
            snapshot_id: 0,
            built_from: None,
            skeleton,
            materials,
            enrichments,
        };
        assert!(!snapshot.is_covered());
    }
}
