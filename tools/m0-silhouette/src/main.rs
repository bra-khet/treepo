//! M0's debug renderer: extraction → L-system → lines and thickness, as a PNG.
//!
//! ```text
//! cargo run -p m0-silhouette                     # every corpus fixture
//! cargo run -p m0-silhouette -- --only empty     # one fixture
//! cargo run -p m0-silhouette -- --pin ripgrep    # a pinned tier repository
//! cargo run -p m0-silhouette -- --path .         # any repository on disk
//! cargo run -p m0-silhouette -- lab              # HTML silhouette lab (S4)
//! ```
//!
//! # What this exists for
//!
//! `design/l-system-parameterization.md` §6 prescribes the workflow that turns a parameter
//! table from a guess into a decision:
//!
//! > 1. Lock a coherent row from the decision menu above as "v0.1".
//! > 2. Implement the absolute minimum skeleton drawer (even a Python turtle or a Bevy debug
//! >    pass that draws only lines + thickness).
//! > 3. Feed it 3–4 real or synthetic repositories.
//! > 4. Observe silhouettes. Adjust one parameter family at a time.
//! > 5. Because every path is seeded by its hash, every change is reproducible.
//!
//! Steps 1 and 2 are done. This is step 3, and it is the first time anyone looks at a tree.
//! `assets/params/lsystem.ron` is coherent, validated, and defended by tests, and every
//! number in it is still a hypothesis — the tests can prove a table is *self-consistent* and
//! cannot prove it is *good*. Only step 4 does that, and step 4 needs a picture.
//!
//! The `lab` subcommand is the interactive form of steps 3–4: one family at a time, render
//! history that never overwrites, and finding exports under `qa/sessions/`.

mod canvas;
mod draw;
mod lab;
mod pipeline;
mod png;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use treepo_gen::Table;

use pipeline::{manifest_for, render_manifest, short_digest};

/// The parameter table shipped with the crate, for the report line.
const BUILT_IN: &str = "built-in (assets/params/lsystem.ron)";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        if args.first().is_some_and(|a| a == "lab") {
            lab::print_usage();
        } else {
            print_usage();
        }
        return ExitCode::SUCCESS;
    }

    if args.first().is_some_and(|a| a == "lab") {
        return match lab::run(&args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("\nm0-silhouette lab: {message}");
                ExitCode::FAILURE
            }
        };
    }

    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("\nm0-silhouette: {message}");
            ExitCode::FAILURE
        }
    }
}

fn print_usage() {
    println!(
        "m0-silhouette — M0's debug renderer: lines and thickness only.

    cargo run -p m0-silhouette                     every corpus fixture
    cargo run -p m0-silhouette -- --only empty     one fixture, by name
    cargo run -p m0-silhouette -- --pin ripgrep    a pinned tier repository
    cargo run -p m0-silhouette -- --path .         any repository on disk
    cargo run -p m0-silhouette -- lab              HTML silhouette lab

  --out <dir>      where the PNGs go             [target/m0-silhouette]
  --size <px>      canvas edge, square           [1024]
  --table <file>   parameter table to load       [{BUILT_IN}]
  --fetch          allow --pin to clone          [never automatic]

  `--table` is AC-SKEL-4: edit the file, run again, nothing is rebuilt.
  `--fetch` is the only thing here that touches the network.

  For the lab, see: cargo run -p m0-silhouette -- lab --help"
    );
}

/// Everything the run was asked for, once the arguments agree with each other.
struct Options {
    out: PathBuf,
    size: u32,
    table: Table,
    table_source: String,
}

fn run(args: &[String]) -> Result<(), String> {
    let out = match value(args, "--out")? {
        Some(path) => PathBuf::from(path),
        // `tools/m0-silhouette/` → the workspace root, so the default lands in `target/`
        // whatever directory cargo was invoked from.
        None => Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("the crate is two levels below the workspace root")
            .join("target")
            .join("m0-silhouette"),
    };

    let size: u32 = match value(args, "--size")? {
        Some(text) => text
            .parse()
            .map_err(|_| format!("--size wants a number of pixels, got `{text}`"))?,
        None => 1024,
    };
    if !(64..=4096).contains(&size) {
        return Err(format!("--size {size} is outside 64..=4096"));
    }

    let (table, table_source) = match value(args, "--table")? {
        Some(path) => {
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("reading the parameter table {path}: {e}"))?;
            // The loader validates against the design document's own rules, so a table edited
            // into nonsense is refused here by name rather than drawn as a strange tree.
            let table = Table::from_ron(&text)
                .map_err(|e| format!("the parameter table is not usable — {e}"))?;
            (table, path)
        }
        None => (Table::built_in(), BUILT_IN.to_owned()),
    };

    let options = Options {
        out,
        size,
        table,
        table_source,
    };

    let targets = targets(args)?;
    std::fs::create_dir_all(&options.out)
        .map_err(|e| format!("creating {}: {e}", options.out.display()))?;

    println!("m0-silhouette — lines and thickness only (PRD §4, M0)\n");
    println!("  table  {}", options.table_source);
    println!(
        "  levels {} deep, {} sites per limb",
        options.table.max_levels(),
        options.table.max_sites()
    );
    println!("  canvas {size}×{size}");
    println!("  out    {}\n", options.out.display());

    println!(
        "  {:<18} {:>6} {:>7} {:>7} {:>6} {:>6}  {:<16} png",
        "target", "paths", "nodes", "segs", "aggr", "depth", "skeleton"
    );

    let mut drawn = 0usize;
    let mut skipped = 0usize;
    for (name, path) in targets {
        match render_one(&name, &path, &options) {
            Ok(report) => {
                drawn += 1;
                println!(
                    "  {:<18} {:>6} {:>7} {:>7} {:>6} {:>6}  {:<16} {}",
                    name,
                    report.paths,
                    report.nodes,
                    report.segments,
                    report.aggregates,
                    report.depth,
                    short_digest(report.digest),
                    report.file
                );
            }
            Err(reason) => {
                skipped += 1;
                println!("  {name:<18} — {reason}");
            }
        }
    }

    println!("\n  {drawn} drawn, {skipped} skipped");
    println!(
        "  ink    limbs bark · trunk and group stems near-black · roots slate · \
         containers terracotta"
    );

    if drawn == 0 {
        return Err("nothing was drawn".to_owned());
    }
    Ok(())
}

