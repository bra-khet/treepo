//! The determinism harness — `AC-DET-1`, `AC-DET-2`, `AC-DET-3`.
//!
//! Architecture D2: "CI runs each corpus fixture three times per platform and compares
//! serialized-output hashes across all nine runs."
//!
//! The harness runs in two stages.
//!
//! **The probes** hash the layer everything else is built on: every primitive in `treepo-det`,
//! exercised over a fixed sample, reduced to one digest each. They were the whole of the
//! harness in Phase 0, when there was no corpus and no skeleton to serialize. Phase 4 added
//! four more — `pseudonym` and `author-color`, because `AC-ID-2` is the same claim as
//! `AC-DET-2` about a different output and deserves the same evidence rather than an argument
//! from `treepo-id` being integer-only; `material`, which sweeps the material layer's
//! arithmetic over magnitudes, exact ties and calendar edges that no real repository reliably
//! contains; and `enrichment`, whose fusion rule *branches* on a fixed-point comparison, so a
//! one-ulp disagreement would change which structures grew together rather than moving one of
//! them slightly.
//!
//! **The corpus stage** is D2's sentence, arrived at in Phase 3. Every corpus fixture is
//! extracted and grown, and the resulting [`Skeleton`](treepo_model::Skeleton),
//! [`MaterialMap`](treepo_model::MaterialMap) and
//! [`EnrichmentMap`](treepo_model::EnrichmentMap) are each reduced to a digest — `AC-DET-1`
//! names "skeletons, materials, and enrichment placements", three things, so all three are
//! re-derived from scratch on every run rather than the later passes being computed once over
//! geometry already known to be stable. This is the stage that can fail for an interesting
//! reason: the probes cover the arithmetic, and only this covers what the L-system, the
//! composition order, the aggregation threshold, the trunk column, the role-driven material
//! walk and the enrichment fusion do *with* that arithmetic.
//!
//! Two properties are checked, and they are not the same property:
//!
//! * **`AC-DET-1`, within a platform.** Each probe runs three times in one process, and each
//!   fixture is grown three times from one manifest; the digests must agree. This catches
//!   anything that reads ambient state — a clock, a process-seeded hasher, an address —
//!   because those vary between runs of one binary. Growing repeatedly from *one* manifest is
//!   deliberate: `AC-DET-1` is about two Grow runs on identical repository state, and the
//!   manifest is that state.
//! * **`AC-DET-2`, across platforms.** The report written by `--out` contains nothing
//!   platform-specific, so CI can compare the files from all three runners byte for byte.
//!   This is the check that would catch a platform `libm` creeping into the trig path.
//!
//! # What the corpus stage deliberately does not seed from
//!
//! Extraction takes its root seed from repository identity, and `F-MAN-3`'s third tier is a
//! hash of the absolute path — which is a different number in every checkout, on every
//! machine. Seeding this stage that way would make `AC-DET-2` fail by construction and prove
//! nothing. So the stage supplies a fixed seed of its own. Identity resolution has its own
//! tests; what is under test here is everything downstream of the seed.
//!
//! One consequence is visible in the report and worth expecting: `detached-head`, `shallow`,
//! `no-remote`, and `multi-remote` differ only in refs and remotes, so under a fixed seed they
//! grow the same skeleton and print the same skeleton digest. That is the correct answer here.
//! Those four exist to exercise `F-MAN-3`, and it is `tests/identity.rs` that has to tell them
//! apart.
//!
//! **Their material digests are all different, and that is also correct.** The four hold the
//! same *structure* but not the same bytes — `tools/corpus` seeds generated line widths from
//! the fixture's name, so the one `src/main.rs` runs to 1322, 1251, 883 and 2565 bytes
//! respectively. The skeleton cannot see that, because its size driver is `relative_bytes` and
//! a lone file is all of its parent whatever it weighs. `F-MAT-3`'s budget is measured against
//! an **absolute** scale, deliberately and for the reasons `treepo-gen::normalize` records, so
//! the material layer separates repositories the geometry cannot. Four identical skeleton
//! digests beside four distinct material digests is that decision being visible rather than a
//! disagreement between the two stages.
//!
//! **Their enrichment digests then coincide again**, and that is a third correct answer rather
//! than the material distinction leaking away. Each of the four is one small source file, and
//! one small source file offers no documentation, no assets, no test directory and too little
//! churn to clear `F-MAT-5`'s presence floor — so all four are furnished with nothing, and
//! nothing hashes the same as nothing. A byte difference that moves a budget need not move a
//! threshold it was never near.
//!
//! Fixtures that only some platforms can build are excluded for the same reason: a report
//! listing `symlinks` on two runners and not the third would differ for a reason that is not a
//! finding. `tools/corpus` records which those are, and this reads that rather than guessing.
//!
//! # Commit timestamps reach the digest, and the corpus was already built for that
//!
//! `F-MAT-4` puts an age on every node, so from Phase 4 the material digests depend on *when*
//! each fixture's commits are dated. A corpus that dated from the wall clock would then produce
//! a different report on every runner and break `AC-DET-2` by construction — the same trap the
//! identity seed above describes, arriving through the calendar instead.
//!
//! It does not, and this was checked rather than assumed: `tools/corpus` steps a fixed
//! `EPOCH` of 2021-01-01T00:00:00Z by `COMMIT_INTERVAL` and writes `"{epoch} +0000"` into both
//! `GIT_AUTHOR_DATE` and `GIT_COMMITTER_DATE`. Absolute integers with an explicit offset, so
//! neither the runner's clock nor its timezone can reach a fixture's history.
//!
//! Thread count is *not* pinned. `AC-DET-3` forbids hardware-dependent values in the
//! generative pipeline, and the history pass is threaded — so leaving it at the product's
//! default is what would let a thread-order dependency surface as a cross-platform difference.
//!
//! Phase 6 extends the harness once more, with the `GrowTimeline` hash (D6).

