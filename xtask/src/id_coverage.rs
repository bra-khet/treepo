//! ★ The `N7`/`P1` gate — every coloured pixel carries an element id, and every id is real.
//!
//! > `N7` appearance from primitives only — every baked pixel carries an element ID in a
//! > parallel ID buffer. **A pixel with color and no ID is an unaccountable pixel.**
//! > Automated ID-coverage scan (see D5) — this makes `P1` machine-checkable.
//!
//! Two constraints, two failures, two causes, and the report keeps them apart:
//!
//! * **`N7` — unaccountable.** A texel with a colour and no id. Cause: a rasterizer that painted
//!   without recording. Today that is structurally impossible — `bake::fill` writes both planes
//!   from one loop — which is a claim, and this is the scan that makes it a checked one.
//! * **`P1` — unresolved.** An id naming a node the skeleton does not have. Cause: an id that
//!   outlived the snapshot it belonged to. Different bug, different fix, so a report that said
//!   only "coverage failed" would send someone to the wrong file.
//!
//! A third case is counted and also fails: an id with *no* colour. `N7` does not forbid it, but
//! the two planes are written by one statement, so a disagreement in either direction means
//! that statement stopped being one thing.
//!
//! # It bakes the real thing, and that is why this crate depends on bevy
//!
//! The scan runs `treepo-render`'s own `chunk::pieces` and `bake::rasterize` over skeletons
//! grown by `treepo-gen` from corpus fixtures extracted by `treepo-vcs`. Nothing here
//! reimplements a rasterizer; a gate that scanned its own copy would be gating on the copy.
//! That reasoning is the same one `readonly-audit` is built on and it is recorded, with what it
//! costs, in `xtask/Cargo.toml`.
//!
//! # Two bands, deliberately
//!
//! Once with the whole tree framed, where a chunk is usually one piece, and once eight times
//! sharper, where large chunks subdivide and the piece grid appears. The second is where the
//! interesting failure would live: a piece is a *sub*-rectangle of its chunk, the rasterizer
//! clips to it, and a clip that dropped an id while keeping a colour would show up nowhere
//! else.

use std::fmt::Write as _;
use std::path::Path;

use treepo_det::Seed;
use treepo_model::{MaterialMap, Skeleton};
use treepo_render::{Band, ChunkSet, Coverage, Extent};

use crate::flag_value;

/// The seed every fixture is extracted with.
///
/// The same constant `determinism` uses and for the same reason: `F-MAN-3`'s third identity
/// tier hashes an absolute path, which differs per checkout, and a gate whose answer moved with
/// the directory it ran in would be reporting on the directory.
const CORPUS_SEED: &[u8] = b"treepo-corpus-fixed-seed";

/// How wide the framed tree is taken to be, in texels.
///
/// Stands in for a window, since there is no camera here. A thousand is the order of a real
/// viewport, which keeps the framed band's piece sizes in the range the app actually bakes.
const REFERENCE_TEXELS: f32 = 1000.0;

/// How much sharper the second band is than the framed one.
///
/// Three doublings. Far enough in that a chunk spanning the tree subdivides — which is the
/// case worth scanning — and not so far that the fixtures bake for a minute.
const NEAR_DOUBLINGS: i32 = 3;

