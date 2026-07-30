//! ★ The element-ID plane — `N7`, `P1`, and what makes them machine-checkable.
//!
//! > `N7` appearance from primitives only — every baked pixel carries an element ID in a
//! > parallel ID buffer. **A pixel with color and no ID is an unaccountable pixel.**
//!
//! Architecture D5 rasterizes a `u32` per texel alongside the colour, and answers "what did I
//! click" by sampling it. The property that buys is not accuracy, it is **agreement**: the id
//! under the cursor is the id of whatever painted that texel, so the click and the picture
//! cannot disagree by construction. `pick.rs`, which this module replaces, computed the answer
//! a second time from the same segments — correct nearly always, and free to drift from the
//! drawn answer wherever two limbs overlapped.
//!
//! # Two planes, two homes, and the split is deliberate
//!
//! Colour goes to the GPU and is dropped from main memory ([`LAYER_USAGE`] is
//! `RENDER_WORLD`). Ids stay on the CPU and never reach the GPU at all. That is not an
//! oversight: the two planes are read by different things — the GPU draws the colour, and the
//! CPU answers clicks — and shipping the id plane to the GPU would pay for an upload nothing
//! samples. It does mean a texel now costs eight bytes rather than four, which is why
//! [`RESIDENT_TEXEL_BUDGET`] is written in *texels* and its note says what the two planes
//! together come to.
//!
//! [`LAYER_USAGE`]: crate::chunk::LAYER_USAGE
//! [`RESIDENT_TEXEL_BUDGET`]: crate::chunk::RESIDENT_TEXEL_BUDGET
//!
//! # Why a search radius rather than an exact texel
//!
//! A limb one texel wide is one texel wide on screen — that is what an LOD band *is* — so
//! requiring the click to land on a painted texel would ask the user to hit a single pixel.
//! [`pick`] therefore searches a small neighbourhood and takes the nearest painted texel.
//!
//! The pleasant consequence of bands being chosen at roughly one texel per screen pixel:
//! **a radius in texels is a radius in screen pixels**, at every zoom level, with no conversion
//! and no camera in the signature. [`SEARCH_RADIUS_TEXELS`] is the whole of the old
//! `CLICK_RADIUS_PIXELS` tolerance, and it needs no projection to stay true.

use bevy::prelude::*;
use treepo_model::{NodeId, Skeleton};

use crate::chunk::Extent;

/// Which element painted a texel, or that none did.
///
/// A `u32` rather than an `Option<NodeId>` because that is D5's word — "u32 per pixel" — and
/// because the niche matters at this size: an `Option<NodeId>` is eight bytes, which would
/// double the plane to buy a sentinel the plane can express for free.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct ElementId(u32);

impl ElementId {
    /// No element painted this texel.
    ///
    /// `u32::MAX` rather than zero, because zero is [`NodeId::new(0)`](NodeId::new) — the basal
    /// node, which is a real element and the one at the bottom of every tree. A sentinel of
    /// zero would make the trunk unaccountable and the gate that checks for it would pass.
    pub const NONE: Self = Self(u32::MAX);

    /// The id of a node.
    #[must_use]
    pub const fn of(node: NodeId) -> Self {
        Self(node.index() as u32)
    }

    /// The node this id names, or `None` for an unpainted texel.
    #[must_use]
    pub const fn node(self) -> Option<NodeId> {
        if self.0 == Self::NONE.0 {
            None
        } else {
            Some(NodeId::new(self.0))
        }
    }

    /// Whether nothing painted this texel.
    #[must_use]
    pub const fn is_none(self) -> bool {
        self.0 == Self::NONE.0
    }