use std::fmt::Write as _;
use std::path::Path;

use treepo_det::{Angle, ChaCha8Rng, Digest, Fx, Seed, Sha256, sin_cos};

/// A named, reproducible computation over the determinism primitives.
struct Probe {
    name: &'static str,
    what: &'static str,
    run: fn() -> Digest,
}

const PROBES: &[Probe] = &[
    Probe {
        name: "trig",
        what: "sin/cos over 10,000 sampled angles (F-SKEL-6)",
        run: probe_trig,
    },
    Probe {
        name: "fixed",
        what: "Fx arithmetic over 2,000 sampled operand pairs",
        run: probe_fixed,
    },
    Probe {
        name: "angle",
        what: "Angle construction and conversion over 4,096 samples",
        run: probe_angle,
    },
    Probe {
        name: "rng",
        what: "ChaCha8 keystream and bounded draws over 16 seeds",
        run: probe_rng,
    },
    Probe {
        name: "seed-tree",
        what: "hierarchical path-hash seeding over a synthetic tree (P2)",
        run: probe_seed_tree,
    },
    Probe {
        name: "pseudonym",
        what: "roster assignment over 512 contributors (F-ID-3, AC-ID-2)",
        run: probe_pseudonym,
    },
    Probe {
        name: "author-color",
        what: "palette draw over the same 512 contributors (F-ID-4, AC-ID-2)",
        run: probe_author_color,
    },
    Probe {
        name: "material",
        what: "families, budgets, mosaics and ages over sampled inputs (F-MAT-1…4, AC-DET-1)",
        run: probe_material,
    },
    Probe {
        name: "enrichment",
        what: "placement, fusion and densification over sampled limbs (F-MAT-5, AC-DET-1)",
        run: probe_enrichment,
    },
];

/// The contributor set the two `AC-ID-2` probes run over.
///
/// Five hundred and twelve, which is not an arbitrary round number: the built-in wordlist has
/// 16,384 pairs, so a set this size draws a handful of collisions and the probe therefore
/// covers the salted-redraw path rather than only the happy one. A set of ten would hash the
/// same code either way and prove less.
fn probe_contributors() -> Vec<treepo_model::identity::AuthorKey> {
    (0..512u32)
        .map(|n| {
            treepo_model::identity::AuthorKey::from_email(
                format!("contributor-{n}@determinism.invalid").as_bytes(),
            )
        })
        .collect()
}

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let runs: usize = match crate::flag_value(args, "--runs")? {
        None => 3,
        Some(text) => text
            .parse()
            .map_err(|_| format!("--runs expects a number, got `{text}`"))?,
    };
    if runs == 0 {
        return Err("--runs must be at least 1".to_owned());
    }
    let out = crate::flag_value(args, "--out")?;
    let check = crate::flag_value(args, "--check")?;

    println!("determinism harness — {runs} runs per probe\n");

    let mut report = String::new();
    let mut overall = Sha256::new();

    for probe in PROBES {
        let first = (probe.run)();

        // AC-DET-1: the same computation, repeated in the same process, must not move.
        for repeat in 1..runs {
            let again = (probe.run)();
            if again != first {
                return Err(format!(
                    "probe `{}` is not reproducible within one process\n  \
                     run 1: {first}\n  run {}: {again}\n\
                     Something in this path reads ambient state.",
                    probe.name,
                    repeat + 1
                ));
            }
        }

        println!("  {:<13} {first}", probe.name);
        println!("  {:<13} {}", "", probe.what);
        writeln!(report, "{} {first}", probe.name).expect("writing to a String cannot fail");
        overall.update(probe.name.as_bytes());
        overall.update(first.as_bytes());
    }

    corpus_skeletons(runs, &mut report, &mut overall)?;

    let overall = overall.finalize();
    writeln!(report, "overall {overall}").expect("writing to a String cannot fail");
    println!("\n  {:<13} {overall}", "overall");

    // The report is deliberately free of platform, path, and toolchain information. It is
    // compared byte for byte against the reports from the other two platforms, so anything
    // that varies by machine would defeat the comparison it exists to enable.
    if let Some(path) = out {
        std::fs::write(&path, &report).map_err(|e| format!("writing {path}: {e}"))?;
        println!("\nwrote {path}");
    }

    if let Some(path) = check {
        let expected =
            std::fs::read_to_string(&path).map_err(|e| format!("reading {path}: {e}"))?;
        if expected.replace("\r\n", "\n") != report {
            return Err(format!(
                "report does not match {path}\n\n--- expected ---\n{expected}\n--- actual ---\n{report}"
            ));
        }
        println!("matches {path}");
    }

    println!("\nall probes reproducible ({runs} runs each)");
    Ok(())
}

