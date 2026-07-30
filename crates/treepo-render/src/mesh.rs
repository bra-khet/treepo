//! Skeleton geometry to one drawable mesh.
//!
//! # This is not `bake.rs`, and the difference is the whole of RISK-B
//!
//! Architecture D5 bakes the static tree into **chunked layer textures per LOD band**, plus a
//! parallel element-ID buffer, so that Thrive's frame cost scales with visible chunks rather
//! than with the eighty thousand paths of a T3 repository. That is what makes `NFR-2` true and
//! it is what `bake.rs` and `chunk.rs` will be.
//!
//! This module is the step before it: **one triangle-list mesh for the whole tree, in one draw
//! call**, with per-vertex colour standing in for materials. It is honest about what it is —
//! a first vertical slice that proves the pipeline reaches a window — and it is deliberately
//! not on the road to the chunked bake, because a mesh per limb and a baked texture per chunk
//! are different answers to the same question. What it does share with the real thing is the
//! part that matters downstream: every triangle belongs to exactly one
//! [`NodeId`](treepo_model::NodeId), which is what `N7` will need a *pixel* to carry.
//!
//! Two consequences worth stating rather than discovering:
//!
//! * **`AC-NAV-2` (30 fps far→near on T3) is not answered here.** One mesh means the whole
//!   tree is submitted at every zoom level, which is precisely the cost LOD exists to avoid.
//! * **Memory is not yet the measured quantity RISK-B asks for.** Vertices are cheap next to
//!   baked layer textures; measuring this would be measuring the wrong thing.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use treepo_det::Fx;
use treepo_model::{MaterialFamily, MaterialMap, NodeId, Point, Skeleton};

