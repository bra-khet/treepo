//! Opening a repository: identity, then a manifest, then a tree.
//!
//! Everything here is blocking and none of it knows about Bevy — it is a plain function from a
//! path to a [`WorldSnapshot`], which is what lets [`phase`](crate::phase) run it on a
//! background thread without a `World` anywhere near it. The split is architecture D1 applied
//! one level down: the *decision* to run this off-thread is an app concern, and the work
//! itself has no reason to be able to see the ECS.
//!
//! # The order, and what each step is for
//!
//! ```text
//! discover   F-ASSOC-2   is this a repository, is it shallow, does it have commits
//! resolve    F-MAN-3     which store does it own, by remote / root commit / path
//! read       F-MAN-8     is there a manifest already, and is it still current
//! extract    F-EXT-*     one history traversal, if there is not
//! grow       F-SKEL-*    manifest → skeleton
//! materialize F-MAT-1..4 skeleton → material
//! enrich     F-MAT-5     material → what is built on it
//! ```
//!
//! # Staleness is decided on HEAD, not on time
//!
//! A stored manifest is reused only when its `built_from_commit` matches the repository's
//! current HEAD. Anything else — a commit since the last open, a branch switch, a manifest
//! from before a schema bump — re-extracts.
//!
//! This is deliberately the blunt version of `AC-EXT-2`. The acceptance criterion asks for
//! *incremental* re-extraction, where one new commit costs one commit's work; what happens
//! here is that one new commit costs a full traversal. That is Phase 6/7 territory — a Grow
//! trigger is the thing that notices a repository has moved (`F-GROW-2`) — and the cheap wrong
//! alternative is worse than the expensive right one: showing yesterday's tree because
//! yesterday's tree was already on disk is the kind of bug a user cannot diagnose.
//!
//! `NFR-4`'s five-second cold launch is still met where it is claimed, because the criterion
//! is about *a cached repository*, and an unchanged repository is exactly the case that hits
//! the cache.

use std::fmt;
use std::path::{Path, PathBuf};

use treepo_gen::{MaterialTable, Table};
use treepo_model::{Manifest, WorldSnapshot};
use treepo_store::identity_io::tier_name;
use treepo_store::{RepositoryStore, StoreRoot};
use treepo_vcs::lang::Catalogue;
use treepo_vcs::{ExtractOptions, FilterSet, Notice};

/// A repository, opened and grown.
#[derive(Debug)]
pub(crate) struct Opened {
    /// What the render layer draws.
    pub(crate) snapshot: WorldSnapshot,
    /// What the UI says about it.
    pub(crate) summary: Summary,
}

/// What one opened repository is worth telling the user.
///
/// Strings and counts rather than borrowed model types, because this crosses a thread boundary
/// and lands in a text overlay. No contributor appears in it at all — `N9` means the shell has
/// no reason to hold an identity, and `N4` means a count of people is the only shape a
/// contributor total may take.
#[derive(Debug, Clone)]
pub(crate) struct Summary {
    /// The directory that was opened.
    pub(crate) root: PathBuf,
    /// Which identity tier claimed it (`F-MAN-3`).
    pub(crate) tier: &'static str,
    /// Whether the manifest came off disk rather than out of a traversal.
    pub(crate) from_cache: bool,
    /// Paths in the manifest.
    pub(crate) paths: usize,
    /// Nodes in the skeleton.
    pub(crate) nodes: usize,
    /// Drawn segments.
    pub(crate) segments: usize,
    /// Aggregate containers (`F-SKEL-7`).
    pub(crate) aggregates: usize,
    /// Everything `F-ASSOC-2` says the user must be told.
    pub(crate) notices: Vec<&'static str>,
}

