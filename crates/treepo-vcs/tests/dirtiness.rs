//! `F-THR-4` against a real working tree.
//!
//! The unit tests in `status.rs` build [`Dirtiness`] values directly, which proves the
//! attachment and rollup arithmetic but not that `gix`'s status states map onto the five
//! `F-THR-4` names correctly. That mapping is the part that can be wrong in a way nothing
//! else notices — `NeedsUpdate` misread as a modification would light up a whole repository
//! and still pass every test in this file's sibling module.
//!
//! So this runs the real pass over the `dirty-worktree` fixture, which is built into all five
//! states at once and stays that way.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use treepo_model::path::RepoPath;
use treepo_vcs::status::{DirtyState, StatusOptions, WorkingTreeStatus, status};
use treepo_vcs::{FilterSet, Structure, Target, WalkOptions, discover, walk};

fn fixture(name: &str) -> std::path::PathBuf {
    static ONCE: std::sync::Once = std::sync::Once::new();
    let root = corpus::default_root();
    ONCE.call_once(|| {
        corpus::ensure(&root).expect("the corpus builds");
    });
    root.join(name)
}

fn path(s: &str) -> RepoPath {
    RepoPath::new(s.as_bytes()).expect("valid path")
}

/// Discover, walk, then read status — the order a caller would use it in.
fn read(name: &str) -> (Target, Structure, WorkingTreeStatus) {
    let target = discover(fixture(name)).expect("fixture opens");
    let filter = FilterSet::built_in();
    let structure = walk(&target, &filter, WalkOptions::default()).expect("walk");
    let status = status(&target, &filter, &StatusOptions::bounded()).expect("status reads");
    (target, structure, status)
}

/// One shared read of the dirty fixture, for the tests that only want to look at it.
///
/// A single read costs about 15 ms; eleven of them running *concurrently* cost twenty
/// seconds, because `gix`'s status spawns worker threads and polls a channel, and a test
/// harness running every test at once oversubscribes the machine many times over. The
/// product reads status one repository at a time and infrequently (`F-THR-6`), so this is a
/// property of the harness rather than of the pass — but it is paid on three platforms on
/// every push, so the tests that can share a read do.
///
/// Sharing is safe precisely because this pass is read-only: the same tree read twice gives
/// the same answer, which `an_untouched_file_is_not_dirty` asserts with its own reads rather
/// than through this cache.
fn shared() -> &'static (Structure, WorkingTreeStatus) {
    static SHARED: std::sync::OnceLock<(Structure, WorkingTreeStatus)> = std::sync::OnceLock::new();
    SHARED.get_or_init(|| {
        let (_, structure, status) = read("dirty-worktree");
        (structure, status)
    })
}

fn state_of(status: &WorkingTreeStatus, p: &str) -> treepo_vcs::Dirtiness {
    status
        .paths
        .iter()
        .find(|entry| entry.path == path(p))
        .map(|entry| entry.state)
        .unwrap_or_else(|| panic!("`{p}` should be dirty; got {:?}", status.paths))
}

/// All five states, from one read of one tree.
#[test]
fn every_dirtiness_state_is_recognized() {
    let (_, status) = shared();

    assert!(state_of(status, "src/modified.rs").modified);
    assert!(state_of(status, "src/staged.rs").staged);
    assert!(state_of(status, "src/deleted.rs").pending_delete);
    assert!(state_of(status, "src/untracked.rs").untracked);
    assert!(state_of(status, "conflict.txt").conflicted);

    // Every state F-THR-4 names is represented, so the fixture has not quietly stopped
    // covering one of them.
    for state in DirtyState::ALL {
        assert!(
            status.count(state) > 0,
            "no path is `{}` — the fixture no longer covers it",
            state.name()
        );
    }
}

/// The failure that would be invisible: a file nobody touched must not be reported.
///
/// `gix` reports `NeedsUpdate` for unchanged files whose cached stat is stale, which is most
/// of a repository right after a checkout. Reading that as a modification is the single most
/// plausible way to make this feature look broken while every unit test still passes.
#[test]
fn an_untouched_file_is_not_dirty() {
    let (_, _, status) = read("dirty-worktree");
    assert!(
        !status
            .paths
            .iter()
            .any(|entry| entry.path == path("src/clean.rs")),
        "src/clean.rs was never touched: {:?}",
        status.paths
    );
    // And the same read twice in a row agrees — the first read is the one with a cold stat
    // cache, so if `NeedsUpdate` leaked in, the two would disagree.
    let (_, _, again) = read("dirty-worktree");
    assert_eq!(status, again);
}

/// `F-EXT-8` decides what is structure, here exactly as in the walk.
#[test]
fn ignored_output_is_not_dirtiness() {
    let (_, status) = shared();
    assert!(
        !status
            .paths
            .iter()
            .any(|entry| entry.path.starts_with(&path("build"))),
        "an ignored build directory is not news: {:?}",
        status.paths
    );
}

