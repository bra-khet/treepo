//! What a contributor is *shown as* — `F-ID-3`, `F-ID-4`, `N9`.
//!
//! `treepo-model` holds contributors as [`AuthorKey`](treepo_model::identity::AuthorKey): a
//! one-way hash of a mailmap-normalized email, carrying no name. That is deliberate and it
//! leaves a gap — something has to turn a key into a thing a person can see and recognise.
//! This crate is that something, and it is the *only* one, which is what makes `N9`
//! enforceable rather than merely intended.
//!
//! Two independent functions of the key, and one gate over them:
//!
//! * [`pseudonym`] — `F-ID-3`. A stable two-word name, drawn from a themed wordlist, with
//!   deterministic collision resolution within a repository.
//! * [`palette`] — `F-ID-4`. A stable colour, drawn from a palette whose entries are
//!   guaranteed to be perceptually separated (`AC-MAT-4`).
//! * [`policy`] — `F-ID-5`, `F-ID-7`. [`IdentityView`], the only thing that turns a key into
//!   something a person sees, and the only place a real name can enter.
//!
//! # `F-ID-1` is not here, and that was a decision
//!
//! The architecture's file tree put `self_ident.rs` in this crate. It lives in
//! `treepo-vcs::self_ident` instead: reading `user.email` means opening a git config, and
//! this crate is `no_std` with no I/O — which is precisely the property that keeps it from
//! quietly acquiring a repository dependency later. `treepo-vcs` reads the config and hands
//! over an [`AuthorKey`](treepo_model::identity::AuthorKey); this crate stays a pure
//! function of one. The architecture document was amended to match rather than the roles
//! being bent to fit it.
//!
//! # Where a real name can be
//!
//! In exactly one place: [`RealNames`], held by a view built with
//! [`IdentityView::revealed`]. Under the default constructor the field is `None`, so a
//! pseudonymous view does not merely decline to show a name — it holds none to show. That
//! is what makes `AC-ID-1` survive a bug in the display path rather than depend on there
//! being no bug.
//!
//! `treepo-model` carries no names either, so [`RealNames`] cannot be read out of a stored
//! manifest. Reveal needs the repository, which means a shared manifest (`F-MAN-11`) cannot
//! be de-anonymized by toggling a setting.
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
pub mod policy;
pub mod pseudonym;

pub use palette::{AuthorColor, Oklab, Palette, PaletteError};
pub use policy::{Identification, IdentityPolicy, IdentityView, RealNames};
pub use pseudonym::{Pseudonym, Roster, Wordlist, WordlistError};
