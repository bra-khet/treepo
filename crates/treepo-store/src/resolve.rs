//! `F-MAN-3` — which repository is this?
//!
//! Three rules, in strict order, first match wins:
//!
//! 1. **Normalized primary remote URL.** Prefer `origin`, otherwise the alphabetically first
//!    remote. `git@github.com:foo/bar.git`, `https://github.com/foo/bar.git`, and
//!    `https://github.com/Foo/Bar/` all resolve to one identity (`AC-MAN-4`).
//! 2. **Root commit SHA**, for a repository with history but no remote. This is what survives
//!    a folder being moved or renamed (`AC-MAN-5`), which a path hash does not.
//! 3. **Normalized absolute path hash**, for a non-git directory or a repository with no
//!    commits.
//!
//! The order is the point. A fork shares its upstream's root commit but has its own remote,
//! and tier 1 winning is what keeps the two from sharing a store (PRD §6).
//!
//! # The evidence is returned, not just the answer
//!
//! `F-MAN-2` says `identity.json` records "resolved identity **and how it was derived**, for
//! inspection", and `F-MAN-3` wants a user to be able to see "why two checkouts did or did not
//! share a store". [`Resolution`] is that: the identity, every remote that was considered, the
//! one that won, and — as [`Skipped`] — the reason each higher tier did not apply. A user
//! looking at two repositories that unexpectedly share a directory can read the answer rather
//! than infer it.
//!
//! # Raw remote URLs do not leave this module
//!
//! A configured remote can carry a credential. `https://x-access-token:ghp_…@github.com/o/r`
//! is what a CI checkout leaves behind in `.git/config`, and hand-pasted tokens are common
//! enough. [`normalize_url`] strips credentials, and [`Resolution`] carries only normalized
//! forms — so no code path leads from `.git/config` to `identity.json`, to a store directory
//! name, or to a shared package (`F-MAN-11`).
//!
//! # Determinism
//!
//! Two rules that look like details and are not:
//!
//! * **Case folding is ASCII-only**, as in [`AuthorKey`](treepo_model::identity::AuthorKey).
//!   Full Unicode lowercasing needs tables, and a table that decides which store a repository
//!   lands in is a determinism input that changes with the Unicode version.
//! * **`url.<base>.insteadOf` rewrites are not applied.** They are user-local git config, so
//!   honouring them would give two developers different identities for the same clone. The
//!   configured `remote.<name>.url` is read verbatim and normalized here.

use std::path::{Path, PathBuf};
use treepo_model::identity::{CommitId, IdentityTier, RepoIdentity};

/// Everything resolution concluded, and how (`F-MAN-2`, `F-MAN-3`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    /// The identity itself. Its key names the store directory.
    pub identity: RepoIdentity,
    /// Every remote found, in name order, with its normalized URL.
    ///
    /// `None` where a remote has no URL or one that does not normalize. Raw URLs are
    /// deliberately absent — see the module docs.
    pub remotes: Vec<RemoteUrl>,
    /// The remote tier 1 used, by name.
    pub chosen: Option<String>,
    /// The root commit, when tier 2 resolved one.
    ///
    /// Absent when tier 1 won: finding it costs a full graph walk, and paying that on every
    /// open of every repository with a remote would put a T3 repository's identity resolution
    /// alone into seconds against `NFR-4`'s 5 s cold launch. `F-MAN-5`'s relink index wants
    /// root commits for every repository, and the place to collect them is the history pass,
    /// which walks the graph once anyway.
    pub root_commit: Option<CommitId>,
    /// Why each tier above the winning one did not apply, in tier order.
    pub skipped: Vec<Skipped>,
}

/// A remote, as identity resolution saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteUrl {
    /// The remote's name — `origin`, `upstream`, whatever the user called it.
    pub name: String,
    /// Its URL after normalization, or `None` if it has none or it does not normalize.
    pub normalized: Option<String>,
}

/// Why a tier did not apply (`F-MAN-3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skipped {
    /// No `.git` at all — tiers 1 and 2 have nothing to read.
    NoRepository,
    /// A repository with no remotes configured.
    NoRemotes,
    /// Remotes exist, but none has a URL that normalizes to anything usable.
    NoRemoteUrl,
    /// A repository with no commits, so there is no root commit to name it by.
    NoCommits,
    /// A shallow clone: the history it holds does not reach its own root.
    TruncatedHistory,
    /// History exists and is not truncated, but no parentless commit could be read.
    NoRootCommit,
}