/// The seed the corpus stage grows from.
///
/// Fixed rather than resolved, for the reason the module docs give: `F-MAN-3`'s path-hash tier
/// would put the checkout directory into the answer.
const CORPUS_SEED: &[u8] = b"treepo/determinism-corpus";

/// The fixture extraction is *supposed* to decline — PRD §6's bare repository, which has no
/// working tree to read.
///
/// Named, so that any other fixture becoming unextractable is an error rather than a line of
/// report that still matches across three platforms.
const REFUSED: &[&str] = &["bare"];

/// Grows every all-platform corpus fixture and hashes the skeleton — `AC-DET-1`, `AC-DET-2`.
fn corpus_skeletons(runs: usize, report: &mut String, overall: &mut Sha256) -> Result<(), String> {
    let root = corpus::default_root();
    let built = corpus::ensure(&root).map_err(|e| format!("building the corpus: {e}"))?;

    println!("\nskeletons and materials — every corpus fixture, grown {runs} times\n");
    println!("  corpus  {}", root.display());
    println!("  tables  built-in (assets/params/{{lsystem,materials}}.ron)\n");

    let table = treepo_gen::Table::built_in();
    let materials = treepo_gen::MaterialTable::built_in();

    for shape in corpus::all_shapes() {
        // Shapes only some platforms can build would make the report differ for a reason that
        // is not a finding. See the module docs.
        if shape.platforms != corpus::shapes::Platforms::All {
            continue;
        }
        let Some(fixture) = built.iter().find(|fixture| fixture.name == shape.name) else {
            return Err(format!(
                "`{}` is an all-platform shape but the corpus did not build it",
                shape.name
            ));
        };

        let (outcome, material) = match manifest_for(&fixture.path) {
            Ok(manifest) => {
                let skeleton = treepo_gen::grow(&manifest, &table);
                let first = skeleton.digest();
                let first_materials = treepo_gen::materialize(&manifest, &skeleton, &materials);
                let first_material = first_materials.digest();
                let first_enrichment =
                    treepo_gen::enrich(&manifest, &skeleton, &first_materials, &materials).digest();

                // AC-DET-1: the same repository state, grown again, must not move — and the
                // criterion names all three, so all three are re-derived from scratch each run
                // rather than the later passes being computed once over a skeleton that is
                // already known to be stable.
                for repeat in 1..runs {
                    let regrown = treepo_gen::grow(&manifest, &table);
                    let again = regrown.digest();
                    if again != first {
                        return Err(format!(
                            "`{}` does not grow the same skeleton twice\n  \
                             run 1: {first}\n  run {}: {again}\n\
                             Something in the generative path reads ambient state.",
                            shape.name,
                            repeat + 1
                        ));
                    }
                    let again_materials = treepo_gen::materialize(&manifest, &regrown, &materials);
                    let again_material = again_materials.digest();
                    if again_material != first_material {
                        return Err(format!(
                            "`{}` grows the same skeleton but not the same materials\n  \
                             run 1: {first_material}\n  run {}: {again_material}\n\
                             The geometry is stable and the material pass is not, so the \
                             ambient read is in `treepo-gen::material` or `::normalize`.",
                            shape.name,
                            repeat + 1
                        ));
                    }
                    let again_enrichment =
                        treepo_gen::enrich(&manifest, &regrown, &again_materials, &materials)
                            .digest();
                    if again_enrichment != first_enrichment {
                        return Err(format!(
                            "`{}` grows the same materials but not the same enrichment\n  \
                             run 1: {first_enrichment}\n  run {}: {again_enrichment}\n\
                             The material pass is stable and the placement pass is not, so the \
                             ambient read is in `treepo-gen::enrich`.",
                            shape.name,
                            repeat + 1
                        ));
                    }
                }

                (
                    first.to_string(),
                    Some((
                        first_material.to_string(),
                        first_enrichment.to_string(),
                        skeleton.nodes().len(),
                    )),
                )
            }
            Err(why) if REFUSED.contains(&shape.name) => {
                println!("  {:<18} refused — {why}", shape.name);
                ("refused".to_owned(), None)
            }
            Err(why) => {
                return Err(format!(
                    "`{}` could not be extracted: {why}\n\
                     Only {REFUSED:?} may refuse; anything else is a regression in extraction \
                     rather than a determinism finding.",
                    shape.name
                ));
            }
        };

        if let Some((material, enrichment, nodes)) = &material {
            println!("  {:<18} {outcome}  skeleton", shape.name);
            println!("  {:<18} {material}  material, {nodes} nodes", "");
            println!("  {:<18} {enrichment}  enrichment", "");
        }

        // Report and digest in the order they are printed. A reader comparing two platforms'
        // reports by eye should not have to hold a different order in their head from the one
        // the terminal showed them.
        writeln!(report, "skeleton/{} {outcome}", shape.name)
            .expect("writing to a String cannot fail");
        overall.update(shape.name.as_bytes());
        overall.update(outcome.as_bytes());

        if let Some((material, enrichment, _)) = &material {
            writeln!(report, "material/{} {material}", shape.name)
                .expect("writing to a String cannot fail");
            overall.update(material.as_bytes());

            writeln!(report, "enrichment/{} {enrichment}", shape.name)
                .expect("writing to a String cannot fail");
            overall.update(enrichment.as_bytes());
        }
    }

    Ok(())
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
        // A constant, not the crate version. The report is compared byte for byte, and a
        // version string reaching a digest would make every release look like a regression.
        "determinism-harness".to_owned(),
        ExtractOptions::default(),
    )
    .map_err(|e| format!("extract: {e}"))
}

