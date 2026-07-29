//! `F-ID-1` — which contributor is the person running treepo.
//!
//! > Self-identification reads `user.email` from git config (repository, then global).
//! > Identities matching after `.mailmap` normalization are "you."
//!
//! # Why this lives in `treepo-vcs` and not in `treepo-id`
//!
//! The architecture's file tree put `self_ident.rs` in `treepo-id`, beside the pseudonym and
//! the palette. It is here instead, and the decision was taken deliberately rather than for
//! convenience: `treepo-id` is `no_std` and reads no files, which is the property that makes
//! it *unable* to acquire a repository dependency or a filesystem read on some later "while
//! we're here" afternoon. Giving it a `std` feature to open one config file would trade a
//! structural guarantee for a file placement.
//!
//! So the crate that already opens repositories, already reads `.mailmap`, and already
//! advertises every filesystem read it performs does this one too, and hands `treepo-id` a
//! key. `treepo-id` stays a pure function of an [`AuthorKey`], which is what lets it be the
//! `N9` gate rather than merely the place the gate is written down.
//!
//! # This module was extracted, not invented
//!
//! Phase 1 already resolved the viewer — as a private `self_author_key` inside
//! [`log_pass`](crate::log_pass), which is where the author table is built. That worked and
//! it was invisible: `F-ID-1` is a named feature with no file of its own, and the next person
//! to look for it wrote a second copy. The logic now lives here, `log_pass` calls it, and
//! there is one implementation.
//!
//! Two things changed on the way. `user.name` is now read and passed to `.mailmap`, because a
//! mapping may be keyed on the full `(name, email)` pair and the old helper passed an empty
//! name — a `Name <canonical> <alias>` rule would not have matched the viewer. And
//! [`IdentityScope`] reports which config file won.
//!
//! # No name comes out of here
//!
//! `user.name` is read *only* so `.mailmap` can resolve the way git does. It is not
//! returned, not stored, and not carried into [`SelfIdentity`].
//!
//! That is a tightening on the PRD, which permits the viewer's own name to be shown
//! (`AC-ID-1` protects contributors "other than the user"). It buys something real: a
//! rendered tree — and therefore an export, and therefore a screenshot someone posts — says
//! "You" where the viewer appears, so sharing a tree does not announce who made it. The
//! viewer already knows their own name, so nothing is lost. `treepo-id`'s
//! `Identification::Yourself` carries no name for the same reason.
//!
//! # What this is *not*
//!
//! It answers "who are you", not "are you a contributor here". The second is
//! [`AuthorTable::self_author`](treepo_model::manifest::AuthorTable::self_author) being
//! `None`, which is `F-ID-7`'s ordinary case — the common one, for a repository the user
//! merely cloned — and not an error.

use crate::discover::Target;
use crate::mailmap::Identities;
use gix::actor::SignatureRef;
use gix::bstr::ByteSlice as _;
use treepo_model::identity::AuthorKey;

/// Which configuration file the identity came from.
///
/// `F-ID-1` names the precedence, and git's own merge already implements it — this records
/// which side won, so a user who cannot work out why treepo does not recognise them can be
/// told where to look (`N2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityScope {
    /// `.git/config` or a worktree config — set for this repository specifically.
    Repository,
    /// The user's global or system config — the usual case.
    Global,
}

/// The contributor key of the person running treepo (`F-ID-1`).
///
/// Deliberately just a key and a provenance note. See the module docs for why no name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelfIdentity {
    key: AuthorKey,
    scope: IdentityScope,
}

impl SelfIdentity {
    /// The viewer's contributor key, mailmap-normalized.
    #[must_use]
    pub const fn key(&self) -> AuthorKey {
        self.key
    }

    /// Which config file supplied `user.email`.
    #[must_use]
    pub const fn scope(&self) -> IdentityScope {
        self.scope
    }
}

/// Reads `user.email` and resolves it to a contributor key (`F-ID-1`).
///
/// `None` when there is no repository to read config from, or when `user.email` is unset or
/// blank. All three are ordinary: a plain directory has no config, and a machine where git
/// has never been configured is a machine where treepo simply shows everyone as a pseudonym.
///
/// The mailmap is loaded here rather than taken as an argument, so a caller cannot pair the
/// viewer's identity with a *different* normalization than author attribution used — which
/// would make the viewer fail to match their own commits. [`log_pass`](crate::log_pass) has
/// one already and uses [`key_for`] to avoid the second read.
#[must_use]
pub fn self_identity(target: &Target) -> Option<SelfIdentity> {
    let repo = target.repository()?;
    let identities = Identities::load(repo);
    let key = key_for(repo, &identities)?;
    Some(SelfIdentity {
        key,
        scope: scope_of(repo),
    })
}

/// The viewer's key, for a caller that already holds the repository's [`Identities`].
///
/// The single implementation of `F-ID-1`'s resolution; [`self_identity`] is this plus the
/// mailmap load and the scope.
#[must_use]
pub fn key_for(repo: &gix::Repository, identities: &Identities) -> Option<AuthorKey> {
    let snapshot = repo.config_snapshot();

    let email = snapshot.string("user.email")?;
    let email = email.trim();
    if email.is_empty() {
        return None;
    }

    // Read only so `.mailmap` can resolve a (name, email) pair the way git does — a mapping
    // may be keyed on both. Discarded immediately afterwards.
    let name = snapshot.string("user.name").unwrap_or_default();

    Some(identities.key(SignatureRef {
        name: name.as_bstr(),
        email: email.as_bstr(),
        // Never read: `Identities::key` uses the resolved address only. A fixed value rather
        // than a clock, because `N3` bans one and this would be the least defensible place
        // in the workspace to read it.
        time: "0 +0000",
    }))
}

/// Which config file supplied `user.email`.
///
/// `string` has already applied git's precedence; this only asks which side won.
fn scope_of(repo: &gix::Repository) -> IdentityScope {
    let snapshot = repo.config_snapshot();
    let local = snapshot.plumbing().string_filter("user.email", |meta| {
        matches!(
            meta.source,
            gix::config::Source::Local | gix::config::Source::Worktree
        )
    });
    if local.is_some() {
        IdentityScope::Repository
    } else {
        IdentityScope::Global
    }
}

// No unit tests here, and that is the crate's existing convention rather than a gap:
// everything this module does needs a real repository on disk, and fixture-backed tests live
// in `tests/` so the `src/` suite stays hermetic. `tests/privacy.rs` covers all three
// branches — repository-scoped config, no config at all, and the mailmap resolution — and
// `tests/history_self.rs` covers the `is_self` marking the history pass does with it.
