//! Which element is under a point — `F-INSP-1`, the placeholder for `id_buffer.rs`.
//!
//! # This is not the ID buffer, and the difference is `N7`
//!
//! Architecture D5 answers "what did I click" by rasterizing an **element-ID buffer** in
//! parallel with the baked layers and sampling it. That answer is correct *by construction* —
//! the id under the cursor is the id of whatever painted that pixel, so the click and the
//! picture cannot disagree — and it is what makes `P1` machine-checkable, because
//! `xtask id-coverage` can then scan for a coloured pixel with no id.
//!
//! What is here instead is a geometric hit test against the same segments the mesh was built
//! from. It gives `AC-INSP-1`'s guarantee — every click resolves to a real path or an explicit
//! aggregate — for the vertical slice, and it has one honest weakness the buffer does not: the
//! *drawn* answer and the *computed* answer are two calculations, so they can drift. Where two
//! limbs overlap, this returns the nearer centre line while the picture shows whichever
//! triangle was drawn last. Both name a real element, so no click lands on nothing; they can
//! simply name different real elements.
//!
//! It is therefore a placeholder with a stated defect rather than an implementation of `N7`,
//! and `xtask id-coverage` stays unimplemented until the buffer it scans exists.

use bevy::prelude::*;
use treepo_model::{NodeId, Skeleton};

/// The node whose drawn geometry is nearest `at`, within its own width plus `tolerance`.
///
/// `tolerance` is in world units and exists so that a thin twig can be clicked at all: at far
/// zoom a limb may be a fraction of a world unit wide while a pixel is several, and requiring
/// the click to land inside the limb's true width would make most of the tree unclickable
/// precisely where the user can see the least.
///
/// Returns `None` for a click on empty space, which is a deselection rather than a failure.
#[must_use]
pub fn pick_node(skeleton: &Skeleton, at: Vec2, tolerance: f32) -> Option<NodeId> {
    let mut best: Option<(f32, NodeId)> = None;

    for segment in skeleton.segments() {
        let start = point(segment.start);
        let end = point(segment.end);
        let (distance, along) = distance_to_segment(at, start, end);

        // The limb tapers, so the width that decides a hit is the width *where the click
        // landed* — testing against the base width would make every limb clickable out to its
        // thickest point, which on a trunk is a large piece of empty sky.
        let half_width = 0.5
            * lerp(
                segment.base_width.to_f64() as f32,
                segment.tip_width.to_f64() as f32,
                along,
            );
        if distance > half_width + tolerance {
            continue;
        }
        if best.is_none_or(|(closest, _)| distance < closest) {
            best = Some((distance, segment.node));
        }
    }

    best.map(|(_, node)| node)
}

/// Distance from `at` to the segment `start..end`, and how far along it the nearest point lies.
///
/// `along` is clamped to `0..=1`, so a point past either end measures to that end rather than
/// to the infinite line — which is the difference between a click beyond a limb's tip missing
/// it and hitting it from any distance.
fn distance_to_segment(at: Vec2, start: Vec2, end: Vec2) -> (f32, f32) {
    let span = end - start;
    let length_squared = span.length_squared();
    if length_squared <= 0.0 {
        return (at.distance(start), 0.0);
    }
    let along = ((at - start).dot(span) / length_squared).clamp(0.0, 1.0);
    (at.distance(start + span * along), along)
}

/// Linear interpolation, because `f32::lerp` is not stable in the pinned toolchain.
fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

/// A skeleton point in world units — the same conversion the mesh uses.
fn point(value: treepo_model::Point) -> Vec2 {
    Vec2::new(value.x.to_f64() as f32, value.y.to_f64() as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use treepo_det::{Angle, Fx, Seed};
    use treepo_model::{NodeRole, Point, RepoPath, Segment};

    /// A skeleton with two nodes and one segment on each: a vertical trunk from the origin,
    /// and a horizontal limb well away from it.
    fn two_limbs() -> Skeleton {
        let mut skeleton = Skeleton::new();
        for name in ["trunk", "limb"] {
            skeleton.push_node(
                None,
                Point::ORIGIN,
                Angle::ZERO,
                Seed::root(b"pick-test"),
                NodeRole::Limb {
                    path: RepoPath::new(name.as_bytes()).unwrap(),
                },
            );
        }
        skeleton.extend_segments([
            segment((0, 0), (0, 100), 10, 10, 0),
            segment((50, 50), (150, 50), 4, 4, 1),
        ]);
        skeleton
    }

    fn segment(start: (i32, i32), end: (i32, i32), base: i32, tip: i32, node: u32) -> Segment {
        Segment {
            start: Point::new(Fx::from_int(start.0), Fx::from_int(start.1)),
            end: Point::new(Fx::from_int(end.0), Fx::from_int(end.1)),
            base_width: Fx::from_int(base),
            tip_width: Fx::from_int(tip),
            node: NodeId::new(node),
            generation: 0,
        }
    }

    #[test]
    fn a_click_on_a_limb_finds_it() {
        let skeleton = two_limbs();
        assert_eq!(
            pick_node(&skeleton, Vec2::new(0.0, 40.0), 0.0),
            Some(NodeId::new(0))
        );
        assert_eq!(
            pick_node(&skeleton, Vec2::new(100.0, 50.0), 0.0),
            Some(NodeId::new(1))
        );
    }

    #[test]
    fn a_click_on_empty_space_finds_nothing() {
        assert_eq!(pick_node(&two_limbs(), Vec2::new(400.0, 400.0), 0.0), None);
    }

    /// The reason `along` is clamped: a click past the tip is off the end of the limb, not on
    /// the line the limb happens to lie along.
    #[test]
    fn a_click_past_the_tip_misses() {
        let skeleton = two_limbs();
        assert_eq!(pick_node(&skeleton, Vec2::new(0.0, 140.0), 0.0), None);
        // …and is found again once the tolerance reaches it.
        assert_eq!(
            pick_node(&skeleton, Vec2::new(0.0, 140.0), 45.0),
            Some(NodeId::new(0))
        );
    }

    /// A tapering limb is only as clickable as it is wide at the point clicked.
    #[test]
    fn a_taper_narrows_what_can_be_clicked() {
        let mut skeleton = Skeleton::new();
        skeleton.push_node(
            None,
            Point::ORIGIN,
            Angle::ZERO,
            Seed::root(b"pick-test"),
            NodeRole::Limb {
                path: RepoPath::root(),
            },
        );
        skeleton.extend_segments([segment((0, 0), (0, 100), 40, 2, 0)]);

        // Eight units off the centre line: inside the twenty-unit half-width at the base…
        assert!(pick_node(&skeleton, Vec2::new(8.0, 2.0), 0.0).is_some());
        // …and outside the one-unit half-width at the tip.
        assert!(pick_node(&skeleton, Vec2::new(8.0, 98.0), 0.0).is_none());
    }

    #[test]
    fn tolerance_makes_a_thin_limb_clickable_at_far_zoom() {
        let skeleton = two_limbs();
        let just_off = Vec2::new(100.0, 56.0);
        assert_eq!(pick_node(&skeleton, just_off, 0.0), None);
        assert_eq!(pick_node(&skeleton, just_off, 6.0), Some(NodeId::new(1)));
    }
}