/// The sampled angles. Stride 429,497 is odd, so it is coprime with 2³² and the 10,000
/// samples are distinct; it is also not a multiple of the table stride, so samples land on
/// table entries, beside them, and everywhere between.
fn sampled_angles() -> impl Iterator<Item = Angle> {
    (0..10_000u32).map(|i| Angle::from_bits(i.wrapping_mul(429_497)))
}

/// `F-SKEL-6`. Identical by construction to the golden-digest test in `treepo_det::trig`,
/// so the two report the same number and a reader can tell they are the same check.
fn probe_trig() -> Digest {
    let mut hasher = Sha256::new();
    for angle in sampled_angles() {
        let (sine, cosine) = sin_cos(angle);
        hasher.update(&angle.to_bits().to_le_bytes());
        hasher.update(&sine.to_bits().to_le_bytes());
        hasher.update(&cosine.to_bits().to_le_bytes());
    }
    hasher.finalize()
}

fn probe_fixed() -> Digest {
    let mut hasher = Sha256::new();
    let mut rng = ChaCha8Rng::from_u64(0x5EED_C0DE_1234_5678);
    for _ in 0..2_000 {
        // Shifted down so products stay clear of saturation for most pairs, while a few
        // deliberately reach it — the saturating path needs to be covered too.
        let a = Fx::from_bits(rng.next_u64() as i64 >> 12);
        let b = Fx::from_bits(rng.next_u64() as i64 >> 12);
        let t = rng.unit_fx();

        for value in [
            a + b,
            a - b,
            a * b,
            a.lerp(b, t),
            a.abs(),
            -a,
            a.min(b),
            a.max(b),
            a.signum(),
            a.fract(),
            a.scale(7, 3),
            a.checked_div(b).unwrap_or(Fx::MAX),
            a.abs().sqrt(),
        ] {
            hasher.update(&value.to_bits().to_le_bytes());
        }
        for whole in [a.floor(), a.ceil(), a.round()] {
            hasher.update(&whole.to_le_bytes());
        }
    }
    hasher.finalize()
}