/// The fixtures that legitimately have no tree to bake.
///
/// `bare` is a bare repository: no working directory, refused at association (`F-ASSOC-*`). The
/// list is named rather than "any extraction error is a skip", so that a fixture which quietly
/// stopped extracting cannot drop out of the scan — a gate gets greener as it covers less, and
/// that is the failure mode a coverage gate is most prone to.
const REFUSED: &[&str] = &["bare"];

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--self-test") {
        return self_test();
    }
    let only = flag_value(args, "--fixture")?;

    let root = corpus::default_root();
    let built = corpus::ensure(&root).map_err(|e| format!("building the corpus: {e}"))?;

    println!("id-coverage — every baked texel carries an element id (N7, P1)\n");
    println!("  corpus  {}\n", root.display());
    println!(
        "  {:<18} {:>12} {:>10} {:>8}  result",
        "fixture", "accounted", "pieces", "chunks"
    );

    let table = treepo_gen::Table::built_in();
    let materials = treepo_gen::MaterialTable::built_in();

    let mut total = Coverage::default();
    let mut scanned = 0;
    let mut refused = 0;
    let mut failures = String::new();

    for shape in corpus::all_shapes() {
        // Shapes only some platforms can build would make the report differ for a reason that
        // is not a finding — the same exclusion `determinism` makes, for the same reason.
        if shape.platforms != corpus::shapes::Platforms::All {
            continue;
        }
        if only.as_deref().is_some_and(|name| name != shape.name) {
            continue;
        }
        let Some(fixture) = built.iter().find(|fixture| fixture.name == shape.name) else {
            return Err(format!(
                "`{}` is an all-platform shape but the corpus did not build it",
                shape.name
            ));
        };

        let manifest = match manifest_for(&fixture.path) {
            Ok(manifest) => manifest,
            // `bare` has no working directory, so it is refused at association and there is no
            // tree to bake. Named rather than "any error is fine", for the same reason
            // `determinism` names it: a fixture that silently stopped extracting would drop out
            // of the scan and the gate would get greener as it covered less.
            Err(_) if REFUSED.contains(&shape.name) => {
                println!(
                    "  {:<18} {:>12} {:>10} {:>8}  refused",
                    shape.name, "—", "—", "—"
                );
                refused += 1;
                continue;
            }
            Err(why) => {
                return Err(format!(
                    "`{}` could not be extracted: {why}\n\
                     Only {REFUSED:?} may refuse; anything else is a regression in extraction \
                     rather than an ID-coverage finding.",
                    shape.name
                ));
            }
        };
        let skeleton = treepo_gen::grow(&manifest, &table);
        let material = treepo_gen::materialize(&manifest, &skeleton, &materials);

        let scan = scan(&skeleton, &material);
        total.absorb(scan.counts);
        scanned += 1;

        let result = if scan.counts.is_clean() && scan.unresolved.is_none() {
            "clean"
        } else {
            "FAIL"
        };
        println!(
            "  {:<18} {:>12} {:>10} {:>8}  {result}",
            shape.name, scan.counts.accounted, scan.pieces, scan.chunks
        );

        if scan.counts.unaccountable > 0 {
            writeln!(
                failures,
                "  {}: {} coloured texel(s) with no element id (N7)",
                shape.name, scan.counts.unaccountable
            )
            .expect("writing to a String cannot fail");
        }
        if scan.counts.invisible > 0 {
            writeln!(
                failures,
                "  {}: {} texel(s) carry an id but no colour — the two planes disagree",
                shape.name, scan.counts.invisible
            )
            .expect("writing to a String cannot fail");
        }
        if let Some(id) = scan.unresolved {
            writeln!(
                failures,
                "  {}: element id {} names no node in a {}-node skeleton (P1)",
                shape.name,
                id.raw(),
                skeleton.nodes().len()
            )
            .expect("writing to a String cannot fail");
        }
    }

    if scanned == 0 {
        return Err(match only {
            Some(name) => format!("no all-platform fixture named `{name}`"),
            None => "the corpus built no all-platform fixtures".to_owned(),
        });
    }

    println!(
        "\n  {scanned} fixture(s) scanned, {refused} refused, {} texel(s) painted and accounted for",
        total.accounted
    );
    println!(
        "  {} unaccountable, {} identified but unpainted",
        total.unaccountable, total.invisible
    );

    if !failures.is_empty() {
        return Err(format!(
            "the element-ID plane does not cover what the bake painted\n\n{failures}\n\
             `N7` says a pixel with colour and no id is an unaccountable pixel. Both planes are\n\
             written by `treepo-render::bake::fill`; a disagreement means that stopped being\n\
             one write."
        ));
    }

    // A gate that passes because it scanned nothing is worse than no gate. The corpus has
    // fixtures with geometry, so zero painted texels means the pipeline stopped producing a
    // tree, not that the tree is clean.
    if total.accounted == 0 {
        return Err(
            "no texel was painted at all — the scan proved nothing.\nEither the corpus grew \
             empty skeletons or the bake drew nothing into them."
                .to_owned(),
        );
    }

    println!("\n  zero unaccountable pixels");
    Ok(())
}