/// `F-EXT-7`: the overlay marks the skeleton, it does not extend it.
#[test]
fn an_untracked_file_marks_its_folder_without_becoming_one() {
    let (structure, status) = shared();
    let before = structure.records.len();
    let overlay = status.overlay(&structure.records);

    assert_eq!(
        structure.records.len(),
        before,
        "the overlay borrowed the records and gave them back unchanged"
    );
    assert_eq!(overlay.len(), structure.records.len());
    assert_eq!(overlay.orphaned(), 0, "the root catches everything");

    // `src/untracked.rs` is not in HEAD, so it has no record. `src` does, and carries it.
    assert!(
        !structure
            .records
            .iter()
            .any(|record| record.path == path("src/untracked.rs")),
        "an untracked file is not structure"
    );
    let src = structure
        .records
        .iter()
        .position(|record| record.path == path("src"))
        .expect("src is in the skeleton");
    assert!(overlay.beneath(src).untracked);

    // A tracked file that *is* in the skeleton marks itself instead.
    let modified = structure
        .records
        .iter()
        .position(|record| record.path == path("src/modified.rs"))
        .expect("a tracked file is structure");
    assert!(overlay.here(modified).modified);
}

/// The root should read as dirty when anything anywhere is, or the overlay is unusable at the
/// zoom level where the whole tree is one shape.
#[test]
fn the_root_sees_everything_beneath_it() {
    let (structure, status) = shared();
    let overlay = status.overlay(&structure.records);
    let root = overlay.combined(0);

    assert_eq!(structure.records[0].path, RepoPath::root());
    assert!(root.modified && root.staged && root.pending_delete && root.untracked);
    assert_eq!(root.dominant(), Some(DirtyState::Conflicted));
}

/// A committed, untouched repository has nothing to say.
#[test]
fn a_clean_repository_reports_clean() {
    let (_, _, status) = read("single-author");
    assert!(status.is_clean(), "unexpected dirt: {:?}", status.paths);
    assert!(!status.truncated);
    assert_eq!(status.unrepresentable, 0);
}

/// PRD §6: a directory with no `.git` is an ordinary path, not an error.
#[test]
fn a_plain_directory_has_no_dirtiness_and_does_not_fail() {
    let (target, structure, status) = read("no-git");
    assert!(matches!(target, Target::PlainDirectory { .. }));
    assert!(status.is_clean());
    assert_eq!(status.head, None);
    assert_eq!(status.overlay(&structure.records).attached(), 0);
}

/// PRD §6: a repository with no commits has no HEAD tree to diff the index against.
#[test]
fn a_repository_with_no_commits_reads_without_a_head_tree() {
    let (_, _, status) = read("empty");
    assert_eq!(status.head, None);
    assert!(status.is_clean(), "nothing is on disk either");
}

/// The overlay is only valid over the structure it was measured against. A commit between
/// extraction and this read is a Grow trigger, not something to draw.
#[test]
fn the_status_records_which_head_it_saw() {
    let (target, _, status) = read("dirty-worktree");
    let head = match &target {
        Target::Repository(repo) => repo.head,
        Target::PlainDirectory { .. } => unreachable!("the fixture is a repository"),
    };
    assert_eq!(status.head, head);
    assert!(status.is_current_for(head));
    assert!(!status.is_current_for(None));
}

/// `F-THR-6` calls the caller "cancellable". A flag already set means the read gives up
/// immediately and says so, rather than erroring or running to completion.
#[test]
fn an_interrupted_read_returns_what_it_had() {
    // No walk. `status` takes no `Structure`, so "is anything dirty?" is answerable without
    // building a skeleton first — which is what makes it usable as the cheap check before
    // deciding a Grow is worth triggering.
    let target = discover(fixture("dirty-worktree")).expect("fixture opens");
    let filter = FilterSet::built_in();

    let flag = Arc::new(AtomicBool::new(true));
    let cancelled = status(
        &target,
        &filter,
        &StatusOptions {
            max_paths: 0,
            should_interrupt: Some(Arc::clone(&flag)),
        },
    )
    .expect("cancelling is not an error");
    assert!(
        cancelled.truncated,
        "a cancelled read says it is incomplete"
    );

    // The same read with the flag clear runs to completion, so the assertion above is about
    // cancellation rather than about this fixture having nothing in it.
    flag.store(false, Ordering::Relaxed);
    let full = status(&target, &filter, &StatusOptions::bounded()).expect("status reads");
    assert!(!full.truncated);
    assert!(!full.is_clean());
}

/// `AC-THR-2` gives the overlay two seconds, which an unbounded read cannot promise.
#[test]
fn the_path_cap_truncates_rather_than_running_long() {
    let target = discover(fixture("dirty-worktree")).expect("fixture opens");
    let filter = FilterSet::built_in();

    let options = StatusOptions {
        max_paths: 2,
        should_interrupt: None,
    };
    let status = status(&target, &filter, &options).expect("status reads");
    assert!(status.paths.len() <= 2);
    assert!(status.truncated);
}
