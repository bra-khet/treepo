//! ★ What a chunk *is*, and what keeps one in memory — architecture D5.
//!
//! > Grow rasterizes the static tree into chunked layer textures per LOD band […] Thrive
//! > renders visible chunks as a small number of quads. […] Thrive's frame cost scales with
//! > *visible chunks*, not with the 80k paths of a T3 repository.
//!
//! D5 says the tree is chunked. It does not say what a chunk is a chunk *of*, and that choice
//! decides more than it looks like it does.
//!
//! # Chunk identity is subtree-anchored, not screen-space
//!
//! **Adopted 2026-07-30, Phase 5; recorded in architecture D5 as D5.1.** A chunk is a
//! connected piece of the skeleton hierarchy identified by an **anchor node** — the root of the
//! subtree it covers, minus whatever descendant subtrees were already cut into chunks of their
//! own. The alternative was uniform screen-space tiles, which are simpler to bake and simpler
//! to stream. Three things decided it:
//!
//! * **Dirtying.** `AC-GROW-4` wants a one-file change to confine itself to the affected limb.
//!   With an anchor, "this limb changed" is a chunk-level fact — invalidate every key whose
//!   anchor is on the changed node's ancestor chain. With screen tiles it is a spatial query
//!   that gets the answer right and cannot explain it.
//! * **The Thrive UX.** Focus a major branch and that limb should stay sharp and forward while
//!   overlapping siblings recede — a 2.5-D layer feel. Layer membership is then a property a
//!   chunk *has*. Screen tiles cut across siblings, so a tile would have to be in two layers.
//! * **`F-INSP-*`.** A chunk already names a node, so "what is this region of the picture"
//!   has an answer before the ID buffer exists.
//!
//! The cost is admitted rather than designed away: **chunk sizes are uneven.** A greedy cut on
//! subtree weight bounds them from below and only loosely from above, and a chunk's *world*
//! extent is not bounded by its segment count at all — a three-segment trunk spans the whole
//! tree. That second one is why [`ChunkId`] has a `piece`.
//!
//! # `piece` is the secondary index, and it is per-band on purpose
//!
//! Texture size is `world extent × texel density`, and those two are independent of how many
//! segments a chunk holds. So a chunk whose texture would exceed [`MAX_PIECE_SIDE`] is split
//! into a uniform grid of pieces over its own extent — **that limb only**, leaving every other
//! chunk alone. The grid is recomputed per LOD band, because the density is what made it
//! necessary; the anchor never is. A residency key is therefore `(anchor, piece, band)` and an
//! identity is `anchor`, which is the distinction the whole module is built around.
//!
//! # Residency is visible-first and budget-capped
//!
//! RISK-B: baked layers plus ID buffers for an 80k-path tree can exceed `NFR-3`'s 4 GB if held
//! resident. [`stream`] bakes only pieces that intersect the viewport plus a margin, sorts what
//! it wants by distance from the camera, and stops at [`RESIDENT_TEXEL_BUDGET`]. Everything
//! else is despawned, which drops the last strong handle to its image and frees the texture.

use alloc_arc::Arc;
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use treepo_det::Fx;
use treepo_model::{NodeId, Point, Skeleton, WorldSnapshot};

use crate::bake;
use crate::id_buffer::IdPlane;
use crate::lod::Band;

/// `alloc::sync::Arc`, spelled once so the import reads as the handoff type it is (D4).
mod alloc_arc {
    pub(super) use std::sync::Arc;
}

/// The rectangle something occupies, in world units.
///
/// Used for three things that all need the same answer: framing the camera on load (`F-NAV-1`),
/// bounding a chunk's baked texture, and deciding what is visible. It includes limb *widths*
/// rather than just centre lines, so framing does not clip the trunk of a repository whose
/// basal mass is its widest feature.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Extent {
    /// Lower-left corner.
    pub min: Vec2,
    /// Upper-right corner.
    pub max: Vec2,
}

impl Extent {
    /// The extent of every drawn segment, or `None` for a skeleton that draws nothing.
    ///
    /// `None` is a real case rather than a guard: `AC-SKEL-2` says an empty repository is a
    /// seed and a root cluster, and a skeleton can carry nodes before it carries geometry.
    #[must_use]
    pub fn of(skeleton: &Skeleton) -> Option<Self> {
        Self::of_segments(
            skeleton,
            (0..skeleton.segments().len() as u32).collect::<Vec<_>>(),
        )
    }

    /// The extent of a chosen subset of a skeleton's segments, by index.
    fn of_segments(skeleton: &Skeleton, indices: impl IntoIterator<Item = u32>) -> Option<Self> {
        let segments = skeleton.segments();
        let mut extent: Option<Self> = None;
        for index in indices {
            let Some(segment) = segments.get(index as usize) else {
                continue;
            };
            for (point, pad) in [
                (segment.start, half(segment.base_width)),
                (segment.end, half(segment.tip_width)),
            ] {
                let at = world(point);
                extent = Some(match extent {
                    None => Self {
                        min: at - Vec2::splat(pad),
                        max: at + Vec2::splat(pad),
                    },
                    Some(current) => Self {
                        min: current.min.min(at - Vec2::splat(pad)),
                        max: current.max.max(at + Vec2::splat(pad)),
                    },
                });
            }
        }
        extent
    }

    /// The middle of the rectangle.
    #[must_use]
    pub fn center(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }

    /// Width and height. Never negative; zero on a degenerate axis.
    #[must_use]
    pub fn size(&self) -> Vec2 {
        (self.max - self.min).max(Vec2::ZERO)
    }

    /// Whether two rectangles share any area — the visibility test.
    ///
    /// Touching edges count as overlapping. A chunk exactly on the viewport boundary is one
    /// the user is about to see, and the margin in [`stream`] is what actually decides the
    /// question; being strict here would only produce a pop at the seam.
    #[must_use]
    pub fn intersects(&self, other: &Self) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
    }

    /// The rectangle grown by `margin` world units on every side.
    #[must_use]
    pub fn expanded(&self, margin: Vec2) -> Self {
        Self {
            min: self.min - margin,
            max: self.max + margin,
        }
    }
}

/// Which baked piece of the tree this is.
///
/// `anchor` is the identity and `piece` is a subdivision of it. Two keys with the same anchor
/// are the same limb at different granularity, which is what makes "invalidate this limb" a
/// prefix operation rather than a search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkId {
    /// The node whose subtree this chunk covers.
    pub anchor: NodeId,
    /// Which spatial sub-piece of that subtree, in row-major order over its extent.
    ///
    /// `0` for a chunk small enough to bake as one texture, which is the common case.
    pub piece: u32,
}

