//! M0's debug renderer: extraction → L-system → lines and thickness, as a PNG.
//!
//! ```text
//! cargo run -p m0-silhouette                     # every corpus fixture
//! cargo run -p m0-silhouette -- --only empty     # one fixture
//! cargo run -p m0-silhouette -- --pin ripgrep    # a pinned tier repository
//! cargo run -p m0-silhouette -- --path .         # any repository on disk
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
//! # Which acceptance criteria it answers
//!
//! - **`AC-SKEL-1`** — orderly versus wild, from one table. A metric can report that two
//!   silhouettes differ; whether one reads as *orderly* and the other as *wild* is a
//!   judgement, and this is what it is made against.
//! - **`AC-SKEL-2`** — an empty repository is a seed and a root cluster, not a lonely trunk.
//! - **`AC-SKEL-4`** — `--table <file>` loads a parameter table at run time. Editing the file
//!   and running again changes the silhouette with nothing rebuilt. That flag *is* the
//!   criterion; `F-SKEL-5` made it nearly free.
//! - **`AC-DET-1`/`AC-DET-2`** — every run prints a skeleton digest, and the rasterizer is
//!   integer-only, so the PNG bytes are comparable across platforms too. This does not
//!   replace `cargo xtask determinism`; it makes disagreement visible while tuning.
//!
//! # Why it is a separate binary and not an xtask command
//!
//! `xtask` answers "does this constraint still hold" with an exit code, and everything in it
//! belongs in CI. This answers "what does it look like", which is a question with no pass and
//! no fail. Putting a tuning tool behind a gate runner would imply the pictures could be
//! wrong, and they cannot be — only the parameters can.

mod canvas;
mod draw;
mod png;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use treepo_det::{Digest, Sha256};
use treepo_gen::{Table, grow};
use treepo_model::{Manifest, NodeRole, Skeleton};