/// The rectangle a skeleton occupies, in world units.
///
/// Used to frame the camera on load (`F-NAV-1`'s "the whole tree is the default view"). It
/// includes limb *widths* rather than just centre lines, so framing does not clip the trunk of
/// a repository whose basal mass is its widest feature.
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
        let mut extent: Option<Self> = None;
        for segment in skeleton.segments() {
            let half_base = half(segment.base_width);
            let half_tip = half(segment.tip_width);
            for (point, pad) in [(segment.start, half_base), (segment.end, half_tip)] {
                let at = world(point);
                let corner = Self {
                    min: at - Vec2::splat(pad),
                    max: at + Vec2::splat(pad),
                };
                extent = Some(match extent {
                    None => corner,
                    Some(current) => Self {
                        min: current.min.min(corner.min),
                        max: current.max.max(corner.max),
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
}

/// One mesh for the whole tree, coloured by material family and shaded by age.
///
/// Each [`Segment`](treepo_model::Segment) becomes a trapezoid — base width at the root end,
/// tip width at the far end — as two triangles. A segment with no length is skipped: it has no
/// perpendicular, so it has no quad, and drawing a degenerate triangle would put NaNs in a
/// vertex buffer.
///
/// `materials` may be shorter than the skeleton's node list, in which case the missing nodes
/// draw in the fallback colour rather than not drawing. That is a rendering decision and not a
/// tolerance for a malformed snapshot —
/// [`WorldSnapshot::is_covered`](treepo_model::WorldSnapshot::is_covered) is where a caller
/// asks whether the maps agree.
#[must_use]
pub fn tree_mesh(skeleton: &Skeleton, materials: &MaterialMap) -> Mesh {
    let segments = skeleton.segments();
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(segments.len() * 4);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(segments.len() * 4);
    let mut indices: Vec<u32> = Vec::with_capacity(segments.len() * 6);

    for segment in segments {
        let start = world(segment.start);
        let end = world(segment.end);
        let Some(along) = (end - start).try_normalize() else {
            continue;
        };
        let across = along.perp();

        let base_offset = across * half(segment.base_width);
        let tip_offset = across * half(segment.tip_width);

        let (base_color, tip_color) = segment_colors(materials, segment.node);

        // Counter-clockwise, which the 2-D pipeline does not currently care about — it culls
        // nothing — and which a custom material or a later pipeline would. Getting the winding
        // right costs the order of four lines; getting it wrong costs an invisible tree and a
        // long afternoon.
        let first = u32::try_from(positions.len()).unwrap_or(u32::MAX);
        for (corner, color) in [
            (start + base_offset, base_color),
            (start - base_offset, base_color),
            (end - tip_offset, tip_color),
            (end + tip_offset, tip_color),
        ] {
            positions.push([corner.x, corner.y, 0.0]);
            colors.push(color);
        }
        indices.extend_from_slice(&[first, first + 1, first + 2, first, first + 2, first + 3]);
    }

    // `RenderAssetUsages::default()` keeps the CPU copy as well as uploading it. The
    // render-world-only variant would halve the memory, and the reason not to take it here is
    // that it also removes the mesh Bevy computes a 2-D bounding box from — a trade worth
    // making deliberately when chunked residency arrives (RISK-B), not incidentally now.
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// The placeholder colour of one material family.
///
/// **A placeholder, and a labelled one.** `F-MAT-1`'s six families are surfaces —
/// "wood-like, crystalline, metallic, leafy, dusty" — and a surface is a shader and a tile
/// atlas, not a hex value. What these six do is make the families *distinguishable* so that
/// the wiring from `materialize` to a pixel can be seen to work. They are not tuned, they have
/// not been through the perceptual-separation check `AC-MAT-4` applies to the author palette,
/// and they are not in `assets/palettes/` for exactly that reason: a file there would look
/// like a decision.
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

/// One segment's base and tip colours: its family, shaded by its age gradient.
fn segment_colors(materials: &MaterialMap, node: NodeId) -> ([f32; 4], [f32; 4]) {
    let Some(material) = materials.get(node) else {
        return (UNMATERIALIZED.to_f32_array(), UNMATERIALIZED.to_f32_array());
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

/// A colour darkened by a normalized age in `0..=1`.
fn shaded(color: LinearRgba, age: f32) -> [f32; 4] {
    let factor = 1.0 - AGE_DARKENING * age.clamp(0.0, 1.0);
    LinearRgba::new(
        color.red * factor,
        color.green * factor,
        color.blue * factor,
        color.alpha,
    )
    .to_f32_array()
}

/// A skeleton point in Bevy world units.
///
/// The skeleton's own units are whatever the parameter table's lengths are, and the camera
/// frames the extent rather than assuming a scale — so there is no conversion factor here to
/// get wrong, and adding one would be a second place for the picture and the picking to
/// disagree about where a limb is.
fn world(point: Point) -> Vec2 {
    Vec2::new(point.x.to_f64() as f32, point.y.to_f64() as f32)
}

/// Half a fixed-point width, in world units.
fn half(width: Fx) -> f32 {
    (width.to_f64() as f32) * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;
    use treepo_det::{Angle, Seed};
    use treepo_model::{NodeRole, RepoPath, Segment};

    fn skeleton_with(segments: impl IntoIterator<Item = Segment>) -> Skeleton {
        let mut skeleton = Skeleton::new();
        skeleton.push_node(
            None,
            Point::ORIGIN,
            Angle::ZERO,
            Seed::root(b"render-test"),
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

    #[test]
    fn a_skeleton_that_draws_nothing_has_no_extent() {
        assert!(Extent::of(&skeleton_with([])).is_none());
    }

    /// The reason the extent includes widths: a trunk two units wide reaches one unit either
    /// side of its centre line, and framing on centre lines alone would clip it.
    #[test]
    fn the_extent_includes_limb_width() {
        let extent = Extent::of(&skeleton_with([segment((0, 0), (0, 10), 4, 2)])).unwrap();
        assert_eq!(extent.min.x, -2.0);
        assert_eq!(extent.max.x, 2.0);
        assert_eq!(extent.min.y, -2.0);
        assert_eq!(extent.max.y, 11.0);
    }

    #[test]
    fn each_segment_becomes_two_triangles() {
        let skeleton = skeleton_with([
            segment((0, 0), (0, 10), 2, 1),
            segment((0, 10), (5, 15), 1, 1),
        ]);
        let mesh = tree_mesh(&skeleton, &MaterialMap::new());
        assert_eq!(mesh.count_vertices(), 8);
        assert_eq!(mesh.indices().map(Indices::len), Some(12));
    }

    /// A zero-length segment has no direction, so it has no quad. Emitting one anyway would
    /// put a NaN normal into a vertex buffer and take the whole mesh with it.
    #[test]
    fn a_segment_with_no_length_is_skipped_rather_than_drawn() {
        let skeleton = skeleton_with([segment((3, 3), (3, 3), 2, 2)]);
        let mesh = tree_mesh(&skeleton, &MaterialMap::new());
        assert_eq!(mesh.count_vertices(), 0);
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
    }
}
