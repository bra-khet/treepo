//! Turning a repository into a [`Manifest`](treepo_model::Manifest).
//!
//! This is the only crate that reads a repository, and it reads it **once** — `F-EXT-2`'s
//! single history pass is what makes the `AC-EXT-1` budget reachable at all, and `N6` keeps
//! every byte of this work on the rare, expensive side of the phase boundary. Once a
//! manifest exists, nothing downstream opens the repository again.
//!
//! # Reading, never writing, never executing
//!
//! `N1` is not a policy this crate follows, it is a property of what it is built from.
//! Architecture D3 chose `gix` over subprocess `git` for two reasons that both land here:
//!
//! * A subprocess `git` honours repository config that can execute programs —
//!   `core.fsmonitor`, `core.pager`, aliases, textconv filters. `gix` runs none of them, so
//!   extraction cannot be turned into a code-execution path by a repository treepo does not
//!   trust (`AC-EXT-4`). Its blob-diff path hard-disables external diff commands outright.
//! * `R1` — a consumer machine has no `git` binary, and on Windows most buyers will not
//!   have one.
//!
//! Nothing here opens a file for writing, and the HEAD-tree walk does not touch the working
//! directory at all (`AC-MAN-2`, `F-ASSOC-7`).
//!
//! # The passes
//!
//! * [`discover`] — `F-ASSOC-2`. What is at the path the user picked, and what to tell them.
//! * [`filter`] — `F-EXT-8`. What counts as the repository's structure.
//! * [`walk`] — `F-EXT-1`. Structural and size primitives from the HEAD tree.
//! * [`mailmap`] — `F-EXT-9`. One human is one contributor.
//! * [`log_pass`] — `F-EXT-2`. Every temporal and ownership primitive, from one traversal.
//!
//! Still to come in Phase 1: `lang` (`F-EXT-4`) and `status` (`F-THR-4`).
//!
//! # `git blame` is not reachable from here
//!
//! `F-EXT-3` demotes blame to a resumable pass that runs *after* the first Grow, because
//! `RISK-1` is that it is `O(files × history)` and unaffordable as a gating step. That is
//! enforced structurally rather than by discipline: this crate does not enable `gix`'s
//! `blame` feature, so `gix::blame` does not exist in this build and reaching for it does
//! not compile.
//!
//! ```compile_fail
//! // F-EXT-3 / RISK-1: blame must not be reachable from extraction.
//! let _: fn() = || { let _ = gix::blame::Error::default(); };
//! ```
//!
//! A `compile_fail` test passes when the code fails to compile for *any* reason, so this
//! one needs a control. The same reference through a feature that *is* enabled compiles,
//! which is what makes the failure above mean "the feature is off" rather than "`gix` is
//! not in scope":
//!
//! ```
//! let _: fn() = || { let _: Option<gix::diff::Rewrites> = None; };
//! ```
//!
//! # Determinism
//!
//! `AC-DET-3` names unsorted directory reads as a leak `treepo-det` cannot close, and this
//! is the crate where it would happen. Both walks sort explicitly; see [`walk`] for why the
//! HEAD-tree path sorts even though git already returns entries in an order.

#![forbid(unsafe_code)]
// N3, as in treepo-det and treepo-model: extraction produces values that flow into
// generated output, and none of them may come from float arithmetic.
#![deny(clippy::float_arithmetic)]
#![deny(clippy::float_cmp)]

pub mod discover;
pub mod filter;
pub mod lang;
pub mod log_pass;
pub mod mailmap;
pub mod walk;

pub use discover::{DiscoverError, Notice, RepoTarget, Target, discover};
pub use filter::{Decision, DefaultExclusions, ExclusionGroup, FilterSet};
pub use log_pass::{History, HistoryError, HistoryOptions, PathHistory, log_pass};
pub use mailmap::Identities;
pub use walk::{Structure, StructureSource, WalkError, WalkOptions, walk};
