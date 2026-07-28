//! `AC-MAN-1`, `AC-MAN-4` and `F-MAN-8` against manifests built from real repositories.
//!
//! The unit tests in `manifest_io` use a hand-built manifest with every field set, which is
//! the right instrument for the *encoding*. It is the wrong instrument for `AC-MAN-1`, whose
//! whole question is whether anything in the pipeline leaks in — a clock, an environment
//! variable, a hash seed, a directory read order. Only a manifest extracted twice from a real
//! repository can answer that.
//!
//! Manifest assembly is [`treepo_vcs::extract`] — the product API. These tests call it rather
//! than composing passes themselves.

use std::path::PathBuf;
use treepo_model::Manifest;
use treepo_store::{RepositoryStore, StoreRoot};
use treepo_vcs::lang::Catalogue;
use treepo_vcs::{ExtractOptions, FilterSet, discover, extract};

/// Builds the corpus once per test binary, then hands out fixture paths.
fn fixture(name: &str) -> PathBuf {
    static ONCE: std::sync::Once = std::sync::Once::new();
    let root = corpus::default_root();
    ONCE.call_once(|| {
        corpus::ensure(&root).expect("the corpus builds");
    });
    root.join(name)
}

fn scratch(name: &str) -> StoreRoot {
    let root = corpus::default_root()
        .parent()
        .expect("target/")
        .join("persistence-scratch")
        .join(name);
    if root.exists() {
        std::fs::remove_dir_all(&root).expect("clearing scratch");
    }
    StoreRoot::at(root)
}

/// Discover, resolve identity, extract — the product open path in miniature.
fn extract_fixture(path: &std::path::Path) -> (Manifest, treepo_store::Resolution) {
    let target = discover(path).expect("the fixture opens");
    let resolution =
        treepo_store::resolve(target.root(), target.repository(), 0).expect("identity");
    let manifest = extract(
        &target,
        &FilterSet::built_in(),
        &Catalogue::built_in(),
        treepo_store::resolve::root_seed(&resolution.identity),
        env!("CARGO_PKG_VERSION").to_string(),
        ExtractOptions::default(),
    )
    .expect("extraction");
    (manifest, resolution)
}

fn store_for(root: &StoreRoot, resolution: &treepo_store::Resolution) -> RepositoryStore {
    root.repository(&resolution.identity)
}

/// **`AC-MAN-1`** — deleting the store and extracting again reproduces identical bytes.
///
/// Five shapes rather than one: history and no history, one author and many, a `.mailmap`,
/// deep nesting, and filtered content. The failure this guards against is a value that varies
/// between runs, and different shapes populate different primitives.
#[test]
fn deleting_the_store_and_regenerating_reproduces_identical_bytes() {
    for name in [
        "single-author",
        "many-authors",
        "mailmap",
        "deep-nesting",
        "excluded-content",
    ] {
        let root = scratch(&format!("regenerate-{name}"));
        let path = fixture(name);

        let (first, resolution) = extract_fixture(&path);
        let store = store_for(&root, &resolution);
        treepo_store::write(&store, &first).expect("the first write");
        let before = std::fs::read(store.manifest_file()).expect("the first manifest");

        // Delete the store, exactly as `F-MAN-8` allows a user to.
        std::fs::remove_dir_all(store.dir()).expect("deleting the store");
        assert!(matches!(
            treepo_store::read(&store),
            Err(treepo_store::ReadError::Absent)
        ));

        let (second, again) = extract_fixture(&path);
        assert_eq!(
            again.identity, resolution.identity,
            "{name}: identity is stable across a re-extraction"
        );
        treepo_store::write(&store, &second).expect("the second write");
        let after = std::fs::read(store.manifest_file()).expect("the second manifest");

        assert_eq!(first, second, "{name}: the manifest itself is reproducible");
        assert_eq!(before, after, "{name}: and so are its bytes");
        assert!(!before.is_empty());
    }
}