    /// The raw value, for a scan that wants to compare without unwrapping.
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

impl Default for ElementId {
    fn default() -> Self {
        Self::NONE
    }
}

/// One baked piece's ID plane, texel-for-texel with its colour texture.
///
/// A component rather than an asset: it never goes to the GPU, and `Assets<T>` exists to manage
/// things that do. It rides on the same entity as the [`Sprite`], so the plane and the picture
/// it explains are despawned together — a plane that outlived its texture would answer clicks
/// about a limb that is no longer drawn.
#[derive(Component, Debug, Clone)]
pub struct IdPlane {
    size: UVec2,
    ids: Vec<ElementId>,
}

impl IdPlane {
    /// Wraps a rasterized plane, or an empty one if the length disagrees with the size.
    ///
    /// The disagreement cannot happen from [`rasterize`](crate::bake::rasterize), which sizes
    /// both planes from one number. It is rejected rather than trusted because every read below
    /// indexes with arithmetic, and a plane one row short would answer clicks with whatever the
    /// next row holds — a wrong path, confidently.
    #[must_use]
    pub fn new(size: UVec2, ids: Vec<ElementId>) -> Self {
        if ids.len() == size.x as usize * size.y as usize {
            Self { size, ids }
        } else {
            Self {
                size: UVec2::ZERO,
                ids: Vec::new(),
            }
        }
    }

    /// The plane's resolution.
    #[must_use]
    pub fn size(&self) -> UVec2 {
        self.size
    }

    /// Every id, row-major from the top-left — the same order the colour plane is written in.
    #[must_use]
    pub fn ids(&self) -> &[ElementId] {
        &self.ids
    }

    /// Whether the plane holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// The id at a texel, or [`ElementId::NONE`] outside the plane.
    #[must_use]
    pub fn at(&self, column: u32, row: u32) -> ElementId {
        if column >= self.size.x || row >= self.size.y {
            return ElementId::NONE;
        }
        self.ids
            .get(row as usize * self.size.x as usize + column as usize)
            .copied()
            .unwrap_or(ElementId::NONE)
    }
}

/// How far a click may miss a limb and still find it, in texels.
///
/// Six, matching the six logical pixels the geometric picker used — and meaning the same thing
/// without a conversion, because an LOD band is chosen at roughly one texel per screen pixel.
/// A click that lands exactly on a painted texel never consults this; it is what makes a
/// hairline twig clickable at the far band, where the limb is one texel wide and a user cannot
/// be asked to hit it exactly.
pub const SEARCH_RADIUS_TEXELS: u32 = 6;

/// One resident piece, as picking sees it.
#[derive(Debug, Clone, Copy)]
pub struct Painted<'a> {
    /// The world rectangle the plane covers.
    pub region: Extent,
    /// The plane itself.
    pub plane: &'a IdPlane,
}

/// The element painted nearest a world point, within [`SEARCH_RADIUS_TEXELS`].
///
/// `None` is a click on the background, which is a deselection rather than a failure.
///
/// # Nearest in *world* units, searched in *texels*
///
/// The search window is a texel radius, so it is a constant on screen. The comparison between
/// candidates from different pieces is in world units, so it stays correct when two pieces are
/// at different bands — which happens for the frame or two after a zoom crosses a boundary,
/// while `chunk::stream` still holds the previous band's layers. Comparing texel distances
/// there would let a coarse layer win by arithmetic rather than by being nearer.
#[must_use]
pub fn pick<'a>(pieces: impl IntoIterator<Item = Painted<'a>>, at: Vec2) -> Option<NodeId> {
    let mut best: Option<(f32, NodeId)> = None;

    for piece in pieces {
        let span = piece.region.size();
        let size = piece.plane.size();
        if piece.plane.is_empty() || span.x <= 0.0 || span.y <= 0.0 {
            continue;
        }

        // World to texel, the same mapping `bake::Projector` rasterized through: row zero is
        // the top of the region, because image rows run down and world `y` runs up.
        let per_texel = span / size.as_vec2();
        let column = (at.x - piece.region.min.x) / per_texel.x;
        let row = (piece.region.max.y - at.y) / per_texel.y;
        if !column.is_finite() || !row.is_finite() {
            continue;
        }

        let radius = SEARCH_RADIUS_TEXELS as f32;
        // The window may sit entirely outside this piece, which is the common case once a
        // handful are resident — rejecting it here costs one comparison per piece instead of
        // a hundred and sixty-nine misses.
        if column < -radius
            || row < -radius
            || column > size.x as f32 + radius
            || row > size.y as f32 + radius
        {
            continue;
        }

        for step_row in window(row, size.y) {
            for step_column in window(column, size.x) {
                let Some(node) = piece.plane.at(step_column, step_row).node() else {
                    continue;
                };
                // The texel's centre, so the distance to a hit is measured from where the
                // texel actually is rather than from its corner.
                let offset = Vec2::new(
                    (step_column as f32 + 0.5 - column) * per_texel.x,
                    (step_row as f32 + 0.5 - row) * per_texel.y,
                );
                let distance = offset.length_squared();
                if best.is_none_or(|(closest, _)| distance < closest) {
                    best = Some((distance, node));
                }
            }
        }
    }

    best.map(|(_, node)| node)
}

