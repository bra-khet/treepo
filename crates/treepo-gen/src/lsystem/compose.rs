//! Hierarchical composition — `F-SKEL-2`, `F-SKEL-7`, `A3`, `P6`, `P7`.
//!
//! §2.4: "The global tree is a hierarchical composition of smaller L-systems rather than one
//! gigantic flat derivation." This module is that composition, and it is where the one
//! genuinely open question in the skeleton design is answered.
//!
//! # The phase transition
//!
//! A directory's children are not all the same kind of thing to draw. The rule here is
//! neither "one branch per child" nor "a fixed arity":
//!
//! * While a limb's children stay **below its critical density**, each is a first-class
//!   hierarchical L-system instance in its own right, seeded by its own path hash — the
//!   recursive, structure-driven composition `F-SKEL-2` asks for.
//! * When the local structure **crosses** that threshold, the excess collapses into one or
//!   more [`AggregateNode`]s. A container is a first-class skeleton citizen: it owns a
//!   position, a heading, a seed, and a place in the parent chain, and a later phase is free
//!   to give it a form and hang enrichment off it.
//! * **Inside** any one instance the productions stay classic low-arity parametric and
//!   stochastic. The phase decision lives here, at the composition boundary, and never
//!   inside [`grammar`](super::grammar).
//!
//! That is a phase transition, not a truncation, and the distinction is `P6` exactly:
//! *legibility bounds detail; honesty bounds data*. The manifest is untouched; what changed
//! is how much of it gets its own geometry. `F-MAT-3` completes the argument from the other
//! end — a container "discharges the [minimum representation] floor on behalf of everything
//! it represents" — which is what makes `P7` and a legible T3 repository compatible at all.
//!
//! # Where the threshold comes from
//!
//! Two limits meet, and both are `A3`.
//!
//! **Per limb**, [`LimbParams::branch_capacity`] says how many children may stay first-class.
//! It is driven by bushiness, convention, mass and skew through the parameter table, because
//! a bushy, conventionally-named directory stays readable at a width a lopsided one does
//! not. It is bounded above by the attachment sites the limb's own derivation offers —
//! `2^generations` — so the table cannot promise more children than the grammar can carry.
//!
//! **Per hierarchy**, [`Table::max_levels`] says how many levels of directory nesting get
//! limbs at all. Past it, an entire subtree becomes one container, which is `A3`'s "deeper
//! content is aggregated" and `design/design-outline.md` §6's "an object can simply represent
//! *this directory and all its contents*".
//!
//! # How the residue is split, and why it is split that way
//!
//! Three decisions, each of which has a plausible alternative that is worse:
//!
//! 1. **Which children stay first-class: the largest.** Significance is subtree bytes, ties
//!    broken by path so the order is total and deterministic. `N4` is untouched — this ranks
//!    paths, never people.
//! 2. **How many containers: proportional to the residue's mass.** A residue that is 60% of
//!    the children but 5% of the bytes gets one container; a flat directory of a thousand
//!    equal files gets containers across nearly every site. That is `F-SKEL-7`'s
//!    "proportional container" applied to the count as well as the size, and it means the
//!    silhouette reports the shape of what it compressed.
//! 3. **What goes in which container: path-adjacent runs.** The residue is re-sorted by path
//!    and cut into contiguous runs, so a container holds siblings that sit together in the
//!    repository rather than an arbitrary scattering. Grouping by significance instead would
//!    make "open this cluster" show an unrelated assortment, which is the mental model
//!    `design/design-outline.md` §6 exists to protect.
//!
//! A container's seed comes from its anchor and its index, never from its membership — so a
//! file arriving in a container does not reroll the container's appearance. The cluster stays
//! the cluster and gains a member, which is what keeps `AC-GROW-4`'s "localized to the
//! affected limb" true of aggregated regions too.

use super::grammar;
use super::turtle::{self, Start, Tip};
use crate::params::{SkeletonInputs, Table};
use alloc::vec::Vec;
use treepo_det::{Angle, Fx};
use treepo_model::{
    AggregateNode, Manifest, NodeId, NodeRole, PathRecord, Point, RepoPath, Skeleton,
};

/// Builds a repository's structural skeleton.
///
/// `origin` and `heading` are where the basal limb starts and which way it leaves;
/// `trunk.rs` will supply them from total root mass and primary limb count (`F-SKEL-3`).
/// Until then a caller passes the origin and straight up.
#[must_use]
pub fn compose(manifest: &Manifest, table: &Table, origin: Point, heading: Angle) -> Skeleton {
    let mut skeleton = Skeleton::new();
    let root = RepoPath::root();
    place(
        &mut Context {
            manifest,
            table,
            skeleton: &mut skeleton,
        },
        &root,
        None,
        Site {
            position: origin,
            heading,
            // Nothing upstream: the root limb is as wide as its own mass says and no wider.
            carried: None,
        },
        0,
    );
    skeleton
}

/// Everything the recursion carries unchanged.
///
/// Shared with [`trunk`](crate::trunk), which owns the frame this composition starts from and
/// then hands each primary limb back here. Fields stay private so the one mutable thing —
/// the skeleton under construction — is reached through a method rather than being a field
/// anyone can swap.
pub(crate) struct Context<'a> {
    manifest: &'a Manifest,
    table: &'a Table,
    skeleton: &'a mut Skeleton,
}

impl<'a> Context<'a> {
    /// A context for composing into `skeleton`.
    pub(crate) const fn new(
        manifest: &'a Manifest,
        table: &'a Table,
        skeleton: &'a mut Skeleton,
    ) -> Self {
        Self {
            manifest,
            table,
            skeleton,
        }
    }

    /// The parameter table in force.
    pub(crate) const fn table(&self) -> &Table {
        self.table
    }

    /// The skeleton under construction.
    pub(crate) const fn skeleton(&mut self) -> &mut Skeleton {
        self.skeleton
    }

    /// A path's generator seed (`P2`).
    pub(crate) fn seed_for(&self, path: &RepoPath) -> treepo_det::Seed {
        self.manifest.seed_for(path)
    }
}

/// Where a limb attaches, and what it inherits from there.
///
/// The three things travel together everywhere — a tip hands all of them to the child that
/// grows from it — so they are one value rather than three parameters that a call site could
/// pair up wrongly.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Site {
    /// The point the limb leaves from.
    pub position: Point,
    /// The direction it leaves in.
    pub heading: Angle,
    /// The width of the branch at this point, or `None` at the base of the tree, where
    /// nothing upstream constrains how thick the first thing may be.
    pub carried: Option<Fx>,
}

impl From<&turtle::Tip> for Site {
    fn from(tip: &turtle::Tip) -> Self {
        Self {
            position: tip.position,
            heading: tip.heading,
            carried: Some(tip.width),
        }
    }
}

