//! `F-ASSOC-2` — what treepo found at the path the user picked.
//!
//! > On association, treepo validates the target and reports specifically what it found: git
//! > repository, plain directory (no `.git`), shallow clone, bare repository (unsupported),
//! > or inaccessible.
//!
//! Those five outcomes are the whole API. [`Target`] carries the two that can be extracted
//! from and [`DiscoverError`] the two that cannot; shallow is a property of a repository
//! rather than a sixth case, because PRD §6 requires treepo *warn and offer to proceed*
//! rather than refuse.
//!
//! # Nothing here writes
//!
//! `N1` and `F-ASSOC-7`: the repository is opened read-only and a read-only mount, a
//! restricted-permission directory, or a foreign clone is an ordinary path rather than a
//! degraded one. `gix` never runs a hook, a filter, or a pager, so opening a repository
//! executes none of its contents (`AC-EXT-4`) — the concrete reason architecture D3 chose it
//! over subprocess `git`.
//!
//! # The chosen directory is the subject
//!
//! [`discover`] opens the path it is given and does not search upward for an enclosing
//! repository. Picking `myrepo/src` yields a plain directory, not `myrepo`: growing a tree
//! several times larger than the folder the user chose would be a worse surprise than the
//! notice they get instead. `F-ASSOC-2`'s report is what makes the distinction visible.

use gix::bstr::ByteSlice;
use std::path::{Path, PathBuf};
use treepo_model::identity::CommitId;

/// A repository or directory treepo can extract from.
#[derive(Debug)]
pub enum Target {
    /// A git repository with a working tree.
    Repository(Box<RepoTarget>),
    /// A directory with no `.git` (`AC-ASSOC-3`).
    ///
    /// Extraction still produces a tree, from filesystem primitives alone, with an explicit
    /// notice that age, churn, and ownership are unavailable.
    PlainDirectory {
        /// The directory the user chose.
        root: PathBuf,
    },
}

impl Target {
    /// The directory being visualized, either way.
    #[must_use]
    pub fn root(&self) -> &Path {
        match self {
            Self::Repository(target) => &target.root,
            Self::PlainDirectory { root } => root,
        }
    }

    /// The repository, when there is one.
    #[must_use]
    pub fn repository(&self) -> Option<&gix::Repository> {
        match self {
            Self::Repository(target) => Some(&target.repo),
            Self::PlainDirectory { .. } => None,
        }
    }

    /// Everything the user should be told about what was found (`F-ASSOC-2`).
    #[must_use]
    pub fn notices(&self) -> Vec<Notice> {
        match self {
            Self::PlainDirectory { .. } => vec![Notice::NoRepository],
            Self::Repository(target) => {
                let mut notices = Vec::new();
                if target.is_shallow {
                    notices.push(Notice::ShallowClone);
                }
                if target.head.is_none() {
                    notices.push(Notice::NoCommits);
                }
                if target.is_detached {
                    notices.push(Notice::DetachedHead);
                }
                notices
            }
        }
    }
}

/// A git repository, opened read-only.
#[derive(Debug)]
pub struct RepoTarget {
    /// The open handle. `gix` holds no write path to it.
    pub repo: gix::Repository,
    /// The working directory being visualized.
    pub root: PathBuf,
    /// The commit structure is extracted from — HEAD (`F-EXT-7`).
    ///
    /// `None` for a repository with no commits (PRD §6), which is an ordinary path.
    pub head: Option<CommitId>,
    /// Whether history is truncated by a `--depth` clone.
    pub is_shallow: bool,
    /// Whether HEAD points at a commit rather than a branch.
    ///
    /// Supported and treated as the current commit (PRD §6); recorded because it is worth
    /// telling the user, not because it changes anything.
    pub is_detached: bool,
}

/// Something the user should be told after association (`F-ASSOC-2`).
///
/// Returned rather than logged: these are product surface, and the shell decides how to
/// present them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    /// No `.git`. Structure only; age, churn, and ownership are unavailable.
    NoRepository,
    /// History is truncated by a `--depth` clone.
    ///
    /// PRD §6 is emphatic that this must be said out loud: a shallow clone grows an ageless
    /// tree, and "silently producing a history-less tree is a defect — this is common and
    /// would otherwise look like a bug."
    ShallowClone,
    /// A git repository with no commits yet.
    NoCommits,
    /// HEAD points at a commit rather than a branch.
    DetachedHead,
}

impl Notice {
    /// The user-facing explanation.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::NoRepository => {
                "No git repository here. The tree will show structure and size, but not age, \
                 churn, or who wrote what."
            }
            Self::ShallowClone => {
                "This is a shallow clone, so most of its history is missing. The tree will \
                 read as ageless. Unshallow the clone for a complete one."
            }
            Self::NoCommits => {
                "This repository has no commits yet. The tree will show a seed and root \
                 cluster until there is history to grow from."
            }
            Self::DetachedHead => {
                "HEAD is detached. The tree reflects the commit currently checked out."
            }
        }
    }
}

/// Why a path cannot be extracted from.
#[derive(Debug)]
pub enum DiscoverError {
    /// A bare repository — no working tree (PRD §6, rejected at association).
    Bare {
        /// The git directory found.
        git_dir: PathBuf,
    },
    /// The path does not exist, is not a directory, or cannot be read.
    Inaccessible {
        /// The path as given.
        path: PathBuf,
        /// What the filesystem said.
        source: std::io::Error,
    },
    /// The path holds something `gix` recognizes as a repository but cannot open.
    Unreadable {
        /// The path as given.
        path: PathBuf,
        /// What `gix` said.
        message: String,
    },
}