impl Skipped {
    /// The user-facing explanation, in the same shape as
    /// [`Notice`](https://docs.rs/treepo-vcs) — these end up in `identity.json` and in the
    /// answer to "why do these two repositories share a store?".
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::NoRepository => {
                "There is no git repository here, so identity falls back to this folder's \
                 path. Moving the folder will start a new store."
            }
            Self::NoRemotes => {
                "This repository has no remote configured, so identity is its root commit. \
                 That survives the folder being moved or renamed."
            }
            Self::NoRemoteUrl => {
                "None of this repository's remotes has a usable URL, so identity falls back \
                 to the next rule."
            }
            Self::NoCommits => {
                "This repository has no commits yet, so identity falls back to this folder's \
                 path. It will keep that identity even after the first commit."
            }
            Self::TruncatedHistory => {
                "This is a shallow clone, so its root commit is not present to identify it by. \
                 Unshallow the clone, or configure a remote, for an identity that survives a \
                 move."
            }
            Self::NoRootCommit => {
                "This repository's history could not be read back to a first commit, so \
                 identity falls back to this folder's path."
            }
        }
    }
}

/// Why identity could not be resolved at all.
#[derive(Debug)]
pub enum ResolveError {
    /// The last tier needs an absolute path, and the filesystem would not produce one.
    ///
    /// There is deliberately no fallback: hashing the path as given would key the store on
    /// whatever directory treepo happened to be launched from, and the same repository would
    /// land somewhere different next time.
    Path {
        /// The path as given.
        path: PathBuf,
        /// What the filesystem said.
        source: std::io::Error,
    },
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Path { path, source } => write!(
                f,
                "{} has no resolvable absolute path, so treepo cannot decide where to keep \
                 its data: {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ResolveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Path { source, .. } => Some(source),
        }
    }
}

/// Resolves a repository's identity (`F-MAN-3`).
///
/// `root` is the working directory the user chose and `repo` the repository open at it —
/// exactly the pair `treepo_vcs::Target` yields. `repo` is `None` for a plain directory.
///
/// `resolved_at` is stamped onto the identity as housekeeping and is *not* hashed, so it
/// cannot move a repository's data. It is a parameter rather than a clock read because
/// nothing in this crate reads a clock: the caller that persists the identity is the one place
/// that needs one.
///
/// # Errors
///
/// [`ResolveError::Path`] if the last tier cannot obtain an absolute path.
pub fn resolve(
    root: &Path,
    repo: Option<&gix::Repository>,
    resolved_at: u64,
) -> Result<Resolution, ResolveError> {
    let mut skipped = Vec::new();

    let Some(repo) = repo else {
        skipped.push(Skipped::NoRepository);
        return Ok(Resolution {
            identity: path_identity(root, resolved_at)?,
            remotes: Vec::new(),
            chosen: None,
            root_commit: None,
            skipped,
        });
    };

    // Tier 1 — the normalized primary remote URL.
    let remotes = read_remotes(repo);
    match choose_remote(&remotes) {
        Some((name, url)) => {
            return Ok(Resolution {
                identity: RepoIdentity::new(IdentityTier::Remote, url, resolved_at),
                chosen: Some(name),
                remotes,
                root_commit: None,
                skipped,
            });
        }
        None if remotes.is_empty() => skipped.push(Skipped::NoRemotes),
        None => skipped.push(Skipped::NoRemoteUrl),
    }

    // Tier 2 — the earliest root commit.
    //
    // A shallow clone is excluded rather than approximated. Its oldest commit is a boundary,
    // not a root, so keying on it would give a repository one identity today and a different
    // one the moment somebody unshallows it — the store would silently orphan itself.
    if repo.head_id().is_err() {
        skipped.push(Skipped::NoCommits);
    } else if repo.is_shallow() {
        skipped.push(Skipped::TruncatedHistory);
    } else if let Some(root_commit) = earliest_root_commit(repo) {
        return Ok(Resolution {
            identity: RepoIdentity::new(
                IdentityTier::RootCommit,
                root_commit.to_string(),
                resolved_at,
            ),
            remotes,
            chosen: None,
            root_commit: Some(root_commit),
            skipped,
        });
    } else {
        skipped.push(Skipped::NoRootCommit);
    }

    // Tier 3 — the absolute path.
    Ok(Resolution {
        identity: path_identity(root, resolved_at)?,
        remotes,
        chosen: None,
        root_commit: None,
        skipped,
    })
}