/// One connected piece of the skeleton, and the segments it draws.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// The node this chunk hangs from.
    pub anchor: NodeId,
    /// The world rectangle its segments occupy, including limb widths.
    pub extent: Extent,
    /// Indices into [`Skeleton::segments`].
    pub segments: Vec<u32>,
}

/// The whole tree, cut into chunks.
#[derive(Debug, Default, Clone)]
pub struct ChunkSet {
    chunks: Vec<Chunk>,
    extent: Option<Extent>,
}

impl ChunkSet {
    /// Cuts a skeleton into chunks by the greedy subtree rule described in the module header.
    ///
    /// Walks nodes children-first — [`Skeleton::nodes`] is documented as parents before
    /// children, so reverse index order is a valid topological order — accumulating each
    /// subtree's segments into its parent until the accumulation reaches [`target_weight`].
    /// The node it reaches at becomes an anchor and its accumulation becomes a chunk. Whatever
    /// is still accumulating when the walk reaches the basal node becomes the last chunk, which
    /// is why the trunk is one chunk however thin it is.
    #[must_use]
    pub fn build(skeleton: &Skeleton) -> Self {
        let nodes = skeleton.nodes();
        let target = target_weight(skeleton.segments().len());

        // Segments carry a node id rather than the reverse, so the first pass inverts that.
        // A segment naming a node the skeleton does not have is dropped rather than panicked
        // on: this is the render layer, and a malformed snapshot is `WorldSnapshot::is_covered`
        // to answer, not a reason for a window to close.
        let mut pending: Vec<Vec<u32>> = vec![Vec::new(); nodes.len()];
        for (index, segment) in skeleton.segments().iter().enumerate() {
            if let Some(bucket) = pending.get_mut(segment.node.index()) {
                bucket.push(index as u32);
            }
        }

        let mut chunks = Vec::new();
        for node in nodes.iter().rev() {
            let carried = core::mem::take(&mut pending[node.id.index()]);

            let Some(parent) = node.parent else {
                // The basal node: nothing left to carry into, so this is the last chunk.
                push_chunk(&mut chunks, skeleton, node.id, carried);
                continue;
            };
            if carried.len() >= target {
                push_chunk(&mut chunks, skeleton, node.id, carried);
                continue;
            }

            // `nodes()` promises parents before children. If that ever stopped holding, a
            // parent already visited would silently lose its children's segments — so say so
            // in a debug build, and in a release one make the node its own chunk instead,
            // which draws everything at the cost of an extra chunk.
            debug_assert!(
                parent.index() < node.id.index(),
                "skeleton nodes are documented parents-before-children"
            );
            match pending.get_mut(parent.index()) {
                Some(bucket) if parent.index() < node.id.index() => bucket.extend(carried),
                _ => push_chunk(&mut chunks, skeleton, node.id, carried),
            }
        }

        // Reverse so chunks come out root-first, which makes the z-order in [`stream`] run
        // trunk-behind-branch rather than the other way round.
        chunks.reverse();
        let extent = chunks
            .iter()
            .map(|chunk| chunk.extent)
            .reduce(|a, b| Extent {
                min: a.min.min(b.min),
                max: a.max.max(b.max),
            });
        Self { chunks, extent }
    }

    /// Every chunk, root-first.
    #[must_use]
    pub fn chunks(&self) -> &[Chunk] {
        &self.chunks
    }

    /// How many chunks the tree was cut into.
    #[must_use]
    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    /// Whether the tree draws nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// The union of every chunk's extent — what the camera frames on load.
    #[must_use]
    pub fn extent(&self) -> Option<Extent> {
        self.extent
    }
}

/// How many chunks a tree is aimed at, whatever its size.
///
/// A *count* rather than a fixed weight, because the two failure modes are at opposite ends.
/// A fixed weight that suits a T3 monorepo cuts treepo's own repository into one chunk, and
/// nothing about residency can be observed on a tree that is one chunk. A fixed weight that
/// suits a small repository shatters a T3 tree into thousands, and D5's whole claim is "a small
/// number of quads". Sixty-four keeps both ends in the same order of magnitude, and it is also
/// roughly the number of top-level limbs a reader can hold as *places* — which is the unit the
/// 2.5-D focus behaviour will operate on.
///
/// The second failure mode above turns out not to exist, and the measurement is worth keeping
/// because the reasoning was wrong in an instructive direction. The T3 pin's 52,527 paths grow
/// a skeleton of **521 nodes and 1,042 segments** — `P6` aggregation and `F-SKEL-7` containers
/// collapse the tree by two orders of magnitude before it is ever chunked. So at T3 this
/// constant does not bind at all: `target_weight` falls to [`MIN_CHUNK_SEGMENTS`] and the cut
/// yields ~43 chunks. Nothing here needs changing, but "a T3 tree has T3-many segments" is an
/// assumption that should not be made again elsewhere.
pub const TARGET_CHUNKS: usize = 64;

/// The fewest segments worth cutting a chunk at.
///
/// Below this the cut stops subdividing, so a small tree stays a handful of chunks rather than
/// one per twig. Twenty-four is about one directory's worth of limbs.
pub const MIN_CHUNK_SEGMENTS: usize = 24;

/// The segment count at which the greedy cut emits a chunk.
#[must_use]
pub fn target_weight(total_segments: usize) -> usize {
    (total_segments / TARGET_CHUNKS).max(MIN_CHUNK_SEGMENTS)
}

/// Appends a chunk, unless it draws nothing.
fn push_chunk(chunks: &mut Vec<Chunk>, skeleton: &Skeleton, anchor: NodeId, segments: Vec<u32>) {
    // A node with no drawn geometry is not a chunk. Aggregates and root-mass nodes can both
    // hold segments and both go through here; what is filtered is emptiness, not a role.
    let Some(extent) = Extent::of_segments(skeleton, segments.iter().copied()) else {
        return;
    };
    chunks.push(Chunk {
        anchor,
        extent,
        segments,
    });
}

/// The largest texture side one piece may be baked at.
///
/// 256 texels — a piece is at most 256 KiB of RGBA8 plus 256 KiB of element-ID plane. Large
/// enough that the common chunk is still one piece and the grid never appears; small enough
/// that no single bake can spend a frame.
///
/// # This is the constant that bounds the worst frame, not the bake budget
///
/// [`BAKE_TEXELS_PER_FRAME`] bounds what a frame *plans* to draw, but a piece is atomic — half
/// a baked piece is a hole, so the loop always finishes the one it started and the real worst
/// frame is the budget **plus one whole piece**. This constant is what bounds that second term.
///
/// It was 512 until the materials pass, and the measurement is why it moved. At 512 a fully
/// covered piece writes 262 Ki texels before overlap and was observed writing **436 Ki** with
/// it — limbs cross, and [`Layer::texels_drawn`](crate::Layer::texels_drawn) charges each
/// crossing because each is shaded. At the measured 71 ns a texel that is 31 ms in one
/// indivisible unit: the whole frame, spent on one piece, with the budget powerless because it
/// is only consulted between pieces. Quartering the area quarters that to about 8 ms.
///
/// The cost is four times as many pieces, which is four times as many sprites and four times as
/// many per-piece setups. Neither is the expensive axis: memory is unchanged because
/// [`RESIDENT_TEXEL_BUDGET`] is counted in texels rather than pieces, and the per-piece setup is
/// one pass over a chunk's segment list, which is two dozen segments at T3.
pub const MAX_PIECE_SIDE: u32 = 256;

