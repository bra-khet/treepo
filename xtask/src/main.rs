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
//! ```
//!
//! Commands land with the phase that needs them: `id-coverage` in Phase 5 and `budget` in
//! Phase 12 (architecture, xtask file tree).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

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

        // Listed rather than left to fall through as "unknown command", because the
        // campaign document tells a reader these exist before the phase that builds them.
        Some(pending @ ("id-coverage" | "budget")) => Err(format!(
            "`{pending}` lands with a later phase: id-coverage in Phase 5, budget in Phase 12"
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
         \x20                      --self-test       only prove the detector detects\n"
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
