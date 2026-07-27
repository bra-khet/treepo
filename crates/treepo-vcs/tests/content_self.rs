//! The content pass against a real repository: treepo's own.
//!
//! The unit tests in `lang.rs` build synthetic records, which proves the classification and
//! the arithmetic but not that reading blobs out of a real tree produces the numbers this
//! crate thinks it does. This one runs [`walk`] and then [`scan`] over the repository that is
//! guaranteed to be present wherever the suite runs, and checks what later phases assume.
//!
//! It also serves as the `AC-EXT-1` measurement point for the content half of extraction:
//! treepo is a T1 repository, so the numbers here are a floor, not the budget.

use treepo_model::manifest::{LanguageTable, NodeKind};
use treepo_model::path::RepoPath;
use treepo_model::primitives::size::ContentCategory;
use treepo_vcs::lang::{Catalogue, ContentOptions, apply_history_signals, scan};
use treepo_vcs::{FilterSet, Structure, Target, WalkOptions, discover, walk};

use std::path::{Path, PathBuf};

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

/// Walks and scans this repository, returning everything a caller would hold afterwards.
fn scan_self() -> (Structure, LanguageTable, treepo_vcs::ScanReport) {
    let target = discover(workspace_root()).expect("treepo's own repository opens");
    assert!(matches!(target, Target::Repository(_)));
    let mut structure =
        walk(&target, &FilterSet::built_in(), WalkOptions::default()).expect("walk succeeds");
    let mut languages = LanguageTable::new();
    let report = scan(
        &target,
        &mut structure,
        &Catalogue::built_in(),
        &mut languages,
        ContentOptions::default(),
    )
    .expect("content scan succeeds");
    (structure, languages, report)
}

#[test]
fn the_scan_reads_this_repository_as_the_rust_project_it_is() {
    let (structure, languages, report) = scan_self();
    let by_path = treepo_vcs::walk::by_path(&structure.records);
    let root = by_path[&RepoPath::root()];

    assert!(report.scanned > 20, "most of this repository is text");
    assert_eq!(report.too_large, 0, "nothing here is pathologically large");

    // Rust dominates by bytes, and the dominant-language accessor agrees.
    let rust = languages.get("Rust").expect("Rust was interned");
    assert_eq!(root.size.dominant_language(), Some(rust));
    assert!(root.size.language_count() >= 3, "Rust, TOML, Markdown");

    // The three categories this repository is actually made of.
    for category in [
        ContentCategory::Code,
        ContentCategory::Config,
        ContentCategory::Docs,
    ] {
        assert!(
            root.size
                .category_bytes
                .get(&category)
                .copied()
                .unwrap_or(0)
                > 0,
            "{category:?} should be present"
        );
    }

    // And a specific file, counted rather than guessed at.
    let lib = by_path[&path("crates/treepo-det/src/lib.rs")];
    assert!(lib.size.lines.total > 10);
    assert_eq!(
        lib.size.lines.code + lib.size.lines.comment + lib.size.lines.blank,
        lib.size.lines.total
    );
    assert!(
        lib.size.lines.comment > 0,
        "the crate root is nothing but module docs"
    );
}

/// The rollup is only meaningful if the totals add up, same as for the structural pass.
#[test]
fn subtree_line_and_category_totals_agree_with_their_parts() {
    let (structure, _, _) = scan_self();
    let by_path = treepo_vcs::walk::by_path(&structure.records);
    let root = by_path[&RepoPath::root()];

    let file_lines: u64 = structure
        .records
        .iter()
        .filter(|record| record.kind == NodeKind::File)
        .map(|record| record.size.lines.total)
        .sum();
    assert_eq!(root.size.lines.total, file_lines);

    let category_bytes: u64 = root.size.category_bytes.values().sum();
    let file_bytes: u64 = structure
        .records
        .iter()
        .filter(|record| record.kind == NodeKind::File)
        .map(|record| record.size.bytes)
        .sum();
    assert_eq!(
        category_bytes, file_bytes,
        "every file lands in exactly one category"
    );

    // One subtree, checked independently of the root.
    let crates = by_path[&path("crates")];
    let beneath: u64 = structure
        .records
        .iter()
        .filter(|record| record.kind == NodeKind::File && record.path.starts_with(&path("crates")))
        .map(|record| record.size.lines.total)
        .sum();
    assert_eq!(crates.size.lines.total, beneath);
}

/// `F-EXT-4` closes three fields that were deliberately `None` until it existed.
#[test]
fn the_deferred_fields_are_now_measured() {
    let (mut structure, _, _) = scan_self();
    apply_history_signals(&mut structure.records);
    let by_path = treepo_vcs::walk::by_path(&structure.records);
    let root = by_path[&RepoPath::root()];

    assert!(
        root.structural.balance.kind.is_some(),
        "BalanceScore::kind is measured for a directory"
    );
    assert!(
        root.derived.is_measured(),
        "DerivedSignals are no longer all None"
    );
    assert!(root.derived.comment_density.is_some());
    assert!(root.derived.generated_debt.is_some());

    // `stability` needs `log_pass` to have filled churn; without it every path reads as
    // perfectly stable, which is correct for "no churn recorded" and is what this asserts.
    let lib = by_path[&path("crates/treepo-det/src/lib.rs")];
    assert!(
        lib.temporal.stability.is_some(),
        "a file with lines has a stability denominator"
    );
}

/// A file has one category; a directory has a mix. Nothing has zero.
#[test]
fn every_file_lands_in_exactly_one_category() {
    let (structure, _, _) = scan_self();
    for record in &structure.records {
        if record.kind != NodeKind::File {
            continue;
        }
        assert_eq!(
            record.size.category_bytes.len(),
            1,
            "{} has {} categories",
            record.path,
            record.size.category_bytes.len()
        );
        assert!(
            record.size.language_bytes.len() <= 1,
            "{} claims more than one language",
            record.path
        );
    }
}

/// The catalogue is partial on purpose, but it should not be *mostly* partial on the
/// repository it ships with. A silent drop to `Unknown` is exactly what this guards.
#[test]
fn almost_nothing_in_this_repository_is_unrecognized() {
    let (structure, _, _) = scan_self();
    let by_path = treepo_vcs::walk::by_path(&structure.records);
    let root = by_path[&RepoPath::root()];

    let total: u64 = root.size.category_bytes.values().sum();
    let unknown = root
        .size
        .category_bytes
        .get(&ContentCategory::Unknown)
        .copied()
        .unwrap_or(0);
    assert!(
        unknown * 10 < total,
        "{unknown} of {total} bytes unrecognized — the catalogue has a gap"
    );
}

/// `AC-DET-2`: the same repository must produce the same content twice, including the
/// language ids, which are minted in walk order and would drift if the walk did.
#[test]
fn scanning_twice_produces_identical_content() {
    let (first, first_languages, first_report) = scan_self();
    let (second, second_languages, second_report) = scan_self();
    assert_eq!(first.records, second.records);
    assert_eq!(first_report, second_report);
    assert_eq!(first_languages.len(), second_languages.len());
    for index in 0..first_languages.len() {
        let id =
            treepo_model::manifest::LanguageId::new(u16::try_from(index).expect("few languages"));
        assert_eq!(
            first_languages.name(id),
            second_languages.name(id),
            "language ids must be minted in the same order"
        );
    }
}