/// Places one path as a limb, and everything beneath it.
pub(crate) fn place(
    ctx: &mut Context<'_>,
    path: &RepoPath,
    parent: Option<NodeId>,
    at: Site,
    level: u8,
) -> NodeId {
    let inputs = inputs_for(ctx.manifest, ctx.table, path);
    let params = match at.carried {
        Some(carried) => ctx.table.limb_params(&inputs).grafted_onto(carried),
        None => ctx.table.limb_params(&inputs),
    };
    let seed = ctx.manifest.seed_for(path);

    let node = ctx.skeleton.push_node(
        parent,
        at.position,
        at.heading,
        seed,
        NodeRole::Limb { path: path.clone() },
    );

    let children = significant_children(ctx.manifest, path);

    // The derivation is asked for as many sites as there are children, capped by what this
    // limb may carry. Fewer children than capacity is the ordinary case and costs nothing.
    let requested = u16::try_from(children.len())
        .unwrap_or(u16::MAX)
        .min(params.branch_capacity)
        .max(1);

    let modules = grammar::derive(&params, ctx.table.min_length(), &seed, requested);
    let interpretation = turtle::interpret(
        &modules,
        Start {
            position: at.position,
            heading: at.heading,
            node,
        },
        params.tropism,
    );
    ctx.skeleton.extend_segments(interpretation.segments);

    if children.is_empty() {
        return node;
    }

    // The sites the limb actually produced, which is the real capacity: the termination
    // threshold can end a branch before the generation cap does, and a limb that offers six
    // sites cannot carry eight children however generous the table was.
    let tips = &interpretation.tips;
    let capacity = u16::try_from(tips.len()).unwrap_or(u16::MAX);

    // `A3` at the hierarchy scale: past the level cap nothing descends, so every child
    // becomes container content rather than a limb.
    let descend = level < ctx.table.max_levels();
    let plan = partition(&children, capacity, descend);

    attach(ctx, path, node, tips, &plan, level);
    node
}

/// How one limb's children are divided between limbs of their own and containers.
struct Plan<'a> {
    /// Children that stay first-class, in path order.
    limbs: Vec<&'a PathRecord>,
    /// Containers, each holding a path-adjacent run of the residue.
    containers: Vec<Vec<&'a PathRecord>>,
}

/// The phase decision — see the module header for the three rules this implements.
fn partition<'a>(children: &[&'a PathRecord], capacity: u16, descend: bool) -> Plan<'a> {
    let capacity = usize::from(capacity);

    // Past the hierarchy cap nothing stays first-class, whatever the capacity says.
    let below_threshold = descend && children.len() <= capacity;
    if below_threshold {
        let mut limbs: Vec<&PathRecord> = children.to_vec();
        limbs.sort_by(|a, b| a.path.cmp(&b.path));
        return Plan {
            limbs,
            containers: Vec::new(),
        };
    }

    // How many sites the residue deserves, proportional to the mass it carries. Computed
    // against the provisional residue — the children that would not have fitted anyway — so
    // this needs no iteration to settle.
    let first_class = if descend {
        capacity.min(children.len())
    } else {
        0
    };
    let provisional: &[&PathRecord] = &children[first_class..];
    let container_slots = slots_for(provisional, children, capacity);

    let keep = if descend {
        capacity.saturating_sub(container_slots).min(children.len())
    } else {
        0
    };

    let mut limbs: Vec<&PathRecord> = children[..keep].to_vec();
    limbs.sort_by(|a, b| a.path.cmp(&b.path));

    // Path order, not significance order: a container should hold siblings that sit together
    // in the repository, so that opening one shows a region rather than an assortment.
    let mut residue: Vec<&PathRecord> = children[keep..].to_vec();
    residue.sort_by(|a, b| a.path.cmp(&b.path));

    Plan {
        limbs,
        containers: split_evenly(residue, container_slots.max(1)),
    }
}

/// How many attachment sites the residue is entitled to.
fn slots_for(residue: &[&PathRecord], all: &[&PathRecord], capacity: usize) -> usize {
    if residue.is_empty() {
        return 0;
    }
    let total: u64 = all.iter().map(|record| record.size.bytes).sum();
    let share: u64 = residue.iter().map(|record| record.size.bytes).sum();

    // A repository of empty files has no mass to apportion by, and every child is then
    // equally unimportant — one container is the honest answer rather than a division trap.
    // Integer throughout: `N3` forbids a float here as anywhere else in the pipeline.
    let slots = share
        .saturating_mul(capacity as u64)
        .saturating_add(total / 2)
        .checked_div(total)
        .and_then(|scaled| usize::try_from(scaled).ok())
        .unwrap_or(1);
    slots.clamp(1, capacity.max(1))
}

/// Cuts a list into `parts` contiguous runs of near-equal length.
///
/// Equal *count* rather than equal mass: a mass-balanced cut would move every boundary when
/// one file changed size, where this moves at most one member per boundary when a file is
/// added. `AC-GROW-4` wants the visible change localized, and a partition that reshuffles
/// under an unrelated edit is the opposite of that.
fn split_evenly<T>(items: Vec<T>, parts: usize) -> Vec<Vec<T>> {
    let total = items.len();
    if total == 0 {
        return Vec::new();
    }
    let parts = parts.clamp(1, total);
    let base = total / parts;
    // The remainder goes to the earliest runs rather than the last, so lengths differ by at
    // most one and no container is left holding a stray tail.
    let extra = total % parts;

    let mut out = Vec::with_capacity(parts);
    let mut rest = items;
    for part in 0..parts {
        let tail = rest.split_off(base + usize::from(part < extra));
        out.push(rest);
        rest = tail;
    }
    debug_assert!(rest.is_empty(), "every member must land in exactly one run");
    out
}

/// Hangs a limb's children and containers on its attachment sites.
fn attach(
    ctx: &mut Context<'_>,
    path: &RepoPath,
    node: NodeId,
    tips: &[Tip],
    plan: &Plan<'_>,
    level: u8,
) {
    let mut site = tips.iter();

    // Limbs and containers are interleaved by path order, so a limb's children read
    // left-to-right in the order they appear in the repository, containers included.
    let mut order: Vec<Attachment<'_>> = plan
        .limbs
        .iter()
        .map(|record| Attachment::Limb(record))
        .chain(
            plan.containers
                .iter()
                .enumerate()
                .map(|(index, run)| Attachment::Container(index, run)),
        )
        .collect();
    order.sort_by(|a, b| a.sort_key().cmp(b.sort_key()));

    for attachment in order {
        let Some(tip) = site.next() else {
            // Unreachable while `partition` respects the site count, and harmless if it ever
            // does not: the remaining children stay accounted for in the manifest.
            break;
        };
        match attachment {
            Attachment::Limb(record) => {
                place(
                    ctx,
                    &record.path,
                    Some(node),
                    Site::from(tip),
                    level.saturating_add(1),
                );
            }
            Attachment::Container(index, members) => {
                let aggregate = build_aggregate(path, index, members);
                let seed = ctx
                    .manifest
                    .seed_for(path)
                    .derive(b"aggregate")
                    .derive(&aggregate.index.to_le_bytes());
                ctx.skeleton.push_node(
                    Some(node),
                    tip.position,
                    tip.heading,
                    seed,
                    NodeRole::Aggregate(aggregate),
                );
            }
        }
    }
}

