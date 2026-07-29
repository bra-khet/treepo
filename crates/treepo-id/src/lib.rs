//! What a contributor is *shown as* — `F-ID-3`, `F-ID-4`, `N9`.
//!
//! `treepo-model` holds contributors as [`AuthorKey`](treepo_model::identity::AuthorKey): a
//! one-way hash of a mailmap-normalized email, carrying no name. That is deliberate and it
//! leaves a gap — something has to turn a key into a thing a person can see and recognise.
//! This crate is that something, and it is the *only* one, which is what makes `N9`
//! enforceable rather than merely intended.
//!
//! Two independent functions of the key, and nothing else:
//!
//! * [`pseudonym`] — `F-ID-3`. A stable two-word name, drawn from a themed wordlist, with
//!   deterministic collision resolution within a repository.
//! * [`palette`] — `F-ID-4`. A stable colour, drawn from a palette whose entries are
//!   guaranteed to be perceptually separated (`AC-MAT-4`).
//!
//! # Why this crate has no real names in it yet
//!
//! `F-ID-1` (self-identification from `user.email`) and `F-ID-5` (the one setting that
//! governs live view and exports together) are the parts of `N9` that handle a real
//! identity, and they arrive with `self_ident.rs` and `policy.rs`. Until they do, **no code
//! path in treepo can produce a contributor's name at all** — not because a policy forbids
//! it, but because nothing holds one. That is the strongest form `AC-ID-1` can take, and it
//! is worth having for as long as it lasts.
//!
//! What that costs is one thing worth stating plainly: [`Wordlist::draw`] and
//! [`Palette::color_of`] are public and ungated today. When `policy.rs` lands it becomes the
//! single entry point, and these two drop to `pub(crate)`.
//!
//! # `N3`
//!
//! `no_std`, integer-only, and every value a function of an [`AuthorKey`] plus a file. There
//! is no clock to read, no collection whose iteration order is a process property, and no
//! float. `AC-ID-2` — the same repository yielding the same pseudonyms and colours on
//! Windows, macOS and Linux — is checked by `cargo xtask determinism` rather than argued
//! from those properties.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
// `N3`, as in `treepo-det` and `treepo-gen`: no float may enter a value that reaches
// generated output. The palette's perceptual metric is fixed-point for this reason.
#![deny(clippy::float_arithmetic)]
#![deny(clippy::float_cmp)]

extern crate alloc;

pub mod palette;
pub mod pseudonym;

pub use palette::{AuthorColor, Oklab, Palette, PaletteError};
pub use pseudonym::{Pseudonym, Roster, Wordlist, WordlistError};
