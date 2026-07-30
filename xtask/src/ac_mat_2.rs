//! `AC-MAT-2` on a real T2 repository — campaign Phase 4 end condition.
//!
//! > A 2%-share contributor retains visible mosaic presence on the T2 fixture (`AC-MAT-2`).
//!
//! The generative rule is already unit-tested on a synthetic tree that plants a two-percent
//! visitor on every path. That is T0/T1 shape. This command is the named evidence on a
//! **pinned mid-size public repository** (default: `bevy` at its `pins.ron` commit):
//! extract → grow → materialize →
//! [`audit_significant_presence`](treepo_gen::audit_significant_presence).
//!
//! Not run in CI. The pin is a multi-gigabyte clone under `target/corpus-pinned/` and is
//! fetched only by an explicit `cargo xtask budget --fetch`. Re-run locally when materials
//! change:
//!
//! ```text
//! cargo xtask ac-mat-2              # bevy (T2)
//! cargo xtask ac-mat-2 --pin godot  # second T2 reading
//! ```
//!
//! Timing is deliberately not reported here. `cargo xtask budget` owns wall-clock measurement
//! for pins; this gate answers a yes/no about mosaics, and clock reads are banned workspace-
//! wide under `N3` except where a harness's entire job is a budget.

use std::path::Path;

use corpus::pinned::{self, Pin, Pins, Presence};
use treepo_gen::{MaterialTable, audit_significant_presence};
use treepo_vcs::lang::Catalogue;
use treepo_vcs::{ExtractOptions, FilterSet, HistoryOptions};

/// Default pin: the T2 repository the campaign and Phase 1 budget work already use.
const DEFAULT_PIN: &str = "bevy";

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let name = crate::flag_value(args, "--pin")?.unwrap_or_else(|| DEFAULT_PIN.to_owned());
    let threads = crate::flag_value(args, "--threads")?
        .map(|s| s.parse::<usize>().map_err(|e| format!("--threads: {e}")))
        .transpose()?
        .unwrap_or(4);
    if threads == 0 {
        return Err("--threads must be at least 1".to_owned());
    }

    let pins = Pins::built_in();
    let root = pinned::default_root();
    let pin = pins
        .get(&name)
        .ok_or_else(|| {
            let known: Vec<_> = pins.repositories.iter().map(|p| p.name.as_str()).collect();
            format!(
                "unknown pin `{name}`. Known: {}. Run `cargo xtask budget --pins`.",
                known.join(", ")
            )
        })?
        .clone();

    ensure_pinned(&root, &pin)?;

    let path = root.join(&pin.name);
    println!("ac-mat-2 — significant mosaic presence on a real repository (AC-MAT-2)\n");
    println!("  pin      {} ({})  {}", pin.name, pin.tier.name(), pin.tag);
    println!("  commit   {}", pin.commit);
    println!("  path     {}", path.display());
    println!("  threads  {threads} (log_pass)\n");

    let target = treepo_vcs::discover(&path).map_err(|e| format!("discover: {e}"))?;
    let identity = treepo_store::resolve(target.root(), target.repository(), 0)
        .map_err(|e| format!("resolve: {e}"))?
        .identity;
    let root_seed = treepo_store::resolve::root_seed(&identity);
    let manifest = treepo_vcs::extract(
        &target,
        &FilterSet::built_in(),
        &Catalogue::built_in(),
        root_seed,
        env!("CARGO_PKG_VERSION").to_owned(),
        ExtractOptions {
            history: HistoryOptions {
                threads,
                ..HistoryOptions::default()
            },
            ..ExtractOptions::default()
        },
    )
    .map_err(|e| format!("extract: {e}"))?;

    let skeleton = treepo_gen::grow(&manifest, &treepo_gen::Table::built_in());
    let table = MaterialTable::built_in();
    let materials = treepo_gen::materialize(&manifest, &skeleton, &table);
    let report = audit_significant_presence(&manifest, &skeleton, &materials, &table);

    println!("  nodes               {}", skeleton.nodes().len());
    println!("  significant_ppm     {}", table.normalize.significant_ppm);
    println!("  pairs checked       {}", report.pairs_checked);
    println!("  missing             {}", report.missing);
    println!("  saved by quota      {}", report.saved_by_quota);

    if !report.holds() {
        if report.pairs_checked == 0 {
            return Err(
                "AC-MAT-2: nothing significant was attributed — the pin produced no \
                 ownership to audit (shallow clone? empty history?)"
                    .into(),
            );
        }
        return Err(format!(
            "AC-MAT-2 violated: {} significant contributor(s) vanished from their mosaic \
             across {} pairs on {}",
            report.missing, report.pairs_checked, pin.name
        ));
    }

    println!(
        "\nAC-MAT-2 holds on {} ({}): every significant contributor keeps mosaic presence \
         ({} pairs; {} needed the guaranteed quota).",
        pin.name, pin.tag, report.pairs_checked, report.saved_by_quota
    );
    Ok(())
}

fn ensure_pinned(root: &Path, pin: &Pin) -> Result<(), String> {
    match pinned::ensure(root, pin) {
        Presence::Pinned => Ok(()),
        Presence::Absent => Err(format!(
            "`{}` is not on disk under {}. Fetch with:\n  cargo xtask budget --fetch",
            pin.name,
            root.display()
        )),
        Presence::Drifted { found } => Err(format!(
            "`{}` is at {found}, not the pinned commit {}. Re-fetch with:\n  \
             cargo xtask budget --fetch",
            pin.name, pin.commit
        )),
        Presence::Broken => Err(format!(
            "`{}` under {} is not a usable repository. Remove it and run:\n  \
             cargo xtask budget --fetch",
            pin.name,
            root.display()
        )),
    }
}