/// How many texels may be resident at once, across every chunk and band.
///
/// 64 Mi texels is 256 MiB of RGBA8 colour plus 256 MiB of element-ID plane — 512 MiB, an
/// eighth of `NFR-3`'s 4 GB, leaving the manifest, the skeleton, and Bevy's own allocations the
/// rest. RISK-B's mitigation names this as tunable via `F-SET-4`; until settings exist it is a
/// constant.
///
/// **This bounds residency, not selection.** The distinction was not free: for as long as
/// [`stream`] charged only the newly selected pieces, a fast zoom out across eleven bands held
/// every superseded layer on top of a full budget and reached 3.5x it on the T3 pin. Both
/// halves — the selection in [`within_budget`] and the hold-over in [`stream`] — draw on this
/// same number now, so the ceiling is the budget plus one frame of bakes.
///
/// The measurement that set it (T3 pin `rust` 1.83.0, release, 2026-07-30): steady residency
/// peaks at **32 Mi texels** at the sharpest band and 67 Mi in the middle bands, where the
/// budget binds. It is therefore *tight but not wrong* — halving it would put holes in the
/// mid-band picture, and doubling it would buy nothing the camera can see.
pub const RESIDENT_TEXEL_BUDGET: u64 = 64 * 1024 * 1024;

/// How far beyond the viewport a chunk is kept resident, as a fraction of the viewport.
///
/// Half a screen in every direction, so a pan of up to half a window reveals already-baked
/// texture. Zero would make every pan pop; a large margin would spend the budget on chunks the
/// user is moving away from.
pub const RESIDENCY_MARGIN: f32 = 0.5;

/// How much drawing one frame may do, in texels written.
///
/// Baking is CPU rasterization on the main thread, so this is the direct lever on the hitch a
/// zoom costs. Moving the bake onto the async pool is what removes the trade rather than tuning
/// it, and it belongs with `grow_task` in Phase 7, where a producer already publishes while
/// Thrive is reading.
///
/// # It counts texels because pieces are not comparable
///
/// This was `BAKES_PER_FRAME = 4` — a *piece* count — until the materials pass made the
/// per-texel cost large enough for the difference to matter. Two pieces of identical size can
/// differ by fifty times in how much of themselves they draw: a piece at the sharpest band is
/// half-covered by one thick limb, and a piece at a far band holds a few hairlines crossing
/// mostly-empty space. A budget in pieces therefore charges the same for both, which means it
/// is set for the expensive one and starves the cheap one — the far bands, where a fresh
/// viewport needs the most pieces, refill slowest.
///
/// Charged per texel the whole trade inverts the right way. A frame bakes as many cheap pieces
/// as fit and one expensive one, and a far band now fills *faster* than four-per-frame did
/// while a near band costs less.
///
/// # Where the number comes from
///
/// Measured on this machine (release, 2026-07-30): [`bake::rasterize`] costs **13.7 ns per
/// texel with the shading bypassed and 71 ns with it**, so the materials pass is a little over
/// five times the drawing cost it replaced. 128 Ki texels is about 9 ms of that, which leaves
/// most of `AC-NAV-2`'s 33.3 ms for the frame the bake is sharing.
///
/// The overshoot is bounded by one piece, because a piece's cost is not known until it is drawn
/// and stopping half way through one would leave a hole rather than a blur. [`MAX_PIECE_SIDE`]
/// is what bounds that term, and it was lowered in the same measurement that set this one — see
/// there for why the two constants have to be read together.
///
/// A frame always bakes at least one piece. Without that a piece larger than the budget would
/// never be drawn at all, and the budget would express itself as a permanent hole instead of a
/// slower fill.
pub const BAKE_TEXELS_PER_FRAME: u64 = 96 * 1024;

/// What the last frame's bake actually did.
///
/// Two integers written once per frame by [`stream`]. It exists because the render layer is the
/// only thing that knows the difference between a frame that baked and a frame that merely drew,
/// and because "how many texels did that cost" is the question every performance decision about
/// the bake turns on — [`BAKE_TEXELS_PER_FRAME`] was set from it.
///
/// Maintained in every build rather than behind a feature. It is two stores a frame, `F-THR-8`'s
/// debug overlay will want it, and a measurement surface that only exists in measurement builds
/// is one that can quietly stop matching the build that ships.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BakeLoad {
    /// Texels written by the bake, as [`Layer::texels_drawn`](crate::Layer::texels_drawn)
    /// counts them.
    pub texels: u64,
    /// How many pieces were baked.
    pub pieces: u32,
    /// How many pieces the view wanted that were not resident — what is still owed.
    pub missing: u32,
    /// How many resident pieces were released.
    pub released: u32,
}

/// The committed tree, cut into chunks and ready to bake from.
///
/// Set by `treepo-app`'s `snapshot_sync` when a snapshot is committed (architecture D4), read
/// by [`stream`]. The renderer does not decide what is committed and the app does not decide
/// what is resident; this resource is the whole of the interface between those two answers.
#[derive(Resource, Debug, Default)]
pub struct TreePlan {
    /// The snapshot the chunks were cut from, kept so the bake can read materials.
    snapshot: Option<Arc<WorldSnapshot>>,
    /// The cut.
    chunks: ChunkSet,
    /// Bumped on every replacement, so chunks baked from an older snapshot can be found.
    generation: u64,
}

impl TreePlan {
    /// Replaces the plan with one cut from a newly committed snapshot.
    pub fn commit(&mut self, snapshot: Arc<WorldSnapshot>) {
        self.chunks = ChunkSet::build(&snapshot.skeleton);
        self.snapshot = Some(snapshot);
        self.generation = self.generation.wrapping_add(1);
    }

    /// The cut, or an empty one before anything is committed.
    #[must_use]
    pub fn chunks(&self) -> &ChunkSet {
        &self.chunks
    }

