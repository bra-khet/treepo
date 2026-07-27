//! Determinism primitives for treepo.
//!
//! `N3` is absolute: the same repository state produces the same tree, on any machine, at
//! any time. `AC-DET-2` sharpens that into a testable claim — identical output hashes on
//! Windows, macOS, and Linux.
//!
//! This crate is the single place nondeterminism is allowed to enter, so that it can be
//! denied everywhere else (architecture D2). Everything downstream draws its randomness,
//! its trigonometry, its hashing, and its iteration order from here.
//!
//! # The determinism contract
//!
//! Every value this crate produces is a function of its explicit inputs, computed with
//! integer arithmetic only. There is no ambient state to read and no floating-point
//! operation to round differently.
//!
//! * [`fixed`] — [`Fx`], a Q32.32 fixed-point scalar, and [`Angle`], a binary angle whose
//!   arithmetic wraps exactly at a full turn. Saturating throughout, so debug and release
//!   builds agree.
//! * [`trig`] — `F-SKEL-6`. Table-driven sine and cosine. Never the platform `libm`, whose
//!   results are not guaranteed bit-identical across machines (`RISK-2`).
//! * [`rng`] — ChaCha8, explicitly seeded. There is no way to construct one from entropy,
//!   the clock, or the environment.
//! * [`hash`] — SHA-256, and the [`Seed`] tree that turns a path into a generator seed
//!   (`P2` — "seeded hierarchically from path hashes").
//! * [`ordered`] — [`OrderedMap`] and [`OrderedSet`], whose iteration order is a function
//!   of their keys rather than of their insertion history or a per-process hash seed.
//!
//! # What this crate cannot protect you from
//!
//! Three leaks live outside it and are closed elsewhere. They are listed here because this
//! is where someone will look for them.
//!
//! 1. **Unsorted directory reads.** `read_dir` order is a filesystem property.
//!    `treepo-vcs::walk` sorts explicitly (`AC-DET-3`).
//! 2. **`sort_unstable_by`.** The relative order of equal elements is unspecified and may
//!    change between Rust versions. Use the stable `sort_by`/`sort_by_key` for anything
//!    that reaches generated output. The toolchain is pinned partly for this reason.
//! 3. **`#[derive(Hash)]` and `DefaultHasher`.** Stable within a build, not across Rust
//!    versions. Use [`hash`] for anything persisted or compared across machines. Both are
//!    denied by `clippy.toml`.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
// N3, and the reason this crate exists: no float may enter a generated value. Conversions
// at the parameter boundary are permitted and deterministic; arithmetic is not permitted.
#![deny(clippy::float_arithmetic)]
#![deny(clippy::float_cmp)]

extern crate alloc;

pub mod fixed;
pub mod hash;
pub mod ordered;
pub mod rng;
pub mod trig;

pub use fixed::{Angle, Fx};
pub use hash::{Digest, Seed, Sha256};
pub use ordered::{OrderedMap, OrderedSet};
pub use rng::ChaCha8Rng;
pub use trig::{cos, sin, sin_cos};