fn probe_angle() -> Digest {
    let mut hasher = Sha256::new();
    for i in 0..4_096u32 {
        let angle = Angle::from_bits(i.wrapping_mul(1_048_573));
        hasher.update(&angle.to_bits().to_le_bytes());
        hasher.update(&angle.to_millidegrees().to_le_bytes());
        hasher.update(&angle.to_radians().to_bits().to_le_bytes());
        hasher.update(
            &Angle::from_radians(angle.to_radians())
                .to_bits()
                .to_le_bytes(),
        );
        hasher.update(&angle.quadrant().to_le_bytes());
    }
    for degrees in [-720i32, -90, 0, 1, 45, 90, 180, 359, 360, 721] {
        hasher.update(&Angle::from_degrees(degrees).to_bits().to_le_bytes());
        hasher.update(
            &Angle::from_millidegrees(degrees * 1000)
                .to_bits()
                .to_le_bytes(),
        );
    }
    hasher.finalize()
}

fn probe_rng() -> Digest {
    let mut hasher = Sha256::new();
    for seed in 0..16u64 {
        let mut rng = ChaCha8Rng::from_u64(seed).with_stream(seed * 3);
        let mut bytes = [0u8; 96];
        rng.fill_bytes(&mut bytes);
        hasher.update(&bytes);
        for bound in [2u32, 6, 97, 1_000_000] {
            for _ in 0..8 {
                hasher.update(&rng.below_u32(bound).to_le_bytes());
            }
        }
        for _ in 0..8 {
            hasher.update(&rng.unit_fx().to_bits().to_le_bytes());
            hasher.update(&rng.signed_unit_fx().to_bits().to_le_bytes());
            hasher.update(&rng.range_i32(-500, 500).to_le_bytes());
            hasher.update(&[u8::from(rng.chance(1, 3))]);
        }
    }
    hasher.finalize()
}

/// `P2` — "seeded hierarchically from path hashes". Walks a synthetic directory tree and
/// hashes the seed each path resolves to.
fn probe_seed_tree() -> Digest {
    const DIRECTORIES: [&str; 6] = ["src", "docs", "assets", "tests", "crates", ".config"];
    const FILES: [&str; 5] = ["mod.rs", "lib.rs", "README.md", "data.bin", "Ünïcøde.txt"];

    let root = Seed::root(b"treepo/determinism-probe");
    let mut hasher = Sha256::new();
    hasher.update(root.as_bytes());

    for directory in DIRECTORIES {
        let dir_seed = root.derive(directory.as_bytes());
        hasher.update(dir_seed.as_bytes());
        for file in FILES {
            let path_seed = dir_seed.derive(file.as_bytes());
            hasher.update(path_seed.as_bytes());
            hasher.update(&path_seed.to_u64().to_le_bytes());
            for index in 0..4u64 {
                hasher.update(path_seed.derive_index(index).as_bytes());
            }
            // A generator opened from the seed must land in the same place every time.
            let mut rng = path_seed.rng();
            hasher.update(&rng.next_u64().to_le_bytes());
        }
    }
    hasher.finalize()
}

/// `AC-ID-2`, the pseudonym half — "the same repository produces identical pseudonyms […] on
/// Windows, macOS, and Linux".
///
/// The whole roster is hashed rather than a sample of draws, because the interesting failure
/// is not in the draw. A draw is a hash and a modulo; the assignment is a walk over a set in
/// key order with a collision rule, and *that* is where a platform difference would live if
/// one existed.
fn probe_pseudonym() -> Digest {
    let wordlist = treepo_id::Wordlist::built_in();
    let roster = wordlist.assign(probe_contributors());

    let mut hasher = Sha256::new();
    hasher.update(&(roster.len() as u64).to_le_bytes());
    for (key, name) in roster.iter() {
        hasher.update(key.as_bytes());
        // Lengths precede the words, as in `Skeleton::digest`, so two different pairs cannot
        // run into each other in the encoding.
        hasher.update(&(name.first().len() as u32).to_le_bytes());
        hasher.update(name.first().as_bytes());
        hasher.update(&(name.second().len() as u32).to_le_bytes());
        hasher.update(name.second().as_bytes());
        hasher.update(&name.discriminator().to_le_bytes());
    }
    hasher.finalize()
}

/// `AC-ID-2`, the colour half — and `AC-MAT-4` with it.
///
/// The separation of the tightest pair is hashed alongside the drawn colours. That number is
/// what `AC-MAT-4` is about, it is computed through the trig table, and a platform that
/// disagreed about it would be disagreeing about whether the palette is legal at all.
fn probe_author_color() -> Digest {
    let palette = treepo_id::Palette::built_in();

    let mut hasher = Sha256::new();
    if let Some((first, second, separation)) = palette.tightest_pair() {
        hasher.update(&(first as u32).to_le_bytes());
        hasher.update(&(second as u32).to_le_bytes());
        hasher.update(&separation.to_bits().to_le_bytes());
    }
    for key in probe_contributors() {
        let color = palette.color_of(&key);
        hasher.update(&color.family().to_le_bytes());
        hasher.update(&color.lightness().to_bits().to_le_bytes());
        hasher.update(&color.chroma().to_bits().to_le_bytes());
        hasher.update(&color.hue().to_bits().to_le_bytes());
        // The Lab coordinates are what the separation metric and the render layer both read,
        // and they come through the trig table — the one place a platform `libm` could get
        // in (`RISK-2`).
        let lab = color.to_oklab();
        hasher.update(&lab.l.to_bits().to_le_bytes());
        hasher.update(&lab.a.to_bits().to_le_bytes());
        hasher.update(&lab.b.to_bits().to_le_bytes());
    }
    hasher.finalize()
}