    /// Which committed world the resident chunks belong to.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

/// A baked piece on screen.
#[derive(Component, Debug, Clone, Copy)]
pub struct ResidentChunk {
    /// Which piece of which limb.
    pub id: ChunkId,
    /// The band it was baked at.
    pub band: Band,
    /// The world rectangle it covers, so [`stream`] can ask whether it is still in view without
    /// recomputing the grid that produced it.
    pub region: Extent,
    /// The plan generation it was baked from.
    pub generation: u64,
}

/// What [`stream`] decided it wants on screen, before it knows what is already there.
#[derive(Debug, Clone, Copy)]
struct Wanted {
    id: ChunkId,
    /// Index into [`ChunkSet::chunks`], so the bake can find the segments again.
    chunk: usize,
    /// The sub-rectangle of the chunk this piece covers.
    region: Extent,
    /// The texture size to bake it at.
    size: UVec2,
    /// Squared distance from the camera, for the budget's nearest-first ordering.
    distance: f32,
}

/// Keeps the resident set matching what the camera can see, within the texel budget.
///
/// The one system that makes `NFR-2` a structural property rather than a hope: what it spawns
/// is bounded by the viewport and by [`RESIDENT_TEXEL_BUDGET`], and neither term mentions how
/// many paths the repository has.
// A Bevy system's parameters are dependency injection rather than an argument list: nobody
// assembles them at a call site, so the readability this lint protects is not at stake, and the
// alternative — bundling resources into a `SystemParam` struct — hides what the system touches
// in order to satisfy a count. Same category of false positive as the crate-level
// `elided_lifetimes_in_paths` allow, and allowed in the same narrow way.
#[allow(clippy::too_many_arguments)]
pub fn stream(
    mut commands: Commands,
    plan: Res<TreePlan>,
    mut images: ResMut<Assets<Image>>,
    mut load: ResMut<BakeLoad>,
    palette: Res<crate::bake::AuthorPalette>,
    window: Option<Single<&Window>>,
    camera: Option<Single<(&Transform, &Projection, &crate::camera::TreeCamera)>>,
    resident: Query<(Entity, &ResidentChunk, &IdPlane)>,
) {
    *load = BakeLoad::default();
    let (Some(window), Some(camera), Some(snapshot)) = (window, camera, plan.snapshot.as_ref())
    else {
        return;
    };
    let (transform, projection, tree_camera) = camera.into_inner();
    let Projection::Orthographic(ortho) = projection else {
        return;
    };

    let viewport = Vec2::new(window.width(), window.height());
    let visible = viewport * ortho.scale;
    let center = transform.translation.truncate();
    let resident_rect = Extent {
        min: center - visible * 0.5,
        max: center + visible * 0.5,
    }
    .expanded(visible * RESIDENCY_MARGIN);

    // The band the camera is at. The framed scale is the reference, so the bands of a T0
    // repository and a T3 monorepo mean the same thing despite their world units differing by
    // orders of magnitude — the same reasoning that makes `TreeCamera`'s zoom limits relative.
    // Before the tree has been framed the camera's own scale stands in, which lands on band
    // zero and bakes at one texel per pixel: the right answer for a camera with no reference.
    let reference = tree_camera.fit_scale.unwrap_or(ortho.scale);
    let band = Band::for_scale(ortho.scale, reference);
    let texels_per_unit = band.texels_per_unit(reference);

    let wanted = within_budget(select(
        &snapshot.skeleton,
        plan.chunks.chunks(),
        &resident_rect,
        center,
        texels_per_unit,
    ));

    // Two sorted key lists rather than a map: `clippy.toml` denies `std::collections::HashMap`
    // workspace-wide (`N3`), and while this crate is downstream of the determinism boundary,
    // a stable iteration order here is worth having for its own sake — it makes what is on
    // screen a function of what is visible, not of a hash seed.
    let mut keys: Vec<(ChunkId, Band)> = wanted.iter().map(|piece| (piece.id, band)).collect();
    keys.sort_unstable();

    // Three fates, not two. Despawning drops the last strong handle to a piece's image and Bevy
    // frees an asset with no strong handles left — that drop *is* the eviction, and RISK-B is
    // answered by releasing what is not drawn rather than by choosing what is. What the third
    // fate buys is stated at the `keep` despawn below, and what it costs is bounded by the
    // budget charge between the two.
    let mut present: Vec<(ChunkId, Band)> = Vec::new();
    let mut superseded: Vec<(Entity, i32, u64)> = Vec::new();
    let mut held: u64 = 0;
    for (entity, chunk, plane) in &resident {
        let cost = u64::from(plane.size().x) * u64::from(plane.size().y);
        if chunk.generation != plan.generation() || !chunk.region.intersects(&resident_rect) {
            commands.entity(entity).despawn();
        } else if chunk.band == band && keys.binary_search(&(chunk.id, band)).is_ok() {
            present.push((chunk.id, band));
            held += cost;
        } else {
            let distance = (i32::from(chunk.band.index()) - i32::from(band.index())).abs();
            superseded.push((entity, distance, cost));
        }
    }
    present.sort_unstable();

    // BUG FIX: the texel budget bounded the selection, not the residency
    // Fix: `within_budget` caps `wanted`, but superseded pieces are held *in addition* to it and
    // nothing counted them. A zoom that crosses several bands in a second supersedes a set at
    // each one and holds them all, so residency was a multiple of the budget rather than the
    // budget — measured at 3.5x on the T3 pin (238 M texels against a 67 M budget, 1.9 GB of
    // texel data). Held-over pieces are now charged to the same budget, nearest band first.
    //
    // The eviction order falls out rather than being a second rule. Early in a band change
    // `present` is nearly empty, so the budget has room for the whole previous band and the
    // screen stays filled; as bakes land, `present` grows and the softest layers are dropped
    // to pay for them. Residency is bounded by the budget plus one frame of bakes, which is at
    // most one frame's bake budget plus the piece that overshot it.
    let affordable = affordable(&mut superseded, held);
    let mut keep: Vec<Entity> = Vec::with_capacity(affordable);
    for (index, (entity, _, _)) in superseded.into_iter().enumerate() {
        if index < affordable {
            keep.push(entity);
        } else {
            commands.entity(entity).despawn();
            load.released += 1;
        }
    }

    let (mut missing, mut baked) = (0, 0);
    let mut drawn: u64 = 0;
    for piece in &wanted {
        if present.binary_search(&(piece.id, band)).is_ok() {
            continue;
        }
        missing += 1;
        // `baked > 0` rather than an unconditional check, so the first piece of a frame is
        // always drawn — see `BAKE_TEXELS_PER_FRAME` for why a budget that can refuse every
        // piece is a hole rather than a slower fill.
        if baked > 0 && drawn >= BAKE_TEXELS_PER_FRAME {
            continue;
        }

        let chunk = &plan.chunks.chunks()[piece.chunk];
        let layer = bake::rasterize(
            &snapshot.skeleton,
            &snapshot.materials,
            &palette.0,
            &chunk.segments,
            piece.region,
            piece.size,
        );
        // The two planes part company here and never meet again: the colour is uploaded and
        // dropped from main memory, the ids ride on the entity so a click can read them. They
        // are despawned together, which is what keeps the plane from outliving its picture.
        let layer_texels = layer.texels_drawn;
        let plane = IdPlane::new(layer.size, layer.ids);
        let image = images.add(bake::texture(piece.size, layer.color));
        commands.spawn((
            ResidentChunk {
                id: piece.id,
                band,
                region: piece.region,
                generation: plan.generation(),
            },
            plane,
            Sprite {
                image,
                custom_size: Some(piece.region.size()),
                ..Sprite::default()
            },
            // Root-first chunk order becomes back-to-front depth, so a branch drawn over the
            // trunk stays over it however the sprite queue is built. `RenderAssetUsages` and
            // depth are the two places a correct picture can come out invisible, and both are
            // cheaper to get right than to debug.
            Transform::from_xyz(
                piece.region.center().x,
                piece.region.center().y,
                depth_of(piece.chunk),
            ),
        ));
        baked += 1;
        drawn += layer_texels;
    }
    load.texels = drawn;
    load.pieces = baked;
    load.missing = missing - baked;

    // A layer from the previous band is the wrong *resolution*, not the wrong picture. Held
    // until the new band is complete, a zoom across a boundary costs a moment of softness;
    // dropped on the frame the band changes, it costs a blank window for as many frames as
    // the bake budget needs to refill one — which is the difference between a zoom that feels
    // continuous and one that flickers. `missing == baked` is "nothing is still owed".
    //
    // `keep` rather than every superseded piece: the ones the budget could not afford are
    // already gone, and a piece the budget refused is one the picture is better off without.
    if missing == baked {
        for entity in keep {
            commands.entity(entity).despawn();
            load.released += 1;
        }
    }
}

/// One bakeable piece of a chunk: what to rasterize, where, and at what resolution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Piece {
    /// Which piece of which limb.
    pub id: ChunkId,
    /// The world rectangle it covers.
    pub region: Extent,
    /// The texture size to bake it at.
    pub size: UVec2,
}

