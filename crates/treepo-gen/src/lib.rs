//! Turning a [`Manifest`](treepo_model::Manifest) into geometry.
//!
//! This is the crate the visual identity lives or dies in. It holds the first two of the four
//! generative layers `design/visual-construction.md` defines:
//!
//! * **the structural skeleton** — topology, limb geometry, thickness, angles.
//!   [`params`], [`lsystem`], [`trunk`] (`F-SKEL-1`…`F-SKEL-7`).
//! * **material** — what each limb is made of, who is drawn on it, how old it is, how much of
//!   the picture it may occupy, what is built on it, and what is wrong with it. [`material`],
//!   [`normalize`], [`enrich`](mod@enrich), [`stress`] (`F-MAT-1` … `F-MAT-6`).
//!
//! Everything alive arrives in later layers and later phases.
//!
//! Both layers are pure functions of a [`Manifest`](treepo_model::Manifest) and a parameter
//! table, and there are two tables — [`Table`] for the skeleton and [`MaterialTable`] for
//! material — versioned independently, so that tuning a silhouette cannot invalidate a
//! material and vice versa.
//!
//! The three entry points pair up, and a caller wanting a tree calls all of them in order:
//!
//! ```ignore
//! let materials_table = MaterialTable::built_in();
//! let skeleton   = treepo_gen::grow(&manifest, &Table::built_in());
//! let materials  = treepo_gen::materialize(&manifest, &skeleton, &materials_table);
//! let enrichment = treepo_gen::enrich(&manifest, &skeleton, &materials, &materials_table);
//! ```
//!
//! Each visits nodes in the order [`grow`] created them, so the resulting
//! [`MaterialMap`](treepo_model::MaterialMap) and
//! [`EnrichmentMap`](treepo_model::EnrichmentMap) are indexed by the same
//! [`NodeId`](treepo_model::NodeId) — `covers` is the invariant, and it holds by construction.
//! [`enrich`] takes the materials rather than recomputing them because enrichment is placed
//! *on* material: sized by its budget, positioned along its age gradient.
//!
//! # `F-SKEL-1`: the skeleton is a pure function
//!
//! `(subtree primitives, path seed, parameter table) → oriented, thickened segments`.
//!
//! No global state, no time input, no I/O. Grow calls it repeatedly during a cinematic
//! transition and must get the same answer every time, so the only inputs are the three
//! named above and the only randomness is drawn from a seed derived from a path.
//!
//! # Two structural constraints, held the same way `treepo-det` holds its own
//!
//! **`no_std`.** Not portability — it makes `std::time`, `std::collections::HashMap`, and
//! the filesystem *unreachable* from the crate every generated coordinate flows through.
//! The architecture calls this crate "pure generation — no bevy, no I/O", and `no_std` is
//! that sentence expressed as something the compiler checks rather than something a reviewer
//! notices. It also settles `N6`: a crate that cannot name `bevy` cannot be called from a
//! Thrive system with a `World` in scope.
//!
//! **No floats.** `#![deny(clippy::float_arithmetic)]`, as in `treepo-det` and
//! `treepo-model`. Every length, angle, and ratio here is [`Fx`](treepo_det::Fx) or
//! [`Angle`](treepo_det::Angle), and every trigonometric value comes from
//! [`treepo_det::trig`] rather than the platform `libm` (`F-SKEL-6`). `AC-DET-2` — identical
//! output hashes on Windows, macOS and Linux — is the reason both rules exist, and Phase 0
//! measured that they work.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
// N3, and the reason the whole determinism foundation was built first: a float that reaches
// a coordinate is a coordinate that differs by machine. Conversions at the parameter
// boundary would be permitted and deterministic; nothing here needs one.
#![deny(clippy::float_arithmetic)]
#![deny(clippy::float_cmp)]

extern crate alloc;

pub mod enrich;
pub mod lsystem;
pub mod material;
pub mod normalize;
pub mod params;
pub mod stress;
pub mod trunk;

pub use enrich::{EnrichError, enrich};
pub use lsystem::compose;
pub use normalize::{Normalize, NormalizeError};
pub use params::{LimbParams, SkeletonInputs, Table, TableError, TrunkParams};
pub use stress::StressError;
pub use trunk::grow;

// Renamed on the way out, not in its own module. Two parameter tables now exist and
// `crate::Table` has meant the skeleton's since Phase 3; a second bare `Table` at the root
// would make every import site ambiguous to a reader even where it compiles.
pub use material::{AgeSpan, MaterialError, Table as MaterialTable, materialize};
