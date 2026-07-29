//! Full extraction — every pass, one [`Manifest`](treepo_model::Manifest).
//!
//! The crate's other modules each own one pass. This one is the composition the module
//! documentation promises: walk, content, folder signals, history, and the two history
//! applications, assembled into the type Phase 3 consumes. [`status`](crate::status) is not
//! here — it is an overlay over a finished structure, not extraction, and nothing it produces
//! reaches the manifest.
//!
//! # What the caller supplies
//!
//! * **`root_seed`** — what identifies a repository is `treepo-store`'s decision (see
//!   [`Manifest::root_seed`](treepo_model::Manifest::root_seed)). This crate never resolves
//!   identity and never invents a seed of its own.
//! * **`treepo_version`** — the product version writing the manifest, for diagnostics.
//! * **Filter, catalogue, and options** — the same knobs the individual passes take, so a
//!   caller that already customized one can hand the same values through without a second
//!   construction path.
//!
//! # Ordering
//!
//! Fixed, and the only order that works. Folder signals need the content pass, or every
//! weight modulates downward; history-dependent derived signals need both a content pass and
//! a history pass. See the crate docs.

use crate::discover::Target;
use crate::filter::FilterSet;
use crate::lang::{self, Catalogue, ContentOptions, ScanError, scan};
use crate::log_pass::{self, HistoryError, HistoryOptions};
use crate::signals::{self, SignalDictionary};
use crate::walk::{self, WalkError, WalkOptions};
use treepo_det::Seed;
use treepo_model::manifest::{LanguageTable, Manifest};

/// Knobs for a full extraction, one field per pass that takes options.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExtractOptions {
    /// Forwarded to [`walk`](crate::walk::walk).
    pub walk: WalkOptions,
    /// Forwarded to [`scan`](crate::lang::scan).
    pub content: ContentOptions,
    /// Forwarded to [`log_pass`](crate::log_pass::log_pass).
    pub history: HistoryOptions,
}

/// Why a full extraction could not complete.
///
/// Each variant is the error of the pass that failed. Composition invents no new failure
/// modes of its own.
#[derive(Debug)]
pub enum ExtractError {
    /// The HEAD-tree or filesystem walk failed.
    Walk(WalkError),
    /// A blob or file could not be read during the content pass.
    Scan(ScanError),
    /// The history traversal failed.
    History(HistoryError),
}

impl std::fmt::Display for ExtractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Walk(error) => write!(f, "walk failed: {error}"),
            Self::Scan(error) => write!(f, "content scan failed: {error}"),
            Self::History(error) => write!(f, "history pass failed: {error}"),
        }
    }
}

impl std::error::Error for ExtractError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Walk(error) => Some(error),
            Self::Scan(error) => Some(error),
            Self::History(error) => Some(error),
        }
    }
}

impl From<WalkError> for ExtractError {
    fn from(error: WalkError) -> Self {
        Self::Walk(error)
    }
}

impl From<ScanError> for ExtractError {
    fn from(error: ScanError) -> Self {
        Self::Scan(error)
    }
}

impl From<HistoryError> for ExtractError {
    fn from(error: HistoryError) -> Self {
        Self::History(error)
    }
}

/// Runs every extraction pass and returns a complete [`Manifest`].
///
/// This is the product entry point for Phase 1's pipeline. The individual passes remain
/// public for measurement (`xtask budget`) and audit (`xtask readonly-audit`), which call
/// each by name so a pass cannot quietly stop being part of extraction while a helper stayed
/// green. Callers that only need a manifest should call this.
///
/// `root_seed` is supplied by the caller — typically
/// `treepo_store::resolve::root_seed(&identity)` — because the seed tree hangs off repository
/// identity, and identity is not this crate's concern.
///
/// # Errors
///
/// [`ExtractError`] if any pass fails. Passes that produce no history (no `.git`, empty
/// repository) still succeed: the manifest is filled from filesystem primitives alone, with
/// `built_from_commit` left `None` and ages at zero — PRD §6's ordinary path, not an error.
pub fn extract(
    target: &Target,
    filter: &FilterSet,
    catalogue: &Catalogue,
    root_seed: Seed,
    treepo_version: String,
    options: ExtractOptions,
) -> Result<Manifest, ExtractError> {
    let mut structure = walk::walk(target, filter, options.walk)?;

    let mut languages = LanguageTable::new();
    scan(
        target,
        &mut structure,
        catalogue,
        &mut languages,
        options.content,
    )?;

    signals::apply(
        &mut structure.records,
        &SignalDictionary::built_in(),
        catalogue,
    );

    let history = log_pass::log_pass(target, filter, options.history)?;
    log_pass::apply(&mut structure.records, &history);
    lang::apply_history_signals(&mut structure.records);

    let mut manifest = Manifest::new(treepo_version, root_seed);
    manifest.built_from_commit = match target {
        Target::Repository(repo) => repo.head,
        Target::PlainDirectory { .. } => None,
    };
    manifest.reference_time = history.reference_time;
    manifest.is_shallow = matches!(target, Target::Repository(repo) if repo.is_shallow);
    // `F-ID-1` is already resolved inside the history pass, where the author table is built
    // — see `self_ident` and `log_pass`. Nothing to do here.
    manifest.authors = history.authors;
    manifest.languages = languages;
    manifest.set_paths(structure.records);
    Ok(manifest)
}
