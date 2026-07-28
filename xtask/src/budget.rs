//! The extraction budget gate — `AC-EXT-1`, against PRD §7.
//!
//! > **AC-EXT-1** — Full extraction of a T2 repository completes within the §7 budget on
//! > reference hardware, using `git log --numstat` and *without* invoking `git blame`.
//!
//! | Tier | Target | Hard ceiling |
//! |------|--------|--------------|
//! | T1 | 10 s | 30 s |
//! | T2 | 60 s | 3 min |
//! | T3 | 10 min | 30 min |
//!
//! # Why this needs real repositories
//!
//! The synthetic shapes in `tools/corpus` cover the edge cases and cannot cover this. A
//! generated twenty-thousand-commit repository has uniform commit sizes, uniform authorship,
//! no merges worth the name and no rename storms — timing against one would measure the
//! generator's idea of a repository. PRD §3 says so directly, and `tools/corpus/pins.ron` is
//! the answer: real public repositories, held at an exact commit.
//!
//! # Target and ceiling are different failures
//!
//! §7 gives two numbers per tier and means two different things by them. Over the target is a
//! result to look at; over the ceiling is a broken promise. So a target overrun prints and
//! carries on, and only a ceiling breach fails the command. A gate that treated them alike
//! would either cry wolf at 61 seconds or say nothing at 179.
//!
//! # What is measured, and what is not
//!
//! Every Phase 1 pass, timed individually, because a total that has moved is only useful if it
//! says which pass moved. Fetching is never inside the measurement — `--fetch` completes
//! before the clock starts, or the figure would be a broadband test.
//!
//! The first run against a freshly fetched repository reads a cold OS page cache and is
//! consistently the slowest. That is not corrected for. It is the honest first-run number, and
//! `AC-EXT-1` is about first-run extraction; `--runs` shows the spread if you want to see how
//! much of a figure was I/O.
//!
//! # `git blame` is not reachable, and this says so
//!
//! The second half of `AC-EXT-1` is structural rather than measured: `treepo-vcs` does not
//! enable `gix`'s `blame` feature, so `gix::blame` does not exist in this build and a
//! `compile_fail` doctest in that crate holds it that way. This command reports the fact
//! rather than re-testing it, because a budget met by a build that *could* blame would be
//! meeting half the criterion.

use std::path::Path;

use corpus::{Pin, Pins, Presence};

/// A monotonic clock reading.
///
/// `clippy.toml` bans `std::time::Instant` as a type *and* `Instant::now` as a method, so that
/// no wall clock can reach a generated value (`N3`). A budget harness is the one thing in the
/// workspace whose entire purpose is to read one, so unlike `readonly_audit` — which is
/// allowed the type but genuinely never calls `now` — this needs both halves of the exception.
///
/// It is confined to the alias and the one function below. What keeps `N3` intact is not that
/// the clock is hard to reach here but where the reading goes: it is printed, compared against
/// a constant from PRD §7, and dropped. No timing crosses into `treepo-vcs`, and nothing in
/// this module is reachable from the product.
#[allow(clippy::disallowed_types)]
type Clock = std::time::Instant;

/// The only clock read in the workspace.
#[allow(clippy::disallowed_methods)]
fn now() -> Clock {
    Clock::now()
}

