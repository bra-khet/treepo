//! The product UI surface — Bevy UI, not egui (architecture D8).
//!
//! > Shipped UI in Bevy UI with a bespoke theme. The dev/QA surface in `bevy_egui`, compiled
//! > only in dev builds. `R1` sets a consumer polish bar; egui's default look reads
//! > unmistakably as a developer tool and would undercut it.
//!
//! What is here is not that surface. It is three lines of text over the tree, in Bevy UI
//! because starting in the right toolkit costs nothing and starting in the wrong one costs a
//! rewrite. `theme.rs`, `onboarding.rs` and `progress.rs` — the files this phase is actually
//! specified to produce — are the surface, and each of them is a design question rather than a
//! wiring question.
//!
//! # The three lines, and why each is there
//!
//! * **Status** — what was opened, whether it came from the store, how big it is, and every
//!   `F-ASSOC-2` notice. The notices are the load-bearing part: PRD §6 says a shallow clone
//!   must be *told*, because it grows an ageless tree that otherwise reads as a defect.
//! * **Selection** — `AC-INSP-1`'s answer, which has to be visible to be an answer at all.
//! * **Hints** — the three gestures. `AC-NAV-1` gives a participant thirty seconds to find a
//!   directory by eye; thirty seconds spent discovering that the wheel zooms is thirty seconds
//!   not spent looking.

use bevy::prelude::*;

use crate::interact::Selection;
use crate::phase::{CommittedWorld, PhaseFailure, PhaseState};

/// The line describing the repository.
#[derive(Component, Debug, Clone, Copy)]
struct StatusLine;

/// The line describing what was clicked.
#[derive(Component, Debug, Clone, Copy)]
struct SelectionLine;

/// Draws the text over the tree.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn).add_systems(
            Update,
            (
                status.run_if(
                    resource_changed::<CommittedWorld>
                        .or_else(resource_changed::<PhaseFailure>)
                        .or_else(state_changed::<PhaseState>),
                ),
                selection.run_if(resource_changed::<Selection>),
            ),
        );
    }
}

/// Ink, chosen to stay legible over both the darkest and the palest material.
const INK: Color = Color::srgb(0.92, 0.91, 0.87);
/// Ink for the hint line, quieter than the rest.
const FAINT: Color = Color::srgb(0.58, 0.57, 0.54);

fn spawn(mut commands: Commands) {
    commands.spawn((
        StatusLine,
        Text::new("opening…"),
        TextFont::from_font_size(14.0),
        TextColor(INK),
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            left: px(14),
            ..default()
        },
    ));

    commands.spawn((
        SelectionLine,
        Text::new(String::new()),
        TextFont::from_font_size(14.0),
        TextColor(INK),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(34),
            left: px(14),
            ..default()
        },
    ));

    commands.spawn((
        Text::new("drag to pan   ·   scroll to zoom   ·   click to identify"),
        TextFont::from_font_size(12.0),
        TextColor(FAINT),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(12),
            left: px(14),
            ..default()
        },
    ));
}

fn status(
    world: Res<CommittedWorld>,
    failure: Res<PhaseFailure>,
    state: Res<State<PhaseState>>,
    mut text: Query<&mut Text, With<StatusLine>>,
) {
    let next = match (state.get(), &failure.0, world.summary()) {
        (PhaseState::Failed, Some(message), _) => format!("could not open — {message}"),
        (_, _, Some(summary)) => {
            let source = if summary.from_cache {
                "from store"
            } else {
                "extracted"
            };
            let mut line = format!(
                "{}   ·   {} ({source})   ·   {} paths → {} nodes, {} segments, {} containers",
                summary.root.display(),
                summary.tier,
                summary.paths,
                summary.nodes,
                summary.segments,
                summary.aggregates
            );
            for notice in &summary.notices {
                line.push_str("\n! ");
                line.push_str(notice);
            }
            line
        }
        _ => "opening…".to_owned(),
    };

    for mut line in &mut text {
        if line.0 != next {
            line.0.clone_from(&next);
        }
    }
}

fn selection(selection: Res<Selection>, mut text: Query<&mut Text, With<SelectionLine>>) {
    let next = match &selection.0 {
        None => String::new(),
        Some(selected) if selected.detail.is_empty() => {
            format!("{}  {}", selected.kind, display_path(&selected.path))
        }
        Some(selected) => format!(
            "{}  {}  —  {}",
            selected.kind,
            display_path(&selected.path),
            selected.detail
        ),
    };

    for mut line in &mut text {
        if line.0 != next {
            line.0.clone_from(&next);
        }
    }
}

/// The repository root displays as an empty string, which reads as a missing value rather than
/// as the root. Naming it is the difference between "nothing" and "the repository itself".
fn display_path(path: &str) -> &str {
    if path.is_empty() {
        "<repository root>"
    } else {
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_repository_root_is_named_rather_than_shown_as_nothing() {
        assert_eq!(display_path(""), "<repository root>");
        assert_eq!(display_path("src/lib.rs"), "src/lib.rs");
    }
}
