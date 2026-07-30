//! ★ The phase boundary — architecture D1, and the reason the workspace is shaped as it is.
//!
//! > If Grow is a Bevy state mutating the same `World`, nothing prevents a future contributor
//! > from adding a scan to a system that happens to run in that state — the constraint survives
//! > only as discipline. With Grow in a crate that has no Bevy types at all, the violation does
//! > not compile.
//!
//! This module is the *app side* of that. It owns [`PhaseState`], it owns the background task,
//! and it owns [`CommittedWorld`] — the one place a committed [`WorldSnapshot`] lives. What it
//! never does is compute one: the whole of that is [`load::open`](crate::load::open), a plain
//! blocking function in a module that cannot see a `World`, and this module's contribution is
//! to decide *when* it runs and *on which thread*.
//!
//! # Publication is by replacement, not by mutation
//!
//! Architecture D4 makes Grow publish an `Arc<WorldSnapshot>` into an `ArcSwap`, so that Thrive
//! can never observe a half-built tree — a partially constructed snapshot is not reachable, and
//! cancellation is expressed by never publishing one. Here the same property comes from a
//! resource holding an `Arc`, swapped whole on the main thread when the task reports. That is
//! weaker than `ArcSwap` in exactly one way, and the way does not bite yet: nothing else reads
//! the snapshot concurrently, because the only producer is a task whose result is applied by a
//! system. Phase 7's `grow_task` is where a producer publishes while Thrive is reading, and
//! that is when the swap has to become atomic rather than merely exclusive.
//!
//! # `PhaseState` is smaller than it will be
//!
//! D1 names `Idle | Computing | Playing | …` and D11 adds a stage stack on top. Two of those
//! have nothing to do yet: nothing plays, and nothing stages. Adding the variants now would
//! put states in the app that no system can enter, which reads as capability rather than as
//! scaffolding.

use std::path::PathBuf;
use std::sync::Arc;

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, futures::check_ready};
use treepo_model::WorldSnapshot;

use crate::load::{self, Opened, Summary};

/// Where the application is in the Grow/Thrive cycle.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) enum PhaseState {
    /// A repository is being opened on a background thread.
    #[default]
    Opening,
    /// A snapshot is committed and the world is on screen.
    Thriving,
    /// Opening failed and there is nothing to draw.
    ///
    /// A terminal state for this slice. `F-ASSOC-*` makes it recoverable — the user picks
    /// another repository — and that needs a picker, which is onboarding rather than the shell.
    Failed,
}

/// Which repository the app was asked to open.
///
/// Set from the command line before the app runs. `F-ASSOC-1`'s picker replaces this as the
/// *normal* way in; the argument stays, because an app a developer cannot point at a specific
/// repository from a shell is an app that is tedious to work on.
#[derive(Resource, Debug, Clone)]
pub(crate) struct RepositoryRequest(pub(crate) PathBuf);

/// The last committed view of the repository (architecture D4).
///
/// `None` until the first snapshot lands. Everything on screen is derived from this, and
/// nothing writes to it except [`commit`].
#[derive(Resource, Debug, Default)]
pub(crate) struct CommittedWorld {
    snapshot: Option<Arc<WorldSnapshot>>,
    summary: Option<Summary>,
}

impl CommittedWorld {
    /// The committed snapshot, if one has been published.
    #[must_use]
    pub(crate) fn snapshot(&self) -> Option<&Arc<WorldSnapshot>> {
        self.snapshot.as_ref()
    }

    /// What the UI says about the repository behind it.
    #[must_use]
    pub(crate) fn summary(&self) -> Option<&Summary> {
        self.summary.as_ref()
    }
}

/// Why opening failed, for the UI to show.
///
/// A resource rather than a log line: `R1` says no essential flow requires a terminal, and a
/// window that goes black because a path was wrong is a window with no way to say so.
#[derive(Resource, Debug, Default)]
pub(crate) struct PhaseFailure(pub(crate) Option<String>);