/// The root of a manifest's per-path seed tree, for a resolved identity (`P2`).
///
/// `treepo-model` leaves this open — "what it is derived *from* is `treepo-store`'s decision
/// in Phase 2 — root commit and repository identity behave differently for a fork, which is a
/// product question rather than a model one". This is that decision.
///
/// **Keyed on the identity.** Two clones of one remote share an identity, so they grow the
/// same tree — which is `F-MAN-4`'s "the same repository is the same tree" made visible rather
/// than merely stored. A fork shares its upstream's root commit but not its identity, so it
/// looks different, which is the answer `F-MAN-3`'s tier order already gives.
///
/// **The visible consequence:** a repository that gains a remote moves from tier 3 or 2 to
/// tier 1, so its identity changes, so its tree changes. That is the same event that moves it
/// to a new store directory and regenerates its manifest, so the two agree — but somebody will
/// one day add an `origin`, watch their tree change shape, and want to know why.
#[must_use]
pub fn root_seed(identity: &RepoIdentity) -> treepo_det::Seed {
    treepo_model::Manifest::root_seed_for(&identity.key)
}

/// Every remote, in name order, with its URL normalized.
///
/// Read from config rather than through `gix::Remote` so that `url.<base>.insteadOf` rewrites
/// are not applied — see the module docs. `pushurl` is deliberately ignored: `F-MAN-3` names
/// the primary remote URL, and a push URL is an access route to the same repository.
fn read_remotes(repo: &gix::Repository) -> Vec<RemoteUrl> {
    let config = repo.config_snapshot();
    let mut names: Vec<String> = repo
        .remote_names()
        .iter()
        .map(|name| name.to_string())
        .collect();
    names.sort_unstable();

    names
        .into_iter()
        .map(|name| {
            let normalized = config
                .string_by("remote", Some(name.as_str().into()), "url")
                .and_then(|url| normalize_url(&url.to_string()));
            RemoteUrl { name, normalized }
        })
        .collect()
}

/// `origin` if it has a usable URL, otherwise the alphabetically first remote that does.
///
/// Falling through to the next remote rather than abandoning tier 1 outright: `F-MAN-3` is
/// after the repository's primary URL, and an `origin` with an empty or malformed URL is a
/// broken entry rather than a statement that this repository has no remote identity.
fn choose_remote(remotes: &[RemoteUrl]) -> Option<(String, String)> {
    remotes
        .iter()
        .find(|remote| remote.name == "origin" && remote.normalized.is_some())
        .or_else(|| remotes.iter().find(|remote| remote.normalized.is_some()))
        .and_then(|remote| {
            remote
                .normalized
                .clone()
                .map(|url| (remote.name.clone(), url))
        })
}

/// The earliest root commit by commit date, tie-broken by object id (`F-MAN-3` tier 2).
///
/// Walked from **every** reference rather than from HEAD, because identity is a property of
/// the repository and not of what happens to be checked out. Walking from HEAD would give a
/// repository one identity on `main` and another on an orphan branch, and switching branches
/// would orphan its store.
///
/// The cost — a full graph traversal — is only ever paid by a repository with no remote, which
/// in practice means a local-only project rather than a monorepo.
fn earliest_root_commit(repo: &gix::Repository) -> Option<CommitId> {
    let mut tips: Vec<gix::ObjectId> = Vec::new();
    // A detached HEAD may point at a commit no reference reaches.
    if let Ok(head) = repo.head_id() {
        tips.push(head.detach());
    }
    if let Ok(platform) = repo.references()
        && let Ok(all) = platform.all()
    {
        for mut reference in all.flatten() {
            if let Ok(id) = reference.peel_to_id() {
                tips.push(id.detach());
            }
        }
    }
    if tips.is_empty() {
        return None;
    }
    tips.sort_unstable();
    tips.dedup();

    let walk = repo.rev_walk(tips).all().ok()?;
    let mut best: Option<(i64, gix::ObjectId)> = None;
    for info in walk {
        let Ok(info) = info else { continue };
        if !info.parent_ids.is_empty() {
            continue;
        }
        let Ok(commit) = repo.find_commit(info.id) else {
            continue;
        };
        // Committer time, as `F-MAN-3` says "commit date". A missing or unparseable one
        // sorts earliest, which only affects which of several roots wins — and the object id
        // tie-break still makes that choice deterministic.
        let seconds = commit.time().map(|time| time.seconds).unwrap_or_default();
        let candidate = (seconds, info.id);
        if best.as_ref().is_none_or(|current| candidate < *current) {
            best = Some(candidate);
        }
    }

    best.and_then(|(_, id)| to_commit_id(&id))
}

