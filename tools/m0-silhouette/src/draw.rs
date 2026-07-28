//! Turning a [`Skeleton`] into pixels: what fits on the canvas, and what colour it is.
//!
//! # Lines and thickness only
//!
//! `PRD §4` says M0 is "a debug renderer drawing lines and thickness only", and this holds to
//! that. There is no material, no ownership, no enrichment, no lighting — the four generative
//! layers `design/visual-construction.md` defines, minus three. What it *does* add is four
//! ink families, which is not decoration: `NodeRole` already distinguishes a limb from a
//! group stem from an aggregate container from a root boulder, and drawing all four in one
//! colour would make `F-SKEL-7`'s containers and `F2`'s grouping invisible in the exact
//! picture built to check they happen.
//!
//! # The view is fitted per repository, deliberately
//!
//! Every tree is scaled to fill its own frame. That makes absolute size incomparable between
//! images and it is still right: `AC-SKEL-1` compares *shape* between two repositories of
//! similar size, and a fixed scale would render T0 as a speck and T3 as a smear. The world
//! extent is printed alongside each image so the scale is recoverable when it matters.

use treepo_det::Fx;
use treepo_model::{NodeId, NodeRole, Point, Skeleton};

use crate::canvas::{Canvas, FAMILIES, LEVELS, SUB};

/// An ordinary limb: one repository path drawn as itself.
const LIMB: u8 = 0;
/// The basal axiom and `F2`'s group stems — structure that carries other structure.
const STEM: u8 = 1;
/// The root-boulder cluster (`AC-SKEL-2`).
const ROOT: u8 = 2;
/// An `F-SKEL-7` container standing for content it does not draw.
const AGGREGATE: u8 = 3;

/// Paper. Light rather than dark on purpose — a silhouette judged against white is the
/// register the botanical reference in `design/visual-construction.md` works in.
const BACKGROUND: [u8; 3] = [0xF4, 0xF1, 0xEA];

/// Full-coverage ink per family, in family order — which is also draw order.
const INK: [[u8; 3]; FAMILIES] = [
    [0x3A, 0x34, 0x2C], // limb — bark
    [0x1B, 0x16, 0x11], // stem — heavier, so the trunk reads as the trunk
    [0x6E, 0x7A, 0x8A], // root — stone
    [0xB4, 0x56, 0x2D], // aggregate — terracotta, the one colour that is not wood
];

/// The thinnest a stroke may be drawn, in sub-units: a radius of half a pixel.
///
/// Past `A3`'s recursion cap a twig's real width is well under a pixel at any canvas size
/// worth waiting for. Letting it fade out would be more honest about thickness and would
/// hide the topology this picture exists to show, so the floor stays and says so here.
const MIN_RADIUS: i64 = SUB / 2;

/// The blank border kept around the drawing, in pixels.
const MARGIN: u32 = 24;

/// The 256-entry palette: four families, 64 coverage levels each.
#[must_use]
pub(crate) fn palette() -> [[u8; 3]; 256] {
    let mut out = [BACKGROUND; 256];
    for (family, ink) in INK.iter().enumerate() {
        for level in 0..=LEVELS {
            let mut entry = BACKGROUND;
            for (channel, value) in entry.iter_mut().enumerate() {
                let from = i64::from(BACKGROUND[channel]);
                let to = i64::from(ink[channel]);
                *value = (from + (to - from) * level / LEVELS) as u8;
            }
            out[family * 64 + level as usize] = entry;
        }
    }
    out
}

/// Where the skeleton sits on the canvas.
#[derive(Debug, Clone, Copy)]
pub(crate) struct View {
    /// World-space extent, as `(min_x, min_y, max_x, max_y)`. Reported, so a fitted image's
    /// scale can be recovered.
    pub(crate) extent: (Fx, Fx, Fx, Fx),
    /// Sub-units per world unit.
    scale: Fx,
    origin: (Fx, Fx),
    offset: (i64, i64),
}

