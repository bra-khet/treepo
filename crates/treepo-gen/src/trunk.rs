//! The trunk column — `F-SKEL-3`, `F2`, `AC-SKEL-2`.
//!
//! [`grow`] is the product's entry point: a [`Manifest`] in, a [`Skeleton`] out. Everything
//! [`compose`](crate::lsystem::compose) does happens beneath what this module builds.
//!
//! # Why there is a trunk at all, and why it is not a constant
//!
//! `design/visual-construction.md` settled this against two alternatives and recorded why.
//! A **dedicated trunk** — a tall central column the limbs attach to — makes the trunk a
//! constant, so every repository shares a silhouette in the region a viewer looks at first.
//! A **pure trunkless stack** of independent branches was rejected for L-system
//! compatibility, redraw stability, and readable silhouettes: with no axiom there is nothing
//! for the productions to start from, and nothing that stays put when the repository changes.
//!
//! The hybrid takes the axiom from one and the mass from the other, and this module is its
//! second construction. The first was **co-origin**: one minimal basal segment, every primary
//! leaving its tip, and the trunk was purely their overlap. The arithmetic held and the
//! picture did not. Two limbs `θ` apart separate at `(w₁+w₂)/(4·sin(θ/2))`, which reduces to
//! `1/(packing × fan)` stem-widths *however many limbs there are* — so a wide fan left half a
//! stem-width of trunk, and `tools/m0-silhouette` drew an oversized seed with rays coming out
//! of it. A point is not a support column: the limbs had nowhere to leave *from*.
//!
//! # The pipe column
//!
//! The axis is grown by the primaries that need to leave it rather than pre-sized.
//!
//! * A **collar** at the foot, flared into the roots, carrying every primary at once.
//! * One **internode** per primary — the vertical room that limb needs to exist as volume
//!   rather than as a ray. It is the insertion zone, and the limb departs at its top.
//! * The **width at any height is the support still carried there**: below the first
//!   departure that is everything, and each departure takes its own share away. Each internode
//!   tapers across the drop, so the narrowing happens *through* the joint, which is where a
//!   real trunk's wood diverges.
//! * Above the last departure nothing is left to carry, so the column ends. The innermost
//!   primary leaves last and nearly vertical, and reads as the leader continuing.
//!
//! The overlap the first construction relied on has not gone anywhere — the limbs still leave
//! packed tightly enough to fuse, and later enrichment molds that into bark. What is new is
//! that the column has **height and volume of its own**, so there is something for them to
//! fuse *onto*.
//!
//! Nothing draws an *arbitrary* trunk, which is the part of the original decision worth
//! keeping. The column exists only because primaries need somewhere to leave from. A
//! repository with one top-level directory gets one internode and a thin column; an empty one
//! gets no internodes at all.
//!
//! # Which primary leaves where
//!
//! Fan position is **path order**, so a directory gaining bytes never slides sideways
//! (`AC-GROW-4`). Departure *height* is then the outermost fan position first, working inward.
//! Two things fall out: the column does not lean, because the sides alternate as the order
//! walks in; and the last thing to leave is the most vertical, so the trunk hands off to a
//! leader instead of stopping in mid-air.
//!
//! # `F2`: fewer, thicker limbs
//!
//! One primary limb per top-level directory (`F1`) gives a repository with fourteen
//! top-level entries fourteen thin limbs leaving one point, which is a diagram of a
//! directory listing. `F2` groups the small ones: entries below
//! [`TrunkParams::group_below`] of the repository share a stem, and the members still draw
//! individually beneath it.
//!
//! **How many groups is not a parameter.** Small entries are gathered until a group is no
//! longer small — its combined share reaches the same threshold that made its members
//! grouping candidates. A rule that describes itself, rather than a second number to tune
//! against the first.
//!
//! A group is a *column*, not a container: the same construction as the trunk, one level
//! down — [`column`] is called with the group's members as its primaries. `F2` and `F-SKEL-3`
//! are one mechanism used twice rather than two that must be kept consistent.
//! [`NodeRole::Group`] is deliberately not [`NodeRole::Aggregate`] — a group draws what it
//! holds, and conflating them would report drawn paths as compressed.
//!
//! # `AC-SKEL-2`: a seed and a root cluster, never a lonely trunk
//!
//! An empty repository has no primary limbs, so it claims no internodes, so it has no column —
//! and that falls out of the construction rather than needing a special case. What it does
//! have is the root-mass cluster, which every tree has, and a collar that stays at the table's
//! floor because its length is a proportion of its own width and there is nothing here to be
//! wide about. The result is a seed sitting in its roots, which is what the design asks for
//! and what a dedicated trunk could never have produced.

use crate::lsystem::compose::{self, Context, Site};
use crate::params::{SkeletonInputs, Table, TrunkParams};
use alloc::vec::Vec;
use core::cmp::Reverse;
use treepo_det::{Angle, Fx, Seed, sin_cos};
use treepo_model::{Manifest, NodeId, NodeRole, PathRecord, Point, RepoPath, Segment, Skeleton};

/// The total angular spread of the root cluster, centred on straight down.
///
/// Not a table row: it is the difference between roots and branches rather than a tuning
/// knob. At 140° the outermost node sits 70° either side of vertical — splayed wide, but
/// still below the horizon, so no root can be mistaken for a limb pointing the wrong way.
const ROOT_SPREAD: Angle = Angle::from_millidegrees(140_000);

/// How much of its fan offset a primary keeps at the moment it leaves the column.
///
/// The knot. A limb that departs at its full fan angle reads as a ray from a point however
/// much room the internode gave it; one that leaves closer to the axis and opens out as it
/// goes has part of its first stretch fused into the column, which is what an insertion looks
/// like. The remainder is not lost — the limb's own branching and tropism carry it outward
/// over the following segments.
///
/// A constant rather than a table row on purpose. The design document offers it as a row "only
/// if the eye needs it", and one number that nobody has yet had a reason to move is better
/// left where it can be read than promoted to something that must be tuned.
const KNOT_HOLD: Fx = Fx::from_ratio(7, 10);

