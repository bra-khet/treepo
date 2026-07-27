//! Extraction against a real repository: treepo's own.
//!
//! The unit tests build synthetic record lists, which proves the arithmetic but not that the
//! `gix` tree walk returns what this crate thinks it does. This one runs the whole public
//! path — [`discover`], [`FilterSet`], [`walk`] — over the only repository guaranteed to be
//! present wherever the test suite runs, and checks the invariants that later phases will
//! quietly assume.

use std::path::{Path, PathBuf};
use treepo_model::manifest::NodeKind;
use treepo_model::path::RepoPath;
use treepo_vcs::{FilterSet, Target, WalkOptions, discover, walk};

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is `<root>/crates/treepo-vcs`.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn path(s: &str) -> RepoPath {
    RepoPath::new(s.as_bytes()).expect("valid path")
}

fn walk_self() -> treepo_vcs::Structure {
    let target = discover(workspace_root()).expect("treepo's own repository opens");
    assert!(matches!(target, Target::Repository(_)));
    walk(&target, &FilterSet::built_in(), WalkOptions::default()).expect("walk succeeds")
}

#[test]
fn the_walk_finds_this_repository_the_way_it_actually_is() {
    let structure = walk_self();
    let paths: Vec<&RepoPath> = structure.records.iter().map(|r| &r.path).collect();

    assert!(
        paths.contains(&&RepoPath::root()),
        "the root is always a record"
    );
    assert!(paths.contains(&&path("Cargo.toml")));
    assert!(paths.contains(&&path("crates/treepo-det/src/trig.rs")));
    assert!(paths.contains(&&path("docs/PRD.md")));

    let manifest = structure
        .records
        .iter()
        .find(|r| r.path == path("Cargo.toml"))
        .expect("the workspace manifest is tracked");
    assert_eq!(manifest.kind, NodeKind::File);
    assert!(
        manifest.size.bytes > 100,
        "a real blob size, read from the pack index"
    );
}

/// `F-EXT-8` item 1, and the whole reason the HEAD-tree lens is free of `.gitignore` work.
#[test]
fn nothing_untracked_or_excluded_survives() {
    let structure = walk_self();
    for record in &structure.records {
        let rendered = record.path.to_string();
        // By component: `.gitattributes` is a tracked file and must survive, while a `.git`
        // directory must not. Matching on the prefix alone would confuse the two.
        for component in rendered.split('/') {
            assert!(component != ".git", "{rendered} should never appear");
            assert!(component != "target", "{rendered} is build output");
        }
    }
    assert!(
        structure
            .records
            .iter()
            .any(|r| r.path == path(".gitattributes")),
        "a tracked dotfile is structure, not metadata"
    );
    assert!(
        workspace_root().join("target").is_dir(),
        "target/ exists on disk"
    );
}

// Note on what this file does *not* prove: `target/` is both gitignored and in the default
// exclusion set, so its absence does not isolate the HEAD-tree lens from the filter. A test
// that does — a fixture with one committed file and one tracked-then-ignored file — needs a
// repository built for the purpose, and lands with `tools/corpus/`.

/// `N5` in embryo: a record whose parent is missing would be a disconnected node, and every
/// rollup, every LOD aggregation, and every Grow migration assumes it cannot happen.
#[test]
fn every_record_has_its_parent() {
    let structure = walk_self();
    let paths: std::collections::BTreeSet<&RepoPath> =
        structure.records.iter().map(|r| &r.path).collect();
    for record in &structure.records {
        if let Some(parent) = record.path.parent() {
            assert!(
                paths.contains(&parent),
                "{} has no parent record",
                record.path
            );
        }
    }
}

#[test]
fn records_are_sorted_and_unique() {
    let structure = walk_self();
    let paths: Vec<&RepoPath> = structure.records.iter().map(|r| &r.path).collect();
    let mut sorted = paths.clone();
    sorted.sort();
    assert_eq!(paths, sorted, "records come back in RepoPath order");
    sorted.dedup();
    assert_eq!(paths.len(), sorted.len(), "no path appears twice");
}

/// The rollup is only meaningful if the totals actually add up.
#[test]
fn subtree_totals_agree_with_their_parts() {
    let structure = walk_self();
    let by_path = treepo_vcs::walk::by_path(&structure.records);

    let root = by_path[&RepoPath::root()];
    let files = structure
        .records
        .iter()
        .filter(|r| r.kind == NodeKind::File)
        .count();
    assert_eq!(root.structural.descendant_file_count as usize, files);

    let total_bytes: u64 = structure
        .records
        .iter()
        .filter(|r| r.kind == NodeKind::File)
        .map(|r| r.size.bytes)
        .sum();
    assert_eq!(root.size.bytes, total_bytes);

    // And one directory, checked the same way.
    let crates = by_path[&path("crates")];
    let beneath: u64 = structure
        .records
        .iter()
        .filter(|r| r.kind == NodeKind::File && r.path.starts_with(&path("crates")))
        .map(|r| r.size.bytes)
        .sum();
    assert_eq!(crates.size.bytes, beneath);
    assert!(
        crates.structural.max_subtree_depth >= 3,
        "crates/x/src/y.rs"
    );
}

/// `AC-DET-3`: the same repository must walk to the same structure every time. Sorting is
/// explicit precisely so this does not depend on how git or the filesystem felt today.
#[test]
fn walking_twice_produces_identical_structure() {
    let first = walk_self();
    let second = walk_self();
    assert_eq!(first.records, second.records);
    assert_eq!(first.excluded, second.excluded);
}
