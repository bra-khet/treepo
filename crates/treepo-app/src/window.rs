//! The window — `F-WIN-1`.
//!
//! `F-WIN-2`'s cinema mode arrives with playback (Phase 7) and `F-WIN-3`'s widget mode with
//! packaging (Phase 12, and the designated cut line if M3 overruns). What is here is the
//! ordinary window and the plugin set the rest of the app is built on.
//!
//! # The plugin set is deliberately narrow
//!
//! `bevy`'s default features carry audio and the entire 3-D stack; the workspace manifest asks
//! for `2d` and `ui` and nothing else. That is `N2`'s argument applied one level out from the
//! network ban: the way to be sure a capability is not reachable is for it not to be linked.
//! treepo draws a still 2-D picture and plays no sound, so a glTF loader and an audio device in
//! the binary would be surface with no user behind it.

use bevy::prelude::*;
use bevy::window::WindowResolution;

/// The colour behind the tree.
///
/// Dark, because every material family in `treepo-render::mesh` is a mid-tone and a pale
/// background would leave the silhouette — the thing `AC-NAV-1` asks a participant to read —
/// competing with the page it is drawn on.
const BACKGROUND: Color = Color::srgb(0.06, 0.06, 0.07);

/// The default window size.
///
/// `NFR-*` sets no window size, so this is a choice: large enough that a T2 tree is legible at
/// the far zoom `AC-NAV-1` starts from, small enough to open on a 1366×768 laptop.
const SIZE: (u32, u32) = (1280, 800);

/// The Bevy plugins the app runs on, with treepo's window.
#[must_use]
pub(crate) fn plugins() -> impl PluginGroup {
    DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "treepo".to_owned(),
            resolution: WindowResolution::new(SIZE.0, SIZE.1),
            ..default()
        }),
        ..default()
    })
}

/// The clear colour, inserted alongside [`plugins`].
#[must_use]
pub(crate) const fn background() -> ClearColor {
    ClearColor(BACKGROUND)
}
