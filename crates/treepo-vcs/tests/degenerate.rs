//! Every PRD §6 row that extraction can be held to, one test each.
//!
//! > Each is a supported path with defined behavior, not an error state.
//!
//! That sentence is what these tests exist to hold. Most of them assert something *works*
//! rather than something fails, because the defect this catches is treating an unusual
//! repository as broken.
//!
//! Rows about rendering, storage, and Grow (aggregation geometry, store corruption, mid-Grow
//! modification) belong to later phases and are absent here rather than stubbed — a passing
//! test that asserts nothing is worse than a missing one.
//!
//! Placed under `crates/treepo-vcs/tests/` rather than the workspace-root `tests/` the
//! campaign names: this is a virtual workspace with no root package, so a root `tests/`
//! directory is not a cargo target and would never run.

use std::path::PathBuf;
use treepo_model::manifest::NodeKind;
use treepo_model::path::RepoPath;
use treepo_vcs::{
    DiscoverError, FilterSet, HistoryOptions, Notice, Target, WalkOptions, discover, log_pass, walk,
};

/// Builds the corpus once per test binary, then hands out fixture paths.
fn fixture(name: &str) -> PathBuf {
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

/// Discover, filter, walk, and apply history — the whole Phase 1 pipeline.
fn extract(name: &str) -> (Target, treepo_vcs::Structure, treepo_vcs::History) {
    let target = discover(fixture(name)).expect("fixture opens");
    let filter = FilterSet::built_in();
    let mut structure = walk(&target, &filter, WalkOptions::default()).expect("walk");
    let history = log_pass(&target, &filter, HistoryOptions::default()).expect("history");
    treepo_vcs::log_pass::apply(&mut structure.records, &history);
    (target, structure, history)
}

/// | Empty repository | Seed and root-boulder cluster. Never a lonely trunk. (`AC-SKEL-2`) |
#[test]
fn empty_repository_yields_a_root_and_says_so() {
    let (target, structure, history) = extract("empty");
    assert_eq!(structure.records.len(), 1, "just the root");
    assert_eq!(structure.records[0].path, RepoPath::root());
    assert!(!structure.records[0].temporal.has_history());
    assert_eq!(history.commit_count, 0);
    assert!(target.notices().contains(&Notice::NoCommits));
}

/// | No `.git` | Filesystem primitives only. Tree generates. Explicit notice. |
#[test]
fn a_plain_directory_still_grows_a_tree() {
    let (target, structure, history) = extract("no-git");
    assert!(matches!(target, Target::PlainDirectory { .. }));
    assert_eq!(structure.source, treepo_vcs::StructureSource::Filesystem);
    assert_eq!(target.notices(), [Notice::NoRepository]);

    // Structure and size are real; history is absent, which is the notice's point.
    let root = &structure.records[0];
    assert!(root.size.bytes > 0);
    assert!(root.structural.descendant_file_count >= 3);
    assert!(history.paths.is_empty());
    assert!(structure.records.iter().all(|r| !r.temporal.has_history()));
}

/// | Shallow clone | Detect `--depth` truncation, warn explicitly. |
///
/// PRD §6: "Silently producing a history-less tree is a defect."
#[test]
fn a_shallow_clone_is_detected_and_announced() {
    let (target, structure, history) = extract("shallow");
    let Target::Repository(repo) = &target else {
        panic!("the shallow fixture is a repository");
    };
    assert!(repo.is_shallow, "--depth 1 must be detected");
    assert!(target.notices().contains(&Notice::ShallowClone));

    // The tree still generates — the warning is not a refusal.
    assert!(structure.records.len() > 1);
    // And it really is history-less, which is what makes the warning necessary.
    assert_eq!(history.commit_count, 1, "the source had five commits");
}

/// | Single file | Minimal but valid structure. |
#[test]
fn a_single_file_is_a_valid_repository() {
    let (_, structure, _) = extract("single-file");
    assert_eq!(structure.records.len(), 2, "the root and one file");
    let file = &structure.records[1];
    assert_eq!(file.kind, NodeKind::File);
    assert!(file.temporal.has_history());
    assert_eq!(structure.records[0].structural.child_count, 1);
}

/// | Single author | Mosaic degenerates to one material family. No empty ownership UI. |
#[test]
fn a_single_author_holds_the_whole_share() {
    let (_, structure, history) = extract("single-author");
    assert_eq!(history.authors.len(), 1);

    let root = structure
        .records
        .iter()
        .find(|r| r.path == RepoPath::root())
        .expect("root");
    assert_eq!(root.ownership.author_count(), 1);
    let (_, share) = root.ownership.shares().next().expect("one contributor");
    assert_eq!(*share, treepo_model::primitives::AuthorShare::WHOLE);
    assert!(root.ownership.dominant_author().is_some());
    assert_eq!(root.ownership.bus_factor_proxy(), 1);
}

/// | 1000+ authors | Palette assignment stays distinguishable; shares stay proportional. |
#[test]
fn many_authors_divide_one_file_without_losing_anyone() {
    let (_, structure, history) = extract("many-authors");
    assert!(history.authors.len() >= 60, "one key per contributor");

    let shared = structure
        .records
        .iter()
        .find(|r| r.path == path("shared.txt"))
        .expect("the contested file");
    assert!(shared.ownership.author_count() >= 60);

    // Every contributor keeps a non-zero share, and the shares still sum to one whole.
    let total: u32 = shared.ownership.shares().map(|(_, s)| s.to_ppm()).sum();
    assert!(total.abs_diff(1_000_000) < 100, "sums to {total}");
    let present = shared
        .ownership
        .shares()
        .filter(|(_, s)| s.is_present())
        .count();
    assert_eq!(present, shared.ownership.author_count() as usize);
    // Bus factor over many near-equal contributors is most of them, not one.
    assert!(shared.ownership.bus_factor_proxy() > 10);
}

/// `AC-EXT-3` — the mailmap collapses aliases, and the same repository without it does not.
#[test]
fn a_mailmap_collapses_aliases_and_lowers_the_author_count() {
    let (_, _, with_mailmap) = extract("mailmap");
    // Ada under three addresses, plus Bob.
    assert_eq!(with_mailmap.authors.len(), 2, "one Ada, one Bob");

    // The same commits read without the mapping: four distinct addresses.
    let identities = treepo_vcs::Identities::none();
    let addresses: std::collections::BTreeSet<_> = [
        "ada@example.invalid",
        "ada@work.example.invalid",
        "a.lovelace@old.example.invalid",
        "bob@example.invalid",
    ]
    .iter()
    .map(|address| {
        identities.key(gix::actor::SignatureRef {
            name: "x".into(),
            email: (*address).into(),
            time: "0 +0000",
        })
    })
    .collect();
    assert_eq!(addresses.len(), 4);
    assert!(
        addresses.len() > with_mailmap.authors.len(),
        "AC-EXT-3: without the mailmap, author_count is higher"
    );
}

/// | Deep nesting >15 | Aggregation engages; no stack overflow. |
#[test]
fn twenty_levels_of_nesting_do_not_overflow_the_stack() {
    let (_, structure, _) = extract("deep-nesting");
    let deepest = structure
        .records
        .iter()
        .map(|r| r.path.depth())
        .max()
        .expect("records");
    assert!(deepest > 15, "the fixture nests {deepest} deep");

    let root = &structure.records[0];
    assert!(root.structural.max_subtree_depth > 15);
    // A corridor reads as chain-like, which is the whole point of the signed skew.
    assert!(root.structural.hierarchy_skew < treepo_det::Fx::ZERO);
}

/// | One enormous file | Soft clamp prevents it consuming the parent's entire budget (`P7`). |
#[test]
fn one_enormous_file_is_visible_as_an_outlier() {
    let (_, structure, _) = extract("huge-file");
    let assets = structure
        .records
        .iter()
        .find(|r| r.path == path("assets"))
        .expect("the assets directory");
    assert_eq!(assets.size.large_file_count, 1);

    // The distribution shows the spread, not just the total — what `P7`'s clamp reads.
    let root = &structure.records[0];
    assert!(root.size.distribution.max > 4 * 1024 * 1024);
    assert!(root.size.distribution.median < 100 * 1024);
    // And the outlier really does dominate its parent by bytes, which is the problem.
    assert!(assets.size.relative_bytes > treepo_det::Fx::from_ratio(9, 10));
}

/// | Case-colliding paths | Handled deterministically; no duplicate or vanished nodes. |
#[test]
fn case_colliding_paths_both_survive() {
    let (_, structure, _) = extract("case-collision");
    let names: Vec<String> = structure
        .records
        .iter()
        .filter(|r| r.kind == NodeKind::File)
        .map(|r| r.path.to_string())
        .collect();
    assert!(names.contains(&"Readme.md".to_owned()), "{names:?}");
    assert!(names.contains(&"README.md".to_owned()), "{names:?}");
    assert_eq!(names.len(), 2, "neither vanished, neither duplicated");

    // Distinct paths that a case-insensitive filesystem would fold together.
    let lower = path("Readme.md");
    let upper = path("README.md");
    assert_ne!(lower, upper);
    assert_eq!(lower.case_fold_key(), upper.case_fold_key());
}

/// | Detached HEAD | Supported; treated as the current commit. |
#[test]
fn a_detached_head_is_supported_and_mentioned() {
    let (target, structure, history) = extract("detached-head");
    let Target::Repository(repo) = &target else {
        panic!("a repository");
    };
    assert!(repo.is_detached);
    assert!(target.notices().contains(&Notice::DetachedHead));
    // HEAD~1 was checked out, so only the first commit is in history.
    assert_eq!(history.commit_count, 1);
    assert!(structure.records.len() > 1);
}

/// | Bare repository | Rejected at association with a clear message (`F-ASSOC-2`). |
#[test]
fn a_bare_repository_is_rejected_with_an_actionable_message() {
    let error = discover(fixture("bare")).expect_err("bare repositories are rejected");
    assert!(matches!(error, DiscoverError::Bare { .. }));
    let message = error.to_string();
    assert!(message.contains("bare repository"));
    assert!(message.contains("Clone it normally"), "{message}");
}

/// `F-EXT-8` — ignored, vendored, and dependency content is not the repository's structure,
/// but a file tracked *before* it was ignored still is.
#[test]
fn excluded_content_is_filtered_but_tracked_files_are_kept() {
    let (_, structure, _) = extract("excluded-content");
    let names: Vec<String> = structure
        .records
        .iter()
        .map(|r| r.path.to_string())
        .collect();

    assert!(names.contains(&"src/main.rs".to_owned()));
    assert!(names.contains(&".gitignore".to_owned()));
    // Never tracked, so never in the tree — no pattern had to match them.
    assert!(!names.iter().any(|n| n.starts_with("build/")), "{names:?}");
    assert!(!names.iter().any(|n| n == "debug.log"), "{names:?}");
    // Tracked, then matched by the ignore file. Git keeps tracking it and so do we — the
    // reason `filter` reads the tree rather than applying ignore patterns to it.
    assert!(names.contains(&"legacy.log".to_owned()), "{names:?}");
    // Excluded by the built-in default set rather than by git.
    assert!(
        !names.iter().any(|n| n.starts_with("node_modules")),
        "{names:?}"
    );
    assert!(structure.excluded > 0, "the filter did fire");
}

/// Asserts a platform-gated test agrees with its shape's own gate.
///
/// The `cfg` on the test and the `platforms` on the shape are two statements of one fact,
/// and they have already drifted apart once — macOS is Unix but rejects non-UTF-8 names, so
/// a `cfg(unix)` test outlived the fixture it needed. Checking here means the next drift is
/// a failure rather than a test that quietly stops existing.
///
/// `cfg(unix)` because both callers are Unix-gated; on Windows it would be dead code.
#[cfg(unix)]
fn assert_shape_available(name: &str) {
    let shape = corpus::all_shapes()
        .iter()
        .find(|shape| shape.name == name)
        .unwrap_or_else(|| panic!("no shape named {name}"));
    assert!(
        shape.platforms.available(),
        "this test is compiled on a platform where the `{name}` fixture is not built — its \
         cfg and the shape's `platforms` disagree"
    );
}

/// | Symlinks | Not followed. Cycles impossible by construction. |
#[cfg(unix)]
#[test]
fn symlinks_are_recorded_and_never_followed() {
    assert_shape_available("symlinks");
    let (_, structure, _) = extract("symlinks");
    let link = structure
        .records
        .iter()
        .find(|r| r.path == path("link-to-real"))
        .expect("the link is recorded");
    assert_eq!(link.kind, NodeKind::Symlink);
    assert!(link.structural.is_leaf(), "a link has no children");

    // `real/loop` points at its own parent. Following it would not terminate; the walk
    // finished, which is the assertion.
    let looped = structure
        .records
        .iter()
        .find(|r| r.path == path("real/loop"))
        .expect("the loop is recorded");
    assert_eq!(looped.kind, NodeKind::Symlink);
}

/// | Non-UTF8 paths | Lossy display names; the raw path is preserved for `F-INSP-4`. |
///
/// Linux only, not Unix: macOS rejects these names at the syscall (`EILSEQ`), so the fixture
/// cannot exist there.
#[cfg(target_os = "linux")]
#[test]
fn non_utf8_paths_keep_their_bytes() {
    assert_shape_available("non-utf8");
    let (_, structure, _) = extract("non-utf8");
    let odd = structure
        .records
        .iter()
        .find(|r| !r.path.is_utf8())
        .expect("the fixture has a non-utf8 name");
    assert_eq!(odd.path.as_bytes(), b"src/caf\xe9.rs");
    assert_eq!(odd.path.display(), "src/caf\u{fffd}.rs");
    assert_eq!(odd.path.extension(), Some(&b"rs"[..]));
}

/// Every shape must be reachable, so a fixture cannot rot unnoticed.
#[test]
fn every_available_shape_builds_and_opens() {
    for shape in corpus::all_shapes() {
        if !shape.platforms.available() {
            continue;
        }
        let path = fixture(shape.name);
        assert!(path.exists(), "{} was not built", shape.name);
        // `bare` is the one shape that must *fail* to open, and it has its own test.
        if shape.name != "bare" {
            discover(&path).unwrap_or_else(|error| {
                panic!("{} ({}) failed to open: {error}", shape.name, shape.covers)
            });
        }
    }
}
