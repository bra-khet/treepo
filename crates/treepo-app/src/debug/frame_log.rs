//! Frame cost and residency, sampled every frame — the evidence `AC-NAV-2` and `NFR-3` want.
//!
//! Compiled only under `--features brp`, for the same reason as [`brp`](super::brp): a build
//! that carries a measurement surface is not the build that ships. Nothing here is reachable
//! from a default build, and nothing here reads the repository or the store.
//!
//! # Why a resource and not `FrameTimeDiagnosticsPlugin`
//!
//! Bevy's diagnostics keep a smoothed average, and `AC-NAV-2` is not a claim about an average.
//! "Zoom far→near on T3 holds 30 fps" is a claim about the *worst* frame in the zoom — a band
//! crossing that bakes four chunks at once is precisely the frame a moving average hides. So
//! this counts every frame, keeps the maximum, and counts how many exceeded the budget.
//!
//! Polling a diagnostic over BRP would be worse still: a round trip costs tens of milliseconds,
//! so the sampler would miss most of the frames it is meant to be judging.
//!
//! # Attributing the cost without instrumenting the renderer
//!
//! A slow frame during a zoom is either the **bake** ([`treepo_render::chunk::stream`]
//! rasterizing up to `BAKE_TEXELS_PER_FRAME` texels on the main thread) or the **draw** (one sprite
//! per resident piece, on the GPU). Telling them apart normally means a timer inside the render
//! crate, which is instrumentation that would then exist in every build.
//!
//! The first version of this module inferred it: a frame that baked is a frame whose resident
//! piece count *grew*, and this system can see the count. That was free and it was wrong in the
//! one case that matters. A band change bakes and evicts in the same frame, so the count often
//! **falls** on a frame that baked hard — and every such frame was filed as quiet. The
//! materials measurement is where it showed: `worst_quiet_ms` read 72 ms on a band the renderer
//! was rasterizing flat out, which reads as "holding the picture costs 72 ms" when holding it
//! cost nothing.
//!
//! It now reads [`BakeLoad`] instead, which the renderer sets from what it actually did.
//! [`FrameLog::worst_idle_ms`] is the worst frame that drew no texels — steady-state draw cost —
//! and [`FrameLog::worst_bake_ms`] is the worst frame that did. Two numbers that cannot be
//! confused with each other, at the cost of two integers a frame in the render layer.

use bevy::prelude::*;
use treepo_render::{BakeLoad, IdPlane, ResidentChunk};

/// `AC-NAV-2`'s budget: 30 fps is one frame every 33.3 ms.
const BUDGET_MS: f32 = 1000.0 / 30.0;

/// What the last measurement window saw.
///
/// Read over BRP with `world_get_resources`; clear it by mutating [`FrameLog::reset`] to `true`,
/// which is what makes a measurement a measurement *of the zoom* rather than of the zoom plus
/// however long the window sat still beforehand. Startup frames are always slow — shader
/// compilation, the first bake, the window itself — and none of them are what the criterion is
/// about.
#[derive(Resource, Reflect, Debug, Clone, Default)]
#[reflect(Resource)]
pub(crate) struct FrameLog {
    /// Frames observed since the last reset.
    pub(crate) frames: u32,
    /// Frames slower than [`BUDGET_MS`]. `AC-NAV-2` passes when this is zero.
    pub(crate) over_budget: u32,
    /// The slowest frame, in milliseconds.
    pub(crate) worst_ms: f32,
    /// The slowest frame that drew no texels — what the picture costs to *hold*.
    pub(crate) worst_idle_ms: f32,
    /// The slowest frame that drew any — what the picture costs to *build*.
    pub(crate) worst_bake_ms: f32,
    /// The most texels any one frame drew.
    pub(crate) peak_bake_texels: u64,
    /// Every texel drawn since the reset, which with [`FrameLog::baking_frames`] gives the
    /// average a frame's bake was charged.
    pub(crate) total_bake_texels: u64,
    /// The most pieces the view wanted and did not have — how far behind the bake fell.
    pub(crate) peak_missing: u32,
    /// The most pieces released in one frame.
    ///
    /// Separate from the bake because it is the other unbounded thing a frame can do: a band
    /// that completes drops every held layer at once, and at a middle band that is a thousand
    /// entities and a thousand GPU textures in one frame.
    pub(crate) peak_released: u32,
    /// The slowest frame that released anything.
    pub(crate) worst_release_ms: f32,
    /// The mean frame, in milliseconds.
    pub(crate) mean_ms: f32,
    /// Every frame added together, which is what [`FrameLog::mean_ms`] is divided from. Kept
    /// rather than derived so the mean cannot drift with an incremental update.
    pub(crate) total_ms: f32,
    /// Frames on which the renderer drew any texels.
    pub(crate) baking_frames: u32,
    /// Resident pieces right now — D5's claim is that frame cost scales with this, not with
    /// the repository's path count.
    pub(crate) pieces: u32,
    /// The most pieces resident at once since the reset.
    pub(crate) peak_pieces: u32,
    /// Resident texels right now, summed over every piece's ID plane.
    pub(crate) texels: u64,
    /// The most texels resident at once since the reset — the number `RESIDENT_TEXEL_BUDGET`
    /// is a budget for, and the one `NFR-3` cares about.
    pub(crate) peak_texels: u64,
    /// The LOD band of the pieces on screen, or `i8::MIN` when nothing is resident.
    pub(crate) band: i8,
    /// Set to `true` over BRP to clear the window. Cleared by the sampler.
    pub(crate) reset: bool,
}

/// Registers the sampler. Called from [`super::brp::register`].
pub(crate) fn register(app: &mut App) {
    app.register_type::<FrameLog>()
        .init_resource::<FrameLog>()
        .add_systems(Last, sample);
}

/// Folds one frame into the log.
///
/// `Time<Real>` rather than `Time`: the virtual clock can be paused and scaled, and a frame
/// budget measured against a scalable clock is not a frame budget.
fn sample(
    time: Res<Time<Real>>,
    load: Res<BakeLoad>,
    resident: Query<(&ResidentChunk, &IdPlane)>,
    mut log: ResMut<FrameLog>,
) {
    if log.reset {
        *log = FrameLog::default();
    }

    let mut pieces = 0_u32;
    let mut texels = 0_u64;
    let mut band = i8::MIN;
    for (chunk, plane) in &resident {
        let size = plane.size();
        pieces += 1;
        texels += u64::from(size.x) * u64::from(size.y);
        band = band.max(chunk.band.index());
    }

    let frame_ms = time.delta_secs() * 1000.0;

    log.frames += 1;
    log.total_ms += frame_ms;
    log.mean_ms = log.total_ms / log.frames as f32;
    log.worst_ms = log.worst_ms.max(frame_ms);
    if load.texels == 0 {
        log.worst_idle_ms = log.worst_idle_ms.max(frame_ms);
    } else {
        log.baking_frames += 1;
        log.worst_bake_ms = log.worst_bake_ms.max(frame_ms);
        log.total_bake_texels += load.texels;
        log.peak_bake_texels = log.peak_bake_texels.max(load.texels);
    }
    log.peak_missing = log.peak_missing.max(load.missing);
    log.peak_released = log.peak_released.max(load.released);
    if load.released > 0 {
        log.worst_release_ms = log.worst_release_ms.max(frame_ms);
    }
    if frame_ms > BUDGET_MS {
        log.over_budget += 1;
    }

    log.pieces = pieces;
    log.peak_pieces = log.peak_pieces.max(pieces);
    log.texels = texels;
    log.peak_texels = log.peak_texels.max(texels);
    log.band = band;
}