/// Every piece of one chunk that intersects `within` **and draws something**.
///
/// The viewport-free half of what [`stream`] does, factored out because
/// `cargo xtask id-coverage` needs exactly this and a gate that enumerated pieces its own way
/// would be scanning something the renderer never bakes. `stream` passes the resident
/// rectangle; the gate passes the chunk's own extent, which is "all of it".
#[must_use]
pub fn pieces(
    skeleton: &Skeleton,
    chunk: &Chunk,
    texels_per_unit: f32,
    within: &Extent,
) -> Vec<Piece> {
    let mut found = Vec::new();
    if !chunk.extent.intersects(within) {
        return found;
    }
    let grid = piece_grid(chunk.extent, texels_per_unit);
    let cell = chunk.extent.size() / grid.as_vec2();

    // Only the cells the rectangle actually reaches, computed rather than iterated: a chunk
    // zoomed into at the near band can be a grid of thousands, and walking all of them to find
    // the four on screen would put the cost back where D5 took it from.
    let (cols, rows) = cell_span(chunk.extent, grid, cell, within);
    let occupied = occupancy(skeleton, chunk, grid, cell, &cols, &rows);

    for row in rows.clone() {
        for col in cols.clone() {
            // A chunk's texture covers its *bounding box*, and a limb is a line through one —
            // so most of a subdivided chunk's grid holds nothing at all. Baking those is a
            // megabyte of transparency each, and it is the accepted cost of D5.1's anchor
            // identity turning into an unbounded one. This is what bounds it: a piece with no
            // segment in it is not a piece.
            let slot = (row - rows.start) as usize * (cols.end - cols.start) as usize
                + (col - cols.start) as usize;
            if !occupied.get(slot).copied().unwrap_or(true) {
                continue;
            }

            let min = chunk.extent.min + Vec2::new(col as f32, row as f32) * cell;
            let region = Extent {
                min,
                max: (min + cell).min(chunk.extent.max),
            };
            found.push(Piece {
                id: ChunkId {
                    anchor: chunk.anchor,
                    piece: row * grid.x + col,
                },
                region,
                size: texture_size(region, texels_per_unit),
            });
        }
    }
    found
}

/// Every piece of every chunk that intersects the resident rectangle, nearest first.
fn select(
    skeleton: &Skeleton,
    chunks: &[Chunk],
    resident: &Extent,
    center: Vec2,
    texels_per_unit: f32,
) -> Vec<Wanted> {
    let mut wanted = Vec::new();
    for (index, chunk) in chunks.iter().enumerate() {
        wanted.extend(
            pieces(skeleton, chunk, texels_per_unit, resident)
                .into_iter()
                .map(|piece| Wanted {
                    id: piece.id,
                    chunk: index,
                    region: piece.region,
                    size: piece.size,
                    distance: piece.region.center().distance_squared(center),
                }),
        );
    }
    wanted.sort_by(|a, b| a.distance.total_cmp(&b.distance));
    wanted
}

/// Orders held-over pieces by how wrong their resolution is, and returns how many of them
/// [`RESIDENT_TEXEL_BUDGET`] can still afford once `already_held` is spent.
///
/// The same shape as [`within_budget`] one line down, and deliberately: both are "sort by how
/// much the viewer wants it, then truncate where the budget runs out". The two together are
/// what make the budget a bound on residency rather than on selection.
///
/// Generic over the handle so the arithmetic can be tested without a `World`. It is the half of
/// the rule that was wrong, and a rule that was wrong once should be a rule something checks.
fn affordable<T>(superseded: &mut [(T, i32, u64)], already_held: u64) -> usize {
    superseded.sort_by_key(|&(_, distance, _)| distance);
    let mut held = already_held;
    let mut kept = 0;
    for &(_, _, cost) in superseded.iter() {
        if held + cost > RESIDENT_TEXEL_BUDGET {
            break;
        }
        held += cost;
        kept += 1;
    }
    kept
}

/// Truncates a nearest-first selection at [`RESIDENT_TEXEL_BUDGET`].
///
/// Dropping the farthest rather than refusing the whole set: over budget, the user still gets
/// a complete picture where they are looking and a hole at the edge of the screen, which is
/// recoverable by zooming out. Refusing would give them a blank window.
fn within_budget(mut wanted: Vec<Wanted>) -> Vec<Wanted> {
    let mut texels: u64 = 0;
    let mut kept = 0;
    for piece in &wanted {
        let cost = u64::from(piece.size.x) * u64::from(piece.size.y);
        if texels + cost > RESIDENT_TEXEL_BUDGET {
            break;
        }
        texels += cost;
        kept += 1;
    }
    wanted.truncate(kept);
    wanted
}