/// One thing waiting for an attachment site.
enum Attachment<'a> {
    Limb(&'a PathRecord),
    Container(usize, &'a [&'a PathRecord]),
}

impl Attachment<'_> {
    /// Where this sits in path order. A container sorts by its first member.
    fn sort_key(&self) -> &RepoPath {
        match self {
            Self::Limb(record) => &record.path,
            // `partition` never builds an empty run, and `split_evenly` asserts it.
            Self::Container(_, members) => &members[0].path,
        }
    }
}

/// Rolls a run of children up into one container.
fn build_aggregate(anchor: &RepoPath, index: usize, members: &[&PathRecord]) -> AggregateNode {
    let mut bytes = 0u64;
    let mut file_count = 0u32;
    let mut dir_count = 0u32;

    for record in members {
        bytes = bytes.saturating_add(record.size.bytes);
        // A member counts as itself plus everything beneath it: `descendant_*` counts do not
        // include the path they describe, so a directory absorbed here contributes its own
        // entry as well as its subtree's.
        if record.kind.is_container() {
            dir_count = dir_count.saturating_add(1);
        } else {
            file_count = file_count.saturating_add(1);
        }
        file_count = file_count.saturating_add(record.structural.descendant_file_count);
        dir_count = dir_count.saturating_add(record.structural.descendant_dir_count);
    }

    AggregateNode {
        anchor: anchor.clone(),
        index: u16::try_from(index).unwrap_or(u16::MAX),
        members: members.iter().map(|record| record.path.clone()).collect(),
        bytes,
        file_count,
        dir_count,
    }
}

/// A path's children, largest first, ties broken by path.
///
/// `sort_by` rather than `sort_unstable_by`: the tie-break makes the order total, so the two
/// agree — but the unstable sort's behaviour on equal elements is unspecified across Rust
/// versions, and this order reaches generated output.
pub(crate) fn significant_children<'a>(
    manifest: &'a Manifest,
    path: &'a RepoPath,
) -> Vec<&'a PathRecord> {
    let mut children: Vec<&PathRecord> = manifest.children(path).collect();
    children.sort_by(|a, b| {
        b.size
            .bytes
            .cmp(&a.size.bytes)
            .then_with(|| a.path.cmp(&b.path))
    });
    children
}

