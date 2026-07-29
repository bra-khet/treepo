//! `AC-ID-1` end to end, and `F-ID-1`/`F-ID-7` against real repositories.
//!
//! > With default settings, no real name, email, or handle of any contributor other than the
//! > user appears anywhere in the UI or in any exported file, including its metadata.
//!
//! The unit tests in `treepo-id::policy` hold the *gate* — that a pseudonymous view produces
//! no real identity, because it holds none. That is necessary and it is not sufficient: it
//! proves the gate is shut, not that nothing routes around it. This file extracts a real
//! repository whose contributors' names and addresses are known, runs the whole pipeline,
//! and asserts those strings appear nowhere in what comes out.
//!
//! Two layers are checked, and they fail for different reasons:
//!
//! * **The manifest** — what gets persisted, and what `F-MAN-11` would share. A name here
//!   would mean `treepo-model`'s no-names discipline had been broken at extraction.
//! * **The rendered identifications** — what a person sees. A name here would mean the
//!   policy gate had been bypassed.
//!
//! Placed under `crates/treepo-vcs/tests/` rather than the workspace-root `tests/` the
//! campaign names, for the reason `degenerate.rs` gives: this is a virtual workspace with no
//! root package, so a root `tests/` directory is not a cargo target. It lives in `treepo-vcs`
//! rather than `treepo-id` because it needs extraction, and `treepo-id` must never depend on
//! a crate that opens files. `treepo-id` is a dev-dependency here, which `xtask dep-guard`
//! does not traverse (`--edges normal,build`), so `N6` is unaffected.

use std::path::PathBuf;
use treepo_id::{Identification, IdentityPolicy, IdentityView, Palette, Wordlist};
use treepo_model::identity::AuthorKey;
use treepo_model::manifest::Manifest;
use treepo_vcs::lang::Catalogue;
use treepo_vcs::{
    ExtractOptions, FilterSet, IdentityScope, Target, discover, extract, self_identity,
};

fn fixture(name: &str) -> PathBuf {
    static ONCE: std::sync::Once = std::sync::Once::new();
    let root = corpus::default_root();
    ONCE.call_once(|| {
        corpus::ensure(&root).expect("the corpus builds");
    });
    root.join(name)
}

fn manifest_of(name: &str) -> (Target, Manifest) {
    let target = discover(fixture(name)).expect("discover");
    let manifest = extract(
        &target,
        &FilterSet::built_in(),
        &Catalogue::built_in(),
        // Fixed rather than resolved: identity is `treepo-store`'s concern and a path hash
        // would put the checkout directory into the answer.
        treepo_model::Manifest::root_seed_for(b"treepo/privacy-test"),
        "test".to_owned(),
        ExtractOptions::default(),
    )
    .expect("extraction");
    (target, manifest)
}

/// Every real identity string the corpus puts into the fixtures this file uses.
///
/// Lower case, because the comparison folds case — a leak that arrived capitalized
/// differently is still a leak.
const IDENTITIES: &[&str] = &[
    // The configured viewer, in every fixture (`tools/corpus/src/lib.rs`).
    "corpus builder",
    "corpus@treepo.invalid",
    // `mailmap`.
    "lovelace",
    "ada@example.invalid",
    "ada@work.example.invalid",
    "a.lovelace@old.example.invalid",
    "bob@example.invalid",
    // `many-authors`.
    "person0@example.invalid",
    "person59@example.invalid",
    // The shared suffix, which catches any address the lists above missed.
    "@example.invalid",
    "@treepo.invalid",
];

/// Asserts no known identity appears in `haystack`.
fn assert_carries_no_identity(what: &str, haystack: &str) {
    let folded = haystack.to_ascii_lowercase();
    for needle in IDENTITIES {
        assert!(
            !folded.contains(needle),
            "{what} contains the real identity `{needle}` (AC-ID-1)"
        );
    }
}

/// `F-ID-1`. The corpus sets `user.email` in each fixture's own config, so this exercises
/// the repository-scoped branch and does not depend on the machine's global git setup.
#[test]
fn a_repository_config_identifies_the_viewer() {
    let target = discover(fixture("single-author")).expect("discover");
    let me = self_identity(&target).expect("the corpus configures user.email");
    assert_eq!(me.scope(), IdentityScope::Repository);
    assert_eq!(me.key(), AuthorKey::from_email(b"corpus@treepo.invalid"));
}

/// PRD §6, "No `.git`": a plain directory has no config, so there is no viewer. An ordinary
/// state, reached by a different route than `F-ID-7`'s.
#[test]
fn a_plain_directory_has_no_viewer() {
    let target = discover(fixture("no-git")).expect("discover");
    assert!(matches!(target, Target::PlainDirectory { .. }));
    assert!(self_identity(&target).is_none());
}

