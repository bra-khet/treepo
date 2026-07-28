//! The hybrid trunk — `F-SKEL-3`, `F2`, `AC-SKEL-2`.
//!
//! [`grow`] is the product's entry point: a [`Manifest`] in, a [`Skeleton`] out. Everything
//! [`compose`](crate::lsystem::compose) does happens beneath what this module builds.
//!
//! # Why the trunk is hybrid, and what that costs
//!
//! `design/visual-construction.md` settled this against two alternatives and recorded why.
//! A **dedicated trunk** — a tall central column the limbs attach to — makes the trunk a
//! constant, so every repository shares a silhouette in the region a viewer looks at first.
//! A **pure trunkless stack** of independent branches was rejected for L-system
//! compatibility, redraw stability, and readable silhouettes: with no axiom there is nothing
//! for the productions to start from, and nothing that stays put when the repository changes.
//!
//! The hybrid takes the axiom from one and the mass from the other:
//!
//! * a **minimal basal segment**, a real L-system axiom, short and data-driven;
//! * **primary limbs** from the top-level directories, fanned narrowly enough that their
//!   base widths overlap;
//! * the **visual trunk** is that overlap. Nothing draws a trunk; the trunk is what several
//!   thick limbs leaving one point look like.
//!
//! The cost is stated plainly because it will look like a defect one day: **a repository with
//! one top-level directory has almost no trunk.** There is nothing to overlap. That is the
//! construction working as designed — the trunk depicts breadth at the root, and a repository
//! with no breadth there has none to depict.
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
//! A group is a *stem*, not a container: the same shape as the basal axiom, one level down.
//! Its members fan from its tip exactly as the primaries fan from the basal tip, so `F2` and
//! `F-SKEL-3` are one mechanism used twice rather than two that must be kept consistent.
//! [`NodeRole::Group`] is deliberately not [`NodeRole::Aggregate`] — a group draws what it
//! holds, and conflating them would report drawn paths as compressed.
//!
//! # `AC-SKEL-2`: a seed and a root cluster, never a lonely trunk
//!
//! An empty repository has no primary limbs, so it has no overlap, so it has no trunk — and
//! that falls out of the construction rather than needing a special case. What it does have
//! is the root-mass cluster, which every tree has, and a basal segment that stays at the
//! table's floor because its length is a proportion of its own width and there is nothing
//! here to be wide about. The result is a seed sitting in its roots, which is what the design
//! asks for and what a dedicated trunk could never have produced.

use crate::lsystem::compose::{self, Context, Site};
use crate::params::{SkeletonInputs, Table, TrunkParams};
use alloc::vec::Vec;
use treepo_det::{Angle, Fx, Seed, sin_cos};
use treepo_model::{Manifest, NodeId, NodeRole, PathRecord, Point, RepoPath, Segment, Skeleton};