/// How many pieces a chunk is split into at a given density.
///
/// `1×1` for a chunk whose texture fits [`MAX_PIECE_SIDE`], which is the common case and the
/// one the anchor identity is written for. The split is a uniform grid over the chunk's own
/// extent rather than over the world, so it subdivides *that limb only* — the caveat the D5.1
/// decision is explicit about.
#[must_use]
pub fn piece_grid(extent: Extent, texels_per_unit: f32) -> UVec2 {
    let side = extent.size() * texels_per_unit;
    let cells = (side / MAX_PIECE_SIDE as f32).ceil();
    UVec2::new(axis_cells(cells.x), axis_cells(cells.y))
}

/// One axis of [`piece_grid`], with the non-finite cases folded into "one piece".
fn axis_cells(cells: f32) -> u32 {
    if cells.is_finite() && cells >= 1.0 {
        // Saturating rather than wrapping: a density that produces four billion cells is a
        // camera bug, and one enormous piece is a better symptom than a grid of zero.
        cells as u32
    } else {
        1
    }
}

/// The column and row ranges of a chunk's grid that `region` reaches.
///
/// Used twice, for the two things that decide whether a piece is baked: which cells the
/// viewport reaches, and which cells a segment lies in. One function, so the two answers cannot
/// drift — an occupancy map built on a different rounding rule than the loop that reads it is a
/// bug that shows as a limb missing from one piece and present in its neighbour.
fn cell_span(
    extent: Extent,
    grid: UVec2,
    cell: Vec2,
    region: &Extent,
) -> (core::ops::Range<u32>, core::ops::Range<u32>) {
    let index = |value: f32, size: f32, limit: u32| -> u32 {
        let cells = if size > 0.0 {
            (value / size).floor().clamp(0.0, f32::from(u16::MAX)) as u32
        } else {
            0
        };
        cells.min(limit.saturating_sub(1))
    };

    let col_start = index(region.min.x - extent.min.x, cell.x, grid.x);
    let col_end = index(region.max.x - extent.min.x, cell.x, grid.x) + 1;
    let row_start = index(region.min.y - extent.min.y, cell.y, grid.y);
    let row_end = index(region.max.y - extent.min.y, cell.y, grid.y) + 1;
    (
        col_start..col_end.min(grid.x),
        row_start..row_end.min(grid.y),
    )
}

/// Which cells of the visible window hold at least one of the chunk's segments.
///
/// Row-major over `cols × rows`, so the caller indexes it with the same two loop variables it
/// iterates. Walking the *segments* and marking the cells they touch, rather than walking the
/// cells and searching the segments: the first is proportional to what the chunk draws, the
/// second to the grid, and the grid is the term that gets large.
///
/// An empty vector means "do not filter" — the caller reads a missing slot as occupied, so the
/// one-piece chunk skips building a map to be told what it already knows.
fn occupancy(
    skeleton: &Skeleton,
    chunk: &Chunk,
    grid: UVec2,
    cell: Vec2,
    cols: &core::ops::Range<u32>,
    rows: &core::ops::Range<u32>,
) -> Vec<bool> {
    let width = cols.end.saturating_sub(cols.start) as usize;
    let height = rows.end.saturating_sub(rows.start) as usize;
    if grid == UVec2::ONE || width == 0 || height == 0 {
        return Vec::new();
    }

    let mut occupied = vec![false; width * height];
    for index in &chunk.segments {
        let Some(bounds) = Extent::of_segments(skeleton, [*index]) else {
            continue;
        };
        let (segment_cols, segment_rows) = cell_span(chunk.extent, grid, cell, &bounds);
        for row in segment_rows.start.max(rows.start)..segment_rows.end.min(rows.end) {
            for col in segment_cols.start.max(cols.start)..segment_cols.end.min(cols.end) {
                occupied[(row - rows.start) as usize * width + (col - cols.start) as usize] = true;
            }
        }
    }
    occupied
}

/// The texture size a region is baked at, at least one texel and at most [`MAX_PIECE_SIDE`].
#[must_use]
pub fn texture_size(region: Extent, texels_per_unit: f32) -> UVec2 {
    let side = region.size() * texels_per_unit;
    UVec2::new(axis_texels(side.x), axis_texels(side.y))
}

/// One axis of [`texture_size`].
///
/// A degenerate axis still gets one texel rather than none: a vertical limb has no width and a
/// zero-width texture is not an image, but the limb is still a thing that has to be drawn.
fn axis_texels(side: f32) -> u32 {
    if side.is_finite() && side >= 1.0 {
        (side.ceil() as u32).min(MAX_PIECE_SIDE)
    } else {
        1
    }
}

/// The z a chunk is drawn at, from its position in the root-first cut.
///
/// Small steps within Bevy's default 2-D depth range, so a tree with thousands of chunks still
/// fits. This is also where the 2.5-D focus behaviour will attach: bringing a limb forward is a
/// change to this number, and it is a per-chunk number because chunk identity is per-limb.
fn depth_of(index: usize) -> f32 {
    index as f32 * 1e-3
}

/// A skeleton point in Bevy world units.
///
/// The skeleton's own units are whatever the parameter table's lengths are, and the camera
/// frames the extent rather than assuming a scale — so there is no conversion factor here to
/// get wrong, and adding one would be a second place for the picture and the picking to
/// disagree about where a limb is.
pub(crate) fn world(point: Point) -> Vec2 {
    Vec2::new(point.x.to_f64() as f32, point.y.to_f64() as f32)
}

/// Half a fixed-point width, in world units.
pub(crate) fn half(width: Fx) -> f32 {
    (width.to_f64() as f32) * 0.5
}

/// A baked layer image: RGBA8, sRGB-encoded, kept on the GPU only.
///
/// `RENDER_WORLD` rather than `default()`: the CPU copy is what the bake just wrote and will
/// never read again, and at [`RESIDENT_TEXEL_BUDGET`] the difference is 256 MiB of duplicate.
/// The trade `mesh.rs` declined to make now has a measurement behind it.
pub(crate) const LAYER_USAGE: RenderAssetUsages = RenderAssetUsages::RENDER_WORLD;

#[cfg(test)]
mod tests {
    use super::*;
    use treepo_det::{Angle, Seed};
    use treepo_model::{NodeRole, RepoPath, Segment};

