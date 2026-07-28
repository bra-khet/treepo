//! Turning a [`Manifest`](treepo_model::Manifest) into geometry.
//!
//! This is the crate the visual identity lives or dies in. It holds the structural skeleton
//! — the first of the four generative layers `design/visual-construction.md` defines — and
//! nothing else: topology, limb geometry, thickness, angles. Material, ownership colour,
//! enrichment, and everything alive arrive in later layers and later phases.
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

pub mod lsystem;
pub mod params;
pub mod trunk;

pub use lsystem::compose;
pub use params::{LimbParams, SkeletonInputs, Table, TableError, TrunkParams};
pub use trunk::grow;