impl View {
    /// A world point in canvas sub-units.
    ///
    /// The y flip lives here and nowhere else. `SkeletonNode::heading` documents zero as up
    /// and the turtle advances `+y` upward; a canvas counts rows downward.
    fn map(&self, point: Point) -> (i64, i64) {
        (
            point.x.sub(self.origin.0).mul(self.scale).round() + self.offset.0,
            self.origin.1.sub(point.y).mul(self.scale).round() + self.offset.1,
        )
    }

    /// A world width as a stroke radius in sub-units, floored at [`MIN_RADIUS`].
    fn radius(&self, width: Fx) -> i64 {
        width.scale(1, 2).mul(self.scale).round().max(MIN_RADIUS)
    }
}

/// Fits `skeleton` to a `size × size` canvas, uniformly scaled and centred.
#[must_use]
pub(crate) fn fit(skeleton: &Skeleton, size: u32) -> View {
    let mut extent = (Fx::MAX, Fx::MAX, Fx::MIN, Fx::MIN);
    let mut seen = false;
    for segment in skeleton.segments() {
        for (point, width) in [
            (segment.start, segment.base_width),
            (segment.end, segment.tip_width),
        ] {
            let reach = width.scale(1, 2);
            extent.0 = extent.0.min(point.x.sub(reach));
            extent.1 = extent.1.min(point.y.sub(reach));
            extent.2 = extent.2.max(point.x.add(reach));
            extent.3 = extent.3.max(point.y.add(reach));
            seen = true;
        }
    }
    if !seen {
        // A skeleton with no geometry at all. Nothing produces one today — even an empty
        // repository has a basal segment and a root cluster — but a view is a total function.
        extent = (Fx::ZERO, Fx::ZERO, Fx::ONE, Fx::ONE);
    }

    let usable = i32::try_from(size.saturating_sub(2 * MARGIN).max(1) * SUB as u32)
        .expect("a canvas this large would not fit in memory either");
    let span_x = extent.2.sub(extent.0);
    let span_y = extent.3.sub(extent.1);

    // The cap keeps a degenerate skeleton — one point, or a hairline — from asking for a
    // scale that saturates `Fx` and folds the whole drawing into a corner.
    let cap = Fx::from_int(i32::try_from(size * SUB as u32).unwrap_or(i32::MAX));
    let scale = match (
        Fx::from_int(usable).checked_div(span_x),
        Fx::from_int(usable).checked_div(span_y),
    ) {
        (Some(x), Some(y)) => x.min(y),
        (Some(x), None) => x,
        (None, Some(y)) => y,
        (None, None) => cap,
    }
    .min(cap)
    .max(Fx::from_bits(1));

    let full = i64::from(size) * SUB;
    View {
        extent,
        scale,
        origin: (extent.0, extent.3),
        offset: (
            (full - span_x.mul(scale).round()) / 2,
            (full - span_y.mul(scale).round()) / 2,
        ),
    }
}

/// Draws every segment of `skeleton` onto a fresh `size × size` canvas.
#[must_use]
pub(crate) fn draw(skeleton: &Skeleton, size: u32) -> (Canvas, View) {
    let view = fit(skeleton, size);
    let mut canvas = Canvas::new(size, size);

    // Family order is draw order, and `Canvas::stroke` gives ties to whatever was drawn last,
    // so this is what keeps an aggregate container legible where limbs cross it. Sorting
    // rather than four passes over the segment list: one pass, and the order is stated once.
    let mut order: Vec<usize> = (0..skeleton.segments().len()).collect();
    order.sort_by_key(|&index| family_of(skeleton, skeleton.segments()[index].node));

    for index in order {
        let segment = &skeleton.segments()[index];
        canvas.stroke(
            view.map(segment.start),
            view.map(segment.end),
            view.radius(segment.base_width),
            view.radius(segment.tip_width),
            family_of(skeleton, segment.node),
        );
    }

    mark_containers(skeleton, &view, &mut canvas);
    (canvas, view)
}