/// Opens `root`, reusing a stored manifest when it is current and extracting when it is not.
///
/// # Errors
///
/// [`OpenError`] for anything that stops a tree existing. A shallow clone, a repository with
/// no commits, and a directory with no `.git` are **not** errors — each grows a tree and
/// reports a [`Notice`](treepo_vcs::Notice) instead (PRD §6).
pub(crate) fn open(root: &Path) -> Result<Opened, OpenError> {
    let target = treepo_vcs::discover(root).map_err(|source| OpenError::Discover {
        path: root.to_path_buf(),
        detail: source.to_string(),
    })?;

    // `identity_io::now` is the one sanctioned wall-clock read in the product's own crates
    // (`N3`). It records when the identity was resolved and never reaches the seed tree, so
    // it cannot make a tree depend on when it was opened.
    let resolution = treepo_store::resolve(
        target.root(),
        target.repository(),
        treepo_store::identity_io::now(),
    )
    .map_err(|source| OpenError::Resolve {
        detail: source.to_string(),
    })?;

    let store_root = StoreRoot::platform().map_err(|source| OpenError::Store {
        detail: source.to_string(),
    })?;
    let store = store_root.repository(&resolution.identity);

    let (manifest, from_cache) = match cached(&store, target.head()) {
        Some(manifest) => (manifest, true),
        None => {
            let manifest = treepo_vcs::extract(
                &target,
                &FilterSet::built_in(),
                &Catalogue::built_in(),
                treepo_store::resolve::root_seed(&resolution.identity),
                env!("CARGO_PKG_VERSION").to_owned(),
                ExtractOptions::default(),
            )
            .map_err(|source| OpenError::Extract {
                detail: source.to_string(),
            })?;

            // Best effort, and deliberately so. A store that cannot be written costs the next
            // launch its speed; failing the open over it would cost the user their tree. What
            // must never be silent is a failure to *read* the repository, and that is above.
            let _ = treepo_store::identity_io::write(&store, &resolution);
            let _ = treepo_store::write(&store, &manifest);

            (manifest, false)
        }
    };

    let skeleton = treepo_gen::grow(&manifest, &Table::built_in());
    let materials_table = MaterialTable::built_in();
    let materials = treepo_gen::materialize(&manifest, &skeleton, &materials_table);
    let enrichments = treepo_gen::enrich(&manifest, &skeleton, &materials, &materials_table);

    let summary = Summary {
        root: target.root().to_path_buf(),
        tier: tier_name(resolution.identity.tier),
        from_cache,
        paths: manifest.paths().len(),
        nodes: skeleton.nodes().len(),
        segments: skeleton.segments().len(),
        aggregates: skeleton.aggregate_count(),
        notices: target.notices().iter().copied().map(notice_text).collect(),
    };

    Ok(Opened {
        snapshot: WorldSnapshot {
            snapshot_id: 0,
            built_from: manifest.built_from_commit,
            skeleton,
            materials,
            enrichments,
        },
        summary,
    })
}

/// The stored manifest, if there is one and it was built from `head`.
///
/// Every [`ReadError`](treepo_store::ReadError) means "no usable manifest" to this caller,
/// including the I/O case: the store answers `is_regenerable` for a *writer* deciding whether
/// to re-extract or complain, and the shell always re-extracts because it has a tree to draw
/// either way.
fn cached(store: &RepositoryStore, head: Option<treepo_model::CommitId>) -> Option<Manifest> {
    let manifest = treepo_store::read(store).ok()?;
    (manifest.built_from_commit == head).then_some(manifest)
}

/// One `F-ASSOC-2` notice as a sentence.
///
/// PRD §6 is emphatic that a shallow clone must be said out loud, because it grows an ageless
/// tree that otherwise looks like a defect. The UI shows these; it does not get to decide
/// whether they are worth showing.
const fn notice_text(notice: Notice) -> &'static str {
    match notice {
        Notice::NoRepository => "no .git — structure only; age, churn and ownership unavailable",
        Notice::ShallowClone => "shallow clone — history is truncated, so this tree has no age",
        Notice::NoCommits => "no commits yet — structure only",
        Notice::DetachedHead => "detached HEAD — showing the checked-out commit",
    }
}

/// Why a repository could not be opened.
#[derive(Debug, Clone)]
pub(crate) enum OpenError {
    /// The path is not something treepo can visualize (`F-ASSOC-2`).
    Discover {
        /// What the user chose.
        path: PathBuf,
        /// What discovery said.
        detail: String,
    },
    /// The repository could not be given an identity (`F-MAN-3`).
    Resolve {
        /// What resolution said.
        detail: String,
    },
    /// The application-data location could not be determined (`F-MAN-2`).
    Store {
        /// What the layout said.
        detail: String,
    },
    /// Extraction failed.
    Extract {
        /// What extraction said.
        detail: String,
    },
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Discover { path, detail } => {
                write!(f, "cannot open {}: {detail}", path.display())
            }
            Self::Resolve { detail } => write!(f, "cannot identify this repository: {detail}"),
            Self::Store { detail } => write!(f, "cannot locate treepo's data directory: {detail}"),
            Self::Extract { detail } => write!(f, "cannot read this repository: {detail}"),
        }
    }
}

impl std::error::Error for OpenError {}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every notice the discovery layer can produce has a sentence here. A `match` rather than
    /// a lookup is what makes that true at compile time — but only if something asserts the
    /// sentences are distinct, since four arms returning the same string also compiles.
    #[test]
    fn every_notice_says_something_of_its_own() {
        let texts: Vec<&str> = [
            Notice::NoRepository,
            Notice::ShallowClone,
            Notice::NoCommits,
            Notice::DetachedHead,
        ]
        .into_iter()
        .map(notice_text)
        .collect();

        for (index, text) in texts.iter().enumerate() {
            assert!(!text.is_empty());
            assert!(!texts[..index].contains(text), "duplicated notice: {text}");
        }
    }

    /// Opening something that is not a directory is an error rather than an empty tree — the
    /// one case where "grow a tree anyway" would be growing one from nothing.
    #[test]
    fn opening_a_path_that_does_not_exist_is_an_error() {
        let missing = std::env::temp_dir().join("treepo-does-not-exist-4f2a9c");
        assert!(matches!(open(&missing), Err(OpenError::Discover { .. })));
    }
}
