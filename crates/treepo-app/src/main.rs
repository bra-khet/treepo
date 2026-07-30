//! treepo — a repository, grown.
//!
//! ```text
//! cargo run -p treepo-app -- <path-to-repository>   # or no argument for the current directory
//! cargo run -p treepo-app --features brp -- <path>  # + agent control on localhost:15702 (D10)
//! ```
//!
//! Agents needing BRP: launch with the second line via shell — **not** `bevy_brp` MCP
//! `brp_launch` (it rebuilds without `--features brp` and can strip the listener). Details in
//! [`debug::brp`](crate::debug).
//!
//! # What this binary is, at M1
//!
//! One of exactly two crates that may name `bevy`. Everything it draws was computed by crates
//! that cannot — extraction, skeleton, material and enrichment all happen behind
//! [`load::open`], on a background thread, in `no_std` crates with no `World` in scope. That is
//! architecture D1, and `cargo xtask dep-guard` is what notices if it stops being true.
//!
//! The shell is honest about which of Phase 5's end conditions it does *not* meet. Two are not
//! attempted here and each has a named home:
//!
//! * **`P1`/`N7`/`xtask id-coverage`** need the element-ID buffer. `treepo-render::pick` answers
//!   clicks geometrically instead, which is `AC-INSP-1` without the machine-checkable half.
//! * **`AC-NAV-1`** is a recorded user test with three participants, which needs materials to
//!   have an appearance first.
//!
//! `AC-NAV-2` and `NFR-2` now have their mechanism — architecture D5's chunked bake, in
//! `treepo-render::{chunk, bake, lod}` — and what they still want is a measurement on a T3
//! repository rather than more code.
//!
//! `NFR-4`'s five-second cold launch on a cached T2 repository is met by the store path in
//! [`load`], and `AC-MAN-2` is unaffected: nothing here opens a repository for write.

// See `treepo-render`'s crate header: `elided_lifetimes_in_paths` and Bevy's lifetime-generic
// system parameters do not get along, and this is one of the two crates that has them.
#![allow(elided_lifetimes_in_paths)]

mod debug;
mod interact;
mod load;
mod phase;
mod snapshot_sync;
mod ui;
mod window;

use std::path::PathBuf;

use bevy::prelude::*;

fn main() {
    let mut app = App::new();

    app.add_plugins(window::plugins())
        .insert_resource(window::background())
        .insert_resource(phase::RepositoryRequest(requested_path()))
        .add_plugins((
            treepo_render::TreepoRenderPlugin,
            phase::PhasePlugin,
            snapshot_sync::SnapshotSyncPlugin,
            interact::InteractPlugin,
            ui::UiPlugin,
        ));

    // The only `brp` reference outside `debug/brp.rs` and the manifest. A default build does
    // not compile the module, does not link `bevy_remote`, and opens no socket (D10, RISK-D).
    #[cfg(feature = "brp")]
    debug::brp::register(&mut app);

    app.run();
}

/// The repository to open: the first argument, or the current directory.
///
/// Defaulting to the current directory rather than refusing is what makes `cargo run -p
/// treepo-app` do something on its own — treepo's own repository is a real T1 subject, and an
/// app that needs an argument before it will show anything is an app nobody runs by accident.
///
/// `F-ASSOC-1`'s picker is the product's way in and it replaces this as the *normal* path, not
/// as the only one: a developer pointing the shell at a specific fixture from a terminal is a
/// permanent need, and `R1` says no *essential* flow requires a terminal, not that none may.
fn requested_path() -> PathBuf {
    std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
}