/// The total angular spread of the root cluster, centred on straight down.
///
/// Not a table row: it is the difference between roots and branches rather than a tuning
/// knob. At 140° the outermost node sits 70° either side of vertical — splayed wide, but
/// still below the horizon, so no root can be mistaken for a limb pointing the wrong way.
const ROOT_SPREAD: Angle = Angle::from_millidegrees(140_000);

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

    // The basal node is the repository itself, so a click anywhere on the trunk resolves to
    // the repository rather than to nothing.
    let basal = skeleton.push_node(
        None,
        Point::ORIGIN,
        Angle::ZERO,
        seed,
        NodeRole::Limb { path: root.clone() },
    );

    let primaries = assign_primaries(manifest, table, &root, trunk.group_below);
    let width = stem_width(table, &primaries, trunk.packing);
    // Length from width, not from a row of its own: the stem is as wide as what it carries,
    // so tying its length to its width is what keeps a broad repository's axiom a short stem
    // rather than a disc, and a bare one's a seed.
    let tip = Point::new(Fx::ZERO, trunk.basal_length(width));

    skeleton.extend_segments([Segment {
        start: Point::ORIGIN,
        end: tip,
        base_width: width,
        // Near-cylindrical rather than tapered: the limbs leave the tip still overlapping,
        // and a basal segment that narrowed first would pinch the trunk exactly where the
        // mass is supposed to be.
        tip_width: width,
        node: basal,
        generation: 0,
    }]);

    root_cluster(&mut skeleton, basal, &seed, &root, &trunk, width);

    let mut ctx = Context::new(manifest, table, &mut skeleton);
    fan_out(
        &mut ctx,
        basal,
        Site {
            position: tip,
            heading: Angle::ZERO,
            carried: Some(width),
        },
        trunk.fan,
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

/// How wide a stem is: its limbs' combined base width, packed.
///
/// `F-SKEL-3`'s trunk mass, arithmetically. Summing the limbs' own widths rather than reading
/// a table row is what makes the trunk a consequence of what it carries — a repository that
/// grows a heavy new top-level directory thickens at the base because the limb is thick, not
/// because a separate number was tuned to agree.
fn stem_width(table: &Table, primaries: &[Primary<'_>], packing: Fx) -> Fx {
    let combined = primaries
        .iter()
        .flat_map(Primary::records)
        .map(|record| {
            let inputs = SkeletonInputs::from_record(record, &table.scales);
            table.limb_params(&inputs).base_width
        })
        .fold(Fx::ZERO, Fx::add);

    // An empty repository has no limbs and therefore no trunk to be wide. The seed still
    // needs a width to be drawn at, and the width the table gives a limb with every driver
    // at zero is the honest floor — anything else would be a constant invented here.
    let floor = table.limb_params(&SkeletonInputs::default()).base_width;
    combined.mul(packing).max(floor)
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
    // Root nodes are shorter and thicker than the basal segment: they read as mass at the
    // base rather than as branches pointing the wrong way.
    let length = trunk.basal_length(width).mul(Fx::from_ratio(3, 4));
    let node_width = width.mul(Fx::from_ratio(2, 5)).max(Fx::EPSILON);

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

/// Fans primary limbs from a stem's tip, and grows each.
///
/// `from.carried` is the stem's own width, so a limb leaving it inherits the falloff exactly
/// as a limb leaving another limb does — the basal axiom and an `F2` stem are branches, and a
/// branch is the thing a graft narrows against.
fn fan_out(
    ctx: &mut Context<'_>,
    stem: NodeId,
    from: Site,
    spread_angle: Angle,
    primaries: &[Primary<'_>],
    level: u8,
) {
    let count = u16::try_from(primaries.len()).unwrap_or(u16::MAX);
    for (index, primary) in primaries.iter().enumerate() {
        let at = Site {
            heading: spread(
                from.heading,
                spread_angle,
                u16::try_from(index).unwrap_or(u16::MAX),
                count,
            ),
            ..from
        };
        match primary {
            Primary::One(record) => {
                compose::place(ctx, &record.path, Some(stem), at, level);
            }
            Primary::Group(members) => place_group(ctx, stem, at, members, level),
        }
    }
}

/// Places one `F2` group: a short stem, with its members fanning from its tip.
///
/// The basal axiom's construction, one level down — see the module header on why that is one
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

    // A group stem is a branch leaving another branch, so it narrows at its own graft like
    // any limb would. Without this the trunk could sprout a stem wider than itself.
    let mut width = stem_width(ctx.table(), &held, trunk.packing);
    if let Some(carried) = at.carried {
        width = width.min(carried);
    }

    let (sin, cos) = sin_cos(at.heading);
    let length = trunk.basal_length(width);
    let tip = Point::new(
        at.position.x.add(sin.mul(length)),
        at.position.y.add(cos.mul(length)),
    );

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
    ctx.skeleton().extend_segments([Segment {
        start: at.position,
        end: tip,
        base_width: width,
        tip_width: width,
        node,
        generation: 0,
    }]);

    fan_out(
        ctx,
        node,
        Site {
            position: tip,
            heading: at.heading,
            carried: Some(width),
        },
        trunk.fan,
        &held,
        level,
    );
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

    /// The basal axiom as `(length, width)`, read off the skeleton.
    ///
    /// Read rather than recomputed from the table, deliberately: the axiom's length is a rule
    /// over the stem's width now, and a test that re-evaluated that rule would agree with the
    /// code by construction instead of measuring it.
    fn basal_stem(skeleton: &Skeleton) -> (Fx, Fx) {
        let basal = skeleton
            .nodes()
            .iter()
            .find(|node| node.parent.is_none())
            .expect("every tree has a basal node");
        skeleton
            .segments()
            .iter()
            .find(|segment| segment.node == basal.id)
            .map(|segment| (segment.end.y.sub(segment.start.y), segment.base_width))
            .expect("the basal node draws exactly one segment")
    }

    /// Just the axiom's length.
    fn axiom(skeleton: &Skeleton) -> Fx {
        basal_stem(skeleton).0
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

        // The whole thing is no taller than the basal axiom, which with no limbs to be wide
        // about stays at the table's floor. There is nothing here that reads as a trunk.
        let basal = axiom(&skeleton);
        assert!(
            height(&skeleton) <= basal,
            "an empty repository grew to {:?}, past its own basal segment {:?}",
            height(&skeleton),
            basal
        );
    }

    /// And the contrast that makes the previous test mean something.
    #[test]
    fn a_populated_repository_grows_far_past_its_basal_segment() {
        let manifest = manifest_of(&[
            ("src/main.rs", 5_000),
            ("src/lib.rs", 4_000),
            ("docs/guide.md", 1_000),
            ("tests/it.rs", 800),
        ]);
        let skeleton = grow(&manifest, &table());
        let basal = axiom(&skeleton);

        assert!(
            height(&skeleton) > basal.mul(Fx::from_int(3)),
            "a real repository must grow well past its axiom: {:?} vs {:?}",
            height(&skeleton),
            basal
        );
    }

    /// `F-SKEL-3`: the trunk is what its limbs make it. More and heavier top-level content
    /// means a wider base, without any row being tuned to say so.
    #[test]
    fn the_basal_segment_widens_with_what_it_carries() {
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

        let basal_width = |skeleton: &Skeleton| skeleton.segments()[0].base_width;
        assert!(
            basal_width(&broad) > basal_width(&narrow),
            "four heavy limbs must produce a wider base than one: {:?} vs {:?}",
            basal_width(&broad),
            basal_width(&narrow)
        );
    }

    /// How far above the basal tip the primary limbs are still overlapping — the height of
    /// the trunk, since the overlap *is* the trunk.
    ///
    /// Two limbs leaving one point `θ` apart are `2·d·sin(θ/2)` apart at distance `d`, so
    /// they stop overlapping once that exceeds their combined half-width: `d = (w₁+w₂) /
    /// (4·sin(θ/2))`. The trunk ends at the first adjacent pair to separate.
    fn trunk_height(skeleton: &Skeleton) -> Fx {
        let basal = skeleton.nodes()[0].id;
        let mut primaries: Vec<&treepo_model::SkeletonNode> = skeleton
            .nodes()
            .iter()
            .filter(|node| {
                node.parent == Some(basal) && !matches!(node.role, NodeRole::RootMass { .. })
            })
            .collect();
        // Left of centre is a wrapped-negative angle, so order by signed offset from up.
        primaries.sort_by_key(|node| node.heading.to_bits() as i32);

        let width_of = |node: &treepo_model::SkeletonNode| {
            skeleton
                .segments()
                .iter()
                .find(|segment| segment.node == node.id)
                .map_or(Fx::ZERO, |segment| segment.base_width)
        };

        let mut lowest = Fx::MAX;
        for pair in primaries.windows(2) {
            let separation = pair[1].heading - pair[0].heading;
            let (half_sin, _) = sin_cos(Angle::from_bits(separation.to_bits() / 2));
            let combined = width_of(pair[0]).add(width_of(pair[1]));
            let breaks = combined
                .checked_div(half_sin.mul(Fx::from_int(4)))
                .unwrap_or(Fx::MAX);
            lowest = lowest.min(breaks);
        }
        lowest
    }

    /// The axiom is a stem at every scale, because its length is a rule over its own width.
    ///
    /// It used to be an absolute row while the stem's width was the sum of its limbs' widths,
    /// so a broad repository got a stem many limb-widths across and a fifth of a limb-length
    /// long. That draws as a disc, and `m0-silhouette` is where it was finally seen.
    ///
    /// Two claims, and the pair is what makes it a *stem*: the axiom lengthens with the mass
    /// it carries, and it never stops being stubby. Sabotage: pin `basal_length` to a
    /// constant again and the first assertion fails.
    #[test]
    fn the_basal_axiom_stays_a_stem_however_broad_the_repository() {
        let slim = grow(
            &manifest_of(&[("core/a.rs", 10_000), ("docs/b.md", 9_000)]),
            &table(),
        );

        // Twelve equal top-level directories: each is well above `group_below`, so every one
        // earns a primary limb and the stem has twelve widths to add up.
        let files: Vec<(alloc::string::String, u64)> = (0..12)
            .map(|i| (alloc::format!("dir{i:02}/main.rs"), 10_000))
            .collect();
        let listed: Vec<(&str, u64)> = files.iter().map(|(n, b)| (n.as_str(), *b)).collect();
        let broad = grow(&manifest_of(&listed), &table());

        let (slim_length, slim_width) = basal_stem(&slim);
        let (broad_length, broad_width) = basal_stem(&broad);

        assert!(
            broad_width > slim_width,
            "twelve primaries must make a wider stem than two: {broad_width:?} vs {slim_width:?}"
        );
        assert!(
            broad_length > slim_length,
            "a wider stem must get a longer axiom, or it draws as a disc: {broad_length:?} \
             against {slim_length:?}"
        );

        // `F-SKEL-3`'s "minimal", as a shape: stubby at both scales, and never flat at either.
        for (length, width, name) in [
            (slim_length, slim_width, "slim"),
            (broad_length, broad_width, "broad"),
        ] {
            assert!(
                length < width.mul(Fx::from_int(2)),
                "the {name} axiom is longer than twice its width — that is a trunk"
            );
            assert!(
                length.mul(Fx::from_int(4)) > width,
                "the {name} axiom is under a quarter of its own width — that is a disc"
            );
        }
    }

    /// `F-SKEL-3`'s central claim, measured: the trunk is the overlap of the primary limbs,
    /// and the fan is what controls it. Nothing draws a trunk, so if the overlap does not
    /// extend past the axiom there is no trunk however good the numbers look.
    #[test]
    fn the_fan_controls_how_far_the_trunk_extends() {
        let repository = manifest_of(&[
            ("alpha/a.rs", 10_000),
            ("beta/b.rs", 10_000),
            ("gamma/c.rs", 10_000),
            ("delta/d.rs", 10_000),
        ]);

        let at_fan = |millidegrees: i32| {
            let mut table = Table::built_in();
            table.trunk.fan.base = millidegrees;
            table.trunk.fan.min = millidegrees;
            table.trunk.fan.max = millidegrees;
            table
                .validate()
                .expect("a pinned fan is still a valid table");
            let skeleton = grow(&repository, &table);
            (trunk_height(&skeleton), axiom(&skeleton))
        };

        let (narrow, axiom) = at_fan(30_000);
        let (wide, _) = at_fan(150_000);

        assert!(
            narrow > wide,
            "widening the fan must shorten the trunk: {narrow:?} at 30°, {wide:?} at 150°"
        );
        assert!(
            wide > axiom,
            "even at its widest the overlap must outlast the axiom, or there is no trunk \
             at all: {wide:?} against an axiom of {axiom:?}"
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