/// A `gix` object id as the model's, or `None` for a hash width the model does not know.
fn to_commit_id(id: &gix::oid) -> Option<CommitId> {
    match id.as_bytes().len() {
        20 => id.as_bytes().try_into().ok().map(CommitId::sha1),
        32 => id.as_bytes().try_into().ok().map(CommitId::sha256),
        _ => None,
    }
}

/// Tier 3 — the identity of a directory with nothing else to key on.
fn path_identity(root: &Path, resolved_at: u64) -> Result<RepoIdentity, ResolveError> {
    Ok(RepoIdentity::new(
        IdentityTier::PathHash,
        normalize_path(root)?,
        resolved_at,
    ))
}

/// A repository's absolute path, in the one spelling this machine will produce again.
///
/// `canonicalize` does the real work: it absolutizes, removes `.` and `..`, resolves symlinks
/// so a directory reached two ways is one identity, and on case-insensitive filesystems
/// returns the on-disk casing — which is exactly the case folding tier 3 needs, applied by the
/// filesystem that knows whether it applies. Doing it by hand would mean lowercasing on Linux
/// too, where `~/src/Repo` and `~/src/repo` really are two directories.
///
/// # Errors
///
/// [`ResolveError::Path`] if the path cannot be canonicalized.
fn normalize_path(root: &Path) -> Result<String, ResolveError> {
    let canonical = std::fs::canonicalize(root).map_err(|source| ResolveError::Path {
        path: root.to_path_buf(),
        source,
    })?;

    // Windows canonicalization returns a verbatim path. `\\?\C:\x` and `C:\x` are the same
    // directory, and the prefix would otherwise be baked into the hash.
    let text = escape_lossless(&path_bytes(&canonical));
    let text = text
        .strip_prefix("//?/UNC/")
        .map(|rest| format!("//{rest}"))
        .unwrap_or_else(|| text.strip_prefix("//?/").unwrap_or(&text).to_string());

    let trimmed = text.trim_end_matches('/');
    Ok(if trimmed.is_empty() {
        text
    } else {
        trimmed.to_string()
    })
}

/// A path's bytes, with `\` folded to `/` so the two Windows spellings agree.
fn path_bytes(path: &Path) -> Vec<u8> {
    #[cfg(unix)]
    let bytes = {
        use std::os::unix::ffi::OsStrExt as _;
        path.as_os_str().as_bytes().to_vec()
    };
    // Windows paths are UTF-16 and convert without loss in every case a filesystem produces.
    #[cfg(not(unix))]
    let bytes = path.to_string_lossy().into_owned().into_bytes();

    bytes
        .into_iter()
        .map(|byte| if byte == b'\\' { b'/' } else { byte })
        .collect()
}

/// Bytes as a string, percent-escaping anything that is not valid UTF-8.
///
/// Linux permits any byte but `/` and NUL in a filename, and a lossy conversion maps every
/// invalid sequence to the same replacement character — which would give two sibling
/// directories one identity and therefore one store. Escaping instead is injective: `%` is
/// escaped too, so no unescaped path can collide with an escaped one.
fn escape_lossless(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    let mut rest = bytes;
    loop {
        match core::str::from_utf8(rest) {
            Ok(text) => {
                push_escaped(&mut out, text);
                return out;
            }
            Err(error) => {
                let (valid, invalid) = rest.split_at(error.valid_up_to());
                push_escaped(&mut out, core::str::from_utf8(valid).unwrap_or_default());
                let skip = error.error_len().unwrap_or(invalid.len()).max(1);
                for byte in &invalid[..skip.min(invalid.len())] {
                    out.push_str(&format!("%{byte:02x}"));
                }
                rest = &invalid[skip.min(invalid.len())..];
                if rest.is_empty() {
                    return out;
                }
            }
        }
    }
}

