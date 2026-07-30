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
//! # What this crate is not, yet
//!
//! Architecture D5's static bake — chunked layer textures per LOD band plus a parallel
//! element-ID buffer — is what `bake.rs`, `chunk.rs`, `lod.rs` and `id_buffer.rs` will be, and
//! it is what `NFR-2` and `AC-NAV-2` depend on. [`mesh`] and [`pick`] each carry a header
//! saying what they stand in for and where they will disagree with the real thing.

#![forbid(unsafe_code)]
// The workspace turns on `rust_2018_idioms`, which includes `elided_lifetimes_in_paths`. Every
// Bevy system parameter is lifetime-generic — `Commands<'w, 's>`, `Query<'w, 's, D, F>`,
// `Single<'w, D>` — so honouring it means writing `Commands<'_, '_>` in every signature in the
// crate. That is noise on the one thing a reader most needs to see at a glance, and it buys
// nothing here: the lint exists to make *borrowing* visible, and a system parameter is not a
// borrow the reader can act on. Allowed in the two Bevy crates only; the generative set keeps
// the full idiom lints.
#![allow(elided_lifetimes_in_paths)]

pub mod camera;
pub mod mesh;
pub mod pick;

use bevy::prelude::*;

pub use camera::{CameraSystems, FrameTarget, PointerDrag, TreeCamera};
pub use mesh::{Extent, family_color, tree_mesh};
pub use pick::pick_node;

/// Spawns the camera and runs the navigation gestures.
///
/// Deliberately does *not* spawn anything for a tree. What is on screen follows from the
/// committed snapshot, and reconciling ECS entities to a snapshot is `snapshot_sync`'s job in
/// `treepo-app` (architecture D4) — a render plugin that also owned the world's contents would
/// be a second place the two could disagree about what is committed.
#[derive(Debug, Default, Clone, Copy)]
pub struct TreepoRenderPlugin;

impl Plugin for TreepoRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FrameTarget>()
            .init_resource::<PointerDrag>()
            .add_systems(Startup, camera::spawn)
            .add_systems(
                Update,
                (camera::frame, camera::pan, camera::zoom).in_set(CameraSystems),
            );
    }
}
