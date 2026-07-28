//! Where a repository's data lives, and what makes it that repository.
//!
//! `F-MAN-1` puts everything treepo derives in application data rather than in the working
//! tree, which turns one question into the foundation of the whole store: *which* repository
//! is this? Two clones of one remote must land in one directory (`AC-MAN-4`), a folder that
//! moves must keep its directory (`AC-MAN-5`), and a fork must not inherit its upstream's
//! (PRD §6). [`resolve`] answers that; [`paths`] turns the answer into a path.
//!
//! # What writes, and what cannot
//!
//! [`resolve`] and [`paths`] write nothing, and the split is deliberate: a normalized URL is a
//! function of a string and a store path is a function of an identity, so both are testable
//! exhaustively without a filesystem. Their only filesystem call is the `canonicalize` tier 3
//! needs. [`manifest_io`] is where bytes land, and everything difficult about that —
//! atomicity (`F-MAN-7`), schema versioning (`F-MAN-6`) — is confined to it.
//!
//! Nothing anywhere in the crate writes into the *working tree*. Every path it constructs is
//! rooted at [`StoreRoot`], which is application data (`F-MAN-1`, `AC-MAN-2`); the repository
//! is opened read-only and only ever read from. `cargo xtask readonly-audit` runs identity
//! resolution over every corpus fixture for exactly this reason.
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

pub mod identity_io;
pub mod manifest_io;
pub mod paths;
pub mod resolve;

pub use identity_io::IdentityError;
pub use manifest_io::{ReadError, Staged, WriteError, read, stage, write};
pub use paths::{LayoutError, RepositoryStore, StoreRoot};
pub use resolve::{Resolution, ResolveError, Skipped, normalize_url, resolve};
