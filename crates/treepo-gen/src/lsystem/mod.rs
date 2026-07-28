//! The L-system — `F-SKEL-1`, `F-SKEL-2`, `F-SKEL-6`.
//!
//! Three passes, deliberately separable, in the order the design document describes them:
//!
//! 1. [`grammar`] applies parametric, stochastic productions and yields a module string.
//! 2. [`turtle`] interprets that string into oriented, thickened segments, using
//!    [`treepo_det::trig`] rather than the platform `libm` (`F-SKEL-6`).
//! 3. [`compose`] runs one instance per limb, hierarchically, and decides where the tree
//!    stops branching and starts aggregating.
//!
//! Keeping the first two apart is what `design/l-system-parameterization.md` §2.1 already
//! assumes — "the string produced by the L-system is interpreted by a turtle" — and it means
//! the productions can be tested as symbols and the geometry as coordinates. The third is
//! §2.4's hierarchical composition, and it is where the parameter table's numbers become a
//! decision about what the tree draws individually and what it draws as a container.
//!
//! # The whole of `F-SKEL-1` in one signature
//!
//! ```text
//! (subtree primitives, path seed, parameter table) → oriented, thickened segments
//! ```
//!
//! [`compose::compose`] is that function. It takes a manifest, a table, and a starting frame;
//! it reads no clock, opens no file, and consults no global. Every stochastic draw comes from
//! a [`Seed`](treepo_det::Seed) derived from a path, so running it twice on one machine and
//! once each on three others produces the same [`Skeleton`](treepo_model::Skeleton) —
//! `AC-DET-1` and `AC-DET-2`, which Phase 0 built the determinism primitives to make
//! achievable.

pub mod compose;
pub mod grammar;
pub mod turtle;

pub use compose::compose;
pub use grammar::Module;
pub use turtle::{Interpretation, Start, Tip};
