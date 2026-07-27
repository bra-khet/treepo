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
use treepo_model::manifest::{LanguageTable, NodeKind};
use treepo_model::path::RepoPath;
use treepo_model::primitives::size::ContentCategory;
use treepo_vcs::lang::{Catalogue, ContentOptions, ScanReport, apply_history_signals, scan};
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

/// Discover, filter, walk, scan, and apply history — the whole Phase 1 pipeline.
///
/// Every shape runs all of it, including the shapes whose test only looks at one pass. A
/// fixture that breaks a pass nobody thought to check against it is the interesting case,
/// and it is only reachable if every fixture goes through every pass.
fn extract(name: &str) -> (Target, treepo_vcs::Structure, treepo_vcs::History) {
    let (target, structure, history, _, _) = extract_content(name);
    (target, structure, history)
}

/// The same pipeline, keeping what the content pass produced.
fn extract_content(
    name: &str,
) -> (
    Target,
    treepo_vcs::Structure,
    treepo_vcs::History,
    LanguageTable,
    ScanReport,
) {
    let target = discover(fixture(name)).expect("fixture opens");
    let filter = FilterSet::built_in();
    let mut structure = walk(&target, &filter, WalkOptions::default()).expect("walk");

    let mut languages = LanguageTable::new();
    let report = scan(
        &target,
        &mut structure,
        &Catalogue::built_in(),
        &mut languages,
        ContentOptions::default(),
    )
    .expect("content scan");

    let catalogue = Catalogue::built_in();
    treepo_vcs::signals::apply(
        &mut structure.records,
        &treepo_vcs::SignalDictionary::built_in(),
        &catalogue,
    );

    let history = log_pass(&target, &filter, HistoryOptions::default()).expect("history");
    treepo_vcs::log_pass::apply(&mut structure.records, &history);
    // Last, because it needs both of the passes above (see `treepo_vcs::lang`).
    apply_history_signals(&mut structure.records);

    (target, structure, history, languages, report)
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

/// `F-EXT-8` rule 4, against a `.gitattributes` git itself committed.
///
/// The unit tests in `treepo_vcs::lang` prove the parser and the matcher. This proves the
/// blob reaches them at all — that the file is in the tree, found by name, read, and applied
/// to the right paths. Every step between the parser and the walk is only checked here.
#[test]
fn linguist_markers_from_a_committed_gitattributes_are_honoured() {
    let (_, structure, _, _, report) = extract_content("excluded-content");
    assert_eq!(report.attribute_files, 1, "the fixture commits one");

    let by_path = treepo_vcs::walk::by_path(&structure.records);
    let vendored = by_path[&path("vendor/thirdparty/big.js")];
    assert_eq!(
        vendored.size.category_bytes.keys().next(),
        Some(&ContentCategory::Generated),
        "`vendor/** linguist-vendored` reaches the file"
    );
    // It is still JavaScript, and still counted — a marker changes the category, not the
    // language, and generated content has real lines.
    assert!(vendored.size.lines.total > 0);
    assert_eq!(vendored.size.language_bytes.len(), 1);

    // A file the pattern does not cover keeps the category its suffix implies.
    let source = by_path[&path("src/main.rs")];
    assert_eq!(
        source.size.category_bytes.keys().next(),
        Some(&ContentCategory::Code)
    );
    assert!(
        by_path[&RepoPath::root()]
            .derived
            .generated_debt
            .expect("measured")
            > treepo_det::Fx::ZERO
    );
}

/// | One enormous file | ... | `P7` soft clamp; the outlier is data, not an error. |
///
/// The content half: an 8 MB binary asset is categorized without being read. `find_header`
/// gave the walk its size for free, and opening it would buy nothing but the `AC-EXT-1`
/// budget's worst minute.
#[test]
fn an_enormous_binary_asset_is_measured_without_being_read() {
    let (_, structure, _, _, report) = extract_content("huge-file");
    let by_path = treepo_vcs::walk::by_path(&structure.records);

    let enormous = by_path[&path("assets/enormous.bin")];
    assert!(enormous.size.bytes >= 8 * 1024 * 1024);
    assert_eq!(
        enormous.size.category_bytes.keys().next(),
        Some(&ContentCategory::Binary)
    );
    assert_eq!(enormous.size.lines.total, 0, "never opened");
    assert_eq!(enormous.size.language_bytes.len(), 0);

    // Only the two small sources were read, and the default cap was never reached — the
    // asset was skipped by category, which is the cheaper of the two guards.
    assert_eq!(report.scanned, 2);
    assert_eq!(report.too_large, 0);

    // And the debt signal still sees it, because bytes need no read.
    let root = by_path[&RepoPath::root()];
    assert!(root.derived.large_file_debt.expect("measured") > treepo_det::Fx::from_ratio(9, 10));
}

/// A repository with no commits has no blobs; the pass must produce nothing, not fail.
#[test]
fn scanning_an_empty_repository_measures_nothing() {
    let (_, structure, _, languages, report) = extract_content("empty");
    assert_eq!(report, ScanReport::default());
    assert!(languages.is_empty());
    let root = &structure.records[0];
    assert_eq!(root.size.lines.total, 0);
    assert!(root.size.category_bytes.is_empty());
    // Nothing measured is not zero measured, all the way down.
    assert_eq!(root.derived.comment_density, None);
    assert_eq!(root.derived.generated_debt, None);
    assert_eq!(root.temporal.stability, None);
}

/// The no-repository fallback reads content from disk, since there is no tree to read it
/// from. Same records, different source (PRD §6, `AC-ASSOC-3`).
#[test]
fn a_plain_directory_still_gets_its_content_counted() {
    let (_, structure, _, languages, report) = extract_content("no-git");
    assert!(report.scanned > 0, "files on disk are still read");
    assert!(!languages.is_empty());
    let root = &structure.records[0];
    assert!(root.size.lines.total > 0);
    assert!(root.derived.comment_density.is_some());
}

/// `F-EXT-5` against a fixture git built: a `vendor` marked `linguist-vendored`, holding a
/// `src` that belongs to somebody else.
#[test]
fn a_vendored_folder_is_signalled_and_encloses_what_it_contains() {
    let (_, structure, _, _, _) = extract_content("excluded-content");
    let by_path = treepo_vcs::walk::by_path(&structure.records);

    let vendor = by_path[&path("vendor")]
        .folder_signal
        .as_ref()
        .expect("vendor carries a signal");
    assert_eq!(&*vendor.signal_name, "vendor");
    // Somebody else's code is weighted well below the project's own.
    assert!(vendor.effective_weight < treepo_det::Fx::from_ratio(1, 2));
    assert!(!vendor.is_nested());

    // `src` is a signal wherever it is, and the record says whose src it is.
    let theirs = by_path[&path("src")]
        .folder_signal
        .as_ref()
        .expect("src carries a signal");
    assert!(!theirs.position_in_hierarchy.is_within("vendor"));

    // The marked subtree reads as generated, which is what modulates the weight.
    assert!(
        by_path[&path("vendor")]
            .size
            .category_bytes
            .contains_key(&ContentCategory::Generated)
    );
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
