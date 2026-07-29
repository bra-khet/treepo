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
//! two more, `pseudonym` and `author-color`, because `AC-ID-2` is the same claim as
//! `AC-DET-2` about a different output and deserves the same evidence rather than an
//! argument from `treepo-id` being integer-only.
//!
//! **The corpus stage** is D2's sentence, arrived at in Phase 3. Every corpus fixture is
//! extracted and grown, and the resulting [`Skeleton`](treepo_model::Skeleton) is reduced to
//! its digest. This is the stage that can fail for an interesting reason: the probes cover the
//! arithmetic, and only this covers what the L-system, the composition order, the aggregation
//! threshold, and the trunk column do *with* that arithmetic.
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
//! `no-remote`, and `multi-remote` hold the same files and differ only in refs and remotes, so
//! under a fixed seed they grow the same skeleton and print the same digest. That is the
//! correct answer here. Those four exist to exercise `F-MAN-3`, and it is `tests/identity.rs`
//! that has to tell them apart.
//!
//! Fixtures that only some platforms can build are excluded for the same reason: a report
//! listing `symlinks` on two runners and not the third would differ for a reason that is not a
//! finding. `tools/corpus` records which those are, and this reads that rather than guessing.
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

    println!("\nskeletons — every corpus fixture, grown {runs} times\n");
    println!("  corpus  {}", root.display());
    println!("  table   built-in (assets/params/lsystem.ron)\n");

    let table = treepo_gen::Table::built_in();

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

        let outcome = match manifest_for(&fixture.path) {
            Ok(manifest) => {
                let first = treepo_gen::grow(&manifest, &table).digest();
                // AC-DET-1: the same repository state, grown again, must not move.
                for repeat in 1..runs {
                    let again = treepo_gen::grow(&manifest, &table).digest();
                    if again != first {
                        return Err(format!(
                            "`{}` does not grow the same skeleton twice\n  \
                             run 1: {first}\n  run {}: {again}\n\
                             Something in the generative path reads ambient state.",
                            shape.name,
                            repeat + 1
                        ));
                    }
                }
                first.to_string()
            }
            Err(why) if REFUSED.contains(&shape.name) => {
                println!("  {:<18} refused — {why}", shape.name);
                "refused".to_owned()
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

        if outcome != "refused" {
            println!("  {:<18} {outcome}", shape.name);
        }
        writeln!(report, "skeleton/{} {outcome}", shape.name)
            .expect("writing to a String cannot fail");
        overall.update(shape.name.as_bytes());
        overall.update(outcome.as_bytes());
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