/// Proves the scan detects what it claims to detect.
///
/// A gate that has never been seen to fail is a gate nobody can distinguish from a gate that
/// cannot. `readonly-audit --self-test` makes the same argument by mutating a repository under
/// its own detector; this bakes a real layer, breaks it three ways, and requires each break to
/// be reported. Cheap enough to run beside the real scan, and the only thing standing between
/// "zero unaccountable pixels" and "zero pixels examined".
fn self_test() -> Result<(), String> {
    println!("id-coverage --self-test — the scan must catch what it claims to\n");

    // A tree that definitely paints something, without needing the corpus on disk.
    let (skeleton, materials) = probe();
    let chunks = ChunkSet::build(&skeleton);
    let extent = chunks
        .extent()
        .ok_or_else(|| "the probe skeleton drew nothing".to_owned())?;
    let chunk = chunks
        .chunks()
        .first()
        .ok_or_else(|| "the probe skeleton produced no chunk".to_owned())?;
    let piece = *treepo_render::pieces(&skeleton, chunk, densities(extent)[0], &chunk.extent)
        .first()
        .ok_or_else(|| "the probe chunk produced no piece".to_owned())?;

    let baked = treepo_render::bake::rasterize(
        &skeleton,
        &materials,
        &chunk.segments,
        piece.region,
        piece.size,
    );
    let painted = baked
        .ids
        .iter()
        .position(|id| !id.is_none())
        .ok_or_else(|| "the probe baked no painted texel to mutate".to_owned())?;

    let clean = treepo_render::coverage(&baked.color, &baked.ids);
    if !clean.is_clean() {
        return Err("the unmutated probe already fails — fix the bake before the detector".into());
    }

    let mut caught = 0;
    let mut missed = String::new();

    // 1. N7: a colour with the id taken away.
    let mut ids = baked.ids.clone();
    ids[painted] = treepo_render::ElementId::NONE;
    report(
        "a coloured texel with its id removed",
        treepo_render::coverage(&baked.color, &ids).unaccountable == 1,
        &mut caught,
        &mut missed,
    );

    // 2. The other direction: an id with the colour taken away.
    let mut color = baked.color.clone();
    color[painted * 4 + 3] = 0;
    report(
        "an identified texel with its colour removed",
        treepo_render::coverage(&color, &baked.ids).invisible == 1,
        &mut caught,
        &mut missed,
    );

    // 3. P1: an id naming a node past the end of the skeleton.
    let mut ids = baked.ids.clone();
    let past_end = u32::try_from(skeleton.nodes().len()).unwrap_or(u32::MAX - 1);
    ids[painted] = treepo_render::ElementId::of(treepo_model::NodeId::new(past_end));
    report(
        "an id naming a node the skeleton does not have",
        treepo_render::unresolved(&skeleton, &ids).is_some(),
        &mut caught,
        &mut missed,
    );

    println!("\n  detector self-test: {caught} of 3 mutations caught");
    if missed.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "the scan did not notice a broken plane\n\n{missed}\nA gate that cannot fail is \
             not a gate."
        ))
    }
}

/// Prints one self-test line and records whether it held.
fn report(what: &str, caught: bool, total: &mut u32, missed: &mut String) {
    if caught {
        *total += 1;
        println!("  caught   {what}");
    } else {
        println!("  MISSED   {what}");
        writeln!(missed, "  missed: {what}").expect("writing to a String cannot fail");
    }
}

