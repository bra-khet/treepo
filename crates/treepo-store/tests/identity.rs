//! `F-MAN-3`'s three tiers against the `F-CORP-3` fixtures, and the identity half of
//! `AC-MAN-4` and `AC-MAN-5`.
//!
//! Resolution runs on what [`discover`] found rather than on repositories this file opened
//! its own way. The composition is the thing being tested: `Target::repository()` returning
//! `None` for a plain directory is what selects tier 3, and a test that called `gix::open`
//! itself would assert the tiers work while proving nothing about the path the product takes.
//!
//! # What is asserted, and what waits for the store
//!
//! `AC-MAN-4` has two halves — two clones resolve to one identity, and the second open skips
//! extraction. Only the first is an identity question; the second needs a manifest to find,
//! and lands with `manifest_io`. The same split applies to `AC-MAN-5`: "does not orphan its
//! store" is, at this layer, "the identity does not change", and that is what is checked here.
//!
//! Placed under `crates/treepo-store/tests/` rather than the workspace-root `tests/` the
//! campaign names, for the reason `degenerate.rs` already records: this is a virtual
//! workspace with no root package, so a root `tests/` directory is not a cargo target.

use std::path::{Path, PathBuf};
use treepo_model::identity::IdentityTier;
use treepo_store::{Resolution, Skipped, StoreRoot, resolve};
use treepo_vcs::{Target, discover};

/// Builds the corpus once per test binary, then hands out fixture paths.
fn fixture(name: &str) -> PathBuf {
    static ONCE: std::sync::Once = std::sync::Once::new();
    let root = corpus::default_root();
    ONCE.call_once(|| {
        corpus::ensure(&root).expect("the corpus builds");
    });
    root.join(name)
}

/// The product's own path: discover, then resolve what was discovered.
fn identify(path: impl AsRef<Path>) -> Resolution {
    let target: Target = discover(path.as_ref()).expect("the fixture opens");
    resolve(target.root(), target.repository(), 0).expect("an identity")
}

/// A scratch directory for the cases that need repositories the corpus does not hold.
///
/// Under `target/` so it is git-ignored and survives a failure for inspection, and named per
/// test so two of them cannot collide.
fn scratch(name: &str) -> PathBuf {
    let root = corpus::default_root()
        .parent()
        .expect("target/")
        .join("identity-scratch")
        .join(name);
    if root.exists() {
        std::fs::remove_dir_all(&root).expect("clearing scratch");
    }
    std::fs::create_dir_all(&root).expect("creating scratch");
    root
}

/// A repository with one commit, at `path`, optionally with `origin` set to `remote`.
fn repository_at(path: &Path, name: &str, remote: Option<&str>) -> PathBuf {
    let mut builder = corpus::Builder::init(path.to_path_buf(), name).expect("git init");
    builder.write_source("src/main.rs", 20).expect("a file");
    builder.commit("first").expect("a commit");
    if let Some(url) = remote {
        builder
            .git(&["remote", "add", "origin", url])
            .expect("a remote");
    }
    path.to_path_buf()
}

// ---------------------------------------------------------------------------------------
// The three tiers, against F-CORP-3.
// ---------------------------------------------------------------------------------------

/// Tier 1, with the `F-CORP-3` "multiple remotes and no `origin`" fixture.
///
/// Its remotes are `upstream` and `backup`, added in that order. Alphabetically first wins,
/// so `backup` does — the fixture is built in the wrong order on purpose, because a resolver
/// that took the first *configured* remote would pass a fixture built the other way round.
#[test]
fn tier_one_takes_the_alphabetically_first_remote_when_there_is_no_origin() {
    let resolution = identify(fixture("multi-remote"));

    assert_eq!(resolution.identity.tier, IdentityTier::Remote);
    assert_eq!(resolution.identity.source_value, "example.invalid/backup");
    assert_eq!(resolution.chosen.as_deref(), Some("backup"));
    assert!(
        resolution.skipped.is_empty(),
        "nothing was skipped: {:?}",
        resolution.skipped
    );

    let names: Vec<&str> = resolution
        .remotes
        .iter()
        .map(|remote| remote.name.as_str())
        .collect();
    assert_eq!(names, ["backup", "upstream"], "recorded in name order");
    assert!(
        resolution.remotes.iter().all(|r| r.normalized.is_some()),
        "both remotes have usable URLs"
    );
}