/// The texel indices within [`SEARCH_RADIUS_TEXELS`] of a coordinate, clamped to the plane.
fn window(center: f32, limit: u32) -> core::ops::Range<u32> {
    let radius = SEARCH_RADIUS_TEXELS as f32;
    let low = (center - radius).floor().max(0.0);
    let high = (center + radius).ceil().max(0.0);
    if !low.is_finite() || !high.is_finite() {
        return 0..0;
    }
    let start = (low as u32).min(limit);
    let end = (high as u32).saturating_add(1).min(limit);
    start..end.max(start)
}

/// How many texels are painted with a colour but no element id — `N7`'s unaccountable pixels.
///
/// This is what `cargo xtask id-coverage` reports, and it is the reason the ID plane is worth
/// having rather than merely nice: the constraint becomes a number a machine can check, and a
/// non-zero one is a defect nobody has to notice by eye.
///
/// `color` is RGBA8 as [`rasterize`](crate::bake::rasterize) writes it, so a texel is painted
/// when its alpha is non-zero.
#[must_use]
pub fn coverage(color: &[u8], ids: &[ElementId]) -> Coverage {
    let mut counts = Coverage::default();
    for (index, id) in ids.iter().enumerate() {
        let painted = color
            .get(index * crate::bake::BYTES_PER_TEXEL + 3)
            .is_some_and(|alpha| *alpha > 0);
        match (painted, id.is_none()) {
            (true, true) => counts.unaccountable += 1,
            (true, false) => counts.accounted += 1,
            // An id with no colour is not what `N7` forbids, and it is still a disagreement
            // between two planes written by one statement — so it is counted and reported
            // rather than tolerated silently.
            (false, false) => counts.invisible += 1,
            (false, true) => counts.blank += 1,
        }
    }
    counts
}

/// What [`coverage`] found.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Coverage {
    /// Painted, and carrying an element id. The good case.
    pub accounted: u64,
    /// **Painted with no element id.** `N7`'s unaccountable pixel; must be zero.
    pub unaccountable: u64,
    /// Carrying an id but painted with nothing. Must also be zero, for the reason above.
    pub invisible: u64,
    /// Neither painted nor identified — the transparent background of a piece.
    pub blank: u64,
}

impl Coverage {
    /// Whether the two planes agree everywhere.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.unaccountable == 0 && self.invisible == 0
    }

    /// Adds another piece's counts.
    pub fn absorb(&mut self, other: Self) {
        self.accounted += other.accounted;
        self.unaccountable += other.unaccountable;
        self.invisible += other.invisible;
        self.blank += other.blank;
    }
}

