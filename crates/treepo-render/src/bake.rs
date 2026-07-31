//! ★ The static bake — architecture D5, the half that turns geometry into pixels.
//!
//! > Grow rasterizes the static tree into chunked layer textures per LOD band […] Thrive
//! > renders visible chunks as a small number of quads.
//!
//! [`chunk`](crate::chunk) decides *what* a chunk is and *when* it is resident. This module
//! decides what it looks like: a CPU rasterizer that turns one chunk's segments into one RGBA
//! layer texture at one LOD band's density.
//!
//! # Why the tree is rasterized once instead of drawn every frame
//!
//! The whole of `NFR-2`. Drawing the tree as geometry costs a vertex per limb corner on every
//! frame, so a T3 repository pays for eighty thousand paths sixty times a second whether or not
//! any of them is on screen. Rasterizing it once turns that into a texture fetch per screen
//! pixel — a cost bounded by the *window*, not by the repository. `mesh.rs`, which this module
//! replaces, was the honest version of the other answer, and it says so in its own header.
//!
//! It is also why "Grow bakes" in D5 is a claim about *when*, not about which crate: the bake
//! runs when a snapshot is committed, never per frame, and it lives here because a texture is a
//! Bevy type and `N6` will not let a generative crate name one.
//!
//! # The rasterizer is scanline, and that is a performance decision
//!
//! Filling each triangle by testing every texel of its bounding box is four lines shorter and
//! quadratically worse: a limb running corner to corner of a piece has a bounding box the size
//! of the piece and covers a few percent of it. Walking rows and computing the covered span
//! makes the cost proportional to what is drawn, which is what keeps a piece a bounded unit of
//! work — and a bounded unit of work is what could be handed to a thread pool.
//!
//! # Everything here is a pure function, and that is load-bearing
//!
//! [`rasterize`] takes a skeleton, a material map, a palette and a rectangle, and returns a
//! [`Layer`]. It touches no `World`, no `Assets`, no resource and no clock. That is why
//! [`chunk::stream`](crate::chunk::stream) can run it on `AsyncComputeTaskPool` without the
//! rasterizer changing at all, and it is the same property that lets `cargo xtask id-coverage`
//! call it with no Bevy app in existence. Keep it.
//!
//! # One pass writes both planes, and that is the `N7` argument
//!
//! [`fill`] writes a colour **and** an [`ElementId`] at every texel it visits, from the same
//! loop and the same bounds check. There is deliberately no path that writes one without the
//! other: `N7`'s unaccountable pixel — colour with no id — is not prevented by a rule here, it
//! is prevented by there being nowhere to write a colour from. `cargo xtask id-coverage` scans
//! for it anyway, because "structurally impossible" is a claim, and a claim that costs one scan
//! to check is worth checking.
//!
//! # Anti-aliasing is what is deliberately not here
//!
//! Coverage is binary: a texel is inside a limb or it is not. What stands in for it is
//! [`MIN_HALF_TEXELS`] — a limb thinner than a texel still marks a line of them, so zooming out
//! thins the tree rather than deleting it, which is the failure `AC-NAV-1` would actually
//! notice. Blending partial coverage would also make the ID plane ambiguous in a way it is not
//! now: a half-covered texel is painted by two elements, and a `u32` holds one.
//!
//! # Per node, then per texel — the split this module is organized around
//!
//! [`surface`](crate::surface) decides what a material looks like; this module decides what is
//! true of a *node*, so that the per-texel function does not have to work it out again for
//! every texel of a limb. [`Appearances`] is that pass: it walks the chunk's segments once,
//! measures each node's limb, resolves its [`Material`](treepo_model::Material) into a
//! [`Shading`](crate::surface::Shading), and lays its ownership mosaic and — for a container —
//! its inventory out as run tables indexed by fraction along the limb.
//!
//! The rule that keeps the bake affordable is that anything constant over a limb belongs in
//! that pass. `F-MAT-6`'s [`Sparse`](treepo_model::StressKind::Sparse) is the clearest case: it
//! coarsens the noise's *period*, so it costs nothing per texel at all — where the obvious
//! implementation, thinning coverage, would cost a branch on every one and break `N7` besides.
//!
//! # A limb's coordinates come from its corners
//!
//! [`fill`] interpolates *position on the limb* rather than colour. Each corner of a segment's
//! quad carries how far along the node it is and which side of the centre line it is on, and
//! the same barycentric weights that used to blend two colours now produce a
//! [`LimbPoint`](crate::surface::LimbPoint). Colour is computed at the texel instead of between
//! two of them, which is what lets a surface have any detail finer than a segment.
//!
//! A tapered quad is a bilinear map and two triangles are affine, so a limb that narrows
//! sharply has a hairline discontinuity in its texture along the split diagonal. That is the
//! classic cost of drawing a quad as two triangles, it is invisible at the tapers an L-system
//! produces, and the alternative is a divide per texel to fix something nobody can see.

use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageSampler};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::sync::{Arc, LazyLock};
use treepo_id::Palette;
use treepo_model::{
    AgeGradient, Composition, Material, MaterialFamily, MaterialMap, NodeId, NodeRole, RepoPath,
    Skeleton, StressKind,
};

use crate::chunk::{Extent, LAYER_USAGE, half, world};
use crate::id_buffer::{ElementId, IdPlane};
use crate::surface::{Knot, LimbPoint, MAX_KNOTS, Shading, Surface, author_color, run_at};

/// How many bytes one texel of a baked layer's **colour** costs.
///
/// The ID plane costs another four on the CPU; see [`id_buffer`](crate::id_buffer) for why the
/// two live in different places.
pub const BYTES_PER_TEXEL: usize = 4;

/// One baked piece: a colour plane for the GPU and an ID plane for picking.
///
/// Returned as a pair rather than produced by two calls, because the pair is the invariant.
/// Two functions could be called separately, in one order, or with different arguments — and
/// `N7` is exactly the claim that they never are.
#[derive(Debug, Clone)]
pub struct Layer {
    /// The resolution both planes share.
    pub size: UVec2,
    /// RGBA8, sRGB-encoded, row-major from the top-left.
    pub color: Vec<u8>,
    /// One element id per texel, in the same order.
    pub ids: Vec<ElementId>,
    /// How many texels this bake wrote — what it cost, not what it covers.
    ///
    /// Counts *writes*, so a texel two limbs cross is counted twice. That is deliberate: the
    /// number measures the shading this bake performed rather than the area it ended up
    /// covering, which is what makes it comparable between one piece and another and what
    /// [`BakeLoad::texels`](crate::BakeLoad::texels) reports. [`coverage`](crate::coverage) is
    /// the function that counts distinct texels, and it answers a different question.
    pub texels_drawn: u64,
}

impl Layer {
    /// An empty layer of the given size — transparent everywhere, identified nowhere.
    #[must_use]
    fn blank(size: UVec2) -> Self {
        let texels = size.x as usize * size.y as usize;
        Self {
            size,
            color: vec![0u8; texels * BYTES_PER_TEXEL],
            ids: vec![ElementId::NONE; texels],
            texels_drawn: 0,
        }
    }

    /// The ID plane, ready to ride on the entity that draws the colour.
    #[must_use]
    pub fn id_plane(&self) -> IdPlane {
        IdPlane::new(self.size, self.ids.clone())
    }
}

/// The thinnest a limb may be rasterized, in texels.
///
/// A little over half a texel either side of the centre line, so a limb narrower than one texel
/// still marks a continuous line of them. Without it a tree zoomed out to `F-NAV-3`'s far band
/// loses every twig — not gracefully, but by dropping whole limbs between two texel centres,
/// which reads as a repository with missing directories rather than as a distant tree.
///
/// Slightly over, rather than exactly, one half. A closed band of width exactly one always
/// contains a texel centre, but a limb whose centre line lands on a texel *boundary* contains
/// two of them only as an equality — and an equality between floats is not a thing to build
/// legibility on. The margin costs a tenth of a texel and removes the alignment case entirely.
pub const MIN_HALF_TEXELS: f32 = 0.6;