/// Tier 2, with the `F-CORP-3` "no remote" fixture.
#[test]
fn tier_two_names_a_repository_by_its_root_commit() {
    let resolution = identify(fixture("no-remote"));

    assert_eq!(resolution.identity.tier, IdentityTier::RootCommit);
    assert_eq!(resolution.skipped, [Skipped::NoRemotes]);
    assert!(resolution.remotes.is_empty());
    assert!(resolution.chosen.is_none());

    let root = resolution.root_commit.expect("a root commit");
    assert_eq!(
        resolution.identity.source_value,
        root.to_string(),
        "the key is built from the root commit itself"
    );
    assert_eq!(root.to_string().len(), 40, "a SHA-1 object id as hex");
}

/// Tier 3, with the `F-CORP-3` "no commits" fixture.
///
/// Both higher tiers are reported as skipped, in order, which is what `identity.json` shows a
/// user asking why this repository did not get a durable identity.
#[test]
fn tier_three_names_a_repository_with_no_commits_by_its_path() {
    let resolution = identify(fixture("empty"));

    assert_eq!(resolution.identity.tier, IdentityTier::PathHash);
    assert_eq!(
        resolution.skipped,
        [Skipped::NoRemotes, Skipped::NoCommits],
        "in tier order"
    );
    assert!(resolution.root_commit.is_none());
    assert!(
        resolution.identity.source_value.ends_with("empty"),
        "keyed on the folder: {}",
        resolution.identity.source_value
    );
}

/// A directory with no `.git` is tier 3 too, and says so differently.
#[test]
fn a_plain_directory_is_tier_three_for_a_different_reason() {
    let resolution = identify(fixture("no-git"));

    assert_eq!(resolution.identity.tier, IdentityTier::PathHash);
    assert_eq!(resolution.skipped, [Skipped::NoRepository]);
    assert!(resolution.remotes.is_empty());
}

/// A shallow clone has an `origin`, so it never reaches the tier its truncation would break.
///
/// The corpus clones through a `file://` URL, which makes this the one fixture that exercises
/// local-path normalization against a path the test did not choose.
#[test]
fn a_shallow_clone_is_identified_by_the_remote_it_came_from() {
    let resolution = identify(fixture("shallow"));

    assert_eq!(resolution.identity.tier, IdentityTier::Remote);
    assert_eq!(resolution.chosen.as_deref(), Some("origin"));
    let source = &resolution.identity.source_value;
    assert!(
        source.ends_with("shallow-source"),
        "normalized to the source it was cloned from: {source}"
    );
    assert!(
        !source.contains("file:"),
        "the scheme is stripped: {source}"
    );
    assert!(!source.contains('\\'), "separators are folded: {source}");
}

// ---------------------------------------------------------------------------------------
// AC-MAN-4 and AC-MAN-5.
// ---------------------------------------------------------------------------------------

/// `AC-MAN-4` — two clones of one remote, at different paths, are one repository.
///
/// The two are cloned through URLs spelled differently on purpose: one carries a `.git`
/// suffix and mixed case, the other does not. Cloning twice from an identical string would
/// test that two equal strings hash equally.
#[test]
fn two_clones_of_one_remote_share_an_identity_and_a_store() {
    let root = scratch("two-clones");
    let first = repository_at(
        &root.join("checkout-a"),
        "clone-a",
        Some("https://github.com/Example/Widget.git"),
    );
    let second = repository_at(
        &root.join("checkout-b"),
        "clone-b",
        Some("https://github.com/example/widget"),
    );

    let a = identify(&first);
    let b = identify(&second);

    assert_ne!(first, second, "different paths, as AC-MAN-4 requires");
    assert_eq!(a.identity.tier, IdentityTier::Remote);
    assert_eq!(a.identity.source_value, "github.com/example/widget");
    assert_eq!(a.identity.key, b.identity.key);

    let store = StoreRoot::at(root.join("app-data"));
    assert_eq!(
        store.repository(&a.identity),
        store.repository(&b.identity),
        "one identity is one store directory"
    );
}

/// `AC-MAN-5` — moving or renaming a no-remote repository does not orphan its store.
///
/// A real rename rather than a copy: `AC-MAN-5` is about the folder the user already has
/// moving, and a copy would leave the original in place to be found.
#[test]
fn moving_a_no_remote_repository_keeps_its_identity() {
    let root = scratch("moved");
    let before = repository_at(&root.join("original-name"), "movable", None);
    let original = identify(&before);
    assert_eq!(original.identity.tier, IdentityTier::RootCommit);

    let after = root.join("somewhere").join("renamed");
    std::fs::create_dir_all(after.parent().expect("a parent")).expect("the new home");
    std::fs::rename(&before, &after).expect("the move");

    let moved = identify(&after);
    assert_eq!(moved.identity.tier, IdentityTier::RootCommit);
    assert_eq!(
        original.identity.key, moved.identity.key,
        "the store is keyed on history, not on where the folder sits"
    );

    let store = StoreRoot::at(root.join("app-data"));
    assert_eq!(
        store.repository(&original.identity),
        store.repository(&moved.identity)
    );
}

