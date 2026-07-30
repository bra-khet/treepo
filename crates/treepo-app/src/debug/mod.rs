//! The developer surface. None of it is product.
//!
//! Two things will live here and they are not the same kind of thing:
//!
//! * **`brp.rs`** — architecture D10. Agent control over localhost, behind the non-default
//!   Cargo feature `brp`. It is here now, as a scaffold.
//! * **`intensity.rs`** — `F-THR-8`'s debug toggle, present in dev builds and absent from
//!   release ones, asserted by a Phase 8 test. It arrives with the thing it toggles.
//!
//! The egui surface architecture D8 reserves for this module (parameter tuning, determinism
//! inspection) is also later: `bevy_egui` is a dependency worth adding when there is something
//! to inspect, and today the whole of the app's state fits in the three lines of
//! [`ui`](crate::ui).

#[cfg(feature = "brp")]
pub(crate) mod brp;