/// Rasterizes a chunk's segments into one RGBA8 layer, sRGB-encoded.
///
/// `region` is the world rectangle the texture covers and `size` is its resolution; the two
/// together are the LOD band. Segments are indices into [`Skeleton::segments`], and one outside
/// that range is skipped — this is the render layer, and a malformed snapshot is
/// [`WorldSnapshot::is_covered`](treepo_model::WorldSnapshot::is_covered) to answer.
///
/// The result is transparent where nothing is drawn, which is what lets chunks overlap: a
/// chunk's texture covers its whole bounding box, and limbs from neighbouring chunks pass
/// through that box constantly.
#[must_use]
pub fn rasterize(
    skeleton: &Skeleton,
    materials: &MaterialMap,
    palette: &Palette,
    segments: &[u32],
    region: Extent,
    size: UVec2,
) -> Layer {
    let mut layer = Layer::blank(size);
    let Some(to_texel) = Projector::new(region, size) else {
        return layer;
    };

    // The one pass that turns Phase 4's materials into something a texel loop can read. Note
    // that it takes the projection: how many texels a band puts across a limb decides how many
    // octaves of surface detail are worth sampling, and that is a per-node question because a
    // trunk and a twig are resolved very differently by the same band.
    let appearances = Appearances::of(skeleton, materials, palette, segments, &to_texel);

    let all = skeleton.segments();
    for index in segments {
        let Some(segment) = all.get(*index as usize) else {
            continue;
        };
        let start = to_texel.point(world(segment.start));
        let end = to_texel.point(world(segment.end));

        // A segment with no length has no perpendicular, so it has no quad. The world-space
        // direction is the one to take it from: the texel-space one is the same direction only
        // when the projection is isotropic, and it is not when a piece was clamped by
        // `MAX_PIECE_SIDE` or widened to a minimum on a degenerate axis.
        let Some(along) = (world(segment.end) - world(segment.start)).try_normalize() else {
            continue;
        };
        let across = along.perp();
        let base = to_texel.offset(across * half(segment.base_width), end - start);
        let tip = to_texel.offset(across * half(segment.tip_width), end - start);

        let Some(brush) = appearances.brush(segment.node, *index) else {
            continue;
        };
        // Where this segment starts and ends on its node's limb. Both ends of the quad share a
        // position along, so the four corners carry two distinct ones — which is exactly what
        // makes the mosaic and the age gradient run along the *node* rather than repeat once
        // per segment, the approximation this replaces.
        let (from, to) = brush.span;
        let corners = [
            Corner::new(start + base, from, 1.0),
            Corner::new(start - base, from, -1.0),
            Corner::new(end - tip, to, -1.0),
            Corner::new(end + tip, to, 1.0),
        ];
        fill(&mut layer, [corners[0], corners[1], corners[2]], &brush);
        fill(&mut layer, [corners[0], corners[2], corners[3]], &brush);
    }
    layer
}

/// Wraps a layer's colour plane as a texture ready to hand to [`Assets<Image>`].
///
/// Consumes the colour and leaves the ids behind, which is the split
/// [`id_buffer`](crate::id_buffer) exists to explain: this half goes to the GPU and is dropped
/// from main memory, and the other half stays on the CPU because that is where clicks are
/// answered.
///
/// `Rgba8UnormSrgb` rather than `Rgba8Unorm`: the GPU then converts to linear on every sample,
/// which is what the rest of the pipeline expects, and it is why [`shaded`] encodes on the way
/// out. Storing linear bytes would band the dark end of the age gradient — exactly where
/// `F-MAT-4` puts its oldest material.
#[must_use]
pub fn texture(size: UVec2, pixels: Vec<u8>) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: size.x.max(1),
            height: size.y.max(1),
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        LAYER_USAGE,
    );
    // Linear filtering, not nearest. A band is chosen to be at least as sharp as the camera
    // needs and at most a doubling sharper, so a layer is nearly always minified slightly —
    // and nearest minification is where a still picture starts to shimmer under a pan.
    image.sampler = ImageSampler::linear();
    image
}

/// The colours contributors are drawn in — `F-ID-4`, `AC-MAT-4`.
///
/// A resource rather than a constant because `F-SET-*` and `F-MAN-9` will eventually let a user
/// point at a palette file, and because the bake needs it every frame it bakes. It holds the
/// compiled-in palette until something loads one: `Palette::from_ron` validates perceptual
/// separation, so a palette that reached here has already been checked, and there is no state
/// in which the mosaic is drawn from unvalidated colours.
///
/// The one thing it must not become is a *second* source of author colour. `treepo-id` owns
/// what colour a contributor is; this owns which palette that question is asked of.
///
/// Shared rather than owned because every off-thread bake reads it — see
/// [`chunk`](crate::chunk)'s header. An `Arc` is the honest spelling of "immutable once
/// validated, read by whoever is drawing", and it makes handing a bake its palette an atomic
/// increment rather than a copy of the entry table.
#[derive(Resource, Debug, Clone)]
pub struct AuthorPalette(pub Arc<Palette>);

impl Default for AuthorPalette {
    fn default() -> Self {
        Self(Arc::new(Palette::built_in()))
    }
}

/// The colour used where a node has no material.
///
/// Reached by [`Skeleton`]s built without a [`MaterialMap`] — the `id-coverage` probe, and unit
/// tests about geometry. A real snapshot is
/// [`covered`](treepo_model::WorldSnapshot::is_covered).
const UNMATERIALIZED: LinearRgba = LinearRgba::rgb(0.30, 0.30, 0.32);

/// Everything one node's texels share — resolved once per bake, read once per texel.
///
/// Three parallel tables rather than one structure per node, because the per-node records are
/// fixed-size and the mosaic runs and inventory bands are not: a `Vec` inside each record would
/// be one allocation per node per bake, at every band, for a handful of entries.
#[derive(Debug, Default)]
struct Appearances {
    /// One record per node the chunk draws, ordered by node so it can be searched.
    nodes: Vec<NodeRecord>,
    /// `(upper fraction bound, colour)` — the ownership mosaic's runs, `F-MAT-2`.
    accents: Vec<(f32, LinearRgba)>,
    /// `(upper fraction bound, colour)` — a container's inventory, `F-SKEL-7`.
    bands: Vec<(f32, LinearRgba)>,
    /// How far along its node each of the chunk's segments begins and ends, in world units.
    ///
    /// Parallel to the `segments` slice `rasterize` was given, so a segment's span is a lookup
    /// rather than a second walk.
    spans: Vec<(u32, f32, f32)>,
}

/// One node's shared appearance.
#[derive(Debug, Clone, Copy)]
struct NodeRecord {
    node: NodeId,
    shading: Shading,
    base: LinearRgba,
    /// The reciprocal of the node's base half-width — limb coordinates are in those units.
    per_unit: f32,
    /// The reciprocal of the node's total limb length — the mosaic and gradient axis.
    per_total: f32,
    accents: (u32, u32),
    bands: (u32, u32),
}

/// One node's measured limb, plus what the tree around it contributes to its appearance.
///
/// A structure rather than six arguments, because five of the six are only ever read together
/// and the sixth would have pushed [`Appearances::push_node`] past the point where a reader can
/// tell which is which at a call site.
#[derive(Debug, Clone, Copy)]
struct Measured<'a> {
    node: NodeId,
    /// The node's total limb length, in world units.
    total: f32,
    /// Its base half-width, in world units.
    unit: f32,
    material: Option<&'a Material>,
    /// What the node stands for, which is where its surface's seeds come from.
    role: Option<&'a NodeRole>,
    /// The parent's age gradient, if it has one — see [`PARENT_AGE_SHARE`].
    inherited: Option<AgeGradient>,
}