/// Grows a repository's whole skeleton — the entry point (`F-SKEL-1`).
///
/// Pure: no clock, no I/O, no global state. Every stochastic draw descends from a
/// [`Seed`](treepo_det::Seed) derived from a path, so this is reproducible on one machine and
/// across three (`AC-DET-1`, `AC-DET-2`).
#[must_use]
pub fn grow(manifest: &Manifest, table: &Table) -> Skeleton {
    let mut skeleton = Skeleton::new();
    let root = RepoPath::root();

    let inputs = compose::inputs_for(manifest, table, &root);
    let trunk = table.trunk_params(&inputs);
    let seed = manifest.seed_for(&root);

    // The basal node is the repository itself, so a click anywhere on the column resolves to
    // the repository rather than to nothing. The whole column is one node drawing many
    // segments — an axis divided into internodes is still one axis, and giving each internode
    // a node of its own would report the repository several times over.
    let basal = skeleton.push_node(
        None,
        Point::ORIGIN,
        Angle::ZERO,
        seed,
        NodeRole::Limb { path: root.clone() },
    );

    let primaries = assign_primaries(manifest, table, &root, trunk.group_below);

    // Placed before the column so the roots keep the low node indices their position implies,
    // and sized from the pipe rather than from the flared foot: the flare is the collar
    // widening *into* these, so measuring it against them would count it twice.
    root_cluster(
        &mut skeleton,
        basal,
        &seed,
        &root,
        &trunk,
        pipe_width(table, &trunk, combined_width(table, &primaries)),
    );

    let mut ctx = Context::new(manifest, table, &mut skeleton);
    column(
        &mut ctx,
        basal,
        Site {
            position: Point::ORIGIN,
            heading: Angle::ZERO,
            // Nothing upstream: the trunk is as wide as what it carries and no wider.
            carried: None,
        },
        &trunk,
        &primaries,
        1,
    );

    skeleton
}

/// One limb leaving a stem: a single top-level entry, or a group of small ones (`F2`).
#[derive(Debug)]
enum Primary<'a> {
    /// One path, large enough to earn a limb of its own.
    One(&'a PathRecord),
    /// Several small siblings sharing a thicker stem.
    Group(Vec<&'a PathRecord>),
}

impl<'a> Primary<'a> {
    /// Where this sits in path order — a group sorts by its first member.
    fn sort_key(&self) -> &RepoPath {
        match self {
            Self::One(record) => &record.path,
            Self::Group(members) => &members[0].path,
        }
    }

    /// Every record this limb carries.
    fn records(&self) -> &[&'a PathRecord] {
        match self {
            Self::One(record) => core::slice::from_ref(record),
            Self::Group(members) => members,
        }
    }
}

/// Divides a limb's children into primary limbs, grouping the small ones (`F2`).
fn assign_primaries<'a>(
    manifest: &'a Manifest,
    table: &Table,
    anchor: &'a RepoPath,
    group_below: Fx,
) -> Vec<Primary<'a>> {
    let children = compose::significant_children(manifest, anchor);
    if children.is_empty() {
        return Vec::new();
    }

    let total: u64 = children.iter().map(|record| record.size.bytes).sum();
    let threshold = share_of(total, group_below);

    // `significant_children` is largest-first, so the small ones are the tail. Below the
    // threshold *and* not the only child: grouping a lone entry with itself would build a
    // stem carrying one limb, which is a joint with nothing on either side of it.
    let (large, small): (Vec<_>, Vec<_>) = children
        .iter()
        .partition(|record| record.size.bytes > threshold || children.len() == 1);

    let mut primaries: Vec<Primary<'a>> = large.into_iter().map(Primary::One).collect();

    if small.len() > 1 {
        // A stem is a limb, so it carries what a limb carries. Without this bound a
        // repository whose small entries are *all* negligible would gather every one of them
        // onto a single stem, and F2's "fewer, thicker limbs" would arrive at one limb with
        // a fan of forty — the diagram it exists to prevent, moved one level down.
        let capacity = usize::from(
            table
                .limb_params(&compose::inputs_for(manifest, table, anchor))
                .branch_capacity,
        );
        let mut ordered: Vec<&PathRecord> = small;
        ordered.sort_by(|a, b| a.path.cmp(&b.path));
        for group in gather(ordered, threshold, capacity.max(1)) {
            // A run of one is not a group — it is the limb it always was.
            if group.len() == 1 {
                primaries.push(Primary::One(group[0]));
            } else {
                primaries.push(Primary::Group(group));
            }
        }
    } else {
        primaries.extend(small.into_iter().map(Primary::One));
    }

    primaries.sort_by(|a, b| a.sort_key().cmp(b.sort_key()));
    primaries
}

/// Gathers path-adjacent entries into runs, each closing once it is no longer small.
///
/// The self-describing half of `F2`: a group stops taking members at the same threshold that
/// made them candidates, so "how many groups" needs no parameter of its own. `capacity` is
/// the second stop, and it is a limb's own — a stem that carried more than a limb can would
/// just move the problem down a level.
///
/// Path-adjacent for the reason containers are: a stem should carry neighbours, so the base
/// of the tree reads as regions of the repository rather than as an assortment.
fn gather(ordered: Vec<&PathRecord>, threshold: u64, capacity: usize) -> Vec<Vec<&PathRecord>> {
    let mut runs: Vec<Vec<&PathRecord>> = Vec::new();
    let mut current: Vec<&PathRecord> = Vec::new();
    let mut carried = 0u64;

    for record in ordered {
        carried = carried.saturating_add(record.size.bytes);
        current.push(record);
        if carried > threshold || current.len() >= capacity {
            runs.push(core::mem::take(&mut current));
            carried = 0;
        }
    }

    // Whatever is left never reached either stop. It joins the last run where there is room
    // — the point of `F2` is fewer, thicker limbs, and a trailing remainder is exactly the
    // thin limb it exists to remove — and stands alone only where merging would push that
    // run past the capacity the bound above just enforced.
    if !current.is_empty() {
        match runs.last_mut() {
            Some(last) if last.len() + current.len() <= capacity => last.extend(current),
            _ => runs.push(current),
        }
    }
    runs
}