/// A manifest built from a real repository survives the round trip unchanged.
///
/// The populated-by-hand unit test covers every field; this one covers every field a real
/// repository actually produces, which is a different set — and a smaller one, which is why
/// both exist.
#[test]
fn a_real_manifest_round_trips_through_the_store() {
    for name in ["single-author", "mailmap", "no-git", "empty"] {
        let root = scratch(&format!("round-trip-{name}"));
        let (manifest, resolution) = extract_fixture(&fixture(name));
        let store = store_for(&root, &resolution);

        treepo_store::write(&store, &manifest).expect("the write");
        let read_back = treepo_store::read(&store).expect("the read");
        assert_eq!(manifest, read_back, "{name}");
        assert!(store.manifest_meta_file().is_file(), "{name}: sidecar");
    }
}

/// **`AC-MAN-4`** — two clones of one remote resolve to one store, and the second open finds
/// the first's manifest instead of extracting again.
///
/// The identity half was settled in `identity.rs`; this is the half that needed a manifest to
/// exist. What makes it a real test of "skips extraction" is that the second clone is never
/// extracted at all — the manifest read back is the one built from the *first* checkout.
#[test]
fn the_second_clone_of_a_remote_opens_the_first_ones_store() {
    let root = scratch("two-clones");
    let base = root.path().join("checkouts");

    let mut paths = Vec::new();
    for (dir, url) in [
        ("checkout-a", "https://github.com/Example/Widget.git"),
        ("checkout-b", "https://github.com/example/widget"),
    ] {
        let path = base.join(dir);
        let mut builder = corpus::Builder::init(path.clone(), dir).expect("git init");
        builder.write_source("src/main.rs", 20).expect("a file");
        builder.commit("first").expect("a commit");
        builder
            .git(&["remote", "add", "origin", url])
            .expect("a remote");
        paths.push(path);
    }

    let (first, resolution_a) = extract_fixture(&paths[0]);
    let store_a = store_for(&root, &resolution_a);
    treepo_store::write(&store_a, &first).expect("the write");

    // The second open: discover, resolve, look in the store. No extraction.
    let target = discover(&paths[1]).expect("the second clone opens");
    let resolution_b =
        treepo_store::resolve(target.root(), target.repository(), 0).expect("its identity");
    let store_b = store_for(&root, &resolution_b);

    assert_eq!(store_a, store_b, "one remote, one store");
    let found = treepo_store::read(&store_b).expect("the first checkout's manifest");
    assert_eq!(found, first, "and it is exactly what the first open wrote");
    assert!(
        !found.paths().is_empty(),
        "a store that answers with nothing would pass a weaker assertion"
    );
}

/// `F-MAN-2` — a complete open leaves the store laid out as the PRD says, and the identity it
/// recorded is the one the repository resolves to.
///
/// The second half is what `identity.json` is for: `F-MAN-9`'s browser has a directory of hex
/// and needs to tell a user which repository it belongs to, without the repository being to
/// hand. Reading the file back and comparing against a fresh resolution is that question asked
/// from both ends.
#[test]
fn a_complete_open_writes_the_layout_f_man_2_specifies() {
    let root = scratch("layout");
    let path = fixture("multi-remote");
    let (manifest, resolution) = extract_fixture(&path);
    let store = store_for(&root, &resolution);

    treepo_store::identity_io::write(&store, &resolution).expect("the identity");
    treepo_store::write(&store, &manifest).expect("the manifest");

    for file in [
        store.identity_file(),
        store.manifest_file(),
        store.manifest_meta_file(),
    ] {
        assert!(file.is_file(), "{} is missing", file.display());
    }
    assert_eq!(
        store.dir().file_name().and_then(|n| n.to_str()),
        Some(resolution.identity.directory_name().as_str()),
        "the directory is named by the identity"
    );

    let recorded = treepo_store::identity_io::read(&store).expect("reading it back");
    assert_eq!(recorded, resolution, "the store knows what it holds");

    // And the file says so in words, which is the half a test cannot assert structurally.
    let text = std::fs::read_to_string(store.identity_file()).expect("the file");
    assert!(text.contains("example.invalid/backup"), "{text}");
    assert!(text.contains("\"chosen_remote\": \"backup\""), "{text}");
}