/// `AC-DET-1`, the half that names materials.
///
/// > Two Grow runs on identical repository state produce byte-identical serialized skeletons,
/// > **materials**, and enrichment placements.
///
/// Synthetic mixtures rather than the corpus, in the same spirit as the two probes above:
/// what this covers is the *arithmetic*, and a probe over sampled inputs exercises the extreme
/// magnitudes and the tie cases that no real repository reliably contains. The corpus-wide
/// material digest joins `skeleton/*` when a walk over the skeleton exists to produce one.
///
/// `Fx::log2_u64` is the reason this probe earns its place. It is the newest primitive in the
/// generative path and the only one computing a transcendental function without the trig
/// table — if `RISK-2` had a second home, this would be it. The budgets below sweep it from
/// one byte to sixteen exabytes.
fn probe_material() -> Digest {
    use treepo_model::primitives::ownership::OwnershipPrimitives;
    use treepo_model::primitives::size::{ContentCategory, SizePrimitives};
    use treepo_model::segment::NodeRole;

    let table = treepo_gen::MaterialTable::built_in();
    let mut hasher = Sha256::new();

    // Every power of two, plus offsets either side, so the sweep lands on the boundaries where
    // `ilog2` steps and on the mantissa work between them.
    for exponent in 0..64u32 {
        let power = 1u64 << exponent;
        for bytes in [
            power,
            power.saturating_add(1),
            power.saturating_sub(1),
            power.saturating_add(power / 3),
        ] {
            hasher.update(&table.normalize.budget(bytes).to_bits().to_le_bytes());
        }
    }

    // Category mixtures, including the exact ties and the single-category cases. Both roles,
    // because the role is what selects blended against subordinate and a platform that
    // disagreed about one would not necessarily disagree about the other.
    let container = NodeRole::Aggregate(treepo_model::AggregateNode {
        anchor: treepo_model::path::RepoPath::root(),
        index: 0,
        members: vec![treepo_model::path::RepoPath::root()],
        bytes: 4096,
        file_count: 7,
        dir_count: 1,
    });
    let limb = NodeRole::Limb {
        path: treepo_model::path::RepoPath::root(),
    };
    // The mixture sweep is about families and budgets; the mosaic gets its own sweep below,
    // where the contributor counts are what varies.
    let unowned = OwnershipPrimitives::default();

    for (i, first) in ContentCategory::ALL.into_iter().enumerate() {
        for (j, second) in ContentCategory::ALL.into_iter().enumerate() {
            // 0, 250, 500, 750, 1000 of the first against the rest of the second — so every
            // ordered pair is sampled at an exact tie and either side of one.
            for share in [0u64, 250, 500, 750, 1000] {
                let size = SizePrimitives {
                    category_bytes: [(first, share), (second, 1000 - share)]
                        .into_iter()
                        .collect(),
                    ..SizePrimitives::default()
                };
                let bytes = 4096 + (i as u64 * 7 + j as u64) * 131;
                let mut sampled = treepo_model::MaterialMap::new();
                for role in [&limb, &container] {
                    sampled.push(table.material_of(&size, bytes, &unowned, None, role));
                }
                // Through the canonical encoding rather than a local one — `MaterialMap` owns
                // it for the reason `Skeleton` owns its own, and a second copy here would be
                // a second chance for the gate and the corpus lines below to disagree.
                hasher.update(sampled.digest().as_bytes());
            }
        }
    }

    // `F-MAT-2`'s mosaic and `F-MAT-3`'s quota, over contributor counts that straddle the
    // significance threshold: one holder, a handful, and more than the threshold physically
    // permits.
    for count in [1u32, 3, 8, 64, 512] {
        let counts: treepo_det::OrderedMap<treepo_model::identity::AuthorKey, u64> =
            probe_contributors()
                .into_iter()
                .take(count as usize)
                .enumerate()
                .map(|(i, key)| (key, (i as u64 % 17) + 1))
                .collect();
        let ownership =
            OwnershipPrimitives::from_line_counts(&counts, treepo_det::OrderedMap::new());

        // Explicit cell counts, then the budget-driven path — `cells_for` rounds a fixed-point
        // product, so it is arithmetic a platform could disagree about and the sweep above
        // would not have covered.
        for cells in [8u32, 64, 256] {
            hash_mosaic(&mut hasher, &table.normalize.allocate(&ownership, cells));
        }
        for bytes in [0u64, 4096, 1 << 24, u64::MAX] {
            let budget = table.normalize.budget(bytes);
            hasher.update(&table.normalize.cells_for(budget).to_le_bytes());
            hash_mosaic(&mut hasher, &table.normalize.mosaic(&ownership, budget));
        }
    }

    // `F-MAT-4`'s age scale — a second `Fx::log2_u64` against a second divisor, so a platform
    // could disagree here without disagreeing about budgets. Days rather than powers of two,
    // because a calendar is where the interesting values are: today, a week, the windows
    // `F-EXT-2` measures churn over, either side of the full scale, and a clock-skewed commit
    // from the repository's own future.
    for days in [
        -365i64,
        -1,
        0,
        1,
        7,
        29,
        30,
        31,
        89,
        90,
        91,
        364,
        365,
        366,
        3649,
        3650,
        3651,
        36_500,
        i64::MAX,
    ] {
        hasher.update(&table.normalize.age(days).to_bits().to_le_bytes());
    }
    // And the gradient over spans that straddle every one of those, including the degenerate
    // single-commit case and an inverted pair that `AgeGradient::new` has to order.
    for oldest in [0i64, 30, 365, 3650, 36_500] {
        for newest in [0i64, 7, 90, 3650] {
            let gradient = table.normalize.gradient(oldest, newest);
            hasher.update(&gradient.base().to_bits().to_le_bytes());
            hasher.update(&gradient.tip().to_bits().to_le_bytes());
            hasher.update(&gradient.span().to_bits().to_le_bytes());
            hasher.update(&gradient.at(Fx::HALF).to_bits().to_le_bytes());
        }
    }

    hasher.finalize()
}

