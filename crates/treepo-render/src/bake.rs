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
//! makes the cost proportional to what is drawn, which is what lets [`BAKES_PER_FRAME`] be a
//! small number instead of a guess.
//!
//! [`BAKES_PER_FRAME`]: crate::chunk::BAKES_PER_FRAME
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

use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageSampler};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use treepo_model::{MaterialFamily, MaterialMap, NodeId, Skeleton};

use crate::chunk::{Extent, LAYER_USAGE, half, world};
use crate::id_buffer::{ElementId, IdPlane};

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
    segments: &[u32],
    region: Extent,
    size: UVec2,
) -> Layer {
    let mut layer = Layer::blank(size);
    let Some(to_texel) = Projector::new(region, size) else {
        return layer;
    };

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

        let (base_color, tip_color) = segment_colors(materials, segment.node);
        let corners = [start + base, start - base, end - tip, end + tip];
        // The segment's own node, carried into both triangles. `N7` is this argument passed
        // down: a texel cannot be coloured without one, because `fill` has no signature that
        // omits it.
        let element = ElementId::of(segment.node);
        fill(
            &mut layer,
            [corners[0], corners[1], corners[2]],
            [base_color, base_color, tip_color],
            element,
        );
        fill(
            &mut layer,
            [corners[0], corners[2], corners[3]],
            [base_color, tip_color, tip_color],
            element,
        );
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

/// The placeholder colour of one material family.
///
/// **A placeholder, and a labelled one.** `F-MAT-1`'s six families are surfaces —
/// "wood-like, crystalline, metallic, leafy, dusty" — and a surface is a shader and a tile
/// atlas, not a hex value. What these six do is make the families *distinguishable* so that the
/// wiring from `materialize` to a pixel can be seen to work. They are not tuned, they have not
/// been through the perceptual-separation check `AC-MAT-4` applies to the author palette, and
/// they are not in `assets/palettes/` for exactly that reason: a file there would look like a
/// decision.
#[must_use]
pub fn family_color(family: MaterialFamily) -> LinearRgba {
    match family {
        MaterialFamily::Heartwood => LinearRgba::rgb(0.42, 0.26, 0.13),
        MaterialFamily::Ore => LinearRgba::rgb(0.36, 0.38, 0.44),
        MaterialFamily::Machined => LinearRgba::rgb(0.52, 0.54, 0.50),
        MaterialFamily::Parchment => LinearRgba::rgb(0.72, 0.66, 0.48),
        MaterialFamily::Resin => LinearRgba::rgb(0.62, 0.42, 0.14),
        MaterialFamily::Stone => LinearRgba::rgb(0.38, 0.38, 0.40),
    }
}

/// The colour used where a node has no material.
const UNMATERIALIZED: LinearRgba = LinearRgba::rgb(0.30, 0.30, 0.32);

/// How much of its brightness the oldest material loses.
///
/// `F-MAT-4` is a gradient from older-basal to newer-distal, and darkening with age is the
/// cheapest rendering of it that is still the right *direction*. Like [`family_color`] it is a
/// placeholder: §8.3's reading is growth rings and tip vitality, which is a surface treatment.
const AGE_DARKENING: f32 = 0.45;

/// One segment's base and tip colours: its family, shaded by its age gradient, sRGB-encoded.
fn segment_colors(materials: &MaterialMap, node: NodeId) -> ([u8; 4], [u8; 4]) {
    let Some(material) = materials.get(node) else {
        let flat = shaded(UNMATERIALIZED, 0.0);
        return (flat, flat);
    };

    let color = family_color(material.family);
    // `None` is unknown age rather than new age (see `Material::gradient`), so an unaged node
    // draws at full brightness — the same as a brand-new one, which is the honest reading:
    // nothing here can distinguish them, and pretending otherwise would invent a measurement.
    let (base_age, tip_age) = match material.gradient {
        None => (0.0, 0.0),
        Some(gradient) => (
            gradient.base().to_f64() as f32,
            gradient.tip().to_f64() as f32,
        ),
    };

    (shaded(color, base_age), shaded(color, tip_age))
}

/// A colour darkened by a normalized age in `0..=1`, as sRGB bytes.
fn shaded(color: LinearRgba, age: f32) -> [u8; 4] {
    let factor = 1.0 - AGE_DARKENING * age.clamp(0.0, 1.0);
    Srgba::from(LinearRgba::new(
        color.red * factor,
        color.green * factor,
        color.blue * factor,
        color.alpha,
    ))
    .to_u8_array()
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

/// Fills one triangle into both planes, interpolating the corner colours.
///
/// Scanline: for each row the triangle reaches, the two edges it crosses give the span, and
/// only texels inside that span are visited. See the module header for why the bounding-box
/// alternative is not merely slower but asymptotically worse.
///
/// Takes the whole [`Layer`] rather than the colour slice, so that "write a texel" is one place
/// and writes both. That is `N7` expressed as a signature.
fn fill(layer: &mut Layer, corners: [Vec2; 3], colors: [[u8; 4]; 3], element: ElementId) {
    let size = layer.size;
    let area = edge(corners[0], corners[1], corners[2]);
    if area.abs() < f32::EPSILON || !area.is_finite() {
        return;
    }
    let sign = area.signum();

    let top = corners.iter().map(|c| c.y).fold(f32::INFINITY, f32::min);
    let bottom = corners
        .iter()
        .map(|c| c.y)
        .fold(f32::NEG_INFINITY, f32::max);
    let (first, last) = (first_texel(top, size.y), last_texel(bottom, size.y));

    for row in first..last {
        let y = row as f32 + 0.5;
        let Some((left, right)) = span(corners, y) else {
            continue;
        };
        let (from, to) = (first_texel(left, size.x), last_texel(right, size.x));

        for column in from..to {
            let at = Vec2::new(column as f32 + 0.5, y);
            // Recomputed per texel rather than stepped along the row. Incremental edge
            // functions are the classic optimization and they accumulate error across a long
            // span; the span is already short because the scanline bounded it.
            let weights = [
                edge(corners[1], corners[2], at) * sign,
                edge(corners[2], corners[0], at) * sign,
                edge(corners[0], corners[1], at) * sign,
            ];
            if weights.iter().any(|w| *w < 0.0) {
                continue;
            }

            let index = row as usize * size.x as usize + column as usize;
            let offset = index * BYTES_PER_TEXEL;
            let Some(texel) = layer.color.get_mut(offset..offset + BYTES_PER_TEXEL) else {
                continue;
            };
            let total = area.abs();
            for channel in 0..BYTES_PER_TEXEL {
                let blended = (0..3)
                    .map(|corner| weights[corner] * f32::from(colors[corner][channel]))
                    .sum::<f32>()
                    / total;
                texel[channel] = blended.clamp(0.0, 255.0) as u8;
            }

            // The same texel, the same iteration, no branch between them. Later triangles
            // overwrite earlier ones in both planes together, so the id always names whichever
            // element the *visible* colour came from — which is the whole property picking
            // relies on.
            if let Some(id) = layer.ids.get_mut(index) {
                *id = element;
            }
        }
    }
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
    use treepo_det::{Angle, Fx, Seed};
    use treepo_model::{NodeRole, Point, RepoPath, Segment};

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
        let layer = rasterize(&skeleton_with([]), &MaterialMap::new(), &[], region, size);
        assert_eq!(layer.color.len(), 16 * 16 * BYTES_PER_TEXEL);
        assert!(layer.color.iter().all(|byte| *byte == 0));
    }

    /// A wide vertical limb up the middle: the centre column is opaque, the corners are not.
    #[test]
    fn a_limb_covers_the_texels_it_runs_through() {
        let skeleton = skeleton_with([segment((16, 2), (16, 30), 8, 8)]);
        let (region, size) = square(32);
        let layer = rasterize(&skeleton, &MaterialMap::new(), &[0], region, size);

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
        let layer = rasterize(&skeleton, &MaterialMap::new(), &[0], region, size);

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

        let layer = rasterize(&skeleton, &MaterialMap::new(), &[0], region, size);
        let drawn = (0..32)
            .filter(|row| texel(&layer, size, 16, *row)[3] > 0)
            .count();
        assert!(drawn >= 24, "a hairline limb drew {drawn} of 28 rows");
    }

    #[test]
    fn a_segment_with_no_length_draws_nothing() {
        let skeleton = skeleton_with([segment((16, 16), (16, 16), 4, 4)]);
        let (region, size) = square(32);
        let layer = rasterize(&skeleton, &MaterialMap::new(), &[0], region, size);
        assert!(layer.color.iter().all(|byte| *byte == 0));
    }

    /// A segment index the skeleton does not have is skipped, not panicked on.
    #[test]
    fn an_index_past_the_end_is_skipped() {
        let skeleton = skeleton_with([segment((16, 2), (16, 30), 8, 8)]);
        let (region, size) = square(32);
        let layer = rasterize(&skeleton, &MaterialMap::new(), &[0, 99], region, size);
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
            &[0],
            flat,
            UVec2::new(1, 10),
        );
        assert_eq!(layer.color.len(), 10 * BYTES_PER_TEXEL);
    }

    #[test]
    fn every_family_has_a_distinct_placeholder_colour() {
        let mut seen: Vec<[f32; 4]> = Vec::new();
        for family in MaterialFamily::ALL {
            let color = family_color(family).to_f32_array();
            assert!(
                !seen.contains(&color),
                "{family:?} duplicates another family"
            );
            seen.push(color);
        }
    }

    #[test]
    fn older_material_draws_darker_than_newer() {
        let color = family_color(MaterialFamily::Heartwood);
        let new = shaded(color, 0.0);
        let old = shaded(color, 1.0);
        assert!(old[0] < new[0] && old[1] < new[1] && old[2] < new[2]);
        assert_eq!(old[3], 255, "darkening must not touch alpha");
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
        let layer = rasterize(&skeleton, &MaterialMap::new(), &[0, 1], region, size);

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
        let layer = rasterize(&skeleton, &MaterialMap::new(), &[0, 1], region, size);
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
        let layer = rasterize(&skeleton, &MaterialMap::new(), &[0, 1], region, size);
        let plane = layer.id_plane();

        // The crossing limb is rasterized second, so at the intersection both planes hold it.
        assert_eq!(plane.at(16, 16).node(), Some(NodeId::new(1)));
        assert_eq!(texel(&layer, size, 16, 16)[3], 255);
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