fn push_escaped(out: &mut String, text: &str) {
    for ch in text.chars() {
        if ch == '%' {
            out.push_str("%25");
        } else {
            out.push(ch);
        }
    }
}

/// Collapses a remote URL to the identity `F-MAN-3` tier 1 keys on.
///
/// Strips the scheme, credentials, port, trailing slashes and a trailing `.git`, then
/// ASCII-lowercases what is left. `git@github.com:foo/bar.git`,
/// `https://github.com/foo/bar.git`, and `https://github.com/Foo/Bar/` all become
/// `github.com/foo/bar`.
///
/// Returns `None` for anything that does not reduce to a host or a path — an empty value, or
/// one carrying control characters or whitespace, which no real remote does and which would
/// otherwise land unescaped in a directory name.
///
/// # The port is dropped, and that follows from dropping the scheme
///
/// `F-MAN-3` names scheme, credentials, trailing slash and `.git`, and says nothing about
/// ports. Keeping one would undo the rest: `ssh://git@host:22/foo` and `https://host/foo` are
/// the same repository reached two ways, and the whole purpose of stripping the scheme is that
/// the route is not the identity. A port is the same kind of fact as a scheme.
///
/// # What lowercasing the path costs
///
/// Two repositories on a case-sensitive host differing only in case — `host/Foo` and
/// `host/foo` — share an identity and therefore a store. This is the PRD's rule and the right
/// trade: `https://github.com/Foo/Bar/` and `https://github.com/foo/bar` are the same
/// repository far more often than they are two, and GitHub, GitLab and Bitbucket all treat
/// them so. It is recorded because the failure, if it ever happens, will look inexplicable.
#[must_use]
pub fn normalize_url(url: &str) -> Option<String> {
    let url = url.trim();
    if url.is_empty() || url.chars().any(|ch| ch.is_control() || ch.is_whitespace()) {
        return None;
    }

    let (authority, path) = split(url);
    let host = host_of(authority);
    let path = clean_path(path);

    match (host.is_empty(), path.is_empty()) {
        (true, true) => None,
        (true, false) => Some(path),
        (false, true) => Some(host),
        (false, false) => Some(format!("{host}/{path}")),
    }
}

/// Splits a URL into its authority and path, in whichever of the three spellings git accepts.
fn split(url: &str) -> (&str, &str) {
    if let Some((scheme, rest)) = url.split_once("://")
        && !scheme.is_empty()
        && scheme.starts_with(|ch: char| ch.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.'))
    {
        return match rest.split_once('/') {
            // `scheme:///path` — there is no authority, and the slash the split consumed is
            // the path's own root. Dropping it would make `file:///srv/git/app` collide with
            // a relative remote spelled `srv/git/app`.
            Some(("", _)) => ("", rest),
            Some(split) => split,
            None => (rest, ""),
        };
    }

    // scp-like: `[user@]host:path`, where the path does not begin with a separator. A
    // Windows drive letter looks identical (`C:/src/repo`) and is a path, not a host — the
    // one-character test is what git itself uses.
    if let Some((before, after)) = url.split_once(':')
        && !before.is_empty()
        && !before.contains('/')
        && !before.contains('\\')
        && !(before.len() == 1 && before.starts_with(|ch: char| ch.is_ascii_alphabetic()))
    {
        return (before, after);
    }

    ("", url)
}

/// The host alone: credentials and port removed.
fn host_of(authority: &str) -> String {
    let after_credentials = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);

    // `[::1]:22` — the colons inside the brackets are the address, not a port. The brackets
    // are kept, because a bare `2001:db8::1/team/app` in `identity.json` reads as ambiguous
    // with a path where the bracketed form does not.
    let host = if after_credentials.starts_with('[') {
        after_credentials
            .split_once(']')
            .map_or(after_credentials, |(inside, _)| {
                &after_credentials[..inside.len() + 1]
            })
    } else {
        after_credentials
            .split_once(':')
            .map_or(after_credentials, |(host, _)| host)
    };

    host.to_ascii_lowercase()
}