    /// A skeleton of `branches` limbs hanging off a trunk, each with `per_branch` segments.
    fn tree(branches: usize, per_branch: usize) -> Skeleton {
        let mut skeleton = Skeleton::new();
        let root = skeleton.push_node(
            None,
            Point::ORIGIN,
            Angle::ZERO,
            Seed::root(b"chunk-test"),
            NodeRole::Limb {
                path: RepoPath::root(),
            },
        );
        skeleton.extend_segments([segment(root, (0, 0), (0, 10))]);

        for branch in 0..branches {
            let node = skeleton.push_node(
                Some(root),
                Point::ORIGIN,
                Angle::ZERO,
                Seed::root(b"chunk-test"),
                NodeRole::Limb {
                    path: RepoPath::root(),
                },
            );
            let x = branch as i32 * 20;
            skeleton.extend_segments(
                (0..per_branch).map(|i| segment(node, (x, 10 + i as i32), (x, 11 + i as i32))),
            );
        }
        skeleton
    }

    fn segment(node: NodeId, start: (i32, i32), end: (i32, i32)) -> Segment {
        Segment {
            start: Point::new(Fx::from_int(start.0), Fx::from_int(start.1)),
            end: Point::new(Fx::from_int(end.0), Fx::from_int(end.1)),
            base_width: Fx::from_int(2),
            tip_width: Fx::from_int(1),
            node,
            generation: 0,
        }
    }

    #[test]
    fn a_skeleton_that_draws_nothing_has_no_extent() {
        assert!(Extent::of(&Skeleton::new()).is_none());
    }

    /// The reason the extent includes widths: a trunk two units wide reaches one unit either
    /// side of its centre line, and framing on centre lines alone would clip it.
    #[test]
    fn the_extent_includes_limb_width() {
        let mut skeleton = Skeleton::new();
        let root = skeleton.push_node(
            None,
            Point::ORIGIN,
            Angle::ZERO,
            Seed::root(b"chunk-test"),
            NodeRole::Limb {
                path: RepoPath::root(),
            },
        );
        let mut wide = segment(root, (0, 0), (0, 10));
        wide.base_width = Fx::from_int(4);
        wide.tip_width = Fx::from_int(2);
        skeleton.extend_segments([wide]);

        let extent = Extent::of(&skeleton).unwrap();
        assert_eq!(extent.min.x, -2.0);
        assert_eq!(extent.max.x, 2.0);
        assert_eq!(extent.min.y, -2.0);
        assert_eq!(extent.max.y, 11.0);
    }

    /// The whole of the subtree-anchored choice, as a property: every chunk names a node, and
    /// no two chunks name the same one.
    #[test]
    fn every_chunk_is_anchored_at_a_distinct_node() {
        let set = ChunkSet::build(&tree(8, 40));
        assert!(set.len() > 1, "an eight-branch tree should cut into pieces");

        let mut anchors: Vec<NodeId> = set.chunks().iter().map(|chunk| chunk.anchor).collect();
        anchors.sort_unstable();
        let distinct = anchors.len();
        anchors.dedup();
        assert_eq!(anchors.len(), distinct, "two chunks share an anchor");
    }

    /// A chunk is a *partition*: every drawn segment lands in exactly one, so nothing is
    /// missing from the picture and nothing is baked twice.
    #[test]
    fn the_cut_partitions_every_drawn_segment() {
        let skeleton = tree(8, 40);
        let set = ChunkSet::build(&skeleton);

        let mut seen: Vec<u32> = set
            .chunks()
            .iter()
            .flat_map(|chunk| chunk.segments.iter().copied())
            .collect();
        seen.sort_unstable();
        let total = skeleton.segments().len() as u32;
        assert_eq!(seen, (0..total).collect::<Vec<_>>());
    }

    /// A small tree stays a handful of chunks rather than one per twig — [`MIN_CHUNK_SEGMENTS`]
    /// is what keeps `TARGET_CHUNKS` from shattering a T0 repository.
    #[test]
    fn a_small_tree_is_not_shattered() {
        let set = ChunkSet::build(&tree(3, 4));
        assert!(set.len() <= 3, "{} chunks for 13 segments", set.len());
    }

    /// The scaling claim: a tree an order of magnitude larger does not produce an order of
    /// magnitude more chunks. D5's "a small number of quads" is this, measured.
    #[test]
    fn chunk_count_grows_far_slower_than_the_tree() {
        let small = ChunkSet::build(&tree(16, 16)).len();
        let large = ChunkSet::build(&tree(160, 160)).len();
        assert!(
            large < small * 4,
            "{small} chunks at 256 segments, {large} at 25600"
        );
    }

    /// The basal node is always a chunk, however little it draws — otherwise the trunk of a
    /// tree whose branches were all cut away would go missing.
    #[test]
    fn the_trunk_is_a_chunk_even_when_it_is_thin() {
        let skeleton = tree(8, 40);
        let set = ChunkSet::build(&skeleton);
        let basal = skeleton.nodes()[0].id;
        assert!(set.chunks().iter().any(|chunk| chunk.anchor == basal));
    }

    #[test]
    fn a_chunk_that_fits_one_texture_is_not_split() {
        let extent = Extent {
            min: Vec2::ZERO,
            max: Vec2::splat(100.0),
        };
        assert_eq!(piece_grid(extent, 1.0), UVec2::ONE);
    }

    /// The caveat made concrete: a limb too large for one texture splits, and the split is
    /// over its own extent — the grid is a property of that chunk, not of the world.
    #[test]
    fn a_limb_too_large_for_one_texture_splits_into_pieces() {
        let extent = Extent {
            min: Vec2::ZERO,
            max: Vec2::new(2000.0, 600.0),
        };
        let grid = piece_grid(extent, 1.0);
        assert_eq!(grid, UVec2::new(8, 3));
    }

    #[test]
    fn a_degenerate_axis_still_gets_a_texel() {
        let line = Extent {
            min: Vec2::ZERO,
            max: Vec2::new(0.0, 50.0),
        };
        assert_eq!(texture_size(line, 1.0), UVec2::new(1, 50));
    }

    #[test]
    fn a_texture_never_exceeds_the_piece_limit() {
        let huge = Extent {
            min: Vec2::ZERO,
            max: Vec2::splat(100_000.0),
        };
        let size = texture_size(huge, 4.0);
        assert_eq!(size, UVec2::splat(MAX_PIECE_SIDE));
    }

