//! Committed snapshot → ECS entities (architecture D4).
//!
//! One system, one direction. It reads [`CommittedWorld`] and makes the world on screen match
//! it; nothing here writes back, and nothing here decides *what* the tree looks like — the
//! snapshot already settled that, off-thread, in crates that cannot see a `World`.
//!
//! # Replacement rather than reconciliation, for now
//!
//! A committed snapshot despawns the previous geometry and spawns new geometry. That is the
//! crude version of what this file is named for: `AC-GROW-4` wants a one-file change to touch
//! one limb, which means diffing two snapshots and updating the entities that differ. Doing
//! that needs `treepo-grow`'s diff (Phase 6) and chunked geometry to update *part* of (D5), and
//! neither exists — so the honest implementation today is the one that cannot be subtly wrong.
//!
//! It is also not yet costly: this runs once per commit, and this slice commits once.

use bevy::prelude::*;
use treepo_render::{Extent, FrameTarget, tree_mesh};

use crate::phase::CommittedWorld;

/// Marks everything spawned from the committed snapshot, so it can be replaced as a unit.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct TreeGeometry;

/// Keeps the drawn world matching the committed snapshot.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct SnapshotSyncPlugin;

impl Plugin for SnapshotSyncPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, sync.run_if(resource_changed::<CommittedWorld>));
    }
}

fn sync(
    mut commands: Commands,
    world: Res<CommittedWorld>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut frame: ResMut<FrameTarget>,
    drawn: Query<Entity, With<TreeGeometry>>,
) {
    let Some(snapshot) = world.snapshot() else {
        return;
    };

    for entity in &drawn {
        commands.entity(entity).despawn();
    }

    // `ColorMaterial::default()` is white and untextured, so what reaches the screen is the
    // mesh's own vertex colours — which is where `materialize`'s output actually lands.
    // `assets/shaders/tree_static.wgsl` is what replaces it, and a real material is the point
    // at which the placeholder palette in `treepo-render::mesh` stops being defensible.
    commands.spawn((
        TreeGeometry,
        Mesh2d(meshes.add(tree_mesh(&snapshot.skeleton, &snapshot.materials))),
        MeshMaterial2d(materials.add(ColorMaterial::default())),
    ));

    // Frame the tree the first time it appears. A repository that draws nothing — `AC-SKEL-2`'s
    // empty one, before the root cluster has geometry — leaves the camera where it was rather
    // than being framed on a point, which would zoom to infinity.
    if let Some(extent) = Extent::of(&snapshot.skeleton) {
        frame.0 = Some(extent);
    }
}