/// The passes a full extraction runs, in order.
const PASSES: [&str; 7] = [
    "discover",
    "walk",
    "scan",
    "signals",
    "log_pass",
    "apply",
    "history_signals",
];

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let only = crate::flag_value(args, "--pin")?;
    let list_only = args.iter().any(|arg| arg == "--pins");
    let may_fetch = args.iter().any(|arg| arg == "--fetch");
    let threads: usize = match crate::flag_value(args, "--threads")? {
        None => 4,
        Some(text) => text
            .parse()
            .map_err(|_| format!("--threads expects a number, got `{text}`"))?,
    };
    let runs: usize = match crate::flag_value(args, "--runs")? {
        None => 1,
        Some(text) => text
            .parse()
            .map_err(|_| format!("--runs expects a number, got `{text}`"))?,
    };
    if runs == 0 || threads == 0 {
        return Err("--runs and --threads must be at least 1".to_owned());
    }

    let pins = Pins::built_in();
    let root = corpus::pinned::default_root();

    let selected: Vec<&Pin> = pins
        .repositories
        .iter()
        .filter(|pin| only.as_deref().is_none_or(|name| name == pin.name))
        .collect();
    if selected.is_empty() {
        return Err(format!(
            "no pin named `{}`. Known: {}",
            only.unwrap_or_default(),
            pins.repositories
                .iter()
                .map(|pin| pin.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    if list_only {
        return list(&root, &selected);
    }

    println!("budget — full extraction against PRD §7 (AC-EXT-1)\n");
    println!("  cache    {}", root.display());
    println!(
        "  machine  {} logical cores; log_pass held to {threads} (§7 minimum spec is 4)",
        std::thread::available_parallelism().map_or(0, std::num::NonZero::get)
    );
    // Part of the acceptance criterion, not decoration. See the module docs.
    println!("  blame    not linked into this build — `gix`'s `blame` feature is off\n");

    let mut measured = 0usize;
    let mut breaches = Vec::new();
    let mut skipped = Vec::new();

    for pin in selected {
        let path = match prepare(&root, pin, may_fetch) {
            Ok(path) => path,
            Err(why) => {
                skipped.push(format!("{}: {why}", pin.name));
                continue;
            }
        };

        println!("  {} ({})  {}", pin.name, pin.tier.name(), pin.tag);
        let mut runs_seconds = Vec::new();
        let mut last: Option<Measurement> = None;

        for _ in 0..runs {
            let measurement = measure(&path, threads)?;
            runs_seconds.push(measurement.total);
            last = Some(measurement);
        }
        let measurement = last.expect("runs >= 1");
        measured += 1;

        for (name, seconds) in PASSES.iter().zip(&measurement.passes) {
            println!("    {name:<17} {seconds:>8.2} s");
        }
        println!("    {:<17} {:>8.2} s", "TOTAL", measurement.total);
        if runs > 1 {
            let fastest = runs_seconds.iter().copied().fold(f64::INFINITY, f64::min);
            let slowest = runs_seconds.iter().copied().fold(0.0_f64, f64::max);
            println!("    {:<17} {fastest:>8.2} s .. {slowest:.2} s", "over runs");
        }
        println!(
            "    {:<17} {} files ({} paths), {} commits, {} authors",
            "shape", measurement.files, measurement.paths, measurement.commits, measurement.authors
        );
        // A pin never sits on its tier's centroid, so the raw total is only readable next to
        // what it was spent on. The history pass dominates every measurement taken so far and
        // scales with commits, which makes this the figure that compares across pins.
        if measurement.commits > 0 {
            println!(
                "    {:<17} {:.2} s per 1,000 commits; {} at {} nominal commits",
                "rate",
                measurement.per_thousand_commits(),
                seconds(measurement.at_tier_scale(pin.tier)),
                pin.tier.nominal().1,
            );
        }

        report_tier_fit(pin, &measurement);

        match pin.tier.budget_seconds() {
            None => println!("    {:<17} {} has no §7 row", "budget", pin.tier.name()),
            Some((target, ceiling)) => {
                let verdict = verdict(measurement.total, target, ceiling);
                println!(
                    "    {:<17} {verdict} (target {target} s, ceiling {ceiling} s)",
                    "budget"
                );
                #[allow(clippy::cast_precision_loss)]
                if measurement.total > ceiling as f64 {
                    breaches.push(format!(
                        "{} ({}) took {:.1} s against a {} s hard ceiling",
                        pin.name,
                        pin.tier.name(),
                        measurement.total,
                        ceiling
                    ));
                }
            }
        }
        println!();
    }

    for note in &skipped {
        println!("  skipped  {note}");
    }
    if measured == 0 {
        return Err(
            "nothing was measured. Pinned repositories are fetched on demand, never by \
             default — run `cargo xtask budget --fetch` to bring them down, or \
             `cargo xtask budget --pins` to see what is missing."
                .to_owned(),
        );
    }

    if breaches.is_empty() {
        println!("  {measured} repository/repositories measured, no §7 ceiling exceeded");
        Ok(())
    } else {
        for breach in &breaches {
            eprintln!("  ! {breach}");
        }
        Err(format!(
            "{} §7 ceiling breach(es). The ceiling is the promise, not the target — a \
             measurement over it means extraction got slower than the product may be, and \
             the fix is the pass whose line moved rather than the number in the PRD.",
            breaches.len()
        ))
    }
}

fn list(root: &Path, pins: &[&Pin]) -> Result<(), String> {
    println!("pinned repositories — PRD §3\n");
    println!("  cache  {}\n", root.display());
    for pin in pins {
        let presence = match corpus::pinned::ensure(root, pin) {
            Presence::Pinned => "on disk, at the pin".to_owned(),
            Presence::Drifted { found } => {
                format!("on disk but at {}", &found[..12.min(found.len())])
            }
            Presence::Broken => "on disk but unreadable".to_owned(),
            Presence::Absent => "not fetched".to_owned(),
        };
        println!(
            "  {:<10} {:<3} {:<24} {presence}",
            pin.name,
            pin.tier.name(),
            pin.tag
        );
        println!("  {:<10} {} @ {}", "", pin.url, pin.commit);
        println!("  {:<10} {}\n", "", pin.what);
    }
    println!("  `--fetch` is the only thing here that touches the network.");
    Ok(())
}

/// Makes a pin measurable, fetching only if asked.
fn prepare(root: &Path, pin: &Pin, may_fetch: bool) -> Result<std::path::PathBuf, String> {
    match corpus::pinned::ensure(root, pin) {
        Presence::Pinned => Ok(root.join(&pin.name)),
        other if !may_fetch => Err(match other {
            Presence::Absent => "not fetched (pass --fetch)".to_owned(),
            Presence::Drifted { found } => format!(
                "on disk at {} rather than the pin; pass --fetch to correct it",
                &found[..12.min(found.len())]
            ),
            _ => "on disk but unreadable; pass --fetch to re-clone".to_owned(),
        }),
        _ => corpus::pinned::fetch(root, pin).map_err(|e| e.to_string()),
    }
}

/// One timed extraction.
struct Measurement {
    passes: [f64; PASSES.len()],
    total: f64,
    files: usize,
    paths: usize,
    commits: u32,
    authors: usize,
}

impl Measurement {
    /// Seconds per thousand commits — the shape-independent reading.
    #[allow(clippy::cast_precision_loss)]
    fn per_thousand_commits(&self) -> f64 {
        self.total / f64::from(self.commits) * 1000.0
    }

    /// What this rate would cost at the tier's nominal commit count.
    ///
    /// Not a substitute for measuring a repository that really is the tier — it assumes the
    /// cost is linear in commits, which the RISK-A spike found it very nearly is because
    /// effectively all of it is blob diffing. It is here so a pin that overshoots its tier
    /// on commits can still be read against the tier's budget.
    #[allow(clippy::cast_precision_loss)]
    fn at_tier_scale(&self, tier: corpus::Tier) -> f64 {
        self.per_thousand_commits() * tier.nominal().1 as f64 / 1000.0
    }
}

/// `1.15 s`, `2 min 11 s`. §7 states budgets in both, and so should a report read against it.
fn seconds(total: f64) -> String {
    if total < 90.0 {
        format!("{total:.1} s")
    } else {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let whole = total as u64;
        format!("{} min {:02} s", whole / 60, whole % 60)
    }
}

/// Runs a full extraction, timing each pass.
///
/// Deliberately the same sequence `xtask readonly-audit` runs and for the same reason: each
/// pass is named here rather than reached through a helper, so a pass cannot quietly stop
/// being part of "full extraction" while the number keeps being reported as though it were.
fn measure(root: &Path, threads: usize) -> Result<Measurement, String> {
    use treepo_vcs::lang::{Catalogue, ContentOptions, apply_history_signals};
    use treepo_vcs::{FilterSet, HistoryOptions, SignalDictionary, WalkOptions};

    let mut passes = [0.0_f64; PASSES.len()];
    let overall = now();

    let mark = now();
    let target = treepo_vcs::discover(root).map_err(|e| format!("discover: {e}"))?;
    passes[0] = mark.elapsed().as_secs_f64();

    let filter = FilterSet::built_in();
    let catalogue = Catalogue::built_in();

    let mark = now();
    let mut structure = treepo_vcs::walk(&target, &filter, WalkOptions::default())
        .map_err(|e| format!("walk: {e}"))?;
    passes[1] = mark.elapsed().as_secs_f64();

    let mark = now();
    let mut languages = treepo_model::manifest::LanguageTable::new();
    treepo_vcs::scan(
        &target,
        &mut structure,
        &catalogue,
        &mut languages,
        ContentOptions::default(),
    )
    .map_err(|e| format!("scan: {e}"))?;
    passes[2] = mark.elapsed().as_secs_f64();

    let mark = now();
    treepo_vcs::signals::apply(
        &mut structure.records,
        &SignalDictionary::built_in(),
        &catalogue,
    );
    passes[3] = mark.elapsed().as_secs_f64();

    let mark = now();
    let history = treepo_vcs::log_pass(
        &target,
        &filter,
        HistoryOptions {
            threads,
            ..HistoryOptions::default()
        },
    )
    .map_err(|e| format!("log_pass: {e}"))?;
    passes[4] = mark.elapsed().as_secs_f64();

    let mark = now();
    treepo_vcs::log_pass::apply(&mut structure.records, &history);
    passes[5] = mark.elapsed().as_secs_f64();

    let mark = now();
    apply_history_signals(&mut structure.records);
    passes[6] = mark.elapsed().as_secs_f64();

    Ok(Measurement {
        passes,
        total: overall.elapsed().as_secs_f64(),
        // PRD §3's tier table counts *files*; `records` also holds every directory, and at
        // T3 the difference is tens of thousands of entries.
        files: structure
            .records
            .iter()
            .filter(|record| record.kind == treepo_model::manifest::NodeKind::File)
            .count(),
        paths: structure.records.len(),
        commits: history.commit_count,
        authors: history.authors.len(),
    })
}

#[allow(clippy::cast_precision_loss)]
fn verdict(total: f64, target: u64, ceiling: u64) -> &'static str {
    if total <= target as f64 {
        "within target"
    } else if total <= ceiling as f64 {
        "OVER TARGET, within ceiling"
    } else {
        "OVER CEILING"
    }
}

/// Says so when a pin no longer sits in the tier it claims.
///
/// Not a failure. A repository held at a fixed commit cannot drift, so this only fires if the
/// PRD §3 bands were revised or the pin was chosen optimistically — both of which are worth a
/// line of output and neither of which is a regression in extraction.
fn report_tier_fit(pin: &Pin, measurement: &Measurement) {
    let files = measurement.files as u64;
    let commits = u64::from(measurement.commits);
    let mut notes = Vec::new();
    if let Some(note) = fit(files, pin.tier.files(), "files") {
        notes.push(note);
    }
    if let Some(note) = fit(commits, pin.tier.commits(), "commits") {
        notes.push(note);
    }
    if !notes.is_empty() {
        println!(
            "    {:<17} {} for {}",
            "tier fit",
            notes.join(", "),
            pin.tier.name()
        );
    }
}

/// Describes how far a measured figure sits outside its band, or `None` if it is inside.
fn fit(measured: u64, band: (u64, u64), axis: &str) -> Option<String> {
    let (lo, hi) = band;
    if measured < lo {
        Some(format!("light on {axis} ({measured} < {lo})"))
    } else if measured > hi {
        Some(format!("heavy on {axis} ({measured} > {hi})"))
    } else {
        None
    }
}
