//! Committed snapshot → the renderer's chunk plan (architecture D4).
//!
//! One system, one direction. It reads [`CommittedWorld`] and hands the renderer a
//! [`TreePlan`]; nothing here writes back, and nothing here decides *what* the tree looks like
//! — the snapshot already settled that, off-thread, in crates that cannot see a `World`.
//!
//! # It no longer spawns geometry, and that is D5 arriving
//!
//! Until the static bake this system built one mesh for the whole tree and spawned it. Now it
//! cuts the skeleton into subtree-anchored chunks and stops: which of those chunks is in memory
//! is a question about where the camera is, and `treepo-render`'s `stream` answers it every
//! frame. The split is the same one D4 draws — the app owns *what is committed*, the renderer
//! owns *what is resident* — and it is what keeps a repository's size out of the frame loop.
//!
//! # Replacement rather than reconciliation, for now
//!
//! A committed snapshot replaces the plan wholesale, and [`TreePlan`]'s generation counter
//! makes every chunk baked from the old one stale. That is the crude version of what this file
//! is named for: `AC-GROW-4` wants a one-file change to touch one limb, which means diffing two
//! snapshots and re-baking only the chunks whose anchors are on the changed path. The chunk
//! identity that makes it possible now exists; the diff that drives it is `treepo-grow`'s, in
//! Phase 6. Until both are here the honest implementation is the one that cannot be subtly
//! wrong — and it is not yet costly, because this runs once per commit and this slice commits
//! once.

use bevy::prelude::*;
use treepo_render::{FrameTarget, TreePlan};

use crate::phase::CommittedWorld;

/// Keeps the renderer's plan matching the committed snapshot.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct SnapshotSyncPlugin;

impl Plugin for SnapshotSyncPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, sync.run_if(resource_changed::<CommittedWorld>));
    }
}

fn sync(world: Res<CommittedWorld>, mut plan: ResMut<TreePlan>, mut frame: ResMut<FrameTarget>) {
    let Some(snapshot) = world.snapshot() else {
        return;
    };

    plan.commit(snapshot.clone());
    debug!("committed world cut into {} chunks", plan.chunks().len());

    // Frame the tree the first time it appears. A repository that draws nothing — `AC-SKEL-2`'s
    // empty one, before the root cluster has geometry — leaves the camera where it was rather
    // than being framed on a point, which would zoom to infinity.
    //
    // The extent comes from the cut rather than from the skeleton directly, so the camera and
    // the bake agree by construction about how far the tree reaches: framing on a rectangle the
    // chunks do not cover would leave a visible margin that never fills in.
    if let Some(extent) = plan.chunks().extent() {
        frame.0 = Some(extent);
    }
}