/// `F-MAT-5` — where structures land, what fuses, and what the excess densifies into.
///
/// Its own probe rather than more of [`probe_material`], because the arithmetic is a different
/// shape and would be lost inside a sweep of budgets. Three things here could disagree across
/// platforms and nothing above would catch any of them: the mass-weighted `lerp` that
/// [`Placement::fuse`](treepo_model::Placement::fuse) centres a fused structure on, the
/// `position_of` division that turns a normalized age back into a place on the limb, and the
/// gap comparison the densification loop picks its closest pair with — the last of which is a
/// *branch* on fixed-point values, so a one-ulp disagreement would change which two structures
/// grew together rather than moving one of them slightly.
///
/// The sweep is built so every one of those is exercised. Positions land on and either side of
/// the merge window, counts run past `max_per_kind` so the densification loop runs, and the
/// ages cover the range `F-MAT-4`'s scale compresses hardest.
fn probe_enrichment() -> Digest {
    use treepo_model::enrichment::{EnrichmentKind, EnrichmentMap, Placement};

    let table = treepo_gen::MaterialTable::built_in();
    let mut hasher = Sha256::new();

    // The inverse of the age gradient — the division that places a structure on material of its
    // own vintage. Spans that straddle the whole scale, read at every age in it.
    for (oldest, newest) in [(3650i64, 0i64), (365, 30), (90, 89), (36_500, 1), (7, 7)] {
        let gradient = table.normalize.gradient(oldest, newest);
        for days in [0i64, 1, 7, 30, 90, 365, 1000, 3650, 36_500] {
            let age = table.normalize.age(days);
            hasher.update(&[u8::from(gradient.position_of(age).is_some())]);
            hasher.update(
                &gradient
                    .position_of(age)
                    .unwrap_or(Fx::ZERO)
                    .to_bits()
                    .to_le_bytes(),
            );
        }
    }

    // Fusion: the mass-weighted mean, over lopsided pairs where the rounding has somewhere to
    // go, and over chains where it accumulates.
    for kind in EnrichmentKind::ALL {
        for (a, b) in [(1i64, 999i64), (500, 500), (997, 3), (1, 1), (333, 667)] {
            let grown =
                Placement::single(kind, Fx::from_ratio(a, 1000), Fx::from_ratio(a, 1000)).fuse(
                    Placement::single(kind, Fx::from_ratio(b, 1000), Fx::from_ratio(b, 1000)),
                );
            hasher.update(&grown.position.to_bits().to_le_bytes());
            hasher.update(&grown.weight.to_bits().to_le_bytes());
            hasher.update(&grown.sources.to_le_bytes());
        }
    }

    // The whole pass, through the public entry point rather than through its steps — so the
    // table's own thresholds are in the answer and no probe hook has to exist in shipping API.
    //
    // Documents dated `spacing` days apart, which is what puts their structures a controlled
    // distance apart along the limb: position comes off the vintage. Spacings that put
    // neighbours inside, on and outside the merge window, and counts that overrun
    // `max_per_kind` so the densification loop runs.
    let mut sampled = EnrichmentMap::new();
    for spacing in [1i64, 9, 40, 180] {
        for count in [1u32, 2, 5, 13, 40] {
            let (manifest, role) = probe_container(count, spacing);
            let record = manifest
                .path(&treepo_model::path::RepoPath::root())
                .unwrap();
            let reference = manifest.reference_time;
            let material = table.material_of(
                &record.size,
                record.size.bytes,
                &record.ownership,
                Some(treepo_gen::AgeSpan {
                    oldest_days: record
                        .temporal
                        .first_commit_age_days(reference)
                        .unwrap_or(0),
                    newest_days: record.temporal.last_commit_age_days(reference).unwrap_or(0),
                }),
                &role,
            );
            sampled.push(table.enrichment_of(&manifest, &role, &material));
        }
    }
    hasher.update(sampled.digest().as_bytes());

    hasher.finalize()
}