/// Marks every aggregate container, which is the one kind of node that carries no geometry.
///
/// `F-SKEL-7`'s container is a *node* — identity, position, heading, seed, and the full
/// member list for `F-INSP-3` — and its visual **form** is deliberately Phase 4's, so
/// `compose` gives it no segments. That is right for the library and wrong for a picture
/// whose whole job is showing where content was collapsed: an unmarked container is
/// indistinguishable from the truncation `P6` forbids, in exactly the view built to check it
/// is not one.
///
/// So the debug view marks it, and invents nothing while doing so: the mark is a disc as wide
/// as the branch that ends there, a width the skeleton already stated. A container is not
/// claimed to be any particular size, only to be *somewhere* and to carry *something*.
fn mark_containers(skeleton: &Skeleton, view: &View, canvas: &mut Canvas) {
    for node in skeleton.nodes() {
        if !node.role.is_aggregate() {
            continue;
        }

        // The segment this container hangs from: its origin is a tip the parent's turtle
        // produced, so the match is exact rather than nearest.
        let width = skeleton
            .segments()
            .iter()
            .find(|segment| Some(segment.node) == node.parent && segment.end == node.origin)
            .map_or(MIN_RADIUS, |segment| view.radius(segment.tip_width));

        let at = view.map(node.origin);
        canvas.stroke(at, at, width, width, AGGREGATE);
    }
}