/// A two-limb skeleton with no materials, for the self-test to bake.
///
/// Deliberately not a corpus fixture: the self-test is about the detector, and making it depend
/// on `git` being present and a corpus being built would make a detector failure and a corpus
/// failure look the same.
fn probe() -> (Skeleton, MaterialMap) {
    use treepo_det::{Angle, Fx};
    use treepo_model::{NodeRole, Point, RepoPath, Segment};

    let mut skeleton = Skeleton::new();
    let root = skeleton.push_node(
        None,
        Point::ORIGIN,
        Angle::ZERO,
        Seed::root(CORPUS_SEED),
        NodeRole::Limb {
            path: RepoPath::root(),
        },
    );
    skeleton.extend_segments([Segment {
        start: Point::new(Fx::from_int(0), Fx::from_int(0)),
        end: Point::new(Fx::from_int(0), Fx::from_int(100)),
        base_width: Fx::from_int(20),
        tip_width: Fx::from_int(10),
        node: root,
        generation: 0,
    }]);
    (skeleton, MaterialMap::new())
}

/// What one fixture's scan found.
struct Scan {
    counts: Coverage,
    chunks: usize,
    pieces: usize,
    unresolved: Option<treepo_render::ElementId>,
}

/// Bakes every piece of every chunk, at two bands, and scans both planes.
fn scan(skeleton: &Skeleton, materials: &MaterialMap) -> Scan {
    let chunks = ChunkSet::build(skeleton);
    let mut found = Scan {
        counts: Coverage::default(),
        chunks: chunks.len(),
        pieces: 0,
        unresolved: None,
    };
    let Some(extent) = chunks.extent() else {
        // `AC-SKEL-2`'s empty repository: a seed and a root cluster, no geometry. Nothing to
        // scan and nothing wrong — the `total.accounted == 0` check above is what notices if
        // *every* fixture looks like this.
        return found;
    };

    for band in densities(extent) {
        for chunk in chunks.chunks() {
            for piece in treepo_render::pieces(skeleton, chunk, band, &chunk.extent) {
                let layer = treepo_render::bake::rasterize(
                    skeleton,
                    materials,
                    &chunk.segments,
                    piece.region,
                    piece.size,
                );
                found
                    .counts
                    .absorb(treepo_render::coverage(&layer.color, &layer.ids));
                found.unresolved = found
                    .unresolved
                    .or_else(|| treepo_render::unresolved(skeleton, &layer.ids));
                found.pieces += 1;
            }
        }
    }
    found
}

/// The two texel densities each fixture is baked at.
///
/// Derived from the tree's own extent rather than fixed, because skeleton units come out of the
/// parameter table and a corpus fixture and a monorepo do not agree within orders of magnitude
/// about how big a world unit is — the same reason [`Band`] is relative to the framed view.
fn densities(extent: Extent) -> [f32; 2] {
    let span = extent.size().max_element().max(f32::MIN_POSITIVE);
    let fit_scale = span / REFERENCE_TEXELS;
    let near = Band::for_scale(fit_scale / 2f32.powi(NEAR_DOUBLINGS), fit_scale);
    [
        Band::FRAMED.texels_per_unit(fit_scale),
        near.texels_per_unit(fit_scale),
    ]
}

/// One fixture, through the Phase 1 pipeline, seeded from [`CORPUS_SEED`].
fn manifest_for(path: &Path) -> Result<treepo_model::Manifest, String> {
    use treepo_vcs::lang::Catalogue;
    use treepo_vcs::{ExtractOptions, FilterSet};

    let target = treepo_vcs::discover(path).map_err(|e| format!("discover: {e}"))?;
    treepo_vcs::extract(
        &target,
        &FilterSet::built_in(),
        &Catalogue::built_in(),
        Seed::root(CORPUS_SEED),
        "id-coverage-harness".to_owned(),
        ExtractOptions::default(),
    )
    .map_err(|e| format!("extract: {e}"))
}
