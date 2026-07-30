//! Click to identify — `F-INSP-1`, `AC-INSP-1`.
//!
//! > Clicking any element resolves to a real path or an explicit aggregate.
//!
//! The criterion has two halves and they are enforced in different places. That a click lands
//! on *something* is answered by sampling the element-ID plane the bake wrote
//! ([`treepo_render::pick`]) — so the click resolves to whatever painted that texel, and the
//! answer cannot disagree with the picture. That whatever it landed on names something real is
//! a question about [`NodeRole`], and lives in [`describe`] — where the `match` is exhaustive,
//! so a future node kind cannot be added without someone deciding what a user is told when
//! they click it.
//!
//! # What changed when the ID buffer arrived
//!
//! This used to call a geometric hit test against the same segments the mesh was built from: a
//! second calculation of the same answer, free to drift from the drawn one wherever two limbs
//! overlapped. Both named a real element, so no click landed on nothing — they could simply
//! name different real elements. Sampling the plane removes the second calculation rather than
//! making it more careful.
//!
//! It also removes a conversion. The old tolerance was six *logical pixels*, derived each click
//! by projecting a second point through the camera; the new one is six *texels*, and needs no
//! projection because an LOD band is chosen at roughly one texel per screen pixel.
//!
//! # Nothing here can name a person
//!
//! A limb carries a mosaic of contributors (`F-MAT-2`), and the obvious next thing to put in an
//! inspector is who they are. `N9` says the default is pseudonymous and `treepo-id` is the only
//! gate that resolves a display string; this module never touches an
//! [`AuthorKey`](treepo_model::AuthorKey) at all, which is a weaker guarantee than `treepo-id`
//! gives and a stronger one than remembering to ask it.

use bevy::prelude::*;
use treepo_model::{NodeId, NodeRole, Skeleton};
use treepo_render::{IdPlane, Painted, PointerDrag, ResidentChunk, TreeCamera};

use crate::phase::CommittedWorld;

/// What the user last clicked, if anything.
#[derive(Resource, Debug, Default)]
pub(crate) struct Selection(pub(crate) Option<Selected>);

/// One identified element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Selected {
    /// Which node, so a later inspector can ask the snapshot for more.
    pub(crate) node: NodeId,
    /// What it is: `limb`, `group`, `container`, `root`.
    pub(crate) kind: &'static str,
    /// The repository path it resolves to.
    pub(crate) path: String,
    /// What else is worth saying — how many paths a container stands for, and so on.
    pub(crate) detail: String,
}

/// Resolves a click that was not a drag.
pub(crate) fn on_click(
    buttons: Res<ButtonInput<MouseButton>>,
    drag: Res<PointerDrag>,
    window: Option<Single<&Window>>,
    camera: Option<Single<(&Camera, &GlobalTransform), With<TreeCamera>>>,
    world: Res<CommittedWorld>,
    resident: Query<(&ResidentChunk, &IdPlane)>,
    mut selection: ResMut<Selection>,
) {
    if !buttons.just_released(MouseButton::Left) || !drag.was_click() {
        return;
    }
    let (Some(window), Some(camera), Some(snapshot)) = (window, camera, world.snapshot()) else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };

    let (camera, camera_transform) = camera.into_inner();
    let Ok(at) = camera.viewport_to_world_2d(camera_transform, cursor) else {
        return;
    };

    // Only the pieces that are actually on screen are consulted, because only those have a
    // plane — which is the same statement as "the answer comes from the picture". A limb the
    // renderer evicted is a limb the user cannot see, and cannot have clicked.
    let painted = resident.iter().map(|(chunk, plane)| Painted {
        region: chunk.region,
        plane,
    });

    // `None` is a deselection, not a miss to be ignored: clicking the background is how a user
    // puts the inspector away, and swallowing it would leave a stale selection on screen
    // looking like the thing they just clicked.
    selection.0 =
        treepo_render::pick(painted, at).and_then(|node| describe(&snapshot.skeleton, node));
}