/// One node's appearance, ready to draw one of its segments with.
#[derive(Debug, Clone, Copy)]
struct Brush<'a> {
    shading: &'a Shading,
    base: LinearRgba,
    accents: &'a [(f32, LinearRgba)],
    bands: &'a [(f32, LinearRgba)],
    per_unit: f32,
    per_total: f32,
    /// Where this segment starts and ends along its node, in world units.
    span: (f32, f32),
    element: ElementId,
}

impl Appearances {
    /// Resolves every node the chunk's segments belong to.
    ///
    /// Two passes over the segment list. The first measures each node — total limb length, base
    /// half-width, and where each segment sits along it — because a node's mosaic and age
    /// gradient are properties of the whole limb and a chunk holds the whole limb: chunk
    /// identity is subtree-anchored (architecture D5.1), so a node's segments do not straddle
    /// two of them. The second resolves each node's material into something a texel can read.
    ///
    /// The measuring pass walks the segment indices **in skeleton order**, because that is the
    /// order the L-system emitted them and therefore the order base to tip. The slice
    /// `rasterize` is handed makes no such promise, and a chunk whose segments arrived
    /// shuffled would otherwise draw a limb whose gradient ran backwards in places — a defect
    /// that would look like a rendering artefact rather than like a sort that was missing.
    fn of(
        skeleton: &Skeleton,
        materials: &MaterialMap,
        palette: &Palette,
        segments: &[u32],
        to_texel: &Projector,
    ) -> Self {
        let all = skeleton.segments();
        let mut ordered: Vec<u32> = segments
            .iter()
            .copied()
            .filter(|index| (*index as usize) < all.len())
            .collect();
        ordered.sort_unstable();

        let mut measures: Vec<(NodeId, f32, f32)> = Vec::new();
        let mut spans: Vec<(u32, f32, f32)> = Vec::with_capacity(ordered.len());
        for index in ordered {
            let segment = &all[index as usize];
            let length = (world(segment.end) - world(segment.start)).length();
            let widest = half(segment.base_width).max(half(segment.tip_width));

            let slot = match measures.binary_search_by_key(&segment.node, |(node, _, _)| *node) {
                Ok(slot) => slot,
                Err(slot) => {
                    measures.insert(slot, (segment.node, 0.0, 0.0));
                    slot
                }
            };
            let (_, total, unit) = &mut measures[slot];
            spans.push((index, *total, *total + length));
            *total += length;
            *unit = unit.max(widest);
        }

        let mut resolved = Self {
            spans,
            ..Self::default()
        };
        for (node, total, unit) in measures {
            let entry = skeleton.node(node);
            resolved.push_node(
                Measured {
                    node,
                    total,
                    unit,
                    material: materials.get(node),
                    role: entry.map(|entry| &entry.role),
                    // `F-MAT-4` measures one path's own commit span, so two siblings with very
                    // different histories meet at a joint where the picture jumps from grey to
                    // saturated in a single texel. Averaging a node's reading with the limb it
                    // hangs off smooths that without moving where the reading comes from — the
                    // neighbourhood is the one the tree already has, and the node still
                    // dominates it. See `PARENT_AGE_SHARE`.
                    inherited: entry
                        .and_then(|entry| entry.parent)
                        .and_then(|parent| materials.get(parent))
                        .and_then(|material| material.gradient),
                },
                palette,
                to_texel,
            );
        }
        resolved
    }

    /// Resolves one node's material into a record, appending its run tables.
    fn push_node(&mut self, measured: Measured<'_>, palette: &Palette, to_texel: &Projector) {
        let Measured {
            node,
            total,
            unit,
            material,
            role,
            inherited,
        } = measured;
        // A limb with no measurable width still has to be drawn — `MIN_HALF_TEXELS` guarantees
        // it a line of texels — and dividing its coordinates by zero would put a NaN into every
        // one of them. The floor is a world unit rather than an epsilon: below it the whole
        // limb is inside one noise cell anyway, so the exact value cannot be seen.
        let unit = if unit > 1e-4 { unit } else { 1.0 };
        let total = if total > 1e-4 { total } else { unit };

        // The hierarchical half of the surface: coarse structure from the parent path, fine
        // detail from this one, and the whorls placed from the fine half. All three are
        // properties of *where in the repository this is*, which is what makes a limb keep its
        // face across a re-scan — see `Shading::lineage`.
        let (lineage, seed) = grain_of(role, node);
        let knots = knots_of(seed, total / unit);

        let Some(material) = material else {
            // Stone's surface over the unmaterialized colour: "treepo could not name this" is
            // the same thing being said, and a flat patch in a textured tree would read as a
            // hole rather than as an absence.
            self.nodes.push(NodeRecord {
                node,
                shading: Shading {
                    surface: Surface::of(MaterialFamily::Stone),
                    vein: None,
                    age: (0.0, 0.0),
                    cracked: 0.0,
                    restless: 0.0,
                    octaves: 1,
                    seed,
                    lineage,
                    knots,
                },
                base: UNMATERIALIZED,
                per_unit: 1.0 / unit,
                per_total: 1.0 / total,
                accents: (0, 0),
                bands: (0, 0),
            });
            return;
        };

        // `F-MAT-6`: stress coexists with the primary material, so it modifies the surface and
        // never replaces it. `Sparse` is folded into the surface here and therefore costs
        // nothing per texel; the other two carry an intensity into `shade`.
        let intensity = |kind: StressKind| {
            material
                .stress
                .and_then(|stress| stress.intensity_of(kind))
                .map_or(0.0, |value| value.to_f64() as f32)
        };
        let surface = Surface::of(material.family).coarsened(intensity(StressKind::Sparse));

        // How many texels this band puts across half of this limb, which is what decides how
        // much surface detail is resolvable. `min_element` because a piece clamped on one axis
        // resolves no better than its coarser one.
        let texels = unit * to_texel.scale.min_element();

        let vein = match material.composition {
            Composition::Blended { secondary, weight } => {
                Some((Surface::of(secondary).base, weight.to_f64() as f32))
            }
            Composition::Pure | Composition::Subordinate(_) => None,
        };
        // `None` is unknown age rather than new age (see `Material::gradient`), so an unaged
        // node draws at full saturation — the same as a brand-new one, which is the honest
        // reading: nothing here can distinguish them, and pretending otherwise would invent a
        // measurement.
        let age = material
            .gradient
            .map_or((0.0, 0.0), |gradient| blended_age(gradient, inherited));

        let accents = self.push_accents(material, palette);
        let bands = self.push_bands(material);
        self.nodes.push(NodeRecord {
            node,
            shading: Shading {
                surface,
                vein,
                age,
                cracked: intensity(StressKind::Cracked),
                restless: intensity(StressKind::Restless),
                octaves: crate::surface::octaves_for(surface.period().max_element(), texels),
                seed,
                lineage,
                knots,
            },
            base: surface.base,
            per_unit: 1.0 / unit,
            per_total: 1.0 / total,
            accents,
            bands,
        });
    }

    /// Lays this node's ownership mosaic out as runs along its limb — `F-MAT-2`.
    ///
    /// [`Mosaic`](treepo_model::Mosaic) states that a holder occupies a contiguous run and that
    /// the runs follow key order, so "the arrangement is not stored, it *is* holders read in
    /// sequence". This is that sentence as a table, and it makes one choice the model
    /// deliberately leaves open: the held runs are laid from the **base**, so the unclaimed
    /// remainder — which `F-MAT-2` says shows the primary material — is the tip. A limb
    /// therefore reads as owned where it is thick and as its own material where it thins out,
    /// which is the reading that survives being zoomed away from.
    fn push_accents(&mut self, material: &Material, palette: &Palette) -> (u32, u32) {
        let cells = material.mosaic.cells();
        if cells == 0 || material.mosaic.is_empty() {
            return (0, 0);
        }
        let from = self.accents.len() as u32;
        let mut laid = 0u32;
        for (author, held) in material.mosaic.holders() {
            laid = laid.saturating_add(*held);
            let bound = f32::from(u16::try_from(laid).unwrap_or(u16::MAX))
                / f32::from(u16::try_from(cells).unwrap_or(u16::MAX)).max(1.0);
            self.accents
                .push((bound, author_color(&palette.color_of(author))));
        }
        (from, self.accents.len() as u32)
    }

