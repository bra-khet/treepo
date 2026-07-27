//! Builds the corpus fixtures on demand.
//!
//! ```text
//! cargo run -p corpus -- [out-dir]
//! ```
//!
//! Defaults to `target/corpus`, which is gitignored — the fixtures are a build artifact
//! (PRD §3), reproducible from this crate rather than checked in.

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::args()
        .nth(1)
        .map_or_else(corpus::default_root, PathBuf::from);

    println!("building corpus into {}", out.display());
    let mut skipped = 0usize;
    for shape in corpus::all_shapes() {
        if !shape.platforms.available() {
            println!("  {:<18} skipped — {:?} only", shape.name, shape.platforms);
            skipped += 1;
            continue;
        }
        let fixture = corpus::build(&out, *shape)?;
        println!("  {:<18} {}", fixture.name, shape.covers);
    }

    println!(
        "\n{} fixtures built, {skipped} skipped on this platform",
        corpus::all_shapes().len() - skipped
    );
    Ok(())
}