/// A synthetic container standing for `count` documents dated `spacing` days apart.
///
/// The shape [`probe_enrichment`] needs and nothing more: one aggregate over several dated
/// paths, so the positions along the limb are a function of the dates and therefore controllable
/// to either side of the merge window.
fn probe_container(count: u32, spacing: i64) -> (treepo_model::Manifest, treepo_model::NodeRole) {
    use treepo_model::path::RepoPath;
    use treepo_model::primitives::size::ContentCategory;
    use treepo_model::{Manifest, NodeKind, PathRecord};

    const REFERENCE: i64 = 1_800_000_000;
    const DAY: i64 = 86_400;

    let mut records = Vec::new();
    let mut members = Vec::new();
    let mut root = PathRecord::new(RepoPath::root(), NodeKind::Directory);

    for index in 0..count {
        // Two categories, alternating, so bookshelves and stockpiles are both offered and the
        // per-kind rows are both in the digest.
        let (name, category) = if index % 2 == 0 {
            (format!("doc{index}.md"), ContentCategory::Docs)
        } else {
            (format!("blob{index}.png"), ContentCategory::Asset)
        };
        let path = RepoPath::new(name.as_bytes()).expect("a fixture name is a valid path");

        let mut record = PathRecord::new(path.clone(), NodeKind::File);
        record.size.bytes = 4096 + u64::from(index) * 17;
        record.size.category_bytes = core::iter::once((category, record.size.bytes)).collect();
        record.temporal.first_commit_time = Some(REFERENCE - (2000 + i64::from(index)) * DAY);
        record.temporal.last_commit_time = Some(REFERENCE - i64::from(index) * spacing * DAY);
        // Enough recent churn for the work-site row to be reachable on some of them.
        record.temporal.churn.lifetime = 1000;
        record.temporal.churn.days_30 = u64::from(index % 5) * 250;

        root.size.bytes += record.size.bytes;
        *root.size.category_bytes.entry(category).or_insert(0) += record.size.bytes;
        members.push(path);
        records.push(record);
    }

    root.temporal.first_commit_time = Some(REFERENCE - (2000 + i64::from(count)) * DAY);
    root.temporal.last_commit_time = Some(REFERENCE);
    records.push(root);

    let mut manifest = Manifest::new("probe".to_string(), Seed::root(b"enrichment-probe"));
    manifest.reference_time = REFERENCE;
    manifest.set_paths(records);

    let role = treepo_model::NodeRole::Aggregate(treepo_model::AggregateNode {
        anchor: RepoPath::root(),
        index: 0,
        members,
        bytes: 4096 * u64::from(count.max(1)),
        file_count: count,
        dir_count: 0,
    });

    (manifest, role)
}

/// One mosaic, as bytes.
///
/// Local to the probe rather than taken from `MaterialMap::digest`, because a mosaic sampled on
/// its own has no material around it — the encoding that matters for the corpus lines is the
/// canonical one, and this exists to catch a platform disagreeing about the *allocation* before
/// it reaches a whole tree.
fn hash_mosaic(hasher: &mut Sha256, mosaic: &treepo_model::Mosaic) {
    hasher.update(&mosaic.cells().to_le_bytes());
    hasher.update(&mosaic.claimed().to_le_bytes());
    hasher.update(&mosaic.unclaimed().to_le_bytes());
    hasher.update(&(mosaic.holder_count() as u64).to_le_bytes());
    for (key, held) in mosaic.holders() {
        hasher.update(key.as_bytes());
        hasher.update(&held.to_le_bytes());
    }
}