    /// Lays a container's inventory out as bands along its limb — `F-SKEL-7`, `F-INSP-3`.
    ///
    /// A [`Subordinate`](treepo_model::Composition::Subordinate) node "holds materials it is not
    /// made of", and `F-INSP-3` requires it to report what it represents. Banding the mix along
    /// the limb is that report drawn rather than written: a container of mostly documentation is
    /// mostly pale, and the minority families are visible stripes rather than a rounding error.
    ///
    /// Ordered by [`MaterialFamily::ALL`], which is declaration order and carries no ranking —
    /// the same reason the mosaic's runs follow key order.
    fn push_bands(&mut self, material: &Material) -> (u32, u32) {
        let Some(mix) = material.composition.contents() else {
            return (0, 0);
        };
        let from = self.bands.len() as u32;
        let mut laid = 0.0f32;
        for (family, share) in mix.present() {
            laid += share.to_f64() as f32;
            self.bands.push((laid, Surface::of(family).base));
        }
        // The shares are rounded independently and sum to approximately one (`FamilyMix`), so
        // the last band is stretched to the tip rather than leaving a sliver of whatever was
        // underneath. Rounding is not information.
        if let Some(last) = self.bands.last_mut() {
            last.0 = 1.0;
        }
        (from, self.bands.len() as u32)
    }

    /// The brush for one segment, or `None` if its node was not measured.
    fn brush(&self, node: NodeId, index: u32) -> Option<Brush<'_>> {
        let record = self
            .nodes
            .binary_search_by_key(&node, |record| record.node)
            .ok()
            .map(|slot| &self.nodes[slot])?;
        let span = self
            .spans
            .binary_search_by_key(&index, |(at, _, _)| *at)
            .ok()
            .map(|slot| {
                let (_, from, to) = self.spans[slot];
                (from, to)
            })?;
        Some(Brush {
            shading: &record.shading,
            base: record.base,
            accents: slice(&self.accents, record.accents),
            bands: slice(&self.bands, record.bands),
            per_unit: record.per_unit,
            per_total: record.per_total,
            span,
            element: ElementId::of(node),
        })
    }
}

/// A half-open `(from, to)` range of a run table.
fn slice(table: &[(f32, LinearRgba)], range: (u32, u32)) -> &[(f32, LinearRgba)] {
    table
        .get(range.0 as usize..range.1 as usize)
        .unwrap_or_default()
}

/// How much of a node's age reading comes from the limb it hangs off — `F-MAT-4`.
///
/// A quarter. The gradient is a property of one path's own commit span, so two sibling
/// directories with very different histories meet at a joint where the picture jumps from grey
/// to saturated in a single texel — an edge the eye reads as a seam in the rendering rather
/// than as a fact about the repository.
///
/// Averaging over the tree's own neighbourhood is the fix that invents nothing: the parent
/// already aggregates its children (`TemporalPrimitives` rolls up during extraction), so the
/// value being mixed in is a summary of this node *and its siblings*, which is exactly what a
/// neighbourhood average of the age field would be. A quarter, so the node still says what it
/// is and only its edges soften.
const PARENT_AGE_SHARE: f32 = 0.25;

/// One node's age gradient, softened toward the limb it hangs off.
///
/// Both ends move together, so the gradient's *direction* — `F-MAT-4`'s whole content — cannot
/// be reversed by the blend: `AgeGradient` guarantees `base >= tip` for both operands, and a
/// convex combination of two ordered pairs is ordered.
fn blended_age(own: AgeGradient, parent: Option<AgeGradient>) -> (f32, f32) {
    let ends = |gradient: AgeGradient| {
        (
            gradient.base().to_f64() as f32,
            gradient.tip().to_f64() as f32,
        )
    };
    let (base, tip) = ends(own);
    // `None` is unknown rather than new, so there is nothing to average with — a node whose
    // parent has no history keeps its own reading rather than being pulled toward zero.
    let Some((from, to)) = parent.map(ends) else {
        return (base, tip);
    };
    let keep = 1.0 - PARENT_AGE_SHARE;
    (
        base * keep + from * PARENT_AGE_SHARE,
        tip * keep + to * PARENT_AGE_SHARE,
    )
}

/// The two hashes one node's surface is drawn from — coarse from the parent path, fine from
/// this one.
///
/// **The path rather than the node id, and that is a stability argument rather than a
/// determinism one.** Node ids are perfectly deterministic; what they are not is *stable*. Ids
/// are assigned in creation order, so adding one file near the root shifts every id after it —
/// and an id-seeded surface would therefore re-roll the appearance of the entire tree on a
/// re-scan that changed one line. A path-seeded one does not: the limbs that did not change
/// look exactly as they did, which is what makes two screenshots of the same repository a week
/// apart comparable at all.
///
/// The hierarchy falls out of hashing component by component and keeping the value from *before*
/// the last one. Siblings share it, a child's parent-hash is its parent's own hash, and the
/// grain therefore carries across a joint instead of restarting at it (`Shading::lineage`).
///
/// Nodes with no path fall back to the id, mixed. That is the `id-coverage` probe and this
/// module's own geometry tests; a real snapshot is
/// [`covered`](treepo_model::WorldSnapshot::is_covered).
fn grain_of(role: Option<&NodeRole>, node: NodeId) -> (u32, u32) {
    let Some(role) = role else {
        let stirred = avalanche((node.index() as u32).wrapping_mul(0x9e37_79b9));
        return (avalanche(stirred), stirred);
    };
    let (lineage, own) = hash_path(role.anchor());
    match role {
        // One path, several nodes: the root cluster is `AC-SKEL-2`'s answer to an empty
        // repository and every boulder in it anchors to the repository root. The cluster index
        // is what tells them apart, and it is as stable as the path is.
        NodeRole::RootMass { index, .. } => (
            lineage,
            avalanche(own ^ u32::from(*index).wrapping_mul(0x9e37_79b9)),
        ),
        _ => (lineage, own),
    }
}

/// A path hashed component by component: the value before the last component, and after it.
fn hash_path(path: &RepoPath) -> (u32, u32) {
    let mut running = 0x811c_9dc5u32;
    let mut parent = running;
    for component in path.components() {
        parent = running;
        for byte in component {
            running ^= u32::from(*byte);
            running = running.wrapping_mul(0x0100_0193);
        }
        // Separated, so `ab/c` and `a/bc` are different limbs. FNV alone would give them the
        // same running value at the point the components are concatenated.
        running = running.wrapping_add(0x9e37_79b9);
    }
    (avalanche(parent), avalanche(running))
}

/// Spreads a hash's low-order structure across all thirty-two bits.
///
/// FNV mixes well upward and poorly downward, and the surface reads several disjoint bit fields
/// out of these words — a knot's position, its size, the plate phase. An unavalanched FNV would
/// put every limb's knots in nearly the same place.
fn avalanche(value: u32) -> u32 {
    let mut mixed = value ^ (value >> 16);
    mixed = mixed.wrapping_mul(0x85eb_ca6b);
    mixed ^= mixed >> 13;
    mixed = mixed.wrapping_mul(0xc2b2_ae35);
    mixed ^ (mixed >> 16)
}

/// How long a limb has to be, in half-widths, to earn each of its knots.
///
/// Four. A knot disturbs about a half-width across and twice that along, so a limb shorter than
/// this would be mostly knot — a blemish rather than grain. It also means knots appear on
/// boughs and not on twigs, which is both what wood does and what the picture wants: a twig is
/// a dozen texels and has nothing to spare.
const KNOT_INTERVAL: f32 = 4.0;