/// Whether every id in a plane names a node the skeleton has — `P1`.
///
/// `N7` says a coloured pixel carries an id; `P1` says the element it names is a real one. The
/// two are separate failures with separate causes: the first is a rasterizer that painted
/// without recording, the second an id that survived a snapshot it no longer belongs to.
/// Returns the first id that does not resolve.
#[must_use]
pub fn unresolved(skeleton: &Skeleton, ids: &[ElementId]) -> Option<ElementId> {
    ids.iter()
        .copied()
        .find(|id| id.node().is_some_and(|node| skeleton.node(node).is_none()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use treepo_det::{Angle, Seed};
    use treepo_model::{NodeRole, Point, RepoPath};

    fn plane(size: UVec2, ids: &[i64]) -> IdPlane {
        IdPlane::new(
            size,
            ids.iter()
                .map(|value| {
                    if *value < 0 {
                        ElementId::NONE
                    } else {
                        ElementId::of(NodeId::new(*value as u32))
                    }
                })
                .collect(),
        )
    }

    fn region(min: Vec2, max: Vec2) -> Extent {
        Extent { min, max }
    }

    /// The sentinel choice, as the test that would have caught the obvious mistake: node zero
    /// is the basal node, so a sentinel of zero would make the trunk unaccountable.
    #[test]
    fn the_basal_node_is_a_real_id_and_not_the_sentinel() {
        let basal = ElementId::of(NodeId::new(0));
        assert!(!basal.is_none());
        assert_eq!(basal.node(), Some(NodeId::new(0)));
        assert!(ElementId::NONE.is_none());
        assert_eq!(ElementId::NONE.node(), None);
        assert_eq!(ElementId::default(), ElementId::NONE);
    }

    #[test]
    fn a_plane_whose_length_disagrees_with_its_size_holds_nothing() {
        let short = IdPlane::new(UVec2::new(4, 4), vec![ElementId::NONE; 3]);
        assert!(short.is_empty());
        assert_eq!(short.size(), UVec2::ZERO);
        assert!(short.at(0, 0).is_none());
    }

    #[test]
    fn a_texel_outside_the_plane_is_unpainted_rather_than_a_panic() {
        let one = plane(UVec2::ONE, &[7]);
        assert_eq!(one.at(0, 0).node(), Some(NodeId::new(7)));
        assert!(one.at(1, 0).is_none());
        assert!(one.at(0, 9000).is_none());
    }

    /// The whole point of the buffer: the click resolves to whatever painted that texel.
    #[test]
    fn a_click_resolves_to_the_element_that_painted_it() {
        // A 2×2 plane over the unit square. Row zero is the top, so id 3 sits top-left.
        let ids = plane(UVec2::splat(2), &[3, 4, 5, 6]);
        let piece = Painted {
            region: region(Vec2::ZERO, Vec2::splat(2.0)),
            plane: &ids,
        };

        assert_eq!(pick([piece], Vec2::new(0.5, 1.5)), Some(NodeId::new(3)));
        assert_eq!(pick([piece], Vec2::new(1.5, 1.5)), Some(NodeId::new(4)));
        assert_eq!(pick([piece], Vec2::new(0.5, 0.5)), Some(NodeId::new(5)));
        assert_eq!(pick([piece], Vec2::new(1.5, 0.5)), Some(NodeId::new(6)));
    }

    /// `world_up_is_image_up`'s counterpart on the picking side. Getting this sign wrong
    /// produces an app that reports the limb vertically mirrored about the piece — every click
    /// naming a real path, and the wrong one.
    #[test]
    fn picking_agrees_with_the_rasterizer_about_which_way_is_up() {
        let ids = plane(UVec2::new(1, 4), &[-1, -1, -1, 9]);
        let piece = Painted {
            region: region(Vec2::ZERO, Vec2::new(1.0, 4.0)),
            plane: &ids,
        };
        // Id 9 is the last row, which is the *bottom* of the region.
        assert_eq!(pick([piece], Vec2::new(0.5, 0.5)), Some(NodeId::new(9)));
    }

    #[test]
    fn a_click_on_the_background_finds_nothing() {
        let ids = plane(UVec2::splat(2), &[-1, -1, -1, -1]);
        let piece = Painted {
            region: region(Vec2::ZERO, Vec2::splat(2.0)),
            plane: &ids,
        };
        assert_eq!(pick([piece], Vec2::new(1.0, 1.0)), None);
    }

    /// A click far outside every piece is a miss, not the nearest thing in the tree.
    #[test]
    fn a_click_nowhere_near_a_piece_finds_nothing() {
        let ids = plane(UVec2::splat(2), &[3, 3, 3, 3]);
        let piece = Painted {
            region: region(Vec2::ZERO, Vec2::splat(2.0)),
            plane: &ids,
        };
        assert_eq!(pick([piece], Vec2::new(500.0, 500.0)), None);
    }

    /// The search radius, doing the job the old geometric tolerance did: a hairline limb is
    /// clickable from a few texels away.
    #[test]
    fn a_near_miss_still_finds_the_limb() {
        // A 20×1 strip with one painted texel at column 10.
        let mut ids = vec![-1i64; 20];
        ids[10] = 2;
        let ids = plane(UVec2::new(20, 1), &ids);
        let piece = Painted {
            region: region(Vec2::ZERO, Vec2::new(20.0, 1.0)),
            plane: &ids,
        };

        // Four texels away — inside the radius.
        assert_eq!(pick([piece], Vec2::new(14.5, 0.5)), Some(NodeId::new(2)));
        // Nine away — outside it, and a miss rather than a distant guess.
        assert_eq!(pick([piece], Vec2::new(19.5, 0.5)), None);
    }

    /// Two elements within the radius: the nearer one wins, whichever order they arrive in.
    #[test]
    fn the_nearer_element_wins() {
        let ids = plane(UVec2::new(5, 1), &[1, -1, -1, -1, 2]);
        let piece = Painted {
            region: region(Vec2::ZERO, Vec2::new(5.0, 1.0)),
            plane: &ids,
        };
        assert_eq!(pick([piece], Vec2::new(1.2, 0.5)), Some(NodeId::new(1)));
        assert_eq!(pick([piece], Vec2::new(3.8, 0.5)), Some(NodeId::new(2)));
    }

    /// Distances are compared in world units, so a coarse piece cannot beat a fine one by
    /// having larger texels. Both pieces cover the same world span at different resolutions.
    #[test]
    fn pieces_at_different_bands_compare_in_world_units() {
        let coarse = plane(UVec2::new(2, 1), &[-1, 8]);
        let fine = plane(UVec2::new(8, 1), &[-1, -1, -1, 7, -1, -1, -1, -1]);
        let at = Vec2::new(3.6, 0.5);

        let coarse_piece = Painted {
            region: region(Vec2::ZERO, Vec2::new(8.0, 1.0)),
            plane: &coarse,
        };
        let fine_piece = Painted {
            region: region(Vec2::ZERO, Vec2::new(8.0, 1.0)),
            plane: &fine,
        };

        // Id 7 is centred at world x = 3.5; id 8 spans 4..8 and is centred at 6.0.
        assert_eq!(
            pick([coarse_piece, fine_piece], at),
            Some(NodeId::new(7)),
            "the coarse piece won by texel arithmetic"
        );
        assert_eq!(pick([fine_piece, coarse_piece], at), Some(NodeId::new(7)));
    }

    #[test]
    fn coverage_counts_the_four_cases() {
        // Four texels: painted+id, painted+none, unpainted+id, unpainted+none.
        let color = [
            0, 0, 0, 255, //
            0, 0, 0, 255, //
            0, 0, 0, 0, //
            0, 0, 0, 0,
        ];
        let ids = [
            ElementId::of(NodeId::new(1)),
            ElementId::NONE,
            ElementId::of(NodeId::new(2)),
            ElementId::NONE,
        ];

        let found = coverage(&color, &ids);
        assert_eq!(found.accounted, 1);
        assert_eq!(found.unaccountable, 1);
        assert_eq!(found.invisible, 1);
        assert_eq!(found.blank, 1);
        assert!(!found.is_clean());
    }

    #[test]
    fn a_plane_that_agrees_everywhere_is_clean() {
        let color = [0, 0, 0, 255, 0, 0, 0, 0];
        let ids = [ElementId::of(NodeId::new(1)), ElementId::NONE];
        let found = coverage(&color, &ids);
        assert!(found.is_clean());
        assert_eq!(found.accounted, 1);
        assert_eq!(found.blank, 1);
    }

    /// `P1`: an id has to name an element the skeleton actually has.
    #[test]
    fn an_id_that_names_no_node_is_reported() {
        let mut skeleton = Skeleton::new();
        skeleton.push_node(
            None,
            Point::ORIGIN,
            Angle::ZERO,
            Seed::root(b"id-test"),
            NodeRole::Limb {
                path: RepoPath::root(),
            },
        );

        assert_eq!(
            unresolved(&skeleton, &[ElementId::of(NodeId::new(0)), ElementId::NONE]),
            None
        );
        assert_eq!(
            unresolved(&skeleton, &[ElementId::of(NodeId::new(7))]),
            Some(ElementId::of(NodeId::new(7)))
        );
    }
}