    /// RISK-B, as the one assertion that matters: whatever the camera is looking at, the
    /// resident set stops at the budget.
    #[test]
    fn residency_stops_at_the_texel_budget() {
        let piece = |distance: f32| Wanted {
            id: ChunkId {
                anchor: NodeId::new(0),
                piece: 0,
            },
            chunk: 0,
            region: Extent {
                min: Vec2::ZERO,
                max: Vec2::splat(1.0),
            },
            size: UVec2::splat(MAX_PIECE_SIDE),
            distance,
        };
        let per_piece = u64::from(MAX_PIECE_SIDE) * u64::from(MAX_PIECE_SIDE);
        let over = (RESIDENT_TEXEL_BUDGET / per_piece + 8) as usize;

        let kept = within_budget((0..over).map(|i| piece(i as f32)).collect());
        let texels: u64 = kept
            .iter()
            .map(|p| u64::from(p.size.x) * u64::from(p.size.y))
            .sum();
        assert!(kept.len() < over, "the budget did not bind");
        assert!(texels <= RESIDENT_TEXEL_BUDGET);
    }

    /// The other half of RISK-B, and the half that was missing. Selection stopping at the
    /// budget is worth nothing if the layers held over during a band change are free.
    ///
    /// Measured on the T3 pin before this bound existed: a zoom out across eleven bands reached
    /// 238 M resident texels against a 67 M budget, because each band supersedes a set and every
    /// one of them was held. The numbers here are that gesture in miniature.
    #[test]
    fn held_over_layers_are_charged_to_the_budget_too() {
        let per_piece = u64::from(MAX_PIECE_SIDE) * u64::from(MAX_PIECE_SIDE);
        let per_band = (RESIDENT_TEXEL_BUDGET / per_piece) as usize;

        // Eleven bands' worth of superseded pieces, each band a full budget on its own.
        let mut superseded: Vec<(u32, i32, u64)> = (0..11)
            .flat_map(|band| (0..per_band).map(move |piece| (piece as u32, band, per_piece)))
            .collect();

        let kept = affordable(&mut superseded, 0);
        let texels: u64 = superseded[..kept].iter().map(|&(_, _, cost)| cost).sum();

        assert!(texels <= RESIDENT_TEXEL_BUDGET, "{texels} texels held");
        assert!(kept < superseded.len(), "the budget did not bind");
        // Nearest band first: what survives is the least wrong resolution, not an arbitrary set.
        assert!(superseded[..kept].iter().all(|&(_, band, _)| band == 0));
    }

    /// A band change with the new band already paid for holds nothing, because there is nothing
    /// left to hold it with. This is the end of a zoom, where the sharp layer is complete and
    /// the soft one underneath it is only costing memory.
    #[test]
    fn a_full_budget_holds_no_previous_layer() {
        let mut superseded = vec![(0_u32, 1, 1_024_u64)];
        assert_eq!(affordable(&mut superseded, RESIDENT_TEXEL_BUDGET), 0);
    }

    /// ...and the start of the same zoom, where nothing of the new band exists yet, keeps the
    /// whole previous one. Between these two the screen never blanks and never overruns.
    #[test]
    fn an_empty_budget_holds_the_whole_previous_layer() {
        let mut superseded: Vec<(u32, i32, u64)> = (0..8).map(|i| (i, 1, 1_024)).collect();
        assert_eq!(affordable(&mut superseded, 0), 8);
    }

    #[test]
    fn extents_that_share_no_area_do_not_intersect() {
        let left = Extent {
            min: Vec2::ZERO,
            max: Vec2::splat(10.0),
        };
        let right = Extent {
            min: Vec2::splat(20.0),
            max: Vec2::splat(30.0),
        };
        assert!(!left.intersects(&right));
        assert!(left.intersects(&left));
        assert!(left.expanded(Vec2::splat(15.0)).intersects(&right));
    }

    /// A chunk covering a 16×16 grid, whose only geometry is a limb along the bottom edge.
    fn subdivided() -> (Skeleton, Chunk) {
        let mut skeleton = Skeleton::new();
        let root = skeleton.push_node(
            None,
            Point::ORIGIN,
            Angle::ZERO,
            Seed::root(b"chunk-test"),
            NodeRole::Limb {
                path: RepoPath::root(),
            },
        );
        skeleton.extend_segments([segment(root, (0, 100), (8000, 100))]);
        let chunk = Chunk {
            anchor: root,
            extent: Extent {
                min: Vec2::ZERO,
                max: Vec2::splat(8192.0),
            },
            segments: vec![0],
        };
        (skeleton, chunk)
    }

    /// A chunk is only walked where the viewport reaches it. Without this the near band on a
    /// large limb would iterate a grid of thousands to find the handful on screen.
    #[test]
    fn only_the_pieces_the_viewport_reaches_are_wanted() {
        let (skeleton, chunk) = subdivided();
        // A 32×32 grid of `MAX_PIECE_SIDE` pieces, of which a 600-unit viewport reaches a
        // handful.
        let viewport = Extent {
            min: Vec2::splat(100.0),
            max: Vec2::splat(700.0),
        };
        assert_eq!(piece_grid(chunk.extent, 1.0), UVec2::splat(32));

        let (cols, rows) = cell_span(
            chunk.extent,
            UVec2::splat(32),
            chunk.extent.size() / 32.0,
            &viewport,
        );
        assert_eq!((cols.len(), rows.len()), (3, 3));

        let wanted = select(
            &skeleton,
            core::slice::from_ref(&chunk),
            &viewport,
            viewport.center(),
            1.0,
        );
        // Of those nine, only the three the limb runs through are baked — the limb lies along
        // `y = 100`, which is the bottom row of the three.
        assert_eq!(wanted.len(), 3);
        assert!(wanted.iter().all(|piece| piece.region.min.y == 0.0));
    }

    /// The bound on D5.1's accepted cost. A chunk's texture covers its bounding box, a limb is
    /// a line through one, and without this the near band bakes a megabyte of transparency per
    /// empty cell — which is RISK-B arriving through the side door.
    #[test]
    fn a_piece_holding_no_segment_is_not_baked() {
        let (skeleton, chunk) = subdivided();
        let whole = chunk.extent;
        let wanted = select(
            &skeleton,
            core::slice::from_ref(&chunk),
            &whole,
            whole.center(),
            1.0,
        );

        // The limb spans every column of the 32×32 grid and exactly one row.
        assert_eq!(
            wanted.len(),
            32,
            "{} pieces for one row of limb",
            wanted.len()
        );
        assert!(wanted.iter().all(|piece| piece.region.min.y == 0.0));
    }

    /// …and the common chunk, which is one piece, is unaffected: it has segments by
    /// construction, so the map that would say so is never built.
    #[test]
    fn a_single_piece_chunk_skips_the_occupancy_map() {
        let (skeleton, chunk) = subdivided();
        let map = occupancy(
            &skeleton,
            &chunk,
            UVec2::ONE,
            chunk.extent.size(),
            &(0..1),
            &(0..1),
        );
        assert!(map.is_empty());
    }
}