/// The same move, for a repository that only has tier 3 to fall back on.
///
/// This one *does* orphan its store, and that is the documented cost of tier 3 rather than a
/// defect — there is nothing else to key on. Asserted so that a change making tier 3 look
/// move-proof has to be a deliberate one.
#[test]
fn moving_a_repository_with_no_commits_does_change_its_identity() {
    let root = scratch("moved-empty");
    let before = root.join("original-name");
    corpus::Builder::init(before.clone(), "unborn").expect("git init");
    let original = identify(&before);
    assert_eq!(original.identity.tier, IdentityTier::PathHash);

    let after = root.join("renamed");
    std::fs::rename(&before, &after).expect("the move");

    let moved = identify(&after);
    assert_ne!(original.identity.key, moved.identity.key);
    assert_eq!(
        moved.skipped,
        [Skipped::NoRemotes, Skipped::NoCommits],
        "and the reason is on the record"
    );
}

/// PRD §6 — a fork shares its upstream's root commit and must not share its store.
///
/// This is why the tiers are ordered rather than merged. The test asserts the root commits
/// really are identical first, so it cannot pass by the two repositories simply being
/// different.
#[test]
fn a_fork_does_not_inherit_its_upstream_store() {
    let root = scratch("fork");
    let upstream = root.join("upstream");
    repository_at(&upstream, "shared-history", None);
    let shared_root = identify(&upstream).root_commit.expect("a root commit");

    let mut clones = Vec::new();
    for (name, url) in [
        ("upstream-clone", "https://github.com/original/widget.git"),
        ("fork-clone", "https://github.com/someone/widget.git"),
    ] {
        let path = root.join(name);
        let holder = corpus::Builder::plain(path.clone(), name).expect("a directory");
        let url_from = format!(
            "file:///{}",
            upstream.display().to_string().replace('\\', "/")
        );
        holder
            .git(&["clone", "--quiet", &url_from, "."])
            .expect("the clone");
        holder
            .git(&["remote", "set-url", "origin", url])
            .expect("its own remote");
        clones.push(identify(&path));
    }

    let (original, fork) = (&clones[0], &clones[1]);
    assert_eq!(
        resolve_root(&upstream),
        shared_root,
        "the fixture really does share one history"
    );
    assert_eq!(original.identity.tier, IdentityTier::Remote);
    assert_eq!(fork.identity.tier, IdentityTier::Remote);
    assert_ne!(
        original.identity.key, fork.identity.key,
        "tier 1 beating tier 2 is what keeps a fork out of its upstream's store"
    );
}

/// The root commit of a repository, resolved through tier 2.
fn resolve_root(path: &Path) -> treepo_model::identity::CommitId {
    identify(path).root_commit.expect("a root commit")
}

/// Every fixture resolves to something, and no two unrelated ones collide.
///
/// The sweep matters more than any single row: a resolver that returned the same key for
/// everything would pass most of the tests above.
#[test]
fn every_corpus_fixture_resolves_to_a_distinct_identity() {
    let mut seen: Vec<([u8; 32], &str)> = Vec::new();
    let mut tiers = std::collections::BTreeSet::new();

    for shape in corpus::all_shapes() {
        let path = fixture(shape.name);
        if !path.is_dir() {
            continue; // platform-gated shapes, per `Platforms::available`
        }
        let Ok(target) = discover(&path) else {
            continue; // `bare` is rejected at association and never reaches the store
        };
        let resolution = resolve(target.root(), target.repository(), 0).expect("an identity");
        tiers.insert(resolution.identity.tier);

        if let Some((_, other)) = seen.iter().find(|(key, _)| *key == resolution.identity.key) {
            panic!("{} and {other} resolved to one store", shape.name);
        }
        seen.push((resolution.identity.key, shape.name));
    }

    assert!(
        seen.len() >= 15,
        "the corpus was built: {} shapes",
        seen.len()
    );
    assert_eq!(
        tiers.len(),
        3,
        "the corpus exercises all three tiers, not just one: {tiers:?}"
    );
}