/// Places a limb's whorls from its own hash.
///
/// One knot per [`KNOT_INTERVAL`] of limb, each placed inside its own stretch so two cannot
/// land on top of each other, and each sized and positioned from a disjoint bit field of the
/// same word. Three fields out of one hash rather than three hashes: the bits are independent
/// after [`avalanche`], and this runs once per node rather than once per texel, so the saving
/// is not the point — having one obvious source for "everything about this limb's knots" is.
fn knots_of(seed: u32, half_widths: f32) -> [Knot; MAX_KNOTS] {
    let mut knots = [Knot::default(); MAX_KNOTS];
    if !half_widths.is_finite() {
        return knots;
    }
    let count = ((half_widths / KNOT_INTERVAL) as usize).min(MAX_KNOTS);
    let stretch = half_widths / count.max(1) as f32;
    let mut bits = seed;
    for (index, knot) in knots.iter_mut().take(count).enumerate() {
        bits = avalanche(bits ^ 0x9e37_79b9);
        let field = |shift: u32| ((bits >> shift) & 0x3ff) as f32 * (1.0 / 1024.0);
        *knot = Knot {
            // Inside its own stretch, with a margin at each end so a knot never straddles the
            // base or the tip — where the limb is either joined to something or a point.
            at: Vec2::new(
                (index as f32 + 0.2 + 0.6 * field(0)) * stretch,
                field(10) * 1.3 - 0.65,
            ),
            reach: 0.42 + 0.38 * field(20),
            depth: 0.40 + 0.45 * field(0).max(field(20)),
        };
    }
    knots
}

/// World coordinates to texel coordinates, for one baked region.
///
/// Row zero is the *top* of the region, because image rows run downwards and world `y` runs
/// upwards — the one sign in this module worth getting wrong, and the reason it is a named type
/// rather than two multiplications at the call site.
#[derive(Debug, Clone, Copy)]
struct Projector {
    origin: Vec2,
    scale: Vec2,
}

impl Projector {
    /// `None` for a region with no area to project onto, which draws nothing.
    fn new(region: Extent, size: UVec2) -> Option<Self> {
        let span = region.size();
        if !span.is_finite() || span.x <= 0.0 || span.y <= 0.0 || size.x == 0 || size.y == 0 {
            return None;
        }
        let scale = size.as_vec2() / span;
        scale.is_finite().then_some(Self {
            origin: Vec2::new(region.min.x, region.max.y),
            scale,
        })
    }

    /// A world point in texel coordinates.
    fn point(&self, at: Vec2) -> Vec2 {
        Vec2::new(
            (at.x - self.origin.x) * self.scale.x,
            (self.origin.y - at.y) * self.scale.y,
        )
    }

    /// A world *direction* in texel coordinates, never thinner than [`MIN_HALF_TEXELS`].
    ///
    /// `fallback` is the segment's texel-space direction, used when the offset has no length of
    /// its own — a zero-width limb still has a side to be pushed out to.
    fn offset(&self, at: Vec2, fallback: Vec2) -> Vec2 {
        let mapped = Vec2::new(at.x * self.scale.x, -at.y * self.scale.y);
        if mapped.length() >= MIN_HALF_TEXELS {
            return mapped;
        }
        let direction = mapped
            .try_normalize()
            .or_else(|| fallback.perp().try_normalize())
            .unwrap_or(Vec2::X);
        direction * MIN_HALF_TEXELS
    }
}

/// One corner of a segment's quad: where it is, and where that is on the limb.
#[derive(Debug, Clone, Copy)]
struct Corner {
    /// Texel-space position.
    at: Vec2,
    /// Distance from the node's base, in world units.
    distance: f32,
    /// Which side of the centre line — `-1` and `+1` are the limb's edges.
    across: f32,
}

impl Corner {
    fn new(at: Vec2, distance: f32, across: f32) -> Self {
        Self {
            at,
            distance,
            across,
        }
    }
}

/// Fills one triangle into both planes, shading each texel from its position on the limb.
///
/// Scanline: for each row the triangle reaches, the two edges it crosses give the span, and
/// only texels inside that span are visited. See the module header for why the bounding-box
/// alternative is not merely slower but asymptotically worse.
///
/// Takes the whole [`Layer`] rather than the colour slice, so that "write a texel" is one place
/// and writes both. That is `N7` expressed as a signature.
fn fill(layer: &mut Layer, corners: [Corner; 3], brush: &Brush<'_>) {
    let size = layer.size;
    let points = [corners[0].at, corners[1].at, corners[2].at];
    let area = edge(points[0], points[1], points[2]);
    if area.abs() < f32::EPSILON || !area.is_finite() {
        return;
    }
    let sign = area.signum();
    let total = area.abs();
    let per_area = 1.0 / total;
    // Resolved once for the triangle rather than three times per texel. `LazyLock` derefs
    // through an atomic load and a branch, which is cheap on its own and is not cheap when the
    // encoder asks for it on every channel of every texel.
    let srgb: &[u8; SRGB_TABLE_LEN] = &SRGB_TABLE;

    let top = points.iter().map(|c| c.y).fold(f32::INFINITY, f32::min);
    let bottom = points.iter().map(|c| c.y).fold(f32::NEG_INFINITY, f32::max);
    let (first, last) = (first_texel(top, size.y), last_texel(bottom, size.y));

    for row in first..last {
        let y = row as f32 + 0.5;
        let Some((left, right)) = span(points, y) else {
            continue;
        };
        let (from, to) = (first_texel(left, size.x), last_texel(right, size.x));

        for column in from..to {
            let at = Vec2::new(column as f32 + 0.5, y);
            // Recomputed per texel rather than stepped along the row. Incremental edge
            // functions are the classic optimization and they accumulate error across a long
            // span; the span is already short because the scanline bounded it.
            let weights = [
                edge(points[1], points[2], at) * sign,
                edge(points[2], points[0], at) * sign,
                edge(points[0], points[1], at) * sign,
            ];
            if weights.iter().any(|w| *w < 0.0) {
                continue;
            }

            let index = row as usize * size.x as usize + column as usize;
            let offset = index * BYTES_PER_TEXEL;
            let Some(texel) = layer.color.get_mut(offset..offset + BYTES_PER_TEXEL) else {
                continue;
            };

            // Two attributes rather than four colour channels, and the conversion to limb
            // coordinates is two multiplies against reciprocals the brush already holds.
            let distance = (0..3)
                .map(|corner| weights[corner] * corners[corner].distance)
                .sum::<f32>()
                * per_area;
            let across = (0..3)
                .map(|corner| weights[corner] * corners[corner].across)
                .sum::<f32>()
                * per_area;
            let point = LimbPoint {
                along: distance * brush.per_unit,
                across,
                fraction: (distance * brush.per_total).clamp(0.0, 1.0),
            };

            // A container's inventory is read here and its ownership is not, and the asymmetry
            // is the point. An inventory band is a *report* — `F-INSP-3` asks the container to
            // say what it holds — so it wants a crisp boundary at the share it names. Ownership
            // is a surface, so where its table is read is a per-texel decision the shader makes
            // from the same flow field that bends the grain; see `surface::weave`.
            let base = run_at(brush.bands, point.fraction).unwrap_or(brush.base);
            texel.copy_from_slice(&encode(
                srgb,
                crate::surface::shade(brush.shading, base, point, brush.accents),
            ));

            layer.texels_drawn += 1;

            // The same texel, the same iteration, no branch between them. Later triangles
            // overwrite earlier ones in both planes together, so the id always names whichever
            // element the *visible* colour came from — which is the whole property picking
            // relies on.
            if let Some(id) = layer.ids.get_mut(index) {
                *id = brush.element;
            }
        }
    }
}