/// The running open, held while it runs.
#[derive(Resource)]
struct OpenTask(Task<Result<Opened, load::OpenError>>);

/// Wires the phase machine and the background open.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct PhasePlugin;

impl Plugin for PhasePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<PhaseState>()
            .init_resource::<CommittedWorld>()
            .init_resource::<PhaseFailure>()
            .add_systems(Startup, start)
            .add_systems(Update, poll.run_if(in_state(PhaseState::Opening)));
    }
}

/// Starts the open on the async compute pool.
///
/// On the pool rather than inline for the reason `AC-GROW-1` will later insist on: a T2
/// extraction is seconds and a T3 one is minutes, and a window that does not paint until it
/// finishes is a window the operating system offers to kill. The frame loop runs from the
/// first frame; what is missing until this completes is a tree, not an application.
fn start(mut commands: Commands, request: Option<Res<RepositoryRequest>>) {
    let Some(request) = request else {
        commands.insert_resource(PhaseFailure(Some(
            "no repository to open — pass a path on the command line".to_owned(),
        )));
        return;
    };

    let path = request.0.clone();
    let task = AsyncComputeTaskPool::get().spawn(async move { load::open(&path) });
    commands.insert_resource(OpenTask(task));
}

/// Publishes the snapshot when the open finishes.
fn poll(
    mut commands: Commands,
    task: Option<ResMut<OpenTask>>,
    mut world: ResMut<CommittedWorld>,
    mut failure: ResMut<PhaseFailure>,
    mut next: ResMut<NextState<PhaseState>>,
) {
    // No task means `start` never made one — the path was missing, and it already said so.
    let Some(mut task) = task else {
        if failure.0.is_some() {
            next.set(PhaseState::Failed);
        }
        return;
    };

    // `check_ready` rather than blocking on the future: polling with `block_on` costs the
    // frame it is called on and leaves a `Task` that panics if awaited again.
    let Some(outcome) = check_ready(&mut task.0) else {
        return;
    };
    commands.remove_resource::<OpenTask>();

    match outcome {
        Ok(opened) => {
            commit(&mut world, opened);
            next.set(PhaseState::Thriving);
        }
        Err(error) => {
            error!("{error}");
            failure.0 = Some(error.to_string());
            next.set(PhaseState::Failed);
        }
    }
}

/// Replaces the committed world with a newly grown one.
///
/// The whole of `F-GROW-13`'s "atomically publishes" that this phase needs: one assignment,
/// and nothing observes a partial one because nothing else runs during it.
fn commit(world: &mut CommittedWorld, opened: Opened) {
    let Opened { snapshot, summary } = opened;
    debug_assert!(
        snapshot.is_covered(),
        "the pipeline produced a snapshot whose maps disagree about the node count"
    );

    // The same facts the status line shows. Duplicated into the log deliberately: a developer
    // or an agent driving the app over BRP (D10) cannot read a window, and this is the one
    // event in a session where "what did it actually open" is the whole question. It carries
    // no contributor and no author count — `N9` gives the shell no reason to hold either.
    info!(
        "opened {} ({}, {}): {} paths → {} nodes, {} segments, {} containers",
        summary.root.display(),
        summary.tier,
        if summary.from_cache {
            "from store"
        } else {
            "extracted"
        },
        summary.paths,
        summary.nodes,
        summary.segments,
        summary.aggregates,
    );

    world.snapshot = Some(Arc::new(snapshot));
    world.summary = Some(summary);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_committed_before_a_snapshot_lands() {
        let world = CommittedWorld::default();
        assert!(world.snapshot().is_none());
        assert!(world.summary().is_none());
    }

    /// The app starts in `Opening`, not in `Thriving` with an empty world. A default of
    /// "thriving with nothing" would make an app that failed to open look like an app that
    /// opened an empty repository.
    #[test]
    fn the_app_starts_by_opening_something() {
        assert_eq!(PhaseState::default(), PhaseState::Opening);
    }
}
