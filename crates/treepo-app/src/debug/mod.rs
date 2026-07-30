//! The developer surface. None of it is product.
//!
//! Two things will live here and they are not the same kind of thing:
//!
//! * **`brp.rs`** — architecture D10. Agent control over localhost, behind the non-default
//!   Cargo feature `brp`. Launch with `cargo run -p treepo-app --features brp` (shell), not
//!   MCP `brp_launch` — see that module's header.
//! * **`frame_log.rs`** — per-frame cost and residency, behind the same feature and read
//!   through it. `AC-NAV-2` and `NFR-3` are measurements, and a measurement needs somewhere to
//!   land that is not a moving average.
//! * **`intensity.rs`** — `F-THR-8`'s debug toggle, present in dev builds and absent from
//!   release ones, asserted by a Phase 8 test. It arrives with the thing it toggles.
//!
//! The egui surface architecture D8 reserves for this module (parameter tuning, determinism
//! inspection) is also later: `bevy_egui` is a dependency worth adding when there is something
//! to inspect, and today the whole of the app's state fits in the three lines of
//! [`ui`](crate::ui).

#[cfg(feature = "brp")]
pub(crate) mod brp;
#[cfg(feature = "brp")]
pub(crate) mod frame_log;