/// The drivers for a path, or the neutral drivers where the manifest has no record for it.
///
/// The repository root often has no record of its own — the walk describes what is *in* the
/// repository — and a root that failed to compose would be a tree that never grew. Neutral
/// inputs give it the table's base values, which `trunk.rs` will replace with an axiom sized
/// from total root mass and primary limb count (`F-SKEL-3`).
pub(crate) fn inputs_for(manifest: &Manifest, table: &Table, path: &RepoPath) -> SkeletonInputs {
    manifest
        .path(path)
        .map_or_else(SkeletonInputs::default, |record| {
            SkeletonInputs::from_record(record, &table.scales)
        })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use alloc::string::{String, ToString};
    use alloc::vec;
    use treepo_det::{Fx, Seed};
    use treepo_model::{NodeKind, Skeleton};

    /// The content category a fixture path is treated as, from its extension.
    ///
    /// A miniature of `treepo-vcs::lang`, and deliberately not a call into it: this crate has
    /// no dependency on the extractor and should not grow one for a test. Anything
    /// unrecognized is `Unknown`, which is what the real catalogue does too.
    fn category_of(path: &str) -> treepo_model::primitives::size::ContentCategory {
        use treepo_model::primitives::size::ContentCategory as C;
        match path.rsplit('.').next() {
            Some("rs" | "py" | "ts" | "go") => C::Code,
            Some("png" | "jpg" | "woff") => C::Asset,
            Some("toml" | "json" | "yml") => C::Config,
            Some("md" | "txt") => C::Docs,
            Some("lock") => C::Generated,
            Some("bin") => C::Binary,
            _ => C::Unknown,
        }
    }

    /// Per-author attributed lines for one fixture file.
    ///
    /// A miniature of `treepo-vcs::log_pass`, and shaped for the two things the material walk
    /// needs to be able to tell apart. Each top-level subtree has its own owner, so a node
    /// gathering paths from one subtree does not resolve to the same contributors as a node
    /// gathering another — without which
    /// `an_owned_group_is_owned_by_its_members_not_by_its_anchor` could not fail under the bug
    /// it exists to catch. And one contributor touches everything lightly, so every node has a
    /// minority holder and `AC-MAT-2` has something to be true of.
    fn authors_of(path: &str, bytes: u64) -> Vec<(treepo_model::identity::AuthorKey, u64)> {
        use treepo_model::identity::AuthorKey;
        let top = path.split('/').next().unwrap_or(path);
        let owner = AuthorKey::from_email(alloc::format!("{top}@example.test").as_bytes());
        let visitor = AuthorKey::from_email(b"visitor@example.test");

        // Forty bytes a line, near enough for a fixture; the ratio between files is what the
        // weighted merge is measured on, not the absolute count.
        //
        // The visitor holds a fiftieth — two percent, which is the contributor `AC-MAT-2` names
        // by number. Set here rather than at some comfortable ten percent so the criterion is
        // carried by the fixture the walk actually runs on, and so the guaranteed quota is
        // load-bearing on a coarse mosaic instead of only in a unit test.
        let lines = (bytes / 40).max(2);
        let minority = (lines / 50).max(1);
        vec![(owner, lines - minority), (visitor, minority)]
    }

    /// The reference time every fixture age is measured against.
    ///
    /// A fixed constant, not a clock. `Manifest::reference_time` is the newest commit in the
    /// repository precisely so that a tree does not drift daily on a machine doing nothing
    /// (`AC-MAN-1`), and a fixture that read the clock would make its own assertions expire.
    pub(crate) const REFERENCE: i64 = 1_800_000_000;

    /// First and last commit times for one fixture path, as absolute epoch seconds.
    ///
    /// A miniature of `treepo-vcs::log_pass`'s dating, shaped so that `F-MAT-4` has something
    /// to be true of. Both ends vary with the path, and every file spans a real interval, so a
    /// fixture node has an actual gradient rather than a uniform one — without which
    /// `a_long_lived_limb_is_old_at_the_base_and_new_at_the_tip` would pass for the wrong
    /// reason.
    ///
    /// Derived from the path bytes rather than from an index, so a manifest built from the
    /// same files in a different order dates them identically — the same property real
    /// extraction has, and one an index-based fixture would quietly lack.
    fn history_of(path: &str) -> (i64, i64) {
        const DAY: i64 = 86_400;
        let spread = path.bytes().map(i64::from).sum::<i64>();
        // 60..=1259 days back for the first commit, 0..=39 for the last, so the first is
        // always the older of the two however the arithmetic lands.
        let first = REFERENCE - (spread % 1200 + 60) * DAY;
        let last = REFERENCE - (spread % 40) * DAY;
        (first, last)
    }

    /// How many of one fixture file's lines changed inside a churn window of `window` days.
    ///
    /// `F-MAT-5`'s work sites read the thirty-day window and `F-MAT-6`'s restlessness the
    /// ninety-day one, so a fixture leaving either at zero would make a whole signal untestable —
    /// no kind placed, or no node stressed.
    ///
    /// Derived from the date [`history_of`] already assigned rather than from a second
    /// independent knob, so the fixture cannot describe a path that was touched a year ago and
    /// churned heavily last week. Linear decay across the window: a file touched today has all
    /// of its churn inside it, one touched at or beyond the window has none. The ninety-day
    /// window therefore contains the thirty-day one, as a real pair of windows does.
    fn recent_churn_of(path: &str, lines: u64, window: i64) -> u64 {
        const DAY: i64 = 86_400;
        let days = (REFERENCE - history_of(path).1) / DAY;
        if days >= window {
            return 0;
        }
        lines.saturating_sub(lines.saturating_mul(days as u64) / window as u64)
    }

    /// Bytes at or above which a fixture file counts toward `large_file_debt`.
    ///
    /// A fixture threshold, not `treepo-vcs`'s — this crate has no dependency on the extractor
    /// and should not grow one for a test, the same reason [`category_of`] is a miniature. Set so
    /// that the shaped repositories the material tests use contain files on both sides of it;
    /// were everything large or everything small, `Sparse` could not be told from a dead signal.
    const LARGE_FILE_BYTES: u64 = 16_384;

    /// The per-record counters a real content pass accumulates and rolls up.
    ///
    /// [`PathRecord`] has no field for any of them, because extraction divides them away into
    /// [`DerivedSignals`](treepo_model::DerivedSignals) and `stability` before it stores anything.
    /// A fixture that wants those ratios to be *rolled up* rather than per-file has to carry the
    /// numerators the same way `treepo-vcs::lang` does.
    #[derive(Debug, Clone, Copy, Default)]
    struct Counters {
        /// Unfinished-work markers.
        markers: u64,
        /// Bytes in files at or above [`LARGE_FILE_BYTES`].
        large_bytes: u64,
        /// Lines changed inside the thirty-day window — `F-MAT-5`.
        churn_30: u64,
        /// Lines changed inside the ninety-day window — `F-MAT-6`.
        churn_90: u64,
    }

    /// One fixture file's line counts and debt signals — `F-MAT-6`'s inputs.
    ///
    /// A miniature of `treepo-vcs::lang`'s content pass and of `apply_history_signals`. Without
    /// it every `DerivedSignals` field and `stability` stay `None`, `Material::stress` is `None`
    /// on every node, and every assertion about stress over this fixture passes vacuously —
    /// which is the trap the enrichment slice hit with its churn windows and folder signals.
    ///
    /// The three signals are given a **spread** rather than a value, and that matters as much as
    /// their being present: a fixture where every path was equally cracked could not tell a
    /// working floor from a broken one, and one where nothing was cracked could not tell a
    /// working signal from a dead one. Derived from the path bytes, like every other knob here,
    /// so a manifest built from the same files in a different order signals them identically.
    ///
    /// `generated_debt` is set too, for one reason: `treepo-gen::stress` deliberately does *not*
    /// read it, and a fixture that left it `None` could not tell that decision from an oversight.
    fn content_of(path: &str, bytes: u64) -> (treepo_model::primitives::size::LineCounts, u64) {
        use treepo_model::primitives::size::{ContentCategory as C, LineCounts};

        // Asset and binary content is never read, so it has no line count and no marker count —
        // the same refusal `treepo-vcs::lang` makes, and what gives the walk nodes whose debt is
        // genuinely *unmeasured* rather than measured clean. Without it every path in the fixture
        // would carry every signal and `F-MAT-6`'s `None` case would never be reached by a walk.
        if matches!(category_of(path), C::Asset | C::Binary) {
            return (LineCounts::default(), 0);
        }

        // Forty bytes a line, the same ratio `authors_of` uses — a fixture where the two
        // disagreed would describe a file whose attributed lines outnumber its lines.
        let lines = (bytes / 40).max(2);

        // A tenth blank, a tenth comment, the rest code, so `code` is a real denominator for the
        // marker density and `total` a real one for stability.
        let blank = lines / 10;
        let comment = lines / 10;
        let counts = LineCounts {
            total: lines,
            code: lines - blank - comment,
            comment,
            blank,
        };

        // Nought to two markers, capped at the lines there are to put them on. A count rather
        // than a density, because the density is what `treepo-vcs` *divides out* of a count after
        // rolling it up, and a fixture setting the ratio directly would have to reimplement
        // `debt_over`'s weighting to roll it up — the code under test.
        //
        // The consequence is that a small file carrying one marker reads as heavily cracked while
        // a four-hundred-line file carrying two reads as faintly cracked. That is not a fixture
        // artefact; it is what a marker density *is*, and real extraction produces the same thing.
        let spread = path.bytes().map(u64::from).sum::<u64>();
        (counts, (spread % 3).min(counts.code))
    }

    /// The folder-convention record a fixture directory carries, if the name is one.
    ///
    /// A miniature of `treepo-vcs`'s `F-EXT-5` pass over
    /// `assets/params/folder-signals.ron`. `F-MAT-5` reads `test_like_ratio` off this and
    /// nothing else does yet, so the non-test entries are here on purpose: a signal-reading bug
    /// that fired on *any* folder signal rather than on the ratio would pass a fixture where
    /// only `tests` had one.
    fn folder_signal_of(name: &str) -> Option<treepo_model::FolderSignal> {
        let test_like = match name {
            "tests" | "test" => Fx::ONE,
            "src" | "docs" | "assets" | "vendor" | "examples" => Fx::ZERO,
            _ => return None,
        };
        let mut signal =
            treepo_model::FolderSignal::unmodulated(name.into(), Fx::from_ratio(4, 5), 1);
        signal.content_modulation.test_like_ratio = test_like;
        Some(signal)
    }

    /// Builds a manifest from a list of `(path, bytes)` files, synthesizing every ancestor
    /// directory and rolling the structural counts up as `treepo-vcs::walk` would.
    ///
    /// Worth the eighty lines: composition reads `child_count`, `descendant_*`,
    /// `max_subtree_depth`, `relative_bytes` and the branching histogram, and a fixture that
    /// left them at their defaults would exercise the code with every driver reading zero.
    ///
    /// `category_bytes`, per-author line counts and commit dates are filled and rolled up for
    /// the same reason, one phase later. `treepo-gen::material` reads a directory's category
    /// mixture, its contributor set and its history span, and relies on extraction having
    /// gathered all three from the subtree; a fixture that left them empty would make every
    /// directory [`Stone`](treepo_model::MaterialFamily::Stone) with an empty mosaic and no
    /// gradient, and every material assertion vacuously true.
    ///
    /// Note this cannot disturb the skeleton: `params`'s `G1` row keeps every temporal
    /// primitive out of the geometry, and `no_temporal_primitive_reaches_the_skeleton` is the
    /// test that says so.
    pub(crate) fn manifest_of(files: &[(&str, u64)]) -> Manifest {
        use treepo_det::OrderedMap;
        use treepo_model::identity::AuthorKey;

        let mut records: Vec<PathRecord> = Vec::new();
        let mut counts: Vec<OrderedMap<AuthorKey, u64>> = Vec::new();
        let mut counters: Vec<Counters> = Vec::new();
        let mut seen: Vec<RepoPath> = Vec::new();

        for (text, bytes) in files {
            let path = RepoPath::new(text.as_bytes()).unwrap();
            let mut record = PathRecord::new(path.clone(), NodeKind::File);
            record.size.bytes = *bytes;
            record.size.category_bytes = core::iter::once((category_of(text), *bytes)).collect();
            let (first, last) = history_of(text);
            record.temporal.first_commit_time = Some(first);
            record.temporal.last_commit_time = Some(last);
            let (lines, markers) = content_of(text, *bytes);
            record.size.lines = lines;
            seen.push(path.clone());
            records.push(record);

            let attributed: OrderedMap<AuthorKey, u64> =
                authors_of(text, *bytes).into_iter().collect();
            let churned: u64 = attributed.values().sum();
            counters.push(Counters {
                markers,
                large_bytes: if *bytes >= LARGE_FILE_BYTES {
                    *bytes
                } else {
                    0
                },
                churn_30: recent_churn_of(text, churned, 30),
                churn_90: recent_churn_of(text, churned, 90),
            });
            counts.push(attributed);

            let mut ancestor = path.parent();
            while let Some(current) = ancestor {
                ancestor = current.parent();
                if !seen.contains(&current) {
                    seen.push(current.clone());
                    let mut directory = PathRecord::new(current, NodeKind::Directory);
                    directory.folder_signal = directory
                        .path
                        .file_name()
                        .and_then(|name| core::str::from_utf8(name).ok())
                        .and_then(folder_signal_of);
                    records.push(directory);
                    counts.push(OrderedMap::new());
                    counters.push(Counters::default());
                }
            }
        }
        if !seen.contains(&RepoPath::root()) {
            records.push(PathRecord::new(RepoPath::root(), NodeKind::Directory));
            counts.push(OrderedMap::new());
            counters.push(Counters::default());
        }

        roll_up(&mut records, &mut counts, &mut counters);

        // Attributed lines and lifetime churn come out of one per-commit tally in the real log
        // pass, so `lifetime` is exactly the sum of the author counts. `treepo-gen::material`
        // weights by it when merging a container's contributors, and a fixture where the two
        // disagreed would be measuring a weight no repository produces.
        //
        // The thirty-day window is a share of that same tally, rolled up the same way, because
        // `F-MAT-5` reads one against the other — a fixture where the windows came from
        // somewhere else could describe a directory churning harder this month than it ever has.
        for ((record, counts), counters) in records.iter_mut().zip(counts).zip(counters) {
            record.temporal.churn.lifetime = counts.values().sum();
            record.temporal.churn.days_30 = counters.churn_30.min(record.temporal.churn.lifetime);
            record.temporal.churn.days_90 = counters
                .churn_90
                .max(record.temporal.churn.days_30)
                .min(record.temporal.churn.lifetime);
            record.ownership =
                treepo_model::primitives::ownership::OwnershipPrimitives::from_line_counts(
                    &counts,
                    OrderedMap::new(),
                );

            // The `F-EXT-6` signals and `stability`, divided out of the rolled-up counters
            // exactly where `treepo-vcs::lang` divides them — after the roll-up, so a
            // directory's ratio is its whole subtree's rather than an average of its children's.
            // `treepo-gen::material::debt_over` reconstructs these when it merges several
            // records, and a fixture that computed them any other way would be measuring a
            // weight no repository produces.
            let lines = record.size.lines;
            let all_bytes = record.size.bytes;
            let generated = record
                .size
                .category_bytes
                .get(&treepo_model::primitives::size::ContentCategory::Generated)
                .copied()
                .unwrap_or(0);
            record.derived = treepo_model::DerivedSignals {
                todo_density: (lines.code > 0)
                    .then(|| Fx::from_ratio(counters.markers as i64 * 1000, lines.code as i64)),
                large_file_debt: (all_bytes > 0)
                    .then(|| Fx::from_ratio(counters.large_bytes as i64, all_bytes as i64)),
                // Set although `treepo-gen::stress` deliberately never reads it — a fixture that
                // left it `None` could not tell that decision from an oversight.
                generated_debt: (all_bytes > 0)
                    .then(|| Fx::from_ratio(generated as i64, all_bytes as i64)),
                ..treepo_model::DerivedSignals::default()
            };
            record.temporal.stability = (lines.total > 0).then(|| {
                let churn =
                    Fx::from_ratio(record.temporal.churn.days_90 as i64, lines.total as i64);
                Fx::ONE.sub(churn.min(Fx::ONE))
            });
        }

        let mut manifest = Manifest::new("test".to_string(), Seed::root(b"compose-test"));
        manifest.reference_time = REFERENCE;
        manifest.set_paths(records);
        manifest
    }

    /// Fills every directory's structural, size and ownership primitives from its children,
    /// deepest first, the way a real walk does.
    fn roll_up(
        records: &mut [PathRecord],
        counts: &mut [treepo_det::OrderedMap<treepo_model::identity::AuthorKey, u64>],
        counters: &mut [Counters],
    ) {
        let mut order: Vec<usize> = (0..records.len()).collect();
        order.sort_by_key(|&index| core::cmp::Reverse(records[index].path.depth()));

        for index in order {
            let path = records[index].path.clone();
            if !records[index].kind.is_container() {
                continue;
            }

            // Author lines roll up exactly as `treepo-vcs::log_pass` credits them — "every
            // ancestor is touched too" — so a directory's contributors are its whole subtree's.
            let inherited: Vec<(treepo_model::identity::AuthorKey, u64)> = records
                .iter()
                .enumerate()
                .filter(|(_, record)| record.path.parent().as_ref() == Some(&path))
                .flat_map(|(child, _)| counts[child].iter().map(|(&key, &lines)| (key, lines)))
                .collect();
            for (key, lines) in inherited {
                *counts[index].entry(key).or_insert(0) += lines;
            }

            // And the churn windows with them, off the same per-commit tally, alongside the
            // content counters `F-EXT-6`'s ratios are divided out of. All four sum, which is what
            // makes a directory's debt its whole subtree's — the property
            // `treepo-gen::material::debt_over` relies on when it merges several records.
            let gathered: Vec<Counters> = records
                .iter()
                .enumerate()
                .filter(|(_, record)| record.path.parent().as_ref() == Some(&path))
                .map(|(child, _)| counters[child])
                .collect();
            for child in gathered {
                let own = &mut counters[index];
                own.markers = own.markers.saturating_add(child.markers);
                own.large_bytes = own.large_bytes.saturating_add(child.large_bytes);
                own.churn_30 = own.churn_30.saturating_add(child.churn_30);
                own.churn_90 = own.churn_90.saturating_add(child.churn_90);
            }

            // Line counts roll up the same way, because they are the denominators of two of
            // those ratios — `LineCounts::merge` is the method extraction uses for it.
            let child_lines: Vec<treepo_model::primitives::size::LineCounts> = records
                .iter()
                .filter(|record| record.path.parent().as_ref() == Some(&path))
                .map(|record| record.size.lines)
                .collect();
            for lines in child_lines {
                records[index].size.lines = records[index].size.lines.merge(lines);
            }

            // Dates roll up the same way, and in both directions: a directory was created when
            // its earliest child was and touched when its latest child was. That is what gives
            // `F-MAT-4` a real span for a container rather than one borrowed from a member.
            let dates: Vec<(Option<i64>, Option<i64>)> = records
                .iter()
                .filter(|record| record.path.parent().as_ref() == Some(&path))
                .map(|record| {
                    (
                        record.temporal.first_commit_time,
                        record.temporal.last_commit_time,
                    )
                })
                .collect();
            for (first, last) in dates {
                let temporal = &mut records[index].temporal;
                if let Some(first) = first {
                    temporal.first_commit_time =
                        Some(temporal.first_commit_time.map_or(first, |c| c.min(first)));
                }
                if let Some(last) = last {
                    temporal.last_commit_time =
                        Some(temporal.last_commit_time.map_or(last, |c| c.max(last)));
                }
            }

            let children: Vec<(u64, u32, u32, u16)> = records
                .iter()
                .filter(|record| record.path.parent().as_ref() == Some(&path))
                .map(|record| {
                    (
                        record.size.bytes,
                        record.structural.descendant_file_count,
                        record.structural.descendant_dir_count,
                        record.structural.max_subtree_depth,
                    )
                })
                .collect();

            // Categories roll up exactly as `treepo-vcs::lang::roll_up_content` does, so a
            // directory's mixture is its whole subtree's rather than its immediate files'.
            // `treepo-gen::material` depends on that being true of real manifests.
            let categories: Vec<(treepo_model::primitives::size::ContentCategory, u64)> = records
                .iter()
                .filter(|record| record.path.parent().as_ref() == Some(&path))
                .flat_map(|record| record.size.category_bytes.iter())
                .map(|(&category, &bytes)| (category, bytes))
                .collect();

            let files = records
                .iter()
                .filter(|r| r.path.parent().as_ref() == Some(&path) && !r.kind.is_container())
                .count();
            let dirs = records
                .iter()
                .filter(|r| r.path.parent().as_ref() == Some(&path) && r.kind.is_container())
                .count();

            let record = &mut records[index];
            for (category, bytes) in categories {
                *record.size.category_bytes.entry(category).or_insert(0) += bytes;
            }
            record.size.bytes = children.iter().map(|c| c.0).sum();
            record.structural.child_count = u32::try_from(children.len()).unwrap_or(u32::MAX);
            record.structural.descendant_file_count =
                u32::try_from(files).unwrap_or(0) + children.iter().map(|c| c.1).sum::<u32>();
            record.structural.descendant_dir_count =
                u32::try_from(dirs).unwrap_or(0) + children.iter().map(|c| c.2).sum::<u32>();
            record.structural.max_subtree_depth =
                1 + children.iter().map(|c| c.3).max().unwrap_or(0);
            record
                .structural
                .branching
                .observe(record.structural.child_count);

            let total = record.size.bytes;
            let own = record.path.clone();
            for child in records
                .iter_mut()
                .filter(|r| r.path.parent().as_ref() == Some(&own))
            {
                child.size.relative_bytes = if total == 0 {
                    Fx::ZERO
                } else {
                    Fx::from_ratio(child.size.bytes as i64, total as i64)
                };
            }
        }
    }

    fn compose_of(manifest: &Manifest) -> Skeleton {
        compose(manifest, &Table::built_in(), Point::ORIGIN, Angle::ZERO)
    }

    /// `n` sibling files under one directory, all the same size.
    fn flat(count: usize) -> Manifest {
        let names: Vec<String> = (0..count)
            .map(|i| alloc::format!("src/f{i:03}.rs"))
            .collect();
        let files: Vec<(&str, u64)> = names.iter().map(|n| (n.as_str(), 100)).collect();
        manifest_of(&files)
    }

    fn aggregates(skeleton: &Skeleton) -> Vec<&AggregateNode> {
        skeleton
            .nodes()
            .iter()
            .filter_map(|node| match &node.role {
                NodeRole::Aggregate(aggregate) => Some(aggregate),
                _ => None,
            })
            .collect()
    }

    /// `C1`'s falloff across the composition boundary, over a grown tree.
    ///
    /// Whatever a limb's own drivers say, it grafts onto a branch that has already tapered and
    /// takes one more step of `width_ratio` from there — so no limb can be as thick as its
    /// parent, at any depth, in any repository. Before this held, every limb started from its
    /// own `base_width` and a tree rendered at one uniform weight from trunk to twig.
    ///
    /// Sabotage: drop the `.min(carried * width_ratio)` from `LimbParams::grafted_onto` and
    /// this fails on the first nested fixture, naming the pair.
    #[test]
    fn no_limb_is_wider_than_the_branch_it_grows_from() {
        for manifest in [
            manifest_of(&[
                ("src/main.rs", 4_000),
                ("src/net/tcp.rs", 9_000),
                ("src/net/http/client.rs", 6_000),
                ("src/net/http/server.rs", 5_000),
                ("docs/guide.md", 1_500),
            ]),
            flat(40),
            manifest_of(&[("a/b/c/d/e/f/g/deep.rs", 800)]),
        ] {
            let skeleton = compose_of(&manifest);

            // The widest thing each node draws. A limb's first segment is its widest, so this
            // is the limb's own base width as the turtle actually used it.
            let widest = |id: NodeId| -> Option<Fx> {
                skeleton
                    .segments()
                    .iter()
                    .filter(|segment| segment.node == id)
                    .map(|segment| segment.base_width)
                    .max()
            };

            for node in skeleton.nodes() {
                let (Some(parent), Some(child_width)) = (node.parent, widest(node.id)) else {
                    continue;
                };
                let Some(parent_width) = widest(parent) else {
                    continue;
                };
                assert!(
                    child_width < parent_width,
                    "{:?} draws at {child_width:?}, no narrower than its parent's \
                     {parent_width:?}",
                    node.role.anchor()
                );
            }
        }
    }

    /// `P6` and `P7` together, and the single most important property in this module:
    /// aggregation compresses, it never discards. Every path the manifest holds is either a
    /// limb of its own or a member of some container.
    #[test]
    fn every_path_survives_composition_as_a_limb_or_a_container_member() {
        for manifest in [
            flat(3),
            flat(40),
            flat(300),
            manifest_of(&[
                ("src/main.rs", 900),
                ("src/lib/a.rs", 100),
                ("src/lib/deep/b.rs", 50),
                ("src/lib/deep/deeper/c.rs", 25),
                ("src/lib/deep/deeper/deepest/d.rs", 10),
                ("src/lib/deep/deeper/deepest/way/e.rs", 5),
                ("docs/readme.md", 400),
                ("tests/t.rs", 200),
            ]),
        ] {
            let skeleton = compose_of(&manifest);
            for record in manifest.paths() {
                assert!(
                    skeleton.represents(&record.path),
                    "{:?} vanished from the skeleton — aggregation must compress, not discard",
                    record.path
                );
            }
        }
    }

    /// The phase transition, from both sides of the threshold.
    #[test]
    fn children_stay_first_class_below_the_threshold_and_collapse_above_it() {
        let sparse = compose_of(&flat(3));
        assert_eq!(
            sparse.aggregate_count(),
            0,
            "a directory well inside its capacity should draw every child"
        );

        let dense = compose_of(&flat(300));
        assert!(
            dense.aggregate_count() > 0,
            "three hundred siblings must cross the density threshold"
        );

        // And the transition is a compression, not a truncation: far fewer nodes than paths,
        // yet nothing is missing (asserted above).
        let manifest = flat(300);
        assert!(
            dense.nodes().len() < manifest.paths().len(),
            "aggregation must reduce the node count: {} nodes for {} paths",
            dense.nodes().len(),
            manifest.paths().len()
        );
    }

    /// The residue's share of the sites tracks its share of the mass — `F-SKEL-7`'s
    /// "proportional container" applied to the count as well as the size.
    #[test]
    fn a_heavy_residue_earns_more_containers_than_a_light_one() {
        // One dominant file and a long tail of tiny ones: the tail is most of the children
        // and almost none of the bytes, so it deserves few containers.
        let mut light: Vec<(String, u64)> = vec![("src/huge.rs".to_string(), 10_000_000)];
        light.extend((0..60).map(|i| (alloc::format!("src/t{i:03}.rs"), 1)));
        let light_files: Vec<(&str, u64)> = light.iter().map(|(n, b)| (n.as_str(), *b)).collect();

        // The same shape, but the tail carries the repository.
        let heavy: Vec<(String, u64)> = (0..60)
            .map(|i| (alloc::format!("src/t{i:03}.rs"), 10_000))
            .collect();
        let heavy_files: Vec<(&str, u64)> = heavy.iter().map(|(n, b)| (n.as_str(), *b)).collect();

        let light_count = compose_of(&manifest_of(&light_files)).aggregate_count();
        let heavy_count = compose_of(&manifest_of(&heavy_files)).aggregate_count();

        assert!(
            light_count >= 1,
            "a residue always gets at least one container"
        );
        assert!(
            heavy_count > light_count,
            "a residue carrying the repository deserves more containers than a negligible \
             one: heavy {heavy_count}, light {light_count}"
        );
    }

    /// `A3` at the hierarchy scale: past the level cap a subtree becomes one object rather
    /// than more limbs — `design/design-outline.md` §6's "this directory and all its
    /// contents".
    #[test]
    fn nesting_past_the_level_cap_becomes_a_container() {
        let table = Table::built_in();
        let deep = manifest_of(&[("a/b/c/d/e/f/g/h/i/j/deep.rs", 100)]);
        let skeleton = compose_of(&deep);

        let deepest_limb = skeleton
            .nodes()
            .iter()
            .filter_map(|node| match &node.role {
                NodeRole::Limb { path } => Some(path.depth()),
                _ => None,
            })
            .max()
            .unwrap();

        assert!(
            u32::from(deepest_limb) <= u32::from(table.max_levels()),
            "a limb appeared at depth {deepest_limb}, past A3's cap of {}",
            table.max_levels()
        );
        assert!(
            skeleton.aggregate_count() > 0,
            "the content past the cap has to go somewhere"
        );
        // And it is still reachable — P6's "the underlying data is never pruned".
        assert!(
            skeleton.represents(&RepoPath::new(b"a/b/c/d/e/f/g/h/i/j/deep.rs").unwrap()),
            "the deep file must still be represented, by whatever container absorbed it"
        );
    }

    /// `F-SKEL-2`: every limb runs its own instance seeded by its own path hash, so two
    /// structurally identical subtrees at different paths do not draw the same geometry.
    #[test]
    fn identical_subtrees_at_different_paths_grow_differently() {
        let manifest = manifest_of(&[
            ("alpha/a.rs", 100),
            ("alpha/b.rs", 100),
            ("alpha/c.rs", 100),
            ("beta/a.rs", 100),
            ("beta/b.rs", 100),
            ("beta/c.rs", 100),
        ]);
        let skeleton = compose_of(&manifest);

        let geometry = |prefix: &[u8]| -> Vec<Point> {
            skeleton
                .nodes()
                .iter()
                .filter(|node| node.role.anchor().as_bytes().starts_with(prefix))
                .map(|node| node.origin)
                .collect()
        };

        let alpha = geometry(b"alpha");
        let beta = geometry(b"beta");
        assert_eq!(
            alpha.len(),
            beta.len(),
            "the two subtrees have the same shape"
        );
        assert_ne!(
            alpha, beta,
            "but a path-seeded instance must not draw them identically"
        );
    }

    /// `F-SKEL-1`, end to end: the whole composition is a pure function of its inputs.
    #[test]
    fn composition_is_reproducible() {
        let manifest = manifest_of(&[
            ("src/a.rs", 300),
            ("src/b.rs", 200),
            ("docs/x.md", 100),
            ("vendor/lib/1.js", 10),
            ("vendor/lib/2.js", 10),
        ]);
        assert_eq!(compose_of(&manifest), compose_of(&manifest));
    }

    /// The claim the container's seed derivation makes: a file arriving in a container does
    /// not reroll the container's appearance. `AC-GROW-4`, inside an aggregated region.
    #[test]
    fn adding_a_file_to_a_container_does_not_reroll_it() {
        let before = compose_of(&flat(120));
        let after = compose_of(&flat(121));

        let seeds = |skeleton: &Skeleton| -> Vec<(RepoPath, u16, Seed)> {
            skeleton
                .nodes()
                .iter()
                .filter_map(|node| match &node.role {
                    NodeRole::Aggregate(aggregate) => {
                        Some((aggregate.anchor.clone(), aggregate.index, node.seed))
                    }
                    _ => None,
                })
                .collect()
        };

        let (old, new) = (seeds(&before), seeds(&after));
        assert!(!old.is_empty(), "the fixture must actually aggregate");
        assert_eq!(
            old.len(),
            new.len(),
            "one more file should not change how many containers there are"
        );
        assert_eq!(
            old, new,
            "a container's identity comes from its anchor and index, never its membership"
        );
    }

    /// Containers hold path-adjacent runs, so opening one shows a region of the repository
    /// rather than an arbitrary scattering.
    #[test]
    fn a_container_holds_neighbours() {
        let skeleton = compose_of(&flat(80));
        let containers = aggregates(&skeleton);
        assert!(
            containers.len() > 1,
            "the fixture must produce several containers"
        );

        for container in &containers {
            assert!(!container.is_empty(), "an empty container is a defect");
            let mut sorted = container.members.clone();
            sorted.sort();
            assert_eq!(
                sorted, container.members,
                "a container's members must be in path order"
            );
        }

        // And no two containers interleave: every member of one sorts before every member of
        // the next, which is what "adjacent run" means.
        let mut spans: Vec<(&RepoPath, &RepoPath)> = containers
            .iter()
            .map(|c| (&c.members[0], c.members.last().unwrap()))
            .collect();
        spans.sort();
        for pair in spans.windows(2) {
            assert!(
                pair[0].1 < pair[1].0,
                "containers {:?} and {:?} interleave",
                pair[0],
                pair[1]
            );
        }
    }

    /// A container reports the whole weight of what it stands for, not just its member roots
    /// — that is what makes it *proportional* (`F-SKEL-7`) and what `F-MAT-3`'s floor is
    /// discharged against.
    #[test]
    fn a_container_reports_the_mass_beneath_its_members() {
        let manifest = manifest_of(&[
            ("keep/a.rs", 100_000),
            ("v/one/x.rs", 10),
            ("v/one/y.rs", 10),
            ("v/two/z.rs", 10),
            ("v/three/w.rs", 10),
            ("v/four/q.rs", 10),
            ("v/five/r.rs", 10),
            ("v/six/s.rs", 10),
            ("v/seven/t.rs", 10),
            ("v/eight/u.rs", 10),
            ("v/nine/v.rs", 10),
            ("v/ten/p.rs", 10),
            ("v/eleven/o.rs", 10),
            ("v/twelve/n.rs", 10),
        ]);
        let skeleton = compose_of(&manifest);

        for container in aggregates(&skeleton) {
            let represented = container.represented_count();
            assert!(
                represented >= u32::try_from(container.member_count()).unwrap(),
                "a container standing for directories must count what is beneath them"
            );
            assert!(container.bytes > 0, "a container of real files has mass");
        }
    }

    /// `AC-SKEL-2`'s precondition: an empty repository produces a seed, not a lonely trunk
    /// with branches hanging off nothing. The root cluster itself is `trunk.rs`.
    #[test]
    fn an_empty_repository_produces_a_seed_rather_than_a_branching_trunk() {
        let mut manifest = Manifest::new("test".to_string(), Seed::root(b"empty"));
        manifest.set_paths(vec![PathRecord::new(RepoPath::root(), NodeKind::Directory)]);

        let skeleton = compose_of(&manifest);
        assert_eq!(skeleton.nodes().len(), 1, "one node — the seed");
        assert_eq!(skeleton.aggregate_count(), 0);
        assert_eq!(
            skeleton.segments().len(),
            1,
            "and one segment: nothing to branch into"
        );
    }

    /// The invariant `Skeleton`'s docs promise, which a single forward pass relies on.
    #[test]
    fn a_parent_always_precedes_its_children() {
        let skeleton = compose_of(&flat(50));
        for node in skeleton.nodes() {
            if let Some(parent) = node.parent {
                assert!(
                    parent.index() < node.id.index(),
                    "node {} precedes its parent {}",
                    node.id.index(),
                    parent.index()
                );
            }
        }
    }

    /// Every segment belongs to a node that exists — `N7`/`P1` need a pixel to resolve to an
    /// element, and a dangling id would be an unaccountable one.
    #[test]
    fn every_segment_names_a_node_that_exists() {
        let skeleton = compose_of(&flat(60));
        assert!(!skeleton.segments().is_empty());
        for segment in skeleton.segments() {
            assert!(
                skeleton.node(segment.node).is_some(),
                "segment refers to missing node {:?}",
                segment.node
            );
        }
    }

    #[test]
    fn splitting_preserves_every_member() {
        for (total, parts) in [(10usize, 3usize), (1, 4), (7, 7), (100, 6), (5, 1)] {
            let items: Vec<usize> = (0..total).collect();
            let runs = split_evenly(items, parts);
            let flattened: Vec<usize> = runs.iter().flatten().copied().collect();
            assert_eq!(flattened, (0..total).collect::<Vec<_>>());
            assert!(runs.iter().all(|run| !run.is_empty()));

            let longest = runs.iter().map(Vec::len).max().unwrap_or(0);
            let shortest = runs.iter().map(Vec::len).min().unwrap_or(0);
            assert!(longest - shortest <= 1, "runs must differ by at most one");
        }
    }
}
