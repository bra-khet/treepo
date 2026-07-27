//! The history pass against a real repository: treepo's own.
//!
//! `F-EXT-2` is the linchpin `RISK-1` is about, and its unit tests exercise the arithmetic
//! on synthetic events. This runs the whole traversal — graph walk, parallel blob diffing,
//! mailmap resolution, and the merge — over a repository whose history is small enough to
//! reason about and guaranteed to be present.

use std::path::{Path, PathBuf};
use treepo_model::manifest::NodeKind;
use treepo_model::path::RepoPath;
use treepo_vcs::{FilterSet, HistoryOptions, Target, WalkOptions, discover, log_pass, walk};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn path(s: &str) -> RepoPath {
    RepoPath::new(s.as_bytes()).expect("valid path")
}

fn target() -> Target {
    let target = discover(workspace_root()).expect("treepo's own repository opens");
    // These tests assert things about real history, so a shallow checkout would let them
    // pass while proving almost nothing. CI sets `fetch-depth: 0` for exactly this; failing
    // loudly here is what stops that from being quietly reverted.
    if let Target::Repository(repo) = &target {
        assert!(
            !repo.is_shallow,
            "history tests need full history — clone without --depth, or set fetch-depth: 0"
        );
    }
    target
}

fn history(threads: usize) -> treepo_vcs::History {
    log_pass(
        &target(),
        &FilterSet::built_in(),
        HistoryOptions {
            threads,
            ..HistoryOptions::default()
        },
    )
    .expect("history pass succeeds")
}

#[test]
fn the_pass_finds_this_repository_s_history() {
    let history = history(4);

    assert!(history.commit_count > 5, "treepo has commits");
    assert!(!history.authors.is_empty(), "and contributors");
    assert!(
        history.reference_time > 1_700_000_000,
        "the newest commit is recent"
    );

    // The root accumulates every commit that touched anything.
    let root = history
        .paths
        .get(&RepoPath::root())
        .expect("the root has history");
    assert_eq!(
        root.temporal.commit_count,
        history.commit_count - history.merge_count,
        "every non-merge commit touches the root exactly once"
    );
    assert!(root.temporal.churn.lifetime > 0);

    // A file that has existed since the first commit.
    let constitution = history
        .paths
        .get(&path("docs/CONSTITUTION.md"))
        .expect("the constitution is tracked");
    assert!(constitution.temporal.commit_count >= 1);
    assert!(constitution.temporal.first_commit_time.is_some());
}

/// A directory's commit count is commits that touched *anything* beneath it, counted once
/// each — not the sum over its files.
#[test]
fn directory_commit_counts_are_deduplicated() {
    let history = history(1);
    let directory = history
        .paths
        .get(&path("crates/treepo-det/src"))
        .expect("treepo-det has sources");

    let files: u32 = history
        .paths
        .iter()
        .filter(|(p, _)| p.parent().as_ref() == Some(&path("crates/treepo-det/src")))
        .map(|(_, h)| h.temporal.commit_count)
        .sum();

    assert!(directory.temporal.commit_count >= 1);
    assert!(
        directory.temporal.commit_count <= files,
        "a directory cannot have more commits than the sum over its files"
    );
    // treepo-det's sources landed together in one commit, so the sum over files strictly
    // exceeds the directory's own count — which is exactly the double-count being avoided.
    assert!(files > directory.temporal.commit_count);
}

/// `N3`, and the property the spike measured: summing line counts is associative, so the
/// thread count cannot change the answer.
#[test]
fn the_thread_count_does_not_change_the_result() {
    let one = history(1);
    let four = history(4);
    let sixteen = history(16);

    assert_eq!(one.reference_time, four.reference_time);
    assert_eq!(one.commit_count, four.commit_count);
    assert_eq!(one.paths.len(), four.paths.len());
    assert_eq!(one.paths, four.paths);
    assert_eq!(one.paths, sixteen.paths);
}

/// Ownership shares must be proportions of one whole on every path (`N4`, `F-EXT-2`).
#[test]
fn every_path_s_shares_sum_to_a_whole() {
    let history = history(4);
    for (path, entry) in &history.paths {
        if entry.ownership.is_empty() {
            continue;
        }
        let total: u32 = entry.ownership.shares().map(|(_, s)| s.to_ppm()).sum();
        // Rounding to nearest can leave a few parts per million on the table when many
        // contributors divide one path; anything larger is a real defect.
        assert!(
            total.abs_diff(1_000_000) < 100,
            "{path} shares sum to {total}"
        );
    }
}

/// The whole point of the exercise: structure and history compose into one record set.
#[test]
fn history_applies_onto_the_walked_structure() {
    let target = target();
    let filter = FilterSet::built_in();
    let mut structure = walk(&target, &filter, WalkOptions::default()).expect("walk succeeds");
    let history = log_pass(&target, &filter, HistoryOptions::default()).expect("history");

    treepo_vcs::log_pass::apply(&mut structure.records, &history);

    let tracked = structure
        .records
        .iter()
        .filter(|r| r.kind == NodeKind::File)
        .count();
    let with_history = structure
        .records
        .iter()
        .filter(|r| r.kind == NodeKind::File && r.temporal.has_history())
        .count();
    assert_eq!(
        tracked, with_history,
        "every file in the HEAD tree was committed at least once"
    );

    // And ages are measured against the repository's own newest commit, not the clock.
    let record = structure
        .records
        .iter()
        .find(|r| r.path == path("docs/CONSTITUTION.md"))
        .expect("tracked");
    let age = record
        .temporal
        .first_commit_age_days(history.reference_time)
        .expect("has history");
    assert!(age >= 0, "no negative ages");
}