impl std::fmt::Display for DiscoverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bare { git_dir } => write!(
                f,
                "{} is a bare repository. treepo grows a tree from a working directory, and a \
                 bare repository has none. Clone it normally and open the clone.",
                git_dir.display()
            ),
            Self::Inaccessible { path, source } => {
                write!(f, "{} cannot be read: {source}", path.display())
            }
            Self::Unreadable { path, message } => {
                write!(
                    f,
                    "{} is a repository treepo cannot open: {message}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for DiscoverError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Inaccessible { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Opens `path` and reports what is there (`F-ASSOC-2`).
///
/// # Errors
///
/// [`DiscoverError::Inaccessible`] if the path is missing or unreadable,
/// [`DiscoverError::Bare`] if it is a bare repository, and [`DiscoverError::Unreadable`] if
/// `gix` recognizes but cannot open it.
pub fn discover(path: impl AsRef<Path>) -> Result<Target, DiscoverError> {
    let path = path.as_ref();

    // Checked before opening so a missing directory reports as inaccessible rather than as
    // a confusing `gix` error about a missing object database.
    let metadata = std::fs::metadata(path).map_err(|source| DiscoverError::Inaccessible {
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.is_dir() {
        return Err(DiscoverError::Inaccessible {
            path: path.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::NotADirectory,
                "not a directory — treepo grows a tree from a folder",
            ),
        });
    }

    let repo = match gix::open(path) {
        Ok(repo) => repo,
        // No `.git` here. An ordinary path (`AC-ASSOC-3`), not a failure.
        Err(gix::open::Error::NotARepository { .. }) => {
            return Ok(Target::PlainDirectory {
                root: path.to_path_buf(),
            });
        }
        Err(error) => {
            return Err(DiscoverError::Unreadable {
                path: path.to_path_buf(),
                message: error.to_string(),
            });
        }
    };

    if repo.is_bare() || repo.workdir().is_none() {
        return Err(DiscoverError::Bare {
            git_dir: repo.git_dir().to_path_buf(),
        });
    }

    let root = repo
        .workdir()
        .map_or_else(|| path.to_path_buf(), Path::to_path_buf);

    // An unborn HEAD is a repository with no commits, not an error (PRD §6).
    let head = repo.head_id().ok().and_then(|id| to_commit_id(&id));
    let is_detached = repo
        .head()
        .map(|head| head.is_detached())
        .unwrap_or_default();

    Ok(Target::Repository(Box::new(RepoTarget {
        root,
        head,
        is_shallow: repo.is_shallow(),
        is_detached,
        repo,
    })))
}

/// Converts a `gix` object id into the model's.
///
/// `None` for a hash width the model does not know, which no released git produces — better
/// an absent commit than a silently truncated one.
pub(crate) fn to_commit_id(id: &gix::oid) -> Option<CommitId> {
    let bytes = id.as_bytes();
    match bytes.len() {
        20 => bytes.try_into().ok().map(CommitId::sha1),
        32 => bytes.try_into().ok().map(CommitId::sha256),
        _ => None,
    }
}

/// The name of a tree entry as repository path bytes.
pub(crate) fn entry_name(name: &gix::bstr::BStr) -> Vec<u8> {
    name.as_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_path_is_inaccessible() {
        let error = discover("does-not-exist-anywhere-9f3a").expect_err("should fail");
        assert!(matches!(error, DiscoverError::Inaccessible { .. }));
        assert!(error.to_string().contains("cannot be read"));
    }

    #[test]
    fn a_file_is_not_a_directory() {
        let error = discover("Cargo.toml").expect_err("should fail");
        assert!(matches!(error, DiscoverError::Inaccessible { .. }));
    }

    /// treepo's own repository, which is the one fixture guaranteed to exist here.
    #[test]
    fn this_repository_opens_and_reports_a_head() {
        let root = workspace_root();
        let target = discover(&root).expect("treepo's own repository opens");
        let Target::Repository(repo) = &target else {
            panic!("expected a repository at {}", root.display());
        };
        assert!(repo.head.is_some(), "treepo has commits");
        assert_eq!(target.root(), root);
        assert!(target.repository().is_some());
        // Shallowness is not asserted either way: a CI checkout is shallow by default and a
        // developer's is not. What must hold is that the flag and the notice agree, which
        // is the part PRD §6 cares about — a shallow clone must always be said out loud.
        assert_eq!(
            repo.is_shallow,
            target.notices().contains(&Notice::ShallowClone)
        );
    }

    #[test]
    fn a_plain_directory_is_reported_rather_than_refused() {
        // `crates/` has no `.git` of its own, and discovery does not search upward.
        let target = discover(workspace_root().join("crates")).expect("plain directory");
        assert!(matches!(target, Target::PlainDirectory { .. }));
        assert_eq!(target.notices(), [Notice::NoRepository]);
        assert!(target.repository().is_none());
    }

    #[test]
    fn every_notice_says_something_useful() {
        for notice in [
            Notice::NoRepository,
            Notice::ShallowClone,
            Notice::NoCommits,
            Notice::DetachedHead,
        ] {
            let message = notice.message();
            assert!(message.len() > 40, "{notice:?} needs a real explanation");
            assert!(message.ends_with('.'));
        }
    }

    fn workspace_root() -> PathBuf {
        // CARGO_MANIFEST_DIR is `<root>/crates/treepo-vcs`.
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root")
            .to_path_buf()
    }
}