struct CliReport {
    paths: usize,
    nodes: usize,
    segments: usize,
    aggregates: usize,
    depth: u16,
    digest: treepo_det::Digest,
    file: String,
}

/// Extracts, grows, draws, and writes one repository.
fn render_one(name: &str, path: &Path, options: &Options) -> Result<CliReport, String> {
    let manifest = manifest_for(path)?;
    let rendered = render_manifest(
        name,
        path,
        &manifest,
        &options.table,
        &options.table_source,
        options.size,
    );

    let file = format!("{name}.png");
    let destination = options.out.join(&file);
    std::fs::write(&destination, &rendered.png)
        .map_err(|e| format!("writing {}: {e}", destination.display()))?;

    let notes = options.out.join(format!("{name}.txt"));
    std::fs::write(&notes, &rendered.sidecar)
        .map_err(|e| format!("writing {}: {e}", notes.display()))?;

    Ok(CliReport {
        paths: rendered.report.paths,
        nodes: rendered.report.nodes,
        segments: rendered.report.segments,
        aggregates: rendered.report.aggregates,
        depth: rendered.report.depth,
        digest: rendered.report.digest,
        file,
    })
}

/// Which repositories to draw, as `(name, path)`.
fn targets(args: &[String]) -> Result<Vec<(String, PathBuf)>, String> {
    let only = value(args, "--only")?;
    let pin = value(args, "--pin")?;
    let path = value(args, "--path")?;

    match (&only, &pin, &path) {
        (Some(_), Some(_), _) | (Some(_), _, Some(_)) | (_, Some(_), Some(_)) => {
            return Err("--only, --pin and --path each choose the targets; pick one".to_owned());
        }
        _ => {}
    }

    if let Some(path) = path {
        let path = PathBuf::from(path);
        let name = path
            .canonicalize()
            .ok()
            .and_then(|full| {
                full.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "repository".to_owned());
        return Ok(vec![(name, path)]);
    }

    if let Some(wanted) = pin {
        let pins = corpus::Pins::built_in();
        let pin = pins
            .get(&wanted)
            .ok_or_else(|| format!("no pinned repository named `{wanted}`"))?;
        let root = corpus::pinned::default_root();
        let may_fetch = args.iter().any(|arg| arg == "--fetch");

        let path = match corpus::pinned::ensure(&root, pin) {
            corpus::Presence::Pinned => root.join(&pin.name),
            _ if !may_fetch => {
                return Err(format!(
                    "`{wanted}` is not on disk at the pinned commit. Pinned repositories are \
                     fetched on demand, never by default — pass --fetch."
                ));
            }
            _ => corpus::pinned::fetch(&root, pin).map_err(|e| e.to_string())?,
        };
        return Ok(vec![(pin.name.clone(), path)]);
    }

    let root = corpus::default_root();
    let fixtures = corpus::ensure(&root).map_err(|e| format!("building the corpus: {e}"))?;
    let mut targets: Vec<(String, PathBuf)> = fixtures
        .into_iter()
        .map(|fixture| (fixture.name.to_owned(), fixture.path))
        .collect();

    if let Some(wanted) = only {
        targets.retain(|(name, _)| *name == wanted);
        if targets.is_empty() {
            return Err(format!("no corpus fixture named `{wanted}`"));
        }
    }
    Ok(targets)
}

/// Reads `--flag value`, refusing a flag with nothing after it.
fn value(args: &[String], flag: &str) -> Result<Option<String>, String> {
    match args.iter().position(|arg| arg == flag) {
        None => Ok(None),
        Some(at) => args
            .get(at + 1)
            .filter(|next| !next.starts_with("--"))
            .cloned()
            .map(Some)
            .ok_or_else(|| format!("{flag} wants a value")),
    }
}