/// Which ink family a segment's owning node belongs to.
///
/// The basal axiom is a `Limb` over the repository root — `trunk::grow` gives it the root
/// path because `F-INSP-4` has to resolve it to something — so it is identified the one way
/// that cannot be confused with an ordinary limb: it is the node with no parent.
fn family_of(skeleton: &Skeleton, node: NodeId) -> u8 {
    match skeleton.node(node) {
        Some(node) => match &node.role {
            NodeRole::Aggregate(_) => AGGREGATE,
            NodeRole::RootMass { .. } => ROOT,
            NodeRole::Group { .. } => STEM,
            NodeRole::Limb { .. } if node.parent.is_none() => STEM,
            NodeRole::Limb { .. } => LIMB,
        },
        None => LIMB,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use treepo_det::{Angle, Seed};
    use treepo_gen::{Table, grow};
    use treepo_model::{AggregateNode, Manifest, NodeKind, PathRecord, RepoPath, Segment};

    /// Skeletons here are built by hand rather than grown.
    ///
    /// This module's job is `Skeleton → pixels`, and a hand-built skeleton states the input
    /// to that in the test itself: four roles, known coordinates, a known extent. Growing one
    /// instead would make every assertion depend on the parameter table — so tuning
    /// `lsystem.ron`, which is the entire point of the milestone, would start breaking the
    /// rasterizer's tests. Whether a *real* repository looks right is judged by eye, from the
    /// tool's own output. That is not something a unit test can hold.
    fn segment(node: NodeId, from: (i32, i32), to: (i32, i32), base: i32, tip: i32) -> Segment {
        Segment {
            start: Point::new(Fx::from_int(from.0), Fx::from_int(from.1)),
            end: Point::new(Fx::from_int(to.0), Fx::from_int(to.1)),
            base_width: Fx::from_int(base),
            tip_width: Fx::from_int(tip),
            node,
            generation: 0,
        }
    }

    /// A tree with one of everything: basal stem, group stem, limbs, a container, and roots.
    fn one_of_each() -> Skeleton {
        let mut skeleton = Skeleton::new();
        let root = RepoPath::root();
        let src = RepoPath::new(b"src").unwrap();
        let seed = Seed::root(b"m0-draw-test");

        let basal = skeleton.push_node(
            None,
            Point::ORIGIN,
            Angle::ZERO,
            seed,
            NodeRole::Limb { path: root.clone() },
        );
        skeleton.extend_segments([segment(basal, (0, 0), (0, 20), 8, 7)]);

        let boulder = skeleton.push_node(
            Some(basal),
            Point::ORIGIN,
            Angle::ZERO,
            seed.derive(b"root-mass"),
            NodeRole::RootMass {
                anchor: root.clone(),
                index: 0,
            },
        );
        skeleton.extend_segments([segment(boulder, (0, 0), (-14, -8), 6, 4)]);

        let group = skeleton.push_node(
            Some(basal),
            Point::new(Fx::from_int(0), Fx::from_int(20)),
            Angle::ZERO,
            seed.derive(b"group"),
            NodeRole::Group {
                anchor: root,
                members: vec![src.clone()],
            },
        );
        skeleton.extend_segments([segment(group, (0, 20), (18, 44), 6, 4)]);

        let limb = skeleton.push_node(
            Some(group),
            Point::new(Fx::from_int(18), Fx::from_int(44)),
            Angle::ZERO,
            seed.derive(b"limb"),
            NodeRole::Limb { path: src.clone() },
        );
        skeleton.extend_segments([segment(limb, (18, 44), (34, 70), 3, 1)]);

        let container = skeleton.push_node(
            Some(group),
            Point::new(Fx::from_int(0), Fx::from_int(20)),
            Angle::ZERO,
            seed.derive(b"aggregate"),
            NodeRole::Aggregate(AggregateNode {
                anchor: src.clone(),
                index: 0,
                members: vec![src],
                bytes: 4_096,
                file_count: 12,
                dir_count: 2,
            }),
        );
        skeleton.extend_segments([segment(container, (0, 20), (-26, 52), 5, 5)]);

        skeleton
    }

    /// Every level of the ramp must be distinct or the anti-aliasing quantizes into bands,
    /// and the two ends must be exactly background and exactly ink.
    #[test]
    fn the_palette_ramps_from_background_to_ink() {
        let palette = palette();
        for family in 0..FAMILIES {
            assert_eq!(palette[family * 64], BACKGROUND, "family {family} at zero");
            assert_eq!(
                palette[family * 64 + LEVELS as usize],
                INK[family],
                "family {family} at full"
            );
        }

        // Unused entries — coverage above LEVELS within a family — stay background rather
        // than being left uninitialized, so a packing bug shows as a hole, not as noise.
        assert_eq!(palette[LEVELS as usize + 1], BACKGROUND);
    }

    /// The fit must put the whole tree inside the frame. A skeleton that overflowed would be
    /// silently cropped, and a cropped silhouette is worse than no silhouette: it looks
    /// plausible and is wrong.
    #[test]
    fn every_segment_lands_inside_the_canvas() {
        let skeleton = one_of_each();
        let size = 512u32;
        let view = fit(&skeleton, size);
        let full = i64::from(size) * SUB;

        for segment in skeleton.segments() {
            for (point, width) in [
                (segment.start, segment.base_width),
                (segment.end, segment.tip_width),
            ] {
                let (x, y) = view.map(point);
                let r = view.radius(width);
                assert!(
                    x - r >= 0 && x + r <= full && y - r >= 0 && y + r <= full,
                    "a segment left the frame at ({x}, {y}) with radius {r}"
                );
            }
        }
    }

    /// Up must be up. The turtle advances `+y` upward and a canvas counts rows downward, so
    /// a missing flip draws every tree upside down — which looks enough like a tree that it
    /// could survive a glance.
    #[test]
    fn the_canopy_is_above_the_roots_on_the_canvas() {
        let skeleton = one_of_each();
        let view = fit(&skeleton, 256);

        let root_y = skeleton
            .nodes()
            .iter()
            .filter(|node| matches!(node.role, NodeRole::RootMass { .. }))
            .map(|node| view.map(node.origin).1)
            .max()
            .expect("every tree has a root cluster");
        let canopy_y = skeleton
            .segments()
            .iter()
            .map(|segment| view.map(segment.end).1)
            .min()
            .expect("every tree has segments");

        assert!(
            canopy_y < root_y,
            "canopy at row {canopy_y} is not above the roots at row {root_y}"
        );
    }

    /// How much ink of each family reached the canvas.
    fn ink(skeleton: &Skeleton, size: u32) -> [usize; FAMILIES] {
        let mut per_family = [0usize; FAMILIES];
        for index in draw(skeleton, size).0.into_indices() {
            if index & 63 > 0 {
                per_family[usize::from(index >> 6)] += 1;
            }
        }
        per_family
    }

    /// The four families exist so the picture can be read. If a tree with containers drew no
    /// container ink, `F-SKEL-7` would be untestable by eye — which is the one thing this
    /// tool is for. Sabotaged by collapsing `family_of` to a constant: three of these fail.
    #[test]
    fn each_role_reaches_the_canvas_in_its_own_ink() {
        let drawn = ink(&one_of_each(), 256);
        for (family, name) in [
            (LIMB, "limb"),
            (STEM, "stem"),
            (ROOT, "root"),
            (AGGREGATE, "aggregate"),
        ] {
            assert!(
                drawn[usize::from(family)] > 0,
                "no {name} ink on the canvas"
            );
        }
    }

    /// `AC-SKEL-2` by eye, through the real pipeline: an empty repository is a seed in its
    /// roots, not a lonely trunk. There must be root ink and no canopy at all.
    #[test]
    fn an_empty_repository_draws_as_roots_and_a_stub() {
        let mut manifest = Manifest::new("m0-silhouette".to_string(), Seed::root(b"empty"));
        manifest.set_paths(vec![PathRecord::new(RepoPath::root(), NodeKind::Directory)]);

        let drawn = ink(&grow(&manifest, &Table::built_in()), 256);
        assert!(drawn[usize::from(ROOT)] > 0, "no root cluster drawn");
        assert_eq!(drawn[usize::from(LIMB)], 0, "an empty tree grew limbs");
        assert_eq!(
            drawn[usize::from(AGGREGATE)],
            0,
            "an empty tree grew containers"
        );
    }

    /// `F-SKEL-7` through the real pipeline, which is the only way this claim means anything.
    ///
    /// The hand-built fixture above gives its container a segment and so cannot catch the
    /// thing that actually happens: `compose` pushes an aggregate node and no geometry, by
    /// design, and the container is invisible unless the renderer marks it. That gap survived
    /// a passing test once already — hence this one, grown rather than assembled.
    #[test]
    fn a_grown_container_is_marked_even_though_it_has_no_geometry() {
        // Wide and flat: far more children than any `branch_capacity` can hold is what drives
        // composition past its capacity and into containers.
        let mut records = vec![
            PathRecord::new(RepoPath::root(), NodeKind::Directory),
            PathRecord::new(RepoPath::new(b"src").unwrap(), NodeKind::Directory),
        ];
        for index in 0..40u32 {
            let path = RepoPath::new(format!("src/mod{index:02}.rs").as_bytes()).unwrap();
            let mut record = PathRecord::new(path, NodeKind::File);
            record.size.bytes = u64::from(1_000 + index);
            records.push(record);
        }

        let mut manifest = Manifest::new("m0-silhouette".to_string(), Seed::root(b"wide"));
        manifest.set_paths(records);
        let skeleton = grow(&manifest, &Table::built_in());

        assert!(
            skeleton.aggregate_count() > 0,
            "this fixture was meant to aggregate"
        );
        assert!(
            !skeleton.segments().iter().any(|segment| skeleton
                .node(segment.node)
                .is_some_and(|n| n.role.is_aggregate())),
            "an aggregate grew geometry — its visual form is Phase 4's, and if that changed \
             the marker in `mark_containers` is now drawing over it"
        );

        assert!(
            ink(&skeleton, 256)[usize::from(AGGREGATE)] > 0,
            "a collapsed region left no mark, which is what a truncation looks like"
        );
    }

    /// The same skeleton must produce the same bytes. This is the half of `AC-DET-2` the
    /// tool itself is responsible for: the rasterizer may not introduce drift of its own.
    #[test]
    fn drawing_the_same_skeleton_twice_produces_the_same_pixels() {
        let skeleton = one_of_each();
        let first = draw(&skeleton, 128).0.into_indices();
        let second = draw(&skeleton, 128).0.into_indices();
        assert_eq!(first, second);
    }
}
