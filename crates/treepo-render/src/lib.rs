//! The Bevy render layer: geometry a [`Skeleton`](treepo_model::Skeleton) can be drawn from,
//! a camera to look at it with, and an answer to "what did I click".
//!
//! One of exactly two crates in the workspace that may name `bevy` — the other is
//! `treepo-app`. That is architecture D1, the document's own "single most load-bearing
//! decision", and `cargo xtask dep-guard` fails the build if a generative crate acquires the
//! dependency. Read from that side: everything in here is allowed to be a rendering decision
//! precisely because nothing in here can reach the pipeline that produced the tree.
//!
//! # Floats are permitted here, and nowhere upstream
//!
//! `treepo-det`, `treepo-model` and `treepo-gen` all deny `clippy::float_arithmetic`, because a
//! float computed from a platform `libm` is a coordinate that differs by machine and
//! `AC-DET-2` requires three platforms to agree. That constraint ends here: a vertex buffer
//! takes `f32`, a GPU is not bit-identical across vendors anyway (architecture D6/E1), and the
//! determinism boundary is drawn at the data this crate *receives*. Every conversion is one
//! direction — fixed-point in, `f32` out — and no value computed here flows back.
//!
//! # The static bake
//!
//! Architecture D5 is here in full: [`chunk`] cuts the tree into subtree-anchored pieces and
//! keeps the visible ones resident within a texel budget, [`bake`] rasterizes a piece into a
//! layer texture *and* a parallel element-ID plane, [`lod`] quantizes the density it is baked
//! at, and [`id_buffer`] answers "what did I click" by sampling that plane. Together they make
//! `NFR-2` a property rather than a hope — frame cost follows the viewport, not the repository
//! — and `N7`/`P1` a number rather than an argument.

#![forbid(unsafe_code)]
// The workspace turns on `rust_2018_idioms`, which includes `elided_lifetimes_in_paths`. Every
// Bevy system parameter is lifetime-generic — `Commands<'w, 's>`, `Query<'w, 's, D, F>`,
// `Single<'w, D>` — so honouring it means writing `Commands<'_, '_>` in every signature in the
// crate. That is noise on the one thing a reader most needs to see at a glance, and it buys
// nothing here: the lint exists to make *borrowing* visible, and a system parameter is not a
// borrow the reader can act on. Allowed in the two Bevy crates only; the generative set keeps
// the full idiom lints.
#![allow(elided_lifetimes_in_paths)]

pub mod bake;
pub mod camera;
pub mod chunk;
pub mod id_buffer;
pub mod lod;
pub mod surface;

use bevy::prelude::*;

pub use bake::{AuthorPalette, Layer};
pub use camera::{CameraSystems, FrameTarget, PointerDrag, TreeCamera};
pub use chunk::{
    BakeLoad, BakeQueue, Chunk, ChunkId, ChunkSet, Extent, Piece, ResidentChunk, TreePlan, pieces,
};
pub use id_buffer::{Coverage, ElementId, IdPlane, Painted, coverage, pick, unresolved};
pub use lod::Band;
pub use surface::{Surface, family_color};

/// Spawns the camera, runs the navigation gestures, and keeps the baked layers resident.
///
/// Deliberately does *not* decide what the tree is. What is on screen follows from the
/// committed snapshot, and cutting one into a [`TreePlan`] is `snapshot_sync`'s job in
/// `treepo-app` (architecture D4) — a render plugin that also owned the world's contents would
/// be a second place the two could disagree about what is committed. What this plugin owns is
/// the other half: given a plan, which pieces of it are in memory right now.
#[derive(Debug, Default, Clone, Copy)]
pub struct TreepoRenderPlugin;

impl Plugin for TreepoRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FrameTarget>()
            .init_resource::<PointerDrag>()
            .init_resource::<TreePlan>()
            .init_resource::<AuthorPalette>()
            .init_resource::<BakeLoad>()
            .init_resource::<BakeQueue>()
            .add_systems(Startup, camera::spawn)
            .add_systems(
                Update,
                (camera::frame, camera::pan, camera::zoom).in_set(CameraSystems),
            )
            // After the camera, so residency is decided from where the camera *is* rather
            // than from where it was last frame. A zoom that outruns the bake shows the
            // previous band stretched for a frame, which is a blur; deciding on stale input
            // shows a hole, which is a missing limb.
            .add_systems(Update, chunk::stream.after(CameraSystems));
    }
}