/// The parameter table shipped with the crate, for the report line.
const BUILT_IN: &str = "built-in (assets/params/lsystem.ron)";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_usage();
        return ExitCode::SUCCESS;
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

  --out <dir>      where the PNGs go             [target/m0-silhouette]
  --size <px>      canvas edge, square           [1024]
  --table <file>   parameter table to load       [{BUILT_IN}]
  --fetch          allow --pin to clone          [never automatic]

  `--table` is AC-SKEL-4: edit the file, run again, nothing is rebuilt.
  `--fetch` is the only thing here that touches the network."
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
        match render(&name, &path, &options) {
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
                    short(report.digest),
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

/// What one rendered repository is worth reporting.
struct Report {
    paths: usize,
    nodes: usize,
    segments: usize,
    aggregates: usize,
    depth: u16,
    digest: Digest,
    file: String,
}

/// Extracts, grows, draws, and writes one repository.
fn render(name: &str, path: &Path, options: &Options) -> Result<Report, String> {
    let manifest = manifest_for(path)?;
    let skeleton = grow(&manifest, &options.table);

    let (canvas, view) = draw::draw(&skeleton, options.size);
    let image = png::encode(
        canvas.width(),
        canvas.height(),
        &draw::palette(),
        &canvas.into_indices(),
    );

    let file = format!("{name}.png");
    let destination = options.out.join(&file);
    std::fs::write(&destination, &image)
        .map_err(|e| format!("writing {}: {e}", destination.display()))?;

    // The world extent is the one number a fitted image throws away, so it goes to the
    // sidecar rather than being lost. A `.txt` beside each PNG is enough — a run that wants
    // to compare two tables reads these, and a human reading one wants it next to the image.
    let (min_x, min_y, max_x, max_y) = view.extent;
    let digest = digest(&skeleton);
    let sidecar = format!(
        "target      {name}\nsource      {}\ntable       {}\ncanvas      {}x{}\n\
         extent      x {:.3} .. {:.3}   y {:.3} .. {:.3}\n\
         paths       {}\nnodes       {}\nsegments    {}\naggregates  {}\ndepth       {}\n\
         skeleton    {digest}\n",
        path.display(),
        options.table_source,
        options.size,
        options.size,
        min_x.to_f64(),
        max_x.to_f64(),
        min_y.to_f64(),
        max_y.to_f64(),
        manifest.paths().len(),
        skeleton.nodes().len(),
        skeleton.segments().len(),
        skeleton.aggregate_count(),
        depth_of(&skeleton),
    );
    let notes = options.out.join(format!("{name}.txt"));
    std::fs::write(&notes, sidecar).map_err(|e| format!("writing {}: {e}", notes.display()))?;

    Ok(Report {
        paths: manifest.paths().len(),
        nodes: skeleton.nodes().len(),
        segments: skeleton.segments().len(),
        aggregates: skeleton.aggregate_count(),
        depth: depth_of(&skeleton),
        digest,
        file,
    })
}

/// Runs the Phase 1 pipeline over one repository.
fn manifest_for(root: &Path) -> Result<Manifest, String> {
    use treepo_vcs::lang::Catalogue;
    use treepo_vcs::{ExtractOptions, FilterSet};

    let target = treepo_vcs::discover(root).map_err(|e| format!("discover: {e}"))?;

    // `resolved_at` is zero for the reason `readonly-audit` gives: a clock reading here would
    // make the tool's own output depend on when it ran, and this one prints a digest.
    let identity = treepo_store::resolve(target.root(), target.repository(), 0)
        .map_err(|e| format!("resolve: {e}"))?
        .identity;

    treepo_vcs::extract(
        &target,
        &FilterSet::built_in(),
        &Catalogue::built_in(),
        treepo_store::resolve::root_seed(&identity),
        env!("CARGO_PKG_VERSION").to_owned(),
        ExtractOptions::default(),
    )
    .map_err(|e| format!("extract: {e}"))
}

/// A hash of the skeleton's geometry and roles.
///
/// Not the PNG's hash, deliberately. The PNG folds the skeleton through a fit that depends on
/// the canvas size, so two runs at different `--size` values would disagree about a tree that
/// is identical. This is the thing `AC-DET-1` is actually about.
fn digest(skeleton: &Skeleton) -> Digest {
    let mut hasher = Sha256::new();
    hasher.update(b"treepo-skeleton-v1");

    for node in skeleton.nodes() {
        hasher.update(&node.origin.x.to_bits().to_le_bytes());
        hasher.update(&node.origin.y.to_bits().to_le_bytes());
        hasher.update(&node.heading.to_bits().to_le_bytes());
        hasher.update(node.seed.as_bytes());
        // The role's discriminant and its anchor, so a limb that became a container is a
        // different skeleton even where the geometry happens to land in the same place.
        hasher.update(&[match &node.role {
            NodeRole::Limb { .. } => 0,
            NodeRole::Group { .. } => 1,
            NodeRole::Aggregate(_) => 2,
            NodeRole::RootMass { .. } => 3,
        }]);
        hasher.update(node.role.anchor().as_bytes());
        hasher.update(b"\0");
    }

    for segment in skeleton.segments() {
        for value in [
            segment.start.x,
            segment.start.y,
            segment.end.x,
            segment.end.y,
            segment.base_width,
            segment.tip_width,
        ] {
            hasher.update(&value.to_bits().to_le_bytes());
        }
        hasher.update(&segment.node.index().to_le_bytes());
        hasher.update(&[segment.generation]);
    }

    hasher.finalize()
}

/// The deepest limb in the skeleton — `A3`'s cap, as observed rather than as configured.
fn depth_of(skeleton: &Skeleton) -> u16 {
    skeleton
        .nodes()
        .iter()
        .filter_map(|node| match &node.role {
            NodeRole::Limb { path } => Some(path.depth()),
            NodeRole::Aggregate(aggregate) => Some(aggregate.anchor.depth()),
            _ => None,
        })
        .max()
        .unwrap_or(0)
}

/// The first eight bytes of a digest, which is what fits in a table and is plenty to notice a
/// change by. The sidecar carries the whole thing.
fn short(digest: Digest) -> String {
    digest.to_string()[..16].to_owned()
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