/// A raw remote URL must not reach the store, however it got into `.git/config`.
///
/// `resolve` strips credentials and `Resolution` never carries a raw URL, so this is a check
/// that no *later* stage reintroduced one — the store is the last place a token could end up
/// on disk, and the first place someone would find it.
#[test]
fn a_credential_in_a_remote_url_never_lands_in_the_store() {
    let root = scratch("credentials");
    let path = root.path().join("checkout");
    let mut builder = corpus::Builder::init(path.clone(), "with-token").expect("git init");
    builder.write_source("src/main.rs", 12).expect("a file");
    builder.commit("first").expect("a commit");
    builder
        .git(&[
            "remote",
            "add",
            "origin",
            "https://x-access-token:ghp_notarealsecret@github.com/example/widget.git",
        ])
        .expect("a remote");

    // The premise, asserted rather than assumed: a test that scans for a token the fixture
    // never contained would pass whatever the store did.
    let config = std::fs::read_to_string(path.join(".git").join("config")).expect("git config");
    assert!(
        config.contains("ghp_notarealsecret"),
        "the token is really there"
    );

    let (manifest, resolution) = extract_fixture(&path);
    let store = store_for(&root, &resolution);
    treepo_store::identity_io::write(&store, &resolution).expect("the identity");
    treepo_store::write(&store, &manifest).expect("the manifest");

    for file in [
        store.identity_file(),
        store.manifest_file(),
        store.manifest_meta_file(),
    ] {
        let bytes = std::fs::read(&file).expect("reading it back");
        let haystack = String::from_utf8_lossy(&bytes);
        assert!(
            !haystack.contains("ghp_notarealsecret"),
            "{} carries the token",
            file.display()
        );
        assert!(
            !haystack.contains("x-access-token"),
            "{} carries the credential",
            file.display()
        );
    }
    assert_eq!(
        resolution.identity.source_value, "github.com/example/widget",
        "and the identity is still the repository's"
    );
}

/// `F-MAN-8` — the store is regenerable, and every way of losing it says so.
#[test]
fn every_way_of_losing_the_store_asks_for_regeneration_rather_than_failing() {
    let root = scratch("regenerable");
    let (manifest, resolution) = extract_fixture(&fixture("single-author"));
    let store = store_for(&root, &resolution);

    let absent = treepo_store::read(&store).expect_err("nothing stored yet");
    assert!(absent.is_regenerable(), "a first open extracts");

    treepo_store::write(&store, &manifest).expect("the write");
    assert!(treepo_store::read(&store).is_ok());

    // A manifest from a future release.
    let path = store.manifest_file();
    let mut bytes = std::fs::read(&path).expect("reading it back");
    let version = treepo_store::manifest_io::schema_version_of(&bytes).expect("a header");
    bytes[7..11].copy_from_slice(&(version + 1).to_le_bytes());
    std::fs::write(&path, bytes).expect("rewriting");

    let stale = treepo_store::read(&store).expect_err("a newer schema");
    assert!(matches!(
        stale,
        treepo_store::ReadError::SchemaMismatch { .. }
    ));
    assert!(
        stale.is_regenerable(),
        "regenerate, never best-effort parse"
    );
}

/// A manifest is a repository's, not a folder's — the point of `AC-MAN-5`.
///
/// The no-remote fixture is copied to a second path and extracted there. Same history, same
/// identity, same store, byte-identical manifest.
#[test]
fn the_same_history_at_a_second_path_writes_the_same_manifest() {
    let root = scratch("moved");
    let original = fixture("no-remote");
    let moved = root.path().join("elsewhere").join("renamed");
    std::fs::create_dir_all(moved.parent().expect("a parent")).expect("the new home");
    copy_tree(&original, &moved);

    let (here, resolution_here) = extract_fixture(&original);
    let (there, resolution_there) = extract_fixture(&moved);
    assert_eq!(resolution_here.identity, resolution_there.identity);

    let store = store_for(&root, &resolution_here);
    treepo_store::write(&store, &here).expect("the first write");
    let before = std::fs::read(store.manifest_file()).expect("bytes");
    treepo_store::write(&store, &there).expect("the second write");
    let after = std::fs::read(store.manifest_file()).expect("bytes");

    assert_eq!(
        before, after,
        "the manifest describes the repository, not where it sits"
    );
}

/// A recursive copy, because a rename would move the shared corpus fixture out from under
/// every other test in the binary.
fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).expect("the destination");
    for entry in std::fs::read_dir(from).expect("reading the source") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a file type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("copying a file");
        }
    }
}
