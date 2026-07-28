//! Shared extract → grow → draw → PNG path used by both the batch CLI and the lab.

use std::path::Path;

use treepo_det::{Digest, Sha256};
use treepo_gen::{Table, grow};
use treepo_model::{Manifest, NodeRole, Skeleton};

use crate::draw;
use crate::png;

/// What one rendered repository is worth reporting.
#[derive(Debug, Clone)]
pub(crate) struct Report {
    /// Manifest path count.
    pub paths: usize,
    /// Skeleton node count.
    pub nodes: usize,
    /// Skeleton segment count.
    pub segments: usize,
    /// Aggregate-container count.
    pub aggregates: usize,
    /// Deepest limb depth observed.
    pub depth: u16,
    /// Geometry digest (not the PNG hash).
    pub digest: Digest,
}

/// PNG bytes plus the report for one skeleton.
#[derive(Debug, Clone)]
pub(crate) struct Rendered {
    /// Index-color PNG.
    pub png: Vec<u8>,
    /// Counts and digest.
    pub report: Report,
    /// Sidecar text (extent, counts, digest).
    pub sidecar: String,
}

/// Runs the Phase 1 pipeline over one repository.
pub(crate) fn manifest_for(root: &Path) -> Result<Manifest, String> {
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

/// Grows, draws, and encodes one manifest under a table.
pub(crate) fn render_manifest(
    name: &str,
    source: &Path,
    manifest: &Manifest,
    table: &Table,
    table_source: &str,
    size: u32,
) -> Rendered {
    let skeleton = grow(manifest, table);
    let (canvas, view) = draw::draw(&skeleton, size);
    let png = png::encode(
        canvas.width(),
        canvas.height(),
        &draw::palette(),
        &canvas.into_indices(),
    );

    let (min_x, min_y, max_x, max_y) = view.extent;
    let digest = skeleton_digest(&skeleton);
    let report = Report {
        paths: manifest.paths().len(),
        nodes: skeleton.nodes().len(),
        segments: skeleton.segments().len(),
        aggregates: skeleton.aggregate_count(),
        depth: depth_of(&skeleton),
        digest,
    };

    let sidecar = format!(
        "target      {name}\nsource      {}\ntable       {table_source}\ncanvas      {size}x{size}\n\
         extent      x {:.3} .. {:.3}   y {:.3} .. {:.3}\n\
         paths       {}\nnodes       {}\nsegments    {}\naggregates  {}\ndepth       {}\n\
         skeleton    {digest}\n",
        source.display(),
        min_x.to_f64(),
        max_x.to_f64(),
        min_y.to_f64(),
        max_y.to_f64(),
        report.paths,
        report.nodes,
        report.segments,
        report.aggregates,
        report.depth,
    );

    Rendered {
        png,
        report,
        sidecar,
    }
}

/// A hash of the skeleton's geometry and roles.
///
/// Not the PNG's hash, deliberately. The PNG folds the skeleton through a fit that depends on
/// the canvas size, so two runs at different `--size` values would disagree about a tree that
/// is identical. This is the thing `AC-DET-1` is actually about.
pub(crate) fn skeleton_digest(skeleton: &Skeleton) -> Digest {
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
pub(crate) fn depth_of(skeleton: &Skeleton) -> u16 {
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
pub(crate) fn short_digest(digest: Digest) -> String {
    digest.to_string()[..16].to_owned()
}