/// The combined base width of everything one primary carries — a group counts its members.
///
/// Summing the limbs' own widths rather than reading a table row is what makes the trunk a
/// consequence of what it carries: a repository that grows a heavy new top-level directory
/// thickens at the base because the limb is thick, not because a separate number was tuned to
/// agree.
fn primary_width(table: &Table, primary: &Primary<'_>) -> Fx {
    primary
        .records()
        .iter()
        .map(|record| {
            let inputs = SkeletonInputs::from_record(record, &table.scales);
            table.limb_params(&inputs).base_width
        })
        .fold(Fx::ZERO, Fx::add)
}

/// What every primary carries, added up.
fn combined_width(table: &Table, primaries: &[Primary<'_>]) -> Fx {
    primaries
        .iter()
        .map(|primary| primary_width(table, primary))
        .fold(Fx::ZERO, Fx::add)
}

/// How wide a column carrying `combined` is drawn — packed, soft-capped, and floored.
///
/// An empty repository has no limbs and therefore no column to be wide. The seed still needs
/// a width to be drawn at, and the width the table gives a limb with every driver at zero is
/// the honest floor — anything else would be a constant invented here. The same floor is what
/// keeps the top of a column from tapering to nothing once the last primary has left.
fn pipe_width(table: &Table, trunk: &TrunkParams, combined: Fx) -> Fx {
    trunk
        .support(combined)
        .max(table.limb_params(&SkeletonInputs::default()).base_width)
}

/// Places the root-mass cluster at the base (`AC-SKEL-2`).
fn root_cluster(
    skeleton: &mut Skeleton,
    basal: NodeId,
    seed: &Seed,
    anchor: &RepoPath,
    trunk: &TrunkParams,
    width: Fx,
) {
    let count = trunk.root_cluster.max(1);
    // Three quarters of the buttress's width, which clears its edge by a quarter and no more.
    // Measured against the flared foot rather than against the collar's length, which is the
    // correction the column forced: the old rule made a root a fraction of a short axiom, and
    // once the foot flared they were drawn entirely inside it — the first pictures showed a
    // black bulb with a grey smudge in the middle where the root cluster was supposed to be.
    // Setting them to the full foot width fixed that and overshot into a starfish.
    //
    // Still short and thick beside a limb, which is what keeps them reading as mass at the
    // base rather than as branches pointing the wrong way.
    let length = trunk.flared(width).mul(Fx::from_ratio(3, 4));
    let node_width = width.mul(Fx::from_ratio(1, 4)).max(Fx::EPSILON);

    for index in 0..count {
        let heading = spread(Angle::HALF, ROOT_SPREAD, index, count);
        let (sin, cos) = sin_cos(heading);
        let end = Point::new(sin.mul(length), cos.mul(length));

        let node = skeleton.push_node(
            Some(basal),
            Point::ORIGIN,
            heading,
            seed.derive(b"root-mass").derive(&index.to_le_bytes()),
            NodeRole::RootMass {
                anchor: anchor.clone(),
                index,
            },
        );
        skeleton.extend_segments([Segment {
            start: Point::ORIGIN,
            end,
            base_width: node_width,
            tip_width: node_width.mul(Fx::HALF),
            node,
            generation: 0,
        }]);
    }
}

/// Where one primary leaves the column, worked out before anything is placed.
struct Insertion {
    /// Which primary this is — its place in the fan, in path order.
    primary: usize,
    /// The point on the axis it leaves from: the top of the internode it claimed.
    position: Point,
    /// The heading it leaves on, already pulled back toward the axis by the knot.
    heading: Angle,
    /// The column's width just below the join, which is the branch it grafts onto.
    carried: Fx,
}

/// Grows a support column and everything that leaves it, returning its width at the foot.
///
/// The construction the module header describes, and the only one: the trunk calls it with the
/// repository's primaries, and an `F2` group calls it with its members.
///
/// Two passes, and the order matters for legibility rather than for correctness. The column's
/// own geometry is settled first so its segments land contiguously and a reader of
/// `skeleton.segments()` sees an axis rather than an axis interleaved with three subtrees.
fn column(
    ctx: &mut Context<'_>,
    node: NodeId,
    from: Site,
    trunk: &TrunkParams,
    primaries: &[Primary<'_>],
    level: u8,
) -> Fx {
    let table = ctx.table();
    let shares: Vec<Fx> = primaries
        .iter()
        .map(|primary| primary_width(table, primary))
        .collect();
    let combined = shares.iter().copied().fold(Fx::ZERO, Fx::add);

    let mut pipe = pipe_width(table, trunk, combined);
    let mut foot = trunk.flared(pipe);
    // A column leaving a branch cannot be wider than the branch it leaves. Both ends are
    // clamped: without the first a group stem could buttress out past the trunk it grows on.
    if let Some(carried) = from.carried {
        foot = foot.min(carried);
        pipe = pipe.min(foot);
    }
    let floor = table.limb_params(&SkeletonInputs::default()).base_width;

    let (sin, cos) = sin_cos(from.heading);
    let advance =
        |at: Point, length: Fx| Point::new(at.x.add(sin.mul(length)), at.y.add(cos.mul(length)));

    // The collar: everything is still carried here, and the foot flares into the roots.
    let mut at = from.position;
    let mut tip = advance(at, trunk.basal_length(pipe));
    let mut segments: Vec<Segment> = Vec::with_capacity(primaries.len() + 1);
    segments.push(Segment {
        start: at,
        end: tip,
        base_width: foot,
        tip_width: pipe,
        node,
        generation: 0,
    });
    at = tip;

    // Outermost fan position first, working inward — see the module header. Ties by index so
    // the two halves of a symmetric pair have a settled order rather than a sort's whim.
    let count = u16::try_from(primaries.len()).unwrap_or(u16::MAX);
    let mut order: Vec<usize> = (0..primaries.len()).collect();
    order.sort_by_key(|&index| {
        (
            Reverse(offset_rank(u16::try_from(index).unwrap_or(u16::MAX), count)),
            index,
        )
    });

    let mut insertions: Vec<Insertion> = Vec::with_capacity(order.len());
    let mut remaining = combined;
    let mut width = pipe;
    for &index in &order {
        remaining = remaining.sub(shares[index]).max(Fx::ZERO);
        // `min(width)` because the floor can hold the width up while `remaining` still falls,
        // and a column that widened as its limbs left would be a funnel.
        let above = pipe_width(table, trunk, remaining).min(width).max(floor);

        tip = advance(at, trunk.internode_length(width.sub(above)));
        // Tapered across the internode rather than stepped at its top: the drop belongs to
        // the joint, which is where the wood actually divides.
        segments.push(Segment {
            start: at,
            end: tip,
            base_width: width,
            tip_width: above,
            node,
            generation: 0,
        });

        insertions.push(Insertion {
            primary: index,
            position: tip,
            heading: knotted(
                from.heading,
                spread(
                    from.heading,
                    trunk.fan,
                    u16::try_from(index).unwrap_or(u16::MAX),
                    count,
                ),
            ),
            // The width just below the join. `grafted_onto` narrows from here, so a limb
            // leaving the column inherits the falloff exactly as a limb leaving another limb
            // does — a column is a branch, and a branch is what a graft narrows against.
            carried: width,
        });

        at = tip;
        width = above;
    }

    ctx.skeleton().extend_segments(segments);

    for insertion in insertions {
        let site = Site {
            position: insertion.position,
            heading: insertion.heading,
            carried: Some(insertion.carried),
        };
        match &primaries[insertion.primary] {
            Primary::One(record) => {
                compose::place(ctx, &record.path, Some(node), site, level);
            }
            Primary::Group(members) => place_group(ctx, node, site, members, level),
        }
    }

    foot
}

/// Places one `F2` group: a column of its own, with its members leaving it.
///
/// The trunk's construction one level down — see the module header on why that is one
/// mechanism rather than two.
fn place_group(
    ctx: &mut Context<'_>,
    parent: NodeId,
    at: Site,
    members: &[&PathRecord],
    level: u8,
) {
    let anchor = members[0].path.parent().unwrap_or_else(RepoPath::root);

    // Every group under one anchor needs its own seed, and the first member's path is the
    // one thing about a group that a Grow does not shuffle: gathering is path-ordered, so a
    // file arriving elsewhere does not rename this group.
    let seed = ctx
        .seed_for(&anchor)
        .derive(b"group")
        .derive(members[0].path.as_bytes());

    let trunk = ctx.table().trunk_params(&SkeletonInputs::default());
    let held: Vec<Primary<'_>> = members.iter().copied().map(Primary::One).collect();

    let node = ctx.skeleton().push_node(
        Some(parent),
        at.position,
        at.heading,
        seed,
        NodeRole::Group {
            anchor,
            members: members.iter().map(|record| record.path.clone()).collect(),
        },
    );

    column(ctx, node, at, &trunk, &held, level);
}

/// The `index`th of `count` headings spread evenly across `spread` either side of `centre`.
///
/// A lone limb takes the centre exactly rather than an edge, so a repository with one
/// top-level directory grows straight up instead of leaning.
fn spread(centre: Angle, spread: Angle, index: u16, count: u16) -> Angle {
    if count <= 1 {
        return centre;
    }
    let offset = i64::from(spread.to_bits()) * (2 * i64::from(index) - i64::from(count - 1))
        / (2 * i64::from(count - 1));
    centre + Angle::from_bits(offset as u32)
}

/// How far off centre the `index`th of `count` fan positions sits, in half-steps.
///
/// The departure order, and only the order — the magnitude is never converted to an angle, so
/// it needs no units. Symmetric by construction: the two ends of a fan rank equally and leave
/// at consecutive heights, which is what keeps the column from leaning.
fn offset_rank(index: u16, count: u16) -> u32 {
    if count <= 1 {
        return 0;
    }
    (2 * i32::from(index) - i32::from(count - 1)).unsigned_abs()
}

/// A departure heading pulled back toward the axis it leaves — the knot.
///
/// See [`KNOT_HOLD`]. The pull is proportional to the offset, so the centre limb is untouched
/// and the outermost is bent most, which is the right way round: a limb leaving at 70° has the
/// most to gain from spending its first stretch inside the trunk.
fn knotted(axis: Angle, departure: Angle) -> Angle {
    let offset = departure.to_bits().wrapping_sub(axis.to_bits()) as i32;
    // i128 because the offset can be most of a turn and `KNOT_HOLD` most of 2³²; the product
    // is past i64 well before either is unreasonable.
    let held = (i128::from(offset) * i128::from(KNOT_HOLD.to_bits())) >> 32;
    axis + Angle::from_bits(held as u32)
}

/// `total × share`, as whole bytes.
fn share_of(total: u64, share: Fx) -> u64 {
    let scaled = (i128::from(total) * i128::from(share.to_bits())) >> 32;
    u64::try_from(scaled).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;
    use treepo_model::{NodeKind, PathRecord};

    /// Reuses the composition tests' manifest builder — the same rolled-up primitives a real
    /// walk produces, so the trunk responds to real drivers rather than to defaults.
    fn manifest_of(files: &[(&str, u64)]) -> Manifest {
        crate::lsystem::compose::tests::manifest_of(files)
    }

    fn empty() -> Manifest {
        let mut manifest = Manifest::new("test".to_string(), Seed::root(b"empty"));
        manifest.set_paths(vec![PathRecord::new(RepoPath::root(), NodeKind::Directory)]);
        manifest
    }

    fn table() -> Table {
        Table::built_in()
    }

    fn roles(skeleton: &Skeleton) -> (usize, usize, usize) {
        let mut groups = 0;
        let mut roots = 0;
        let mut limbs = 0;
        for node in skeleton.nodes() {
            match &node.role {
                NodeRole::Group { .. } => groups += 1,
                NodeRole::RootMass { .. } => roots += 1,
                NodeRole::Limb { .. } => limbs += 1,
                NodeRole::Aggregate(_) => {}
            }
        }
        (limbs, groups, roots)
    }

    /// How far the skeleton reaches above the origin.
    fn height(skeleton: &Skeleton) -> Fx {
        skeleton
            .segments()
            .iter()
            .map(|segment| segment.end.y.max(segment.start.y))
            .fold(Fx::ZERO, Fx::max)
    }

    /// How far it reaches sideways, either way.
    fn half_width(skeleton: &Skeleton) -> Fx {
        skeleton
            .segments()
            .iter()
            .map(|segment| segment.end.x.abs().max(segment.start.x.abs()))
            .fold(Fx::ZERO, Fx::max)
    }

    /// The trunk column's own segments, foot first.
    ///
    /// Read off the skeleton rather than recomputed from the table, deliberately: every rule
    /// this module applies is a function of the column's width, and a test that re-evaluated
    /// those rules would agree with the code by construction instead of measuring it.
    fn spine(skeleton: &Skeleton) -> Vec<&Segment> {
        let basal = skeleton
            .nodes()
            .iter()
            .find(|node| node.parent.is_none())
            .expect("every tree has a basal node");
        skeleton
            .segments()
            .iter()
            .filter(|segment| segment.node == basal.id)
            .collect()
    }

    /// The column as `(height, width at the foot)`.
    fn trunk_column(skeleton: &Skeleton) -> (Fx, Fx) {
        let spine = spine(skeleton);
        let top = spine
            .iter()
            .map(|segment| segment.end.y)
            .fold(Fx::ZERO, Fx::max);
        (top, spine[0].base_width)
    }

    /// Just the column's height.
    fn column_height(skeleton: &Skeleton) -> Fx {
        trunk_column(skeleton).0
    }

    /// Where each primary leaves the column, lowest first.
    fn departures(skeleton: &Skeleton) -> Vec<Fx> {
        let basal = skeleton.nodes()[0].id;
        let mut heights: Vec<Fx> = skeleton
            .nodes()
            .iter()
            .filter(|node| {
                node.parent == Some(basal) && !matches!(node.role, NodeRole::RootMass { .. })
            })
            .map(|node| node.origin.y)
            .collect();
        heights.sort();
        heights
    }

    /// A table with the fan pinned, so a test can vary it without the drivers interfering.
    fn at_fan(millidegrees: i32) -> Table {
        let mut table = Table::built_in();
        table.trunk.fan.base = millidegrees;
        table.trunk.fan.min = millidegrees;
        table.trunk.fan.max = millidegrees;
        table
            .validate()
            .expect("a pinned fan is still a valid table");
        table
    }

    /// `count` equally-sized top-level directories, each well above the `F2` threshold.
    fn equal_primaries(count: usize) -> Manifest {
        let files: Vec<alloc::string::String> = (0..count)
            .map(|i| alloc::format!("dir{i:02}/main.rs"))
            .collect();
        let listed: Vec<(&str, u64)> = files.iter().map(|name| (name.as_str(), 10_000)).collect();
        manifest_of(&listed)
    }

    /// `AC-SKEL-2`, both halves: an empty repository shows a seed and a root cluster, and it
    /// does **not** show a trunk.
    #[test]
    fn an_empty_repository_is_a_seed_in_its_roots_not_a_lonely_trunk() {
        let table = table();
        let skeleton = grow(&empty(), &table);
        let (limbs, groups, roots) = roles(&skeleton);

        assert_eq!(limbs, 1, "the seed itself");
        assert_eq!(groups, 0);
        assert!(roots >= 1, "a root cluster is not optional");

        // No primaries, so no internodes, so the column is its collar and nothing else — and
        // with nothing to be wide about that collar sits at the table's floor. There is
        // nothing here that reads as a trunk.
        assert_eq!(spine(&skeleton).len(), 1, "a seed claims no internodes");
        let collar = column_height(&skeleton);
        assert!(
            height(&skeleton) <= collar,
            "an empty repository grew to {:?}, past its own collar {:?}",
            height(&skeleton),
            collar
        );
    }

    /// And the contrast that makes the previous test mean something.
    #[test]
    fn a_populated_repository_grows_far_past_its_column() {
        let manifest = manifest_of(&[
            ("src/main.rs", 5_000),
            ("src/lib.rs", 4_000),
            ("docs/guide.md", 1_000),
            ("tests/it.rs", 800),
        ]);
        let skeleton = grow(&manifest, &table());
        let column = column_height(&skeleton);

        assert!(
            height(&skeleton) > column.mul(Fx::from_int(2)),
            "a real repository must carry a crown well past its trunk: {:?} vs {:?}",
            height(&skeleton),
            column
        );
    }

    /// `F-SKEL-3`: the trunk is what its limbs make it. More and heavier top-level content
    /// means a wider base, without any row being tuned to say so.
    #[test]
    fn the_column_widens_with_what_it_carries() {
        let narrow = grow(&manifest_of(&[("only/a.rs", 1_000)]), &table());
        let broad = grow(
            &manifest_of(&[
                ("one/a.rs", 9_000),
                ("two/b.rs", 9_000),
                ("three/c.rs", 9_000),
                ("four/d.rs", 9_000),
            ]),
            &table(),
        );

        assert!(
            trunk_column(&broad).1 > trunk_column(&narrow).1,
            "four heavy limbs must produce a wider base than one: {:?} vs {:?}",
            trunk_column(&broad).1,
            trunk_column(&narrow).1
        );
    }

    /// The pipe, and the whole reason the column is grown rather than pre-sized: the width at
    /// any height is the support still carried there, so it drops as each primary leaves.
    ///
    /// Three claims. The segments join without a step, so the axis is one solid taper and not
    /// a stack of blocks; no segment ever widens, in either direction; and the column really
    /// does narrow, so this is a pipe rather than a cylinder that happens to be in pieces.
    ///
    /// Sabotage: give every internode `tip_width: width` and the third assertion fails.
    #[test]
    fn the_column_narrows_as_each_primary_leaves() {
        let skeleton = grow(&equal_primaries(6), &table());
        let spine = spine(&skeleton);

        assert_eq!(
            spine.len(),
            7,
            "a collar and one internode per primary: {spine:?}"
        );

        for pair in spine.windows(2) {
            assert_eq!(
                pair[1].base_width, pair[0].tip_width,
                "the column steps between {:?} and {:?} instead of joining",
                pair[0], pair[1]
            );
        }
        for segment in &spine {
            assert!(
                segment.tip_width <= segment.base_width,
                "a column segment widened on the way up: {segment:?}"
            );
        }
        assert!(
            spine.last().unwrap().tip_width < spine[0].base_width,
            "the column never narrowed: {:?} at the foot, {:?} at the top",
            spine[0].base_width,
            spine.last().unwrap().tip_width
        );
    }

    /// The other half of the rework: primaries leave *along* the column, not from one point.
    ///
    /// This is the failure the first silhouettes showed. Co-origin gave every primary the same
    /// departure, so the base was a point with rays coming out of it however thick the numbers
    /// made it — an oversized seed. Each primary now claims vertical room of its own.
    #[test]
    fn primaries_leave_along_the_column_rather_than_from_one_point() {
        let skeleton = grow(&equal_primaries(5), &table());
        let departures = departures(&skeleton);

        assert_eq!(departures.len(), 5);
        for pair in departures.windows(2) {
            assert!(
                pair[1] > pair[0],
                "two primaries left from the same height: {departures:?}"
            );
        }
        assert!(
            departures[0] > Fx::ZERO,
            "the lowest primary must still clear the collar: {departures:?}"
        );
    }

    /// Which primary leaves where, and why the column does not lean.
    ///
    /// Fan position is path order; departure height is outermost first, working inward. So the
    /// two ends of the fan leave lowest, the sides alternate as the order walks in, and the
    /// centre — which an odd count places exactly on the axis — leaves last. That last one is
    /// the leader, and it is what keeps the trunk from stopping in mid-air.
    #[test]
    fn the_outermost_primaries_leave_lowest_and_the_leader_leaves_last() {
        let skeleton = grow(&equal_primaries(5), &at_fan(90_000));
        let basal = skeleton.nodes()[0].id;

        let mut primaries: Vec<(i32, Fx)> = skeleton
            .nodes()
            .iter()
            .filter(|node| {
                node.parent == Some(basal) && !matches!(node.role, NodeRole::RootMass { .. })
            })
            // Left of centre is a wrapped-negative angle, so read the signed offset from up.
            .map(|node| (node.heading.to_bits() as i32, node.origin.y))
            .collect();
        assert_eq!(primaries.len(), 5);
        primaries.sort_by_key(|&(heading, _)| heading);

        let leader = primaries[2];
        assert_eq!(leader.0, 0, "an odd fan puts its middle limb on the axis");
        for (heading, height) in &primaries {
            if *heading == 0 {
                continue;
            }
            assert!(
                *height < leader.1,
                "a limb at {heading} left at {height:?}, above the leader's {:?}",
                leader.1
            );
        }

        // And the further off centre, the lower — the outer pair below the inner pair.
        assert!(primaries[0].1 < primaries[1].1, "left side out of order");
        assert!(primaries[4].1 < primaries[3].1, "right side out of order");
    }

    /// `AC-SKEL-2`'s counterpart at the other end, and the shape the rework was for: a
    /// multi-primary repository stands on a column, and it stays a column at every scale.
    ///
    /// The old construction had this backwards. Its axiom was capped at twice its own width
    /// *because* a longer one would have out-reached the overlap that was doing the trunk's
    /// work — so the base could only ever be stubby, and a broad repository drew a disc. The
    /// column carries its own height, so the assertion inverts: taller than it is wide, and
    /// still not a pole.
    ///
    /// Sabotage: set `internode_aspect` to its floor and the first bound fails at both scales.
    #[test]
    fn the_column_keeps_its_proportions_however_broad_the_repository() {
        let slim = grow(
            &manifest_of(&[("core/a.rs", 10_000), ("docs/b.md", 9_000)]),
            &table(),
        );
        // Twelve equal top-level directories: each is well above `group_below`, so every one
        // earns a primary limb and the column has twelve widths to add up.
        let broad = grow(&equal_primaries(12), &table());

        let (slim_height, slim_foot) = trunk_column(&slim);
        let (broad_height, broad_foot) = trunk_column(&broad);

        assert!(
            broad_foot > slim_foot,
            "twelve primaries must make a wider column than two: {broad_foot:?} vs {slim_foot:?}"
        );
        assert!(
            broad_height > slim_height,
            "and a taller one, or the extra mass went nowhere: {broad_height:?} against \
             {slim_height:?}"
        );

        for (height, foot, name) in [
            (slim_height, slim_foot, "slim"),
            (broad_height, broad_foot, "broad"),
        ] {
            assert!(
                height > foot,
                "the {name} column is wider than it is tall — that is a disc, {height:?} \
                 against {foot:?}"
            );
            assert!(
                height < foot.mul(Fx::from_int(4)),
                "the {name} column is four times its own width — that is a pole, {height:?} \
                 against {foot:?}"
            );
        }
    }

    /// The foot is the widest thing in the tree, which is what "planted" means.
    ///
    /// Two rules meet here and both are needed: the flare widens the collar into the roots,
    /// and the pipe narrows everything above it. Without the flare the base is a cylinder
    /// standing on a line; without the pipe the whole column is as wide as the base and the
    /// crown appears to sprout from a chimney.
    #[test]
    fn the_foot_is_the_widest_part_of_the_tree() {
        let skeleton = grow(&equal_primaries(4), &table());
        let spine = spine(&skeleton);
        let foot = spine[0].base_width;

        assert!(
            foot > spine[0].tip_width,
            "the collar must flare into its roots: {foot:?} to {:?}",
            spine[0].tip_width
        );
        for segment in skeleton.segments() {
            assert!(
                segment.base_width <= foot,
                "{segment:?} is wider than the foot of the trunk, {foot:?}"
            );
        }
    }

    /// The fan's new job, and the retirement of its old one.
    ///
    /// It used to be both lateral character and the trunk's height budget: the overlap that
    /// *was* the trunk ended where the fan pulled two limbs apart, so a wide fan meant a short
    /// trunk and neither reading could be tuned without the other. The column's height comes
    /// from the internodes, so the coupling is gone — exactly, not approximately.
    ///
    /// Sabotage: derive an internode's length from the fan and the first assertion fails.
    #[test]
    fn the_fan_spreads_the_crown_without_touching_the_trunk() {
        let repository = equal_primaries(4);
        let narrow = grow(&repository, &at_fan(30_000));
        let wide = grow(&repository, &at_fan(150_000));

        assert_eq!(
            trunk_column(&narrow),
            trunk_column(&wide),
            "the fan must not move the column by one bit"
        );
        assert!(
            half_width(&wide) > half_width(&narrow),
            "but it must still spread the crown: {:?} at 150°, {:?} at 30°",
            half_width(&wide),
            half_width(&narrow)
        );
    }

    /// `P6` at the base: a broader repository draws a wider trunk, and never proportionally.
    ///
    /// Both halves matter. Drop the ordering and a monorepo stops reading as bigger than a
    /// small library, which is dishonest; keep it strictly proportional and sixteen top-level
    /// directories draw a telephone pole, which is illegible. The soft cap above
    /// `support_knee` is the projection between them.
    ///
    /// Sixteen equal directories carry about 3.3 times the combined limb width of four — each
    /// is a smaller share of its parent, so it is not a clean four — and that is the figure
    /// the second bound is set against. Sabotage: delete the knee from
    /// `TrunkParams::support` and the base scales past it.
    #[test]
    fn a_broader_repository_thickens_at_the_base_without_scaling_with_it() {
        let foot = |count: usize| trunk_column(&grow(&equal_primaries(count), &table())).1;

        let (four, sixteen, thirty_two) = (foot(4), foot(16), foot(32));

        assert!(
            sixteen > four && thirty_two > sixteen,
            "more top-level breadth must always read as a wider base: {four:?}, {sixteen:?}, \
             {thirty_two:?}"
        );
        assert!(
            sixteen < four.mul(Fx::from_ratio(5, 2)),
            "sixteen primaries drew {sixteen:?} against four's {four:?} — the base is \
             tracking the mass instead of projecting it"
        );
    }

    /// `F2` and `F-SKEL-3` are one mechanism used twice: a group stem is a column too, with
    /// its members leaving it exactly as the primaries leave the trunk.
    #[test]
    fn a_group_stem_is_a_column_of_its_own() {
        let mut files: Vec<(alloc::string::String, u64)> =
            vec![("core/big.rs".to_string(), 1_000_000)];
        files.extend((0..8).map(|i| (alloc::format!("side{i}/small.rs"), 500)));
        let listed: Vec<(&str, u64)> = files.iter().map(|(n, b)| (n.as_str(), *b)).collect();
        let skeleton = grow(&manifest_of(&listed), &table());

        let groups: Vec<&treepo_model::SkeletonNode> = skeleton
            .nodes()
            .iter()
            .filter(|node| matches!(node.role, NodeRole::Group { .. }))
            .collect();
        assert!(!groups.is_empty(), "the fixture must produce a group");

        for group in groups {
            let stem: Vec<&Segment> = skeleton
                .segments()
                .iter()
                .filter(|segment| segment.node == group.id)
                .collect();
            assert!(
                stem.len() >= 2,
                "a group stem is a collar plus one internode per member, not one block: \
                 {stem:?}"
            );
            for pair in stem.windows(2) {
                assert_eq!(
                    pair[1].base_width, pair[0].tip_width,
                    "a group stem must be as continuous as the trunk"
                );
            }

            // And its members leave it at different heights, for the same reason.
            let mut heights: Vec<Fx> = skeleton
                .nodes()
                .iter()
                .filter(|node| node.parent == Some(group.id))
                .map(|node| node.origin.y)
                .collect();
            heights.sort();
            assert!(heights.len() > 1, "a group of one is just a limb");
            assert!(
                heights[0] < *heights.last().unwrap(),
                "a group's members left from one point: {heights:?}"
            );
        }
    }

    /// One top-level directory is the case the construction is weakest on, and it is worth
    /// saying out loud what it does: a short column with one insertion, and the limb carries
    /// on from there. Better than the old model's nothing at all, and still not a monument.
    #[test]
    fn a_single_primary_gets_a_short_column_rather_than_none() {
        let skeleton = grow(&manifest_of(&[("only/a.rs", 10_000)]), &table());
        let spine = spine(&skeleton);

        assert_eq!(spine.len(), 2, "a collar and one internode");
        let (column, foot) = trunk_column(&skeleton);
        assert!(
            column < foot.mul(Fx::from_int(2)),
            "one primary has almost nothing to be tall about: {column:?} against {foot:?}"
        );
        assert!(
            height(&skeleton) > column.mul(Fx::from_int(4)),
            "and the crown must still dwarf it: {:?} against {column:?}",
            height(&skeleton)
        );
    }

    /// `F2`: small top-level directories share a stem instead of each taking a thin limb.
    #[test]
    fn small_top_level_directories_are_grouped_into_fewer_thicker_limbs() {
        // One dominant directory and eight negligible ones.
        let mut files: Vec<(alloc::string::String, u64)> =
            vec![("core/big.rs".to_string(), 1_000_000)];
        files.extend((0..8).map(|i| (alloc::format!("side{i}/small.rs"), 500)));
        let listed: Vec<(&str, u64)> = files.iter().map(|(n, b)| (n.as_str(), *b)).collect();

        let skeleton = grow(&manifest_of(&listed), &table());
        let (_, groups, _) = roles(&skeleton);
        assert!(groups >= 1, "eight negligible siblings should share stems");

        // And the grouped paths are still drawn — a group is not a container.
        for index in 0..8 {
            let path = RepoPath::new(alloc::format!("side{index}").as_bytes()).unwrap();
            assert!(
                skeleton.represents(&path),
                "side{index} must still be a limb of its own"
            );
        }
    }

    /// The other side: entries that are all substantial each keep their own limb.
    #[test]
    fn comparable_top_level_directories_are_not_grouped() {
        let skeleton = grow(
            &manifest_of(&[
                ("alpha/a.rs", 10_000),
                ("beta/b.rs", 10_000),
                ("gamma/c.rs", 10_000),
            ]),
            &table(),
        );
        let (_, groups, _) = roles(&skeleton);
        assert_eq!(groups, 0, "equals should not be gathered");
    }

    /// A group's members stay path-adjacent, so the base of the tree reads as regions.
    #[test]
    fn a_group_carries_neighbours() {
        let mut files: Vec<(alloc::string::String, u64)> =
            vec![("core/big.rs".to_string(), 1_000_000)];
        files.extend((0..10).map(|i| (alloc::format!("s{i:02}/x.rs"), 400)));
        let listed: Vec<(&str, u64)> = files.iter().map(|(n, b)| (n.as_str(), *b)).collect();
        let skeleton = grow(&manifest_of(&listed), &table());

        for node in skeleton.nodes() {
            if let NodeRole::Group { members, .. } = &node.role {
                assert!(members.len() > 1, "a group of one is just a limb");
                let mut sorted = members.clone();
                sorted.sort();
                assert_eq!(&sorted, members, "a group's members must be path-ordered");
            }
        }
    }

    /// A lone top-level directory grows straight up rather than leaning to one edge of the
    /// fan — the degenerate case of the spread.
    #[test]
    fn a_single_primary_limb_leaves_straight_up() {
        assert_eq!(
            spread(Angle::ZERO, Angle::from_millidegrees(60_000), 0, 1),
            Angle::ZERO
        );

        // And a pair straddles the centre symmetrically.
        let fan = Angle::from_millidegrees(60_000);
        let left = spread(Angle::ZERO, fan, 0, 2);
        let right = spread(Angle::ZERO, fan, 1, 2);
        assert_eq!(left.to_bits().wrapping_add(right.to_bits()), 0);
    }

    /// `F-SKEL-1` end to end, through the trunk.
    #[test]
    fn growing_is_reproducible() {
        let manifest = manifest_of(&[
            ("src/a.rs", 700),
            ("src/b.rs", 300),
            ("docs/x.md", 90),
            ("scripts/y.sh", 40),
            ("tools/z.py", 30),
        ]);
        assert_eq!(grow(&manifest, &table()), grow(&manifest, &table()));
    }

    /// `P6`/`P7` again, now through the trunk: grouping must not lose anything either.
    #[test]
    fn every_path_survives_the_trunk() {
        let mut files: Vec<(alloc::string::String, u64)> =
            vec![("core/big.rs".to_string(), 900_000)];
        files.extend((0..25).map(|i| (alloc::format!("m{i:02}/lib.rs"), 300)));
        let listed: Vec<(&str, u64)> = files.iter().map(|(n, b)| (n.as_str(), *b)).collect();
        let manifest = manifest_of(&listed);

        let skeleton = grow(&manifest, &table());
        for record in manifest.paths() {
            assert!(
                skeleton.represents(&record.path),
                "{:?} was lost between the trunk and the canopy",
                record.path
            );
        }
    }

    #[test]
    fn every_node_hangs_from_the_basal_node() {
        let skeleton = grow(&manifest_of(&[("a/x.rs", 100), ("b/y.rs", 100)]), &table());
        assert_eq!(
            skeleton.nodes()[0].parent,
            None,
            "the basal node is the root"
        );
        for node in skeleton.nodes().iter().skip(1) {
            assert!(node.parent.is_some(), "only the basal node has no parent");
            assert!(node.parent.unwrap().index() < node.id.index());
        }
    }

    /// Root nodes point downward — they are mass at the base, not branches pointing the
    /// wrong way.
    #[test]
    fn the_root_cluster_sits_below_the_origin() {
        let skeleton = grow(&manifest_of(&[("src/a.rs", 100)]), &table());
        let roots: Vec<&Segment> = skeleton
            .segments()
            .iter()
            .filter(|segment| {
                matches!(
                    skeleton.node(segment.node).map(|n| &n.role),
                    Some(NodeRole::RootMass { .. })
                )
            })
            .collect();

        assert!(!roots.is_empty());
        for segment in roots {
            assert!(
                segment.end.y < Fx::ZERO,
                "a root node reached upward to {:?}",
                segment.end
            );
        }
    }

    /// The gathering rule closes a run once it is no longer small, and never leaves a thin
    /// remainder behind.
    #[test]
    fn gathering_closes_runs_at_the_threshold() {
        let manifest = manifest_of(&[
            ("a/f.rs", 10),
            ("b/f.rs", 10),
            ("c/f.rs", 10),
            ("d/f.rs", 10),
            ("e/f.rs", 10),
        ]);
        let root = RepoPath::root();
        let records: Vec<&PathRecord> = manifest
            .children(&root)
            .filter(|record| record.kind.is_container())
            .collect();

        // Ten bytes each against a threshold of 15: a run closes on its second member.
        let runs = gather(records.clone(), 15, 10);
        assert!(runs.iter().all(|run| !run.is_empty()));
        assert_eq!(
            runs.iter().map(Vec::len).sum::<usize>(),
            records.len(),
            "gathering must not drop a member"
        );
        assert_eq!(runs.len(), 2, "and the odd member joins the last run");

        // A threshold nothing reaches yields one run, not one run per member — until the
        // capacity bound takes over.
        assert_eq!(gather(records.clone(), u64::MAX, 10).len(), 1);
        let capped = gather(records.clone(), u64::MAX, 2);
        assert_eq!(capped.len(), 3, "five members at a capacity of two");
        assert!(
            capped.iter().all(|run| run.len() <= 2),
            "no run may exceed the capacity a limb has: {capped:?}"
        );
    }
}
