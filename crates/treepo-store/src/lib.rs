//! Where a repository's data lives, and what makes it that repository.
//!
//! `F-MAN-1` puts everything treepo derives in application data rather than in the working
//! tree, which turns one question into the foundation of the whole store: *which* repository
//! is this? Two clones of one remote must land in one directory (`AC-MAN-4`), a folder that
//! moves must keep its directory (`AC-MAN-5`), and a fork must not inherit its upstream's
//! (PRD §6). [`resolve`] answers that; [`paths`] turns the answer into a path.
//!
//! # Nothing in this crate writes
//!
//! Not yet, and the split is deliberate. Identity resolution and store addressing are pure
//! enough to test exhaustively — a normalized URL is a function of a string, and a store path
//! is a function of an identity. Creating directories and writing files is `manifest_io`,
//! where atomicity (`F-MAN-7`) and schema versioning (`F-MAN-6`) are the whole problem.
//! Keeping them apart means `AC-MAN-2`'s zero-write property holds here by construction: the
//! only filesystem call in the crate is the `canonicalize` that tier 3 needs, and the only
//! environment reads are the ones `F-MAN-2` names.
//!
//! # The identity is not a secret, but it is one-way
//!
//! A remote URL can carry a credential — `https://x-access-token:ghp_…@github.com/o/r.git`
//! is what a CI checkout leaves in `.git/config`, and users paste them by hand too. The
//! normalizer strips credentials before anything hashes or stores the URL, so a token cannot
//! reach `identity.json`, a store directory name, or a shared package (`F-MAN-11`). That is a
//! normalization rule the PRD already asks for; it is written down here because the reason it
//! matters is not the one the PRD gives.

#![forbid(unsafe_code)]

pub mod paths;
pub mod resolve;

pub use paths::{LayoutError, RepositoryStore, StoreRoot};
pub use resolve::{Resolution, ResolveError, Skipped, normalize_url, resolve};