/// How many entries the sRGB encoding table holds.
///
/// Indexed by the *square root* of the linear value, which is what makes a thousand entries
/// enough. sRGB is steep near black — a linearly-indexed table would give the darkest forty
/// output codes a single entry between them, and `F-MAT-4` puts its oldest material exactly
/// there. Taking a square root first spreads the table's resolution into the darks, at the cost
/// of one hardware instruction.
const SRGB_TABLE_LEN: usize = 1024;

/// Linear light to sRGB bytes, precomputed.
///
/// [`Srgba::from`] is one `powf` per channel, which at three channels and a million texels a
/// frame is more than the whole of the rest of the bake. The old rasterizer never paid it —
/// it interpolated already-encoded corner colours — and shading per texel is what makes the
/// conversion per texel too.
static SRGB_TABLE: LazyLock<[u8; SRGB_TABLE_LEN]> = LazyLock::new(|| {
    let mut table = [0u8; SRGB_TABLE_LEN];
    for (index, entry) in table.iter_mut().enumerate() {
        let root = index as f32 / (SRGB_TABLE_LEN - 1) as f32;
        // Bevy's own conversion, so the table cannot drift from what the rest of the engine
        // means by sRGB — `the_srgb_table_agrees_with_the_exact_conversion` holds it to that.
        let encoded = Srgba::from(LinearRgba::new(root * root, 0.0, 0.0, 1.0)).red;
        *entry = (encoded * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    table
});

/// One linear channel as an sRGB byte.
fn srgb_byte(table: &[u8; SRGB_TABLE_LEN], value: f32) -> u8 {
    let index = (value.clamp(0.0, 1.0).sqrt() * (SRGB_TABLE_LEN - 1) as f32) as usize;
    table[index.min(SRGB_TABLE_LEN - 1)]
}

/// A shaded colour as the four bytes a texel holds.
///
/// Alpha is always opaque, and that is `N7` rather than a simplification: [`fill`] writes an
/// element id beside every one of these, so a texel this function could make transparent would
/// be an id with no visible colour — which `Coverage::is_clean` counts and refuses.
fn encode(table: &[u8; SRGB_TABLE_LEN], color: LinearRgba) -> [u8; BYTES_PER_TEXEL] {
    [
        srgb_byte(table, color.red),
        srgb_byte(table, color.green),
        srgb_byte(table, color.blue),
        u8::MAX,
    ]
}

/// Where a horizontal line at `y` enters and leaves a triangle.
///
/// `None` when the line misses it. A convex triangle gives exactly two crossings, and the
/// half-open `<=` test on each edge is what stops a vertex exactly on the line from counting
/// twice — the classic double-crossing bug, which shows up as a dropped scanline.
fn span(corners: [Vec2; 3], y: f32) -> Option<(f32, f32)> {
    let mut low = f32::INFINITY;
    let mut high = f32::NEG_INFINITY;
    for index in 0..3 {
        let (a, b) = (corners[index], corners[(index + 1) % 3]);
        if (a.y <= y) == (b.y <= y) {
            continue;
        }
        let t = (y - a.y) / (b.y - a.y);
        let x = a.x + (b.x - a.x) * t;
        low = low.min(x);
        high = high.max(x);
    }
    (low <= high).then_some((low, high))
}

/// The first texel whose centre is at or after `value`.
///
/// Texel *centres* sit at `index + 0.5`, so the index is offset before rounding. Flooring both
/// ends instead — the obvious version — silently drops the last covered texel on every span,
/// which is invisible on a thick limb and deletes a thin one entirely.
fn first_texel(value: f32, limit: u32) -> u32 {
    if !value.is_finite() {
        return 0;
    }
    (((value - 0.5).ceil()).max(0.0) as u32).min(limit)
}

/// One past the last texel whose centre is at or before `value`.
fn last_texel(value: f32, limit: u32) -> u32 {
    if !value.is_finite() {
        return 0;
    }
    let last = (value - 0.5).floor();
    if last < 0.0 {
        0
    } else {
        (last as u32).saturating_add(1).min(limit)
    }
}

/// Twice the signed area of the triangle `a b p` — positive when `p` is left of `a → b`.
fn edge(a: Vec2, b: Vec2, p: Vec2) -> f32 {
    (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
}

/// Baked layers are GPU-only; the trade is stated where the constant lives.
const _: RenderAssetUsages = LAYER_USAGE;

#[cfg(test)]
mod tests {
    use super::*;
    use treepo_det::{Angle, Fx, OrderedMap, Seed};
    use treepo_model::{
        AgeGradient, AuthorKey, FamilyMix, Mosaic, NodeRole, Point, RepoPath, Segment, Stress,
    };

    fn skeleton_with(segments: impl IntoIterator<Item = Segment>) -> Skeleton {
        let mut skeleton = Skeleton::new();
        skeleton.push_node(
            None,
            Point::ORIGIN,
            Angle::ZERO,
            Seed::root(b"bake-test"),
            NodeRole::Limb {
                path: RepoPath::root(),
            },
        );
        skeleton.extend_segments(segments);
        skeleton
    }

    fn segment(start: (i32, i32), end: (i32, i32), base: i32, tip: i32) -> Segment {
        Segment {
            start: Point::new(Fx::from_int(start.0), Fx::from_int(start.1)),
            end: Point::new(Fx::from_int(end.0), Fx::from_int(end.1)),
            base_width: Fx::from_int(base),
            tip_width: Fx::from_int(tip),
            node: NodeId::new(0),
            generation: 0,
        }
    }

    fn palette() -> Palette {
        Palette::built_in()
    }

    fn square(size: u32) -> (Extent, UVec2) {
        (
            Extent {
                min: Vec2::ZERO,
                max: Vec2::splat(size as f32),
            },
            UVec2::splat(size),
        )
    }

    /// The texel at `(column, row)`, with row zero at the top of the region.
    fn texel(layer: &Layer, size: UVec2, column: u32, row: u32) -> [u8; 4] {
        let offset = (row as usize * size.x as usize + column as usize) * BYTES_PER_TEXEL;
        layer.color[offset..offset + BYTES_PER_TEXEL]
            .try_into()
            .unwrap()
    }

    #[test]
    fn an_empty_chunk_bakes_to_transparency() {
        let (region, size) = square(16);
        let layer = rasterize(
            &skeleton_with([]),
            &MaterialMap::new(),
            &palette(),
            &[],
            region,
            size,
        );
        assert_eq!(layer.color.len(), 16 * 16 * BYTES_PER_TEXEL);
        assert!(layer.color.iter().all(|byte| *byte == 0));
    }

    /// A wide vertical limb up the middle: the centre column is opaque, the corners are not.
    #[test]
    fn a_limb_covers_the_texels_it_runs_through() {
        let skeleton = skeleton_with([segment((16, 2), (16, 30), 8, 8)]);
        let (region, size) = square(32);
        let layer = rasterize(
            &skeleton,
            &MaterialMap::new(),
            &palette(),
            &[0],
            region,
            size,
        );

        assert_eq!(texel(&layer, size, 16, 16)[3], 255, "centre is not drawn");
        assert_eq!(texel(&layer, size, 0, 0)[3], 0, "corner was drawn");
        assert_eq!(texel(&layer, size, 31, 31)[3], 0, "corner was drawn");
    }

    /// Row zero is the top. A limb in the upper half of the world region must land in the
    /// upper rows of the image — the one sign error in the module that produces an
    /// upside-down tree rather than a crash.
    #[test]
    fn world_up_is_image_up() {
        let skeleton = skeleton_with([segment((16, 24), (16, 30), 8, 8)]);
        let (region, size) = square(32);
        let layer = rasterize(
            &skeleton,
            &MaterialMap::new(),
            &palette(),
            &[0],
            region,
            size,
        );

        assert_eq!(
            texel(&layer, size, 16, 4)[3],
            255,
            "top of the image is empty"
        );
        assert_eq!(
            texel(&layer, size, 16, 28)[3],
            0,
            "bottom of the image is drawn"
        );
    }

    /// `MIN_HALF_TEXELS` as the property it exists for: a limb far thinner than a texel is
    /// still on the picture. Without it, `F-NAV-3`'s far band loses whole directories.
    #[test]
    fn a_limb_thinner_than_a_texel_is_still_drawn() {
        let mut thin = segment((16, 2), (16, 30), 1, 1);
        thin.base_width = Fx::from_ratio(1, 500);
        thin.tip_width = Fx::from_ratio(1, 500);
        let skeleton = skeleton_with([thin]);
        let (region, size) = square(32);

        let layer = rasterize(
            &skeleton,
            &MaterialMap::new(),
            &palette(),
            &[0],
            region,
            size,
        );
        let drawn = (0..32)
            .filter(|row| texel(&layer, size, 16, *row)[3] > 0)
            .count();
        assert!(drawn >= 24, "a hairline limb drew {drawn} of 28 rows");
    }

    #[test]
    fn a_segment_with_no_length_draws_nothing() {
        let skeleton = skeleton_with([segment((16, 16), (16, 16), 4, 4)]);
        let (region, size) = square(32);
        let layer = rasterize(
            &skeleton,
            &MaterialMap::new(),
            &palette(),
            &[0],
            region,
            size,
        );
        assert!(layer.color.iter().all(|byte| *byte == 0));
    }

    /// A segment index the skeleton does not have is skipped, not panicked on.
    #[test]
    fn an_index_past_the_end_is_skipped() {
        let skeleton = skeleton_with([segment((16, 2), (16, 30), 8, 8)]);
        let (region, size) = square(32);
        let layer = rasterize(
            &skeleton,
            &MaterialMap::new(),
            &palette(),
            &[0, 99],
            region,
            size,
        );
        assert_eq!(texel(&layer, size, 16, 16)[3], 255);
    }

    /// A region with no area, or a texture with no texels, produces a buffer rather than a
    /// division by zero. `AC-SKEL-2`'s empty repository reaches here.
    #[test]
    fn a_degenerate_region_bakes_without_dividing_by_zero() {
        let skeleton = skeleton_with([segment((0, 0), (0, 10), 2, 1)]);
        let flat = Extent {
            min: Vec2::ZERO,
            max: Vec2::new(0.0, 10.0),
        };
        let layer = rasterize(
            &skeleton,
            &MaterialMap::new(),
            &palette(),
            &[0],
            flat,
            UVec2::new(1, 10),
        );
        assert_eq!(layer.color.len(), 10 * BYTES_PER_TEXEL);
    }

    /// `N7`, as a property of the rasterizer rather than of one drawing: wherever this bake
    /// put a colour it also put an element id, and wherever it put an id it put a colour.
    /// `xtask id-coverage` asserts the same thing over real repositories; this asserts it over
    /// the case that is easy to get wrong — a limb clipped by the edge of its own piece.
    #[test]
    fn every_coloured_texel_carries_an_element_id() {
        let skeleton = skeleton_with([
            segment((16, 2), (16, 30), 8, 8),
            // Running off the right edge, so the fill's bounds check is exercised.
            segment((16, 20), (60, 24), 6, 3),
        ]);
        let (region, size) = square(32);
        let layer = rasterize(
            &skeleton,
            &MaterialMap::new(),
            &palette(),
            &[0, 1],
            region,
            size,
        );

        let found = crate::id_buffer::coverage(&layer.color, &layer.ids);
        assert!(found.accounted > 0, "the test drew nothing");
        assert_eq!(found.unaccountable, 0, "a coloured texel carries no id");
        assert_eq!(found.invisible, 0, "an id was written with no colour");
        assert!(found.is_clean());
    }

    /// The id names the node that painted the texel, not merely *a* node — which is what makes
    /// the click and the picture unable to disagree.
    #[test]
    fn the_id_names_the_element_that_painted_the_texel() {
        let mut skeleton = skeleton_with([]);
        skeleton.push_node(
            None,
            Point::ORIGIN,
            Angle::ZERO,
            Seed::root(b"bake-test"),
            NodeRole::Limb {
                path: RepoPath::root(),
            },
        );
        let mut second = segment((26, 2), (26, 30), 6, 6);
        second.node = NodeId::new(1);
        skeleton.extend_segments([segment((6, 2), (6, 30), 6, 6), second]);

        let (region, size) = square(32);
        let layer = rasterize(
            &skeleton,
            &MaterialMap::new(),
            &palette(),
            &[0, 1],
            region,
            size,
        );
        let plane = layer.id_plane();

        assert_eq!(plane.at(6, 16).node(), Some(NodeId::new(0)));
        assert_eq!(plane.at(26, 16).node(), Some(NodeId::new(1)));
        assert!(
            plane.at(16, 16).is_none(),
            "the gap between them is painted"
        );
    }

    /// Overwriting order is shared, so the id follows the *visible* colour. Two limbs crossing
    /// is exactly where the old geometric picker could name the one that was not drawn.
    #[test]
    fn where_limbs_overlap_the_id_follows_the_visible_colour() {
        let mut skeleton = skeleton_with([]);
        skeleton.push_node(
            None,
            Point::ORIGIN,
            Angle::ZERO,
            Seed::root(b"bake-test"),
            NodeRole::Limb {
                path: RepoPath::root(),
            },
        );
        let mut crossing = segment((2, 16), (30, 16), 8, 8);
        crossing.node = NodeId::new(1);
        skeleton.extend_segments([segment((16, 2), (16, 30), 8, 8), crossing]);

        let (region, size) = square(32);
        let layer = rasterize(
            &skeleton,
            &MaterialMap::new(),
            &palette(),
            &[0, 1],
            region,
            size,
        );
        let plane = layer.id_plane();

        // The crossing limb is rasterized second, so at the intersection both planes hold it.
        assert_eq!(plane.at(16, 16).node(), Some(NodeId::new(1)));
        assert_eq!(texel(&layer, size, 16, 16)[3], 255);
    }

    /// A material for the one node `skeleton_with` creates.
    fn materials(build: impl FnOnce(&mut Material)) -> MaterialMap {
        let mut material = Material {
            family: MaterialFamily::Heartwood,
            composition: Composition::Pure,
            budget: Fx::from_ratio(1, 2),
            mosaic: Mosaic::new(OrderedMap::new(), 0),
            gradient: None,
            stress: None,
        };
        build(&mut material);
        let mut map = MaterialMap::new();
        map.push(material);
        map
    }

    /// How many distinct colours a column of the picture holds.
    fn distinct_down(layer: &Layer, size: UVec2, column: u32) -> usize {
        let mut seen: Vec<[u8; 4]> = Vec::new();
        for row in 0..size.y {
            let found = texel(layer, size, column, row);
            if found[3] > 0 && !seen.contains(&found) {
                seen.push(found);
            }
        }
        seen.len()
    }

    /// The whole point of the slice, as a property of the bake rather than of the shader: a
    /// limb of one material is no longer one colour. Before this, every texel of a segment
    /// interpolated between two values and a long straight limb was a flat stripe.
    #[test]
    fn a_limb_of_one_material_is_not_one_colour() {
        let skeleton = skeleton_with([segment((16, 2), (16, 30), 8, 8)]);
        let (region, size) = square(32);
        let layer = rasterize(
            &skeleton,
            &materials(|_| {}),
            &palette(),
            &[0],
            region,
            size,
        );
        let drawn = distinct_down(&layer, size, 16);
        assert!(drawn > 8, "a whole limb drew {drawn} colours");
    }

    /// The mosaic and the age gradient run along the **node**, not along each segment. A limb
    /// built from several segments used to restart both at every one of them, so a five-segment
    /// limb showed five gradients and five copies of the mosaic — which reads as banding
    /// nobody asked for and is the defect the span table exists to remove.
    #[test]
    fn the_gradient_runs_along_the_node_not_along_each_segment() {
        let skeleton = skeleton_with([
            segment((16, 2), (16, 10), 8, 8),
            segment((16, 10), (16, 18), 8, 8),
            segment((16, 18), (16, 26), 8, 8),
        ]);
        let (region, size) = square(32);
        let layer = rasterize(
            &skeleton,
            &materials(|material| {
                material.gradient = Some(AgeGradient::new(Fx::ONE, Fx::ZERO));
                material.family = MaterialFamily::Machined;
            }),
            &palette(),
            &[0, 1, 2],
            region,
            size,
        );

        // The same offset within each segment, base first. A per-segment gradient makes the
        // three equal; a per-node one makes them a monotone run from old to new.
        let brightness = |row: u32| u32::from(texel(&layer, size, 16, row)[0]);
        let (first, second, third) = (brightness(26), brightness(18), brightness(10));
        assert!(
            first < second && second < third,
            "the gradient restarted per segment: {first}, {second}, {third}"
        );
    }

    /// `F-MAT-2` reaching a pixel. The mosaic is laid in runs along the limb, so a node two
    /// contributors are drawn on has two visibly different regions — and the material's own
    /// texture is still varying inside each of them, which is what "accent over" means.
    #[test]
    fn an_owned_limb_shows_its_contributors_as_runs() {
        let skeleton = skeleton_with([segment((16, 2), (16, 30), 8, 8)]);
        let (region, size) = square(32);
        let mut held = OrderedMap::new();
        held.insert(AuthorKey::from_email(b"one@example.invalid"), 5);
        held.insert(AuthorKey::from_email(b"two@example.invalid"), 5);
        let owned = rasterize(
            &skeleton,
            &materials(|material| material.mosaic = Mosaic::new(held, 10)),
            &palette(),
            &[0],
            region,
            size,
        );
        let plain = rasterize(
            &skeleton,
            &materials(|_| {}),
            &palette(),
            &[0],
            region,
            size,
        );

        assert_ne!(
            texel(&owned, size, 16, 26),
            texel(&plain, size, 16, 26),
            "the mosaic did not reach the picture"
        );
        assert_ne!(
            texel(&owned, size, 16, 26),
            texel(&owned, size, 16, 6),
            "both contributors drew the same colour"
        );
    }

    /// `F-SKEL-7` and `F-INSP-3`: a container reports what it holds. Half the nodes of a T3
    /// tree are containers, so a container that drew as one flat colour would be half the
    /// picture saying nothing.
    #[test]
    fn a_container_draws_its_inventory_along_its_length() {
        let skeleton = skeleton_with([segment((16, 2), (16, 30), 8, 8)]);
        let (region, size) = square(32);
        let mut shares = [Fx::ZERO; MaterialFamily::ALL.len()];
        shares[MaterialFamily::Parchment.position()] = Fx::from_ratio(1, 2);
        shares[MaterialFamily::Ore.position()] = Fx::from_ratio(1, 2);
        let layer = rasterize(
            &skeleton,
            &materials(|material| {
                material.composition = Composition::Subordinate(FamilyMix::new(shares));
            }),
            &palette(),
            &[0],
            region,
            size,
        );

        // The bands run base to tip in family order, so ore — dark and blue-grey — is the basal
        // half and parchment — pale and warm — is the distal half.
        //
        // Averaged over a band rather than read off one texel of it. The surface varies from
        // texel to texel by design — plates, grooves, whorls and grain all move the value — so a
        // single sample is a coin flip on which part of the relief it landed in, and a test that
        // is a coin flip fails for a reason that is not the one it is named after. The claim is
        // about a *band*, so it is measured over one.
        let (base, tip) = (warmth(&layer, size, 22..30), warmth(&layer, size, 2..10));
        assert!(
            tip > base + 0.02,
            "the inventory bands are the wrong way round: {base} then {tip}"
        );
    }

    /// How warm and bright a horizontal band of the limb is, averaged across its rows.
    fn warmth(layer: &Layer, size: UVec2, rows: std::ops::Range<u32>) -> f32 {
        let mut sum = 0.0;
        let mut seen = 0;
        for row in rows {
            for column in 8..24 {
                let found = texel(layer, size, column, row);
                if found[3] == 0 {
                    continue;
                }
                sum += f32::from(found[0]) + f32::from(found[1]);
                seen += 1;
            }
        }
        if seen == 0 {
            0.0
        } else {
            sum / (seen as f32 * 510.0)
        }
    }

    /// `N7` over a node carrying every reading at once. The existing coverage test draws
    /// geometry with no material; this one draws the case where the shading has the most to do,
    /// which is where a transparent texel would come from if one could.
    #[test]
    fn a_fully_dressed_limb_is_still_completely_accounted_for() {
        let skeleton = skeleton_with([
            segment((16, 2), (16, 30), 8, 8),
            segment((16, 20), (60, 24), 6, 3),
        ]);
        let (region, size) = square(32);
        let mut held = OrderedMap::new();
        held.insert(AuthorKey::from_email(b"one@example.invalid"), 3);
        let layer = rasterize(
            &skeleton,
            &materials(|material| {
                material.composition = Composition::Blended {
                    secondary: MaterialFamily::Ore,
                    weight: Fx::from_ratio(1, 4),
                };
                material.mosaic = Mosaic::new(held, 8);
                material.gradient = Some(AgeGradient::new(Fx::ONE, Fx::ZERO));
                material.stress = Stress::new([Some(Fx::ONE), Some(Fx::ONE), Some(Fx::ONE)]);
            }),
            &palette(),
            &[0, 1],
            region,
            size,
        );

        let found = crate::id_buffer::coverage(&layer.color, &layer.ids);
        assert!(found.accounted > 0, "the test drew nothing");
        assert_eq!(found.unaccountable, 0, "a coloured texel carries no id");
        assert_eq!(found.invisible, 0, "an id was written with no colour");
    }

    /// The encoding table stands in for `Srgba::from`, so it has to agree with it. One code of
    /// slack, which is the rounding the table's own quantization can introduce and nothing more
    /// — a wider tolerance would let a wrong curve pass.
    #[test]
    fn the_srgb_table_agrees_with_the_exact_conversion() {
        let mut worst = 0i32;
        for step in 0..2048 {
            let linear = step as f32 / 2047.0;
            let exact = Srgba::from(LinearRgba::new(linear, 0.0, 0.0, 1.0)).to_u8_array()[0];
            let drift = i32::from(srgb_byte(&SRGB_TABLE, linear)) - i32::from(exact);
            worst = worst.max(drift.abs());
        }
        assert!(worst <= 1, "the table drifts from sRGB by {worst} codes");
    }

    /// The dark end is where a linearly-indexed table would fail and where `F-MAT-4` puts its
    /// oldest material, so it is checked separately from the sweep above.
    #[test]
    fn the_srgb_table_resolves_the_dark_end() {
        let mut seen: Vec<u8> = Vec::new();
        for step in 1..40 {
            let byte = srgb_byte(&SRGB_TABLE, step as f32 / 4000.0);
            if !seen.contains(&byte) {
                seen.push(byte);
            }
        }
        assert!(
            seen.len() > 10,
            "the darkest linear values collapsed to {} codes",
            seen.len()
        );
    }

    /// The scanline's job, stated as arithmetic: a horizontal line through the middle of a
    /// triangle enters and leaves once each.
    #[test]
    fn a_scanline_crosses_a_triangle_exactly_twice() {
        let triangle = [
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(5.0, 10.0),
        ];
        let (left, right) = span(triangle, 5.0).unwrap();
        assert!((left - 2.5).abs() < 1e-4, "left edge at {left}");
        assert!((right - 7.5).abs() < 1e-4, "right edge at {right}");
        assert!(span(triangle, 20.0).is_none(), "a miss reported a span");
    }
}