/// `F-ID-1` reaching the manifest: the viewer committed here, so they are marked.
#[test]
fn a_contributing_viewer_is_marked_in_the_manifest() {
    let (_, manifest) = manifest_of("single-author");
    let me = AuthorKey::from_email(b"corpus@treepo.invalid");
    assert_eq!(manifest.authors.self_author(), Some(me));
    // Exactly one, or `self_author` would be reporting whichever came first in hash order.
    assert_eq!(
        manifest.authors.iter().filter(|(_, e)| e.is_self).count(),
        1
    );
}

/// `F-ID-7`, on a real repository. Every commit in the `mailmap` fixture is authored by
/// someone other than the configured identity — the shape of every repository a user merely
/// clones. Nothing is marked, and nothing errors.
#[test]
fn a_viewer_who_never_committed_here_is_the_ordinary_state() {
    let (target, manifest) = manifest_of("mailmap");
    assert!(
        self_identity(&target).is_some(),
        "the viewer is configured — they simply have no commits here"
    );
    assert_eq!(manifest.authors.self_author(), None);
    assert!(
        !manifest.authors.is_empty(),
        "the fixture has contributors, they are just not the viewer"
    );

    let wordlist = Wordlist::built_in();
    let palette = Palette::built_in();
    let view = IdentityView::pseudonymous(
        wordlist.assign(manifest.authors.iter().map(|(&key, _)| key)),
        &palette,
        manifest.authors.self_author(),
    );
    assert!(!view.viewer_is_a_contributor());
    for (_, identification) in view.contributors() {
        assert!(matches!(identification, Identification::Pseudonymous(_)));
    }
}

/// `AC-ID-1`, the layer that gets persisted and shared.
///
/// `treepo-model` has no field a name could live in, so this is really a test that
/// extraction did not smuggle one into a string field — a language name, a path, a version.
#[test]
fn no_manifest_carries_a_contributor_identity() {
    for name in ["mailmap", "many-authors", "single-author"] {
        let (_, manifest) = manifest_of(name);
        assert!(!manifest.authors.is_empty(), "{name} has contributors");
        assert_carries_no_identity(&format!("the {name} manifest"), &format!("{manifest:?}"));
    }
}

/// `AC-ID-1`, the layer a person sees. Sixty contributors with known names and addresses,
/// every one rendered through the default view.
#[test]
fn no_rendered_identification_carries_a_contributor_identity() {
    let (_, manifest) = manifest_of("many-authors");
    assert!(
        manifest.authors.len() > 50,
        "the fixture is the crowded one"
    );

    let wordlist = Wordlist::built_in();
    let palette = Palette::built_in();
    let view = IdentityView::pseudonymous(
        wordlist.assign(manifest.authors.iter().map(|(&key, _)| key)),
        &palette,
        manifest.authors.self_author(),
    );
    assert_eq!(view.policy(), IdentityPolicy::Pseudonymous);

    let rendered: String = view
        .contributors()
        .map(|(_, identification)| identification.to_string())
        .collect::<Vec<_>>()
        .join("\n");

    assert_eq!(rendered.lines().count(), manifest.authors.len());
    assert_carries_no_identity("the rendered contributor list", &rendered);

    // The viewer committed the initial commit here, so `You` is present — and their own
    // name is not. That is the tightening `Identification::Yourself` documents: `AC-ID-1`
    // would permit the viewer's name, and showing it would mean a shared tree announced who
    // made it.
    assert!(view.viewer_is_a_contributor());
    assert!(rendered.lines().any(|line| line == "You"), "{rendered}");
}

/// The same fixture with reveal on, so the test above is known to be measuring something.
///
/// If a pseudonymous view and a revealed one produced the same strings, every assertion
/// about the pseudonymous one would be vacuous.
#[test]
fn revealing_is_what_makes_the_default_view_worth_asserting() {
    let (_, manifest) = manifest_of("mailmap");
    let wordlist = Wordlist::built_in();
    let palette = Palette::built_in();
    let keys: Vec<AuthorKey> = manifest.authors.iter().map(|(&key, _)| key).collect();

    // A name table the way Phase 10 will build one — from the repository, never from the
    // manifest, which has no names to give.
    let names: treepo_id::RealNames = keys
        .iter()
        .map(|&key| (key, "Ada Lovelace".to_owned()))
        .collect();

    let revealed = IdentityView::revealed(
        wordlist.assign(keys.iter().copied()),
        &palette,
        manifest.authors.self_author(),
        names,
    );
    assert_eq!(revealed.policy(), IdentityPolicy::Revealed);

    let shown: String = revealed
        .contributors()
        .map(|(_, identification)| identification.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        shown.to_ascii_lowercase().contains("lovelace"),
        "reveal showed nothing, so the default view's silence proves nothing"
    );

    // And the colours did not move — `F-ID-4`, at the pipeline level rather than the unit.
    let hidden = IdentityView::pseudonymous(
        wordlist.assign(keys.iter().copied()),
        &palette,
        manifest.authors.self_author(),
    );
    for key in &keys {
        assert_eq!(hidden.color_of(key), revealed.color_of(key));
    }
}
