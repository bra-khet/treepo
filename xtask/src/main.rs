//! treepo's task runner.
//!
//! The campaign's Standing Rules are a list of things that must never regress. Each one
//! that cannot be enforced by the compiler is a command here, so that "is this still true?"
//! is a question anyone can answer in one line and CI answers on every push.
//!
//! ```text
//! cargo xtask determinism     # AC-DET-1/2/3 — triple-run digests, compared across platforms
//! cargo xtask dep-guard       # N6 — no generative crate may depend on bevy
//! cargo xtask readonly-audit  # AC-MAN-2, AC-EXT-4 — extraction writes nothing
//! cargo xtask budget          # AC-EXT-1 — full extraction against the PRD §7 budgets
//! cargo xtask ac-mat-2        # AC-MAT-2 — significant mosaic presence on a T2 pin
//! ```
//!
//! Commands land with the phase that needs them: `id-coverage` in Phase 5. `budget` is listed
//! against Phase 12 in the architecture's file tree, but `AC-EXT-1` is a Phase 1 end
//! condition, so it lands here in its extraction-only form and grows Grow and frame budgets
//! later — the same way `determinism` arrived in Phase 0 covering only the primitive layer.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod ac_mat_2;
mod budget;
mod dep_guard;
mod determinism;
mod readonly_audit;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str);
    let rest = args.get(1..).unwrap_or(&[]);

    let outcome = match command {
        Some("determinism") => determinism::run(rest),
        Some("dep-guard") => dep_guard::run(rest),
        Some("readonly-audit") => readonly_audit::run(rest),
        Some("budget") => budget::run(rest),
        Some("ac-mat-2") => ac_mat_2::run(rest),

        // Listed rather than left to fall through as "unknown command", because the
        // campaign document tells a reader this exists before the phase that builds it.
        //
        // It scans the element-ID buffer for a coloured pixel with no id (`P1`, `N7`).
        // `treepo-render::bake` now produces the colour plane; the parallel `u32` plane is the
        // half still missing, and `treepo-render::pick` answers clicks geometrically until it
        // lands. A command that reported "zero unaccountable pixels" by scanning nothing would
        // be a green gate that cannot fail, which is worse than an absent one.
        Some(pending @ "id-coverage") => Err(format!(
            "`{pending}` lands with treepo-render's ID buffer (Phase 5, D5)"
        )),

        Some("help" | "--help" | "-h") | None => {
            print_usage();
            return ExitCode::SUCCESS;
        }
        Some(unknown) => {
            eprintln!("xtask: unknown command `{unknown}`\n");
            print_usage();
            return ExitCode::FAILURE;
        }
    };

    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("\nxtask: {message}");
            ExitCode::FAILURE
        }
    }
}

fn print_usage() {
    println!(
        "treepo task runner\n\
         \n\
         USAGE:\n    cargo xtask <command> [options]\n\
         \n\
         COMMANDS:\n\
         \x20   determinism      Hash every treepo-det primitive over a fixed sample, three\n\
         \x20                    times, and report the digests (AC-DET-1/2/3).\n\
         \x20                      --runs <n>      repetitions per probe (default 3)\n\
         \x20                      --out <path>    write the canonical report for comparison\n\
         \x20                      --check <path>  compare against an existing report\n\
         \n\
         \x20   dep-guard        Assert no crate in the generative set depends on bevy, and\n\
         \x20                    that treepo-det depends on nothing at all (N6).\n\
         \n\
         \x20   readonly-audit   Run every extraction pass over every corpus fixture and\n\
         \x20                    prove nothing was written to it (AC-MAN-2, AC-EXT-4).\n\
         \x20                      --fixture <name>  audit one shape\n\
         \x20                      --self-test       only prove the detector detects\n\
         \n\
         \x20   budget           Time a full extraction of each pinned repository against\n\
         \x20                    the PRD §7 budgets (AC-EXT-1).\n\
         \x20                      --pins            list the pins and what is on disk\n\
         \x20                      --fetch           clone what is missing (the network step)\n\
         \x20                      --pin <name>      measure one\n\
         \x20                      --threads <n>     log_pass threads (default 4, min spec)\n\
         \x20                      --runs <n>        repeat and show the spread\n\
         \n\
         \x20   ac-mat-2         Prove significant contributors keep mosaic presence on a\n\
         \x20                    pinned T2 repository (AC-MAT-2). Local evidence, not CI.\n\
         \x20                      --pin <name>      default bevy; godot is the other T2\n\
         \x20                      --threads <n>     log_pass threads (default 4)\n"
    );
}

/// The workspace root, derived from this crate's manifest directory.
pub(crate) fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask/ always has a parent")
        .to_path_buf()
}

/// The cargo binary that invoked us, so that a pinned toolchain stays pinned.
pub(crate) fn cargo() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned())
}

/// Reads `--name <value>` from an argument list.
pub(crate) fn flag_value(args: &[String], name: &str) -> Result<Option<String>, String> {
    match args.iter().position(|arg| arg == name) {
        None => Ok(None),
        Some(index) => args
            .get(index + 1)
            .cloned()
            .map(Some)
            .ok_or_else(|| format!("`{name}` needs a value")),
    }
}
