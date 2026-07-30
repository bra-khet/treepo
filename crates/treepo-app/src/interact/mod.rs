//! Turning input into questions about the tree.
//!
//! `F-NAV-6`'s search and `F-INSP-3`/`F-INSP-5`'s drill-down and why-panel land here later, as
//! `search.rs` and `inspect.rs`. What exists now is the one interaction M1 has to have:
//! [`pick`], which answers `AC-INSP-1` — every click resolves to a real path or an explicit
//! aggregate.

pub(crate) mod pick;

use bevy::prelude::*;
use treepo_render::CameraSystems;

pub(crate) use pick::Selection;

/// Registers the interactions.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct InteractPlugin;

impl Plugin for InteractPlugin {
    fn build(&self, app: &mut App) {
        // After the camera, because a click is defined as "a press that did not become a
        // drag", and the camera is what measures the drag. Running before it would read last
        // frame's travel and turn the end of every pan into a selection.
        app.init_resource::<Selection>()
            .add_systems(Update, pick::on_click.after(CameraSystems));
    }
}