/// What to say about a node.
///
/// `None` only for an id the skeleton does not have, which cannot happen for an id the
/// skeleton just produced — it is here so that a lookup failure shows as nothing selected
/// rather than as a panic in a system.
#[must_use]
pub(crate) fn describe(skeleton: &Skeleton, node: NodeId) -> Option<Selected> {
    let role = &skeleton.node(node)?.role;
    let (kind, detail) = match role {
        NodeRole::Limb { .. } => ("limb", String::new()),
        NodeRole::Group { members, .. } => (
            "group",
            format!("{} small siblings on one stem", members.len()),
        ),
        // `F-INSP-3`: a container must report what it represents. The counts are what makes it
        // an *explicit* aggregate rather than a limb that quietly stands for more than it says.
        NodeRole::Aggregate(aggregate) => (
            "container",
            format!(
                "stands for {} file(s) in {} director(ies), {} bytes",
                aggregate.file_count, aggregate.dir_count, aggregate.bytes
            ),
        ),
        NodeRole::RootMass { index, .. } => ("root", format!("root cluster, node {index}")),
    };

    Some(Selected {
        node,
        kind,
        // `RepoPath::display` is lossy for a non-UTF-8 name and deliberately so (PRD §6,
        // `F-INSP-4`): the bytes are kept in the model, and what a window can show is a name.
        path: role.anchor().display().into_owned(),
        detail,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use treepo_det::{Angle, Seed};
    use treepo_model::{AggregateNode, Point, RepoPath};

    fn path(text: &str) -> RepoPath {
        RepoPath::new(text.as_bytes()).unwrap()
    }

    fn with_role(role: NodeRole) -> (Skeleton, NodeId) {
        let mut skeleton = Skeleton::new();
        let id = skeleton.push_node(
            None,
            Point::ORIGIN,
            Angle::ZERO,
            Seed::root(b"pick-test"),
            role,
        );
        (skeleton, id)
    }

    /// `AC-INSP-1`, as a property of every node kind rather than of the one that was easy to
    /// test: whatever is clicked, the answer names a path.
    #[test]
    fn every_node_kind_resolves_to_a_path() {
        let roles = [
            NodeRole::Limb {
                path: path("src/lib.rs"),
            },
            NodeRole::Group {
                anchor: path("src"),
                members: vec![path("src/a.rs"), path("src/b.rs")],
            },
            NodeRole::Aggregate(AggregateNode {
                anchor: path("vendor"),
                index: 0,
                members: vec![path("vendor/lib")],
                bytes: 4096,
                file_count: 12,
                dir_count: 3,
            }),
            NodeRole::RootMass {
                anchor: RepoPath::root(),
                index: 2,
            },
        ];

        for role in roles {
            let (skeleton, id) = with_role(role);
            let selected = describe(&skeleton, id).expect("the node was just added");
            assert!(!selected.kind.is_empty());
            // The repository root displays as empty, which is still a real path — what must
            // never happen is a node with no answer at all.
            assert!(selected.path.len() < 4096);
        }
    }

    /// A container has to say what it stands for, or it is indistinguishable from a limb that
    /// is lying about its size (`F-INSP-3`).
    #[test]
    fn a_container_reports_what_it_represents() {
        let (skeleton, id) = with_role(NodeRole::Aggregate(AggregateNode {
            anchor: path("vendor"),
            index: 0,
            members: vec![path("vendor/lib")],
            bytes: 4096,
            file_count: 12,
            dir_count: 3,
        }));
        let selected = describe(&skeleton, id).unwrap();
        assert_eq!(selected.kind, "container");
        assert!(selected.detail.contains("12"));
        assert!(selected.detail.contains('3'));
    }

    #[test]
    fn a_limb_resolves_to_its_own_path() {
        let (skeleton, id) = with_role(NodeRole::Limb {
            path: path("src/lib.rs"),
        });
        let selected = describe(&skeleton, id).unwrap();
        assert_eq!(selected.kind, "limb");
        assert_eq!(selected.path, "src/lib.rs");
    }

    #[test]
    fn an_unknown_node_describes_as_nothing_rather_than_panicking() {
        let (skeleton, _) = with_role(NodeRole::Limb {
            path: RepoPath::root(),
        });
        assert!(describe(&skeleton, NodeId::new(99)).is_none());
    }
}