/// The path alone: separators folded, trailing slashes and one `.git` removed, lowercased.
fn clean_path(path: &str) -> String {
    let path = path.replace('\\', "/");
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let path = path.trim_end_matches('/');

    // `file:///C:/src/app` and `C:\src\app` name one directory. The leading slash is the
    // `file:` URL spelling of a drive path rather than part of it, and git produces exactly
    // this form on Windows — `tools/corpus` clones through it.
    let path = match path.as_bytes() {
        [b'/', drive, b':', ..] if drive.is_ascii_alphabetic() => &path[1..],
        _ => path,
    };

    path.to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The PRD's own example, which is the whole of `AC-MAN-4`.
    #[test]
    fn the_three_spellings_in_f_man_3_agree() {
        let expected = Some("github.com/foo/bar".to_string());
        assert_eq!(normalize_url("git@github.com:foo/bar.git"), expected);
        assert_eq!(normalize_url("https://github.com/foo/bar.git"), expected);
        assert_eq!(normalize_url("https://github.com/Foo/Bar/"), expected);
        assert_eq!(normalize_url("ssh://git@github.com/foo/bar"), expected);
        assert_eq!(normalize_url("git://github.com/foo/bar.git"), expected);
        assert_eq!(normalize_url("  https://GITHUB.com/foo/bar  "), expected);
    }

    /// A token in `.git/config` must not reach a store directory name or `identity.json`.
    #[test]
    fn credentials_are_stripped() {
        let normalized = normalize_url("https://x-access-token:ghp_secret@github.com/o/r.git")
            .expect("a usable URL");
        assert_eq!(normalized, "github.com/o/r");
        assert!(!normalized.contains("ghp_secret"));
        assert!(!normalized.contains('@'));
        assert_eq!(
            normalize_url("https://user@github.com/o/r"),
            normalize_url("https://github.com/o/r")
        );
    }

    /// Dropping the scheme is pointless if the port survives it.
    #[test]
    fn the_route_is_not_the_identity() {
        let expected = Some("git.example.com/team/app".to_string());
        assert_eq!(
            normalize_url("ssh://git@git.example.com:22/team/app.git"),
            expected
        );
        assert_eq!(
            normalize_url("https://git.example.com:8443/team/app"),
            expected
        );
        assert_eq!(normalize_url("git.example.com:team/app.git"), expected);
        // IPv6 literals keep their address and lose their port.
        assert_eq!(
            normalize_url("ssh://git@[2001:db8::1]:2222/team/app.git"),
            Some("[2001:db8::1]/team/app".to_string())
        );
    }

    /// A local path is a legitimate remote, and a Windows drive letter is not a hostname.
    #[test]
    fn local_paths_are_remotes_not_hosts() {
        assert_eq!(
            normalize_url("file:///srv/git/app.git"),
            Some("/srv/git/app".to_string())
        );
        assert_eq!(
            normalize_url("C:\\src\\App"),
            Some("c:/src/app".to_string()),
            "a one-letter prefix is a drive, not a host"
        );
        assert_eq!(normalize_url("C:/src/App"), Some("c:/src/app".to_string()));
        assert_eq!(
            normalize_url("../sibling-repo"),
            Some("../sibling-repo".to_string())
        );
        // The `file:` spelling git itself produces on Windows, and the one a user types.
        assert_eq!(
            normalize_url("file:///C:/src/App.git"),
            normalize_url("C:\\src\\App")
        );
        // An absolute path keeps its root, or it collides with a relative remote.
        assert_ne!(
            normalize_url("file:///srv/git/app.git"),
            normalize_url("srv/git/app")
        );
    }

    #[test]
    fn unusable_urls_fall_through() {
        assert_eq!(normalize_url(""), None);
        assert_eq!(normalize_url("   "), None);
        assert_eq!(normalize_url("https://"), None);
        assert_eq!(normalize_url("/"), None);
        assert_eq!(normalize_url("https://exa mple.com/x"), None);
        assert_eq!(normalize_url("https://example.com/\u{7}x"), None);
    }

    /// `.git` is a suffix, not a substring.
    #[test]
    fn only_a_trailing_dot_git_is_stripped() {
        assert_eq!(
            normalize_url("https://github.com/foo/git.hub"),
            Some("github.com/foo/git.hub".to_string())
        );
        assert_eq!(
            normalize_url("https://github.com/foo/bar.github"),
            Some("github.com/foo/bar.github".to_string())
        );
        assert_eq!(
            normalize_url("https://github.com/foo/bar.git/"),
            Some("github.com/foo/bar".to_string())
        );
    }

    /// `origin` wins; without it, the alphabetically first remote does (`F-MAN-3` tier 1).
    #[test]
    fn origin_wins_and_otherwise_the_first_name_does() {
        let remote = |name: &str, url: Option<&str>| RemoteUrl {
            name: name.to_string(),
            normalized: url.map(ToString::to_string),
        };

        let with_origin = [
            remote("backup", Some("b/b")),
            remote("origin", Some("o/o")),
            remote("upstream", Some("u/u")),
        ];
        assert_eq!(
            choose_remote(&with_origin),
            Some(("origin".to_string(), "o/o".to_string()))
        );

        let without_origin = [
            remote("backup", Some("b/b")),
            remote("upstream", Some("u/u")),
        ];
        assert_eq!(
            choose_remote(&without_origin),
            Some(("backup".to_string(), "b/b".to_string()))
        );

        // A broken `origin` is a broken entry, not a claim to have no remote identity.
        let broken_origin = [remote("backup", Some("b/b")), remote("origin", None)];
        assert_eq!(
            choose_remote(&broken_origin),
            Some(("backup".to_string(), "b/b".to_string()))
        );

        assert_eq!(choose_remote(&[remote("origin", None)]), None);
        assert_eq!(choose_remote(&[]), None);
    }

    /// A lossy conversion would give two different directories one store.
    #[test]
    fn invalid_utf8_escapes_rather_than_collapses() {
        let first = escape_lossless(b"/home/u/\xff");
        let second = escape_lossless(b"/home/u/\xfe");
        assert_ne!(first, second);
        assert_eq!(first, "/home/u/%ff");
        // `%` is escaped too, so an escaped path cannot be spelled literally.
        assert_ne!(escape_lossless(b"/home/u/%ff"), first);
        assert_eq!(escape_lossless(b"/home/u/%ff"), "/home/u/%25ff");
        // Valid UTF-8 is left legible.
        assert_eq!(
            escape_lossless("/home/u/проект".as_bytes()),
            "/home/u/проект"
        );
    }

    #[test]
    fn every_skip_reason_says_something_useful() {
        for skipped in [
            Skipped::NoRepository,
            Skipped::NoRemotes,
            Skipped::NoRemoteUrl,
            Skipped::NoCommits,
            Skipped::TruncatedHistory,
            Skipped::NoRootCommit,
        ] {
            let reason = skipped.reason();
            assert!(reason.len() > 40, "{skipped:?} needs a real explanation");
            assert!(reason.ends_with('.'));
        }
    }

    /// Tier 3 keys on a canonical path, so the same directory reached two ways is one store.
    #[test]
    fn tier_three_canonicalizes() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let direct = normalize_path(manifest).expect("this crate's own directory");
        let roundabout = normalize_path(&manifest.join("src").join("..")).expect("same place");
        assert_eq!(direct, roundabout);
        assert!(!direct.contains('\\'), "separators are folded: {direct}");
        assert!(
            !direct.contains("//?/"),
            "verbatim prefix removed: {direct}"
        );
        assert!(!direct.ends_with('/'));
    }

    #[test]
    fn a_missing_directory_has_no_identity() {
        let error = resolve(Path::new("no-such-directory-4b7e1c"), None, 0)
            .expect_err("nothing to canonicalize");
        assert!(matches!(error, ResolveError::Path { .. }));
        assert!(error.to_string().contains("cannot decide where to keep"));
    }

    /// A plain directory is tier 3, and says so.
    #[test]
    fn a_plain_directory_resolves_to_its_path() {
        let resolution =
            resolve(Path::new(env!("CARGO_MANIFEST_DIR")), None, 0).expect("a directory");
        assert_eq!(resolution.identity.tier, IdentityTier::PathHash);
        assert_eq!(resolution.skipped, [Skipped::NoRepository]);
        assert!(resolution.remotes.is_empty());
        assert!(resolution.chosen.is_none());
        assert!(resolution.root_commit.is_none());
    }
}
