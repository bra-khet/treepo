//! Bevy Remote Protocol for coding agents — architecture D10, **dev-only**.
//!
//! Compiled only under `--features brp`, which is never in `default`, never in a CI build, and
//! never in a release package (RISK-D). Enabling it puts an HTTP listener on
//! `127.0.0.1:15702` inside the process so an agent can query the ECS, drive input, and take
//! screenshots while the app runs.
//!
//! ```text
//! cargo run -p treepo-app --features brp -- <path-to-repository>
//! # then use the bevy_brp MCP tools against port 15702
//! ```
//!
//! **Agent launch (read this):** start the process with that `cargo` line (shell), **not**
//! `bevy_brp_mcp`'s `brp_launch`. That tool's freshness rebuild omits `--features brp` and can
//! silently replace a BRP-enabled binary with one that has no listener. After the app is up on
//! 15702, use the other `bevy_brp` MCP tools (status, screenshot, world_*, input, …).
//!
//! The port is 15702 by default and `BRP_EXTRAS_PORT` overrides it — both are
//! `bevy_brp_extras`' own contract, not treepo's, and are documented in the campaign's
//! deployment notes as dev/agent-only environment variables.
//!
//! # Why this is a feature and not a flag
//!
//! `N2` says repository data never leaves the machine, and its mechanism is that **no
//! network-capable crate is in the product dependency graph** — not that a runtime check
//! refuses to open a socket. A `--debug-brp` command-line flag would put the HTTP stack in
//! every shipped binary and leave `N2` resting on a branch nobody can audit from outside. A
//! Cargo feature keeps the mechanism honest: `deny.toml` bans `bevy_remote` and
//! `bevy_brp_extras` by name, so the check fails the moment this stops being optional.
//!
//! What is here is registration and nothing else. It adds no treepo-specific method, and it is
//! not a place for one: a remote method that could reach the store or the repository would be
//! a control surface the product cannot audit, in a build that exists to be pointed at real
//! repositories.

use bevy::prelude::*;
use bevy_brp_extras::BrpExtrasPlugin;

/// Registers BRP on the default port, and the one resource treepo puts behind it.
///
/// `BrpExtrasPlugin` adds Bevy's `RemotePlugin` and the HTTP transport itself if they are not
/// already present, and registers the extras an agent needs beyond raw entity access —
/// screenshot, input injection, graceful shutdown, and type discovery.
///
/// [`frame_log`](super::frame_log) is the exception to the paragraph above about remote
/// surfaces, and it is the shape of exception that argument allows: a counter of frame times
/// and resident texels, which reaches neither the store nor the repository, and which exists
/// because `AC-NAV-2` and `NFR-3` are claims that have to be measured from inside a running
/// frame loop. It is still not a remote *method* — BRP's own resource access reads it.
pub(crate) fn register(app: &mut App) {
    app.add_plugins(BrpExtrasPlugin::default());
    super::frame_log::register(app);
}
