//! The determinism harness — `AC-DET-1`, `AC-DET-2`, `AC-DET-3`.
//!
//! Architecture D2: "CI runs each corpus fixture three times per platform and compares
//! serialized-output hashes across all nine runs."
//!
//! This is the Phase 0 form of that harness. There is no corpus yet and no skeleton to
//! serialize, so what it hashes is the layer everything else will be built on: every
//! primitive in `treepo-det`, exercised over a fixed sample, reduced to one digest per
//! probe.
//!
//! Two properties are checked, and they are not the same property:
//!
//! * **`AC-DET-1`, within a platform.** Each probe runs three times in one process and the
//!   digests must agree. This catches anything that reads ambient state — a clock, a
//!   process-seeded hasher, an address — because those vary between runs of one binary.
//! * **`AC-DET-2`, across platforms.** The report written by `--out` contains nothing
//!   platform-specific, so CI can compare the files from all three runners byte for byte.
//!   This is the check that would catch a platform `libm` creeping into the trig path.
//!
//! Later phases extend the probe list rather than replacing the harness: Phase 3 adds
//! skeleton hashes, Phase 6 adds the `GrowTimeline` hash (D6).

use std::fmt::Write as _;

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
];

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

        println!("  {:<10} {first}", probe.name);
        println!("  {:<10} {}", "", probe.what);
        writeln!(report, "{} {first}", probe.name).expect("writing to a String cannot fail");
        overall.update(probe.name.as_bytes());
        overall.update(first.as_bytes());
    }

    let overall = overall.finalize();
    writeln!(report, "overall {overall}").expect("writing to a String cannot fail");
    println!("\n  {:<10} {overall}", "overall");

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
