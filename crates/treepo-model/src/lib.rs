//! The types treepo's phases exchange values in.
//!
//! Everything downstream of extraction reads this crate and nothing writes to a repository
//! through it. It holds no I/O, no `gix`, and no Bevy — `treepo-vcs` fills these types in
//! from a repository, `treepo-gen` turns them into geometry, and neither needs to know the
//! other exists.
//!
//! # What lives here
//!
//! * [`path`] — [`RepoPath`], a byte path that survives non-UTF-8 names and compares the
//!   same on every platform.
//! * [`primitives`] — the extracted feature system (`design/feature-system.md` §3), one
//!   module per category: structural, size, temporal, ownership, derived, folder signals.
//! * [`manifest`] — [`Manifest`] and [`PathRecord`], the persisted result of one extraction.
//! * [`identity`] — [`RepoIdentity`] and [`AuthorKey`]. Neither carries a name.
//!
//! Skeleton, material, and snapshot types (`segment.rs`, `material.rs`, `snapshot.rs`,
//! `enrichment.rs`, `aggregate.rs` in the architecture's file tree) arrive with the phases
//! that produce them. Defining them now would be guessing at Phase 3's shape.
//!
//! # Three properties this crate is built to hold
//!
//! **No floats.** `#![deny(clippy::float_arithmetic)]`, as in `treepo-det`. Counts are
//! integers and computed ratios are [`Fx`](treepo_det::Fx). A manifest containing a float
//! computed from a platform `libm` would be a manifest that differs by machine, and
//! `AC-MAN-1` requires that regenerating one produces identical bytes.
//!
//! **No wall clock.** Nothing here stores an age, only absolute timestamps — see
//! [`temporal`](primitives::temporal) for why that distinction is load-bearing rather than
//! pedantic. The one exception, [`RepoIdentity::resolved_at`], is metadata that never
//! reaches generation, and is documented as such where it is declared.
//!
//! **No ranking of people.** `N4` is a constitutional constraint, and in this crate it is a
//! type-level one: [`AuthorShare`](primitives::AuthorShare) deliberately implements neither
//! `Ord` nor `PartialOrd`, so the standard library will not sort contributors by
//! contribution for you. See [`ownership`](primitives::ownership).
//!
//! # `no_std`
//!
//! Same reasoning as `treepo-det`: it makes `std::collections::HashMap` and `std::time`
//! unreachable from the types that flow into generated output, rather than merely
//! discouraged. It also means this crate cannot accept a [`std::path::Path`], which is the
//! intended outcome — turning a platform path into repository bytes is a filesystem
//! concern and belongs in `treepo-vcs`, where the platform differences are already in view.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
// N3. Identical reasoning to `treepo-det`: no float may reach a persisted or generated
// value. There is no exemption in this crate — unlike `treepo-det`, nothing here needs a
// float conversion at a parameter boundary.
#![deny(clippy::float_arithmetic)]
#![deny(clippy::float_cmp)]

extern crate alloc;

pub mod identity;
pub mod manifest;
pub mod path;
pub mod primitives;

pub use identity::{AuthorKey, CommitId, IdentityTier, RepoIdentity};
pub use manifest::{
    AuthorEntry, AuthorTable, FilterOverrides, LanguageId, LanguageTable, Manifest, NodeKind,
    PathRecord, SCHEMA_VERSION,
};
pub use path::{PathError, RepoPath};
pub use primitives::{
    DerivedSignals, FolderSignal, OwnershipPrimitives, SizePrimitives, StructuralPrimitives,
    TemporalPrimitives,
};
