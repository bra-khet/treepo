//! `F-EXT-1` — the structural and size primitives, from one walk.
//!
//! # The walk reads the HEAD tree, not the working directory
//!
//! `F-EXT-7` fixes the lens at `local_committed`: structure comes from HEAD, and the working
//! tree appears only as a Thrive overlay (`F-THR-4`). Walking git's tree objects rather than
//! the filesystem buys three things beyond following the spec:
//!
//! * **`.gitignore` is honoured for free, and correctly.** A tree holds tracked files and
//!   nothing else. See [`filter`](crate::filter) for why pattern-matching would be worse.
//! * **Nothing in the working directory is touched.** Not a `stat`, not a `read_dir`. `N1`
//!   and `AC-MAN-2` become properties of the code path rather than promises about it.
//! * **Sizes are free.** `find_header` reads an object's size from the pack index without
//!   decompressing the blob, so measuring a 50 MB asset costs the same as a 50-byte one.
//!
//! The filesystem fallback exists for the one case with no tree to read: a directory with no
//! `.git` (PRD §6, `AC-ASSOC-3`). It produces the same records by different means.
//!
//! # `AC-DET-3` and deep nesting
//!
//! Both walks sort explicitly, and both are iterative. The sort is not redundant even on the
//! tree path — git orders tree entries by name with directories compared as if they ended in
//! `/`, so `src` and `src.rs` come back in an order that is git's convention rather than
//! [`RepoPath`]'s. Inheriting an ordering treepo did not choose is exactly the dependency
//! `AC-DET-3` exists to remove.
//!
//! Iterative rather than recursive because PRD §6 requires deep nesting produce "no stack
//! overflow", and a repository can nest as deeply as it likes.
//!
//! # What this walk does not fill in
//!
//! [`SizePrimitives::lines`], `language_bytes`, and `category_bytes` stay empty, and
//! [`BalanceScore::kind`] stays `None`. All four need content classification (`F-EXT-4`),
//! which needs to read blobs; that lands with `lang.rs`. They are left unmeasured rather
//! than defaulted so nothing downstream mistakes "not looked at" for "zero".

use crate::discover::{Target, entry_name};
use crate::filter::FilterSet;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use treepo_det::Fx;
use treepo_model::manifest::{NodeKind, PathRecord};
use treepo_model::path::{PathError, RepoPath};
use treepo_model::primitives::structural::{BalanceScore, BranchingHistogram, DepthProfile};

/// How the structure was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructureSource {
    /// From the HEAD tree — the `F-EXT-7` lens.
    HeadTree,
    /// From the filesystem, because there was no repository (PRD §6).
    Filesystem,
}

/// Knobs the walk reads. Defaults are the shipped ones.
#[derive(Debug, Clone, Copy)]
pub struct WalkOptions {
    /// Bytes above which a file counts as a large-file outlier (`P7`, PRD §6).
    ///
    /// Belongs in a parameter file eventually, alongside the other thresholds `F-SKEL-5`
    /// makes editable; a field here keeps it testable until there is a file to put it in.
    pub large_file_bytes: u64,
}

impl Default for WalkOptions {
    fn default() -> Self {
        Self {
            // Large enough that ordinary source, images, and lockfiles are not outliers;
            // small enough to catch the vendored binary that would otherwise consume its
            // parent's whole budget.
            large_file_bytes: 4 * 1024 * 1024,
        }
    }
}

/// What one walk produced.
#[derive(Debug)]
pub struct Structure {
    /// Every surviving path, sorted, with structural and size primitives filled in.
    ///
    /// Always contains the repository root, even for an empty repository — PRD §6 requires
    /// a seed and root cluster rather than nothing (`AC-SKEL-2`).
    pub records: Vec<PathRecord>,
    /// Where the structure came from.
    pub source: StructureSource,
    /// How many paths the filter removed (`F-EXT-8`).
    pub excluded: usize,
    /// Directories skipped because they could not be read.
    ///
    /// Only reachable on the filesystem path. A permission-denied subdirectory loses that
    /// subtree rather than the whole walk — `F-ASSOC-7` treats restricted permissions as an
    /// ordinary path, and failing the extraction would make one unreadable folder fatal.
    pub unreadable: usize,
}

/// Why a walk could not complete.
#[derive(Debug)]
pub enum WalkError {
    /// The filesystem refused.
    Io(std::io::Error),
    /// `gix` could not read an object the tree referenced.
    Object(String),
    /// A repository contained a path treepo cannot represent.
    Path {
        /// The offending name, lossily rendered.
        name: String,
        /// What was wrong with it.
        source: PathError,
    },
}

impl std::fmt::Display for WalkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(source) => write!(f, "walk failed: {source}"),
            Self::Object(message) => write!(f, "could not read a git object: {message}"),
            Self::Path { name, source } => write!(f, "unusable path {name:?}: {source}"),
        }
    }
}

impl std::error::Error for WalkError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
            Self::Path { source, .. } => Some(source),
            Self::Object(_) => None,
        }
    }
}

impl From<std::io::Error> for WalkError {
    fn from(source: std::io::Error) -> Self {
        Self::Io(source)
    }
}

/// Reads the structure of `target`, filtered, with primitives rolled up.
///
/// # Errors
///
/// [`WalkError`] if the tree or the filesystem cannot be read.
pub fn walk(
    target: &Target,
    filter: &FilterSet,
    options: WalkOptions,
) -> Result<Structure, WalkError> {
    let mut structure = match target {
        Target::Repository(repo_target) if repo_target.head.is_some() => {
            walk_head_tree(&repo_target.repo, filter)?
        }
        // A repository with no commits has no tree to read. Its working directory is not
        // the lens `F-EXT-7` chose, so the honest answer is an empty structure — PRD §6's
        // "seed and root cluster", not a filesystem tree pretending to be committed.
        Target::Repository(_) => Structure {
            records: vec![PathRecord::new(RepoPath::root(), NodeKind::Directory)],
            source: StructureSource::HeadTree,
            excluded: 0,
            unreadable: 0,
        },
        Target::PlainDirectory { root } => walk_filesystem(root, filter)?,
    };

    roll_up(&mut structure.records, options);
    Ok(structure)
}

/// Walks the HEAD tree, iteratively.
fn walk_head_tree(repo: &gix::Repository, filter: &FilterSet) -> Result<Structure, WalkError> {
    let head_tree = repo
        .head_commit()
        .map_err(|error| WalkError::Object(error.to_string()))?
        .tree()
        .map_err(|error| WalkError::Object(error.to_string()))?;

    let mut records = vec![PathRecord::new(RepoPath::root(), NodeKind::Directory)];
    let mut excluded = 0usize;
    let mut pending = vec![(RepoPath::root(), head_tree.id)];

    while let Some((prefix, tree_id)) = pending.pop() {
        let tree = repo
            .find_tree(tree_id)
            .map_err(|error| WalkError::Object(error.to_string()))?;

        // Collected owned: `EntryRef` borrows the tree's buffer, which cannot outlive the
        // recursion into a child tree.
        let mut entries = Vec::new();
        for entry in tree.iter() {
            let entry = entry.map_err(|error| WalkError::Object(error.to_string()))?;
            entries.push((
                entry_name(entry.filename()),
                entry.mode().kind(),
                entry.oid().to_owned(),
            ));
        }
        // AC-DET-3. See the module header for why git's own order is not enough.
        entries.sort_by(|a, b| a.0.cmp(&b.0));

        for (name, kind, oid) in entries {
            let path = prefix.join(&name).map_err(|source| WalkError::Path {
                name: String::from_utf8_lossy(&name).into_owned(),
                source,
            })?;
            if !filter.allows(&path) {
                excluded += 1;
                continue;
            }

            let node = match kind {
                gix::objs::tree::EntryKind::Tree => NodeKind::Directory,
                gix::objs::tree::EntryKind::Blob | gix::objs::tree::EntryKind::BlobExecutable => {
                    NodeKind::File
                }
                gix::objs::tree::EntryKind::Link => NodeKind::Symlink,
                // PRD §6: a submodule is a sealed object, never recursed into in v1.
                gix::objs::tree::EntryKind::Commit => NodeKind::Submodule,
            };

            let mut record = PathRecord::new(path.clone(), node);
            if node == NodeKind::File {
                record.size.bytes = repo
                    .find_header(oid)
                    .map_err(|error| WalkError::Object(error.to_string()))?
                    .size();
            }
            records.push(record);

            if node == NodeKind::Directory {
                pending.push((path, oid));
            }
        }
    }

    Ok(Structure {
        records,
        source: StructureSource::HeadTree,
        excluded,
        unreadable: 0,
    })
}

/// Walks the filesystem, for a directory with no repository (PRD §6, `AC-ASSOC-3`).
fn walk_filesystem(root: &Path, filter: &FilterSet) -> Result<Structure, WalkError> {
    let mut records = vec![PathRecord::new(RepoPath::root(), NodeKind::Directory)];
    let mut excluded = 0usize;
    let mut unreadable = 0usize;
    let mut pending = vec![(RepoPath::root(), root.to_path_buf())];

    while let Some((prefix, directory)) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            unreadable += 1;
            continue;
        };

        let mut found = Vec::new();
        for entry in entries {
            let entry = entry?;
            // `symlink_metadata`, so a symlink is recorded rather than followed. PRD §6:
            // "Not followed. Cycles impossible by construction."
            let metadata = entry.path().symlink_metadata()?;
            let node = if metadata.is_symlink() {
                NodeKind::Symlink
            } else if metadata.is_dir() {
                NodeKind::Directory
            } else {
                NodeKind::File
            };
            found.push((file_name_bytes(&entry.file_name()), node, metadata.len()));
        }
        // AC-DET-3: `read_dir` order is a filesystem property, so it is replaced here.
        found.sort_by(|a, b| a.0.cmp(&b.0));

        for (name, node, len) in found {
            let path = prefix.join(&name).map_err(|source| WalkError::Path {
                name: String::from_utf8_lossy(&name).into_owned(),
                source,
            })?;
            if !filter.allows(&path) {
                excluded += 1;
                continue;
            }

            let mut record = PathRecord::new(path.clone(), node);
            if node == NodeKind::File {
                record.size.bytes = len;
            }
            records.push(record);

            if node == NodeKind::Directory {
                pending.push((path, push_name(&directory, &name)));
            }
        }
    }

    Ok(Structure {
        records,
        source: StructureSource::Filesystem,
        excluded,
        unreadable,
    })
}

/// A directory entry's name as repository path bytes.
///
/// On Unix this is exact. On Windows a filename is UTF-16 and unpaired surrogates cannot be
/// rendered as UTF-8, so the conversion is lossy — the only place in extraction where a name
/// can lose information, and it is unreachable on the HEAD-tree path that real repositories
/// take.
fn file_name_bytes(name: &std::ffi::OsStr) -> Vec<u8> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt as _;
        name.as_bytes().to_vec()
    }
    #[cfg(not(unix))]
    {
        name.to_string_lossy().into_owned().into_bytes()
    }
}

/// Appends a name, taken back from bytes, to a filesystem path.
///
/// The inverse of [`file_name_bytes`], and lossy on Windows for the same reason.
fn push_name(directory: &Path, name: &[u8]) -> PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt as _;
        directory.join(std::ffi::OsStr::from_bytes(name))
    }
    #[cfg(not(unix))]
    {
        directory.join(String::from_utf8_lossy(name).as_ref())
    }
}

/// Fills in every primitive that depends on a path's descendants.
///
/// Records are sorted first, which puts every parent before its children — a path is a
/// strict byte prefix of its descendants, so it always sorts earlier. Walking the sorted
/// list backwards therefore visits every child before its parent, which is exactly the order
/// an aggregation needs and costs one sort rather than a tree of pointers.
fn roll_up(records: &mut [PathRecord], options: WalkOptions) {
    records.sort_by(|a, b| a.path.cmp(&b.path));
    let count = records.len();

    let mut children: Vec<Vec<usize>> = vec![Vec::new(); count];
    let mut parent_of: Vec<Option<usize>> = vec![None; count];
    for index in 0..count {
        let Some(parent) = records[index].path.parent() else {
            continue;
        };
        if let Ok(parent_index) = records.binary_search_by(|record| record.path.cmp(&parent)) {
            children[parent_index].push(index);
            parent_of[index] = Some(parent_index);
        }
    }

    // File sizes beneath each path, sorted, for the percentile distribution. A child's
    // vector moves into its parent, so peak memory is one copy of the whole repository's
    // file sizes rather than one per level.
    let mut subtree_sizes: Vec<Vec<u64>> = vec![Vec::new(); count];

    for index in (0..count).rev() {
        if records[index].kind != NodeKind::Directory {
            let bytes = records[index].size.bytes;
            subtree_sizes[index] = vec![bytes];
            let structural = &mut records[index].structural;
            structural.depth_profile = DepthProfile::leaf();
            structural.balance = BalanceScore::EVEN;
            // A leaf is a node with zero children, and the histogram is a distribution over
            // *all* nodes. Omitting leaves would leave `leaf_ratio` reading zero on a tree
            // that is nothing but leaves — the bushy/spindly distinction the histogram
            // exists to preserve runs precisely through this bucket.
            let mut branching = BranchingHistogram::default();
            branching.observe(0);
            structural.branching = branching;
            continue;
        }

        let child_indices = std::mem::take(&mut children[index]);
        let mut bytes = 0u64;
        let mut descendant_files = 0u32;
        let mut descendant_dirs = 0u32;
        let mut max_depth = 0u16;
        let mut large_files = 0u32;
        let mut branching = BranchingHistogram::default();
        let mut profile = DepthProfile::default();
        let mut sizes = Vec::new();
        let mut child_bytes = Vec::with_capacity(child_indices.len());
        let mut child_depths = Vec::with_capacity(child_indices.len());

        for &child in &child_indices {
            let child_record = &records[child];
            bytes = bytes.saturating_add(child_record.size.bytes);
            child_bytes.push(child_record.size.bytes);
            child_depths.push(u64::from(child_record.structural.max_subtree_depth) + 1);
            max_depth = max_depth.max(child_record.structural.max_subtree_depth + 1);

            if child_record.kind == NodeKind::Directory {
                descendant_dirs = descendant_dirs.saturating_add(1);
                descendant_dirs =
                    descendant_dirs.saturating_add(child_record.structural.descendant_dir_count);
                descendant_files =
                    descendant_files.saturating_add(child_record.structural.descendant_file_count);
                large_files = large_files.saturating_add(child_record.size.large_file_count);
            } else {
                descendant_files = descendant_files.saturating_add(1);
                if child_record.size.bytes >= options.large_file_bytes {
                    large_files = large_files.saturating_add(1);
                }
            }

            branching.merge(&child_record.structural.branching);
            profile.absorb_child(&child_record.structural.depth_profile);
            sizes.append(&mut subtree_sizes[child]);
        }

        let child_count = u32::try_from(child_indices.len()).unwrap_or(u32::MAX);
        branching.observe(child_count);
        // u64 values that compare equal are indistinguishable, so the unstable sort's
        // unspecified handling of ties cannot be observed here.
        sizes.sort_unstable();

        let structural = &mut records[index].structural;
        structural.child_count = child_count;
        structural.descendant_file_count = descendant_files;
        structural.descendant_dir_count = descendant_dirs;
        structural.max_subtree_depth = max_depth;
        structural.branching = branching;
        structural.depth_profile = profile;
        structural.balance = BalanceScore {
            size: evenness(&child_bytes),
            depth: evenness(&child_depths),
            // Needs content categories (`F-EXT-4`), which land with `lang.rs`.
            kind: None,
        };
        structural.hierarchy_skew = hierarchy_skew(descendant_files + descendant_dirs, max_depth);

        let size = &mut records[index].size;
        size.bytes = bytes;
        size.large_file_count = large_files;
        size.distribution = distribution(&sizes);
        subtree_sizes[index] = sizes;

        children[index] = child_indices;
    }

    // Relative size needs the parent's total, so it waits until every total is known.
    for index in 0..count {
        let parent_bytes = parent_of[index].map(|parent| records[parent].size.bytes);
        records[index].size.relative_bytes = match parent_bytes {
            Some(0) | None => Fx::ONE,
            Some(total) => Fx::from_ratio(
                i64::try_from(records[index].size.bytes).unwrap_or(i64::MAX),
                i64::try_from(total).unwrap_or(i64::MAX),
            ),
        };
    }
}

/// How evenly a total is spread across parts, in `0..=1`.
///
/// Normalized total-variation distance from a uniform split: 1 when every part is equal, 0
/// when one part holds everything. Chosen over a variance because it is bounded, integer-
/// computable, and says something a reader can check by eye — "one child holds most of this
/// directory" is exactly the question `P7`'s clamp asks.
///
/// A single part is perfectly even by definition, as is a total of zero.
fn evenness(parts: &[u64]) -> Fx {
    let count = parts.len() as u128;
    if count <= 1 {
        return Fx::ONE;
    }
    let total: u128 = parts.iter().map(|&part| u128::from(part)).sum();
    if total == 0 {
        return Fx::ONE;
    }
    // Σ|part·n − total|, which is Σ|pᵢ − 1/n| scaled by n·total.
    let deviation: u128 = parts
        .iter()
        .map(|&part| (u128::from(part) * count).abs_diff(total))
        .sum();
    // All mass in one part gives 2·(n−1)·total.
    let maximum = 2 * (count - 1) * total;
    let ppm = (deviation.min(maximum) * 1_000_000 / maximum) as i64;
    Fx::ONE.sub(Fx::from_ratio(ppm, 1_000_000))
}

/// `hierarchy_skew` — deep chains against wide fans, in `-1..=1`.
///
/// A subtree of `n` nodes is a pure chain when its depth is `n` and a flat fan when its depth
/// is 1. Mapping that span onto `+1` for the fan and `−1` for the chain keeps the two failure
/// modes at opposite ends, which is what `F-SKEL-1` needs to give them different silhouettes.
fn hierarchy_skew(descendants: u32, max_depth: u16) -> Fx {
    if descendants <= 1 {
        return Fx::ZERO;
    }
    let span = u64::from(descendants) - 1;
    let depth = u64::from(max_depth).saturating_sub(1).min(span);
    // normalized ∈ 0..=1, where 1 is a pure chain.
    let normalized = Fx::from_ratio(depth as i64, span as i64);
    Fx::ONE.sub(normalized.mul(Fx::from_int(2)))
}

/// Percentiles over a sorted list of file sizes.
fn distribution(sorted: &[u64]) -> treepo_model::primitives::size::SizeDistribution {
    use treepo_model::primitives::size::SizeDistribution;
    if sorted.is_empty() {
        return SizeDistribution::default();
    }
    let total: u128 = sorted.iter().map(|&size| u128::from(size)).sum();
    let last = sorted.len() - 1;
    SizeDistribution {
        min: sorted[0],
        // Lower median on an even count, so the value is always one that really occurs.
        median: sorted[last / 2],
        p90: sorted[(sorted.len() * 9 / 10).min(last)],
        max: sorted[last],
        mean: (total / sorted.len() as u128) as u64,
    }
}

/// Groups records by path for tests and callers that want a map rather than a slice.
#[must_use]
pub fn by_path(records: &[PathRecord]) -> BTreeMap<&RepoPath, &PathRecord> {
    records
        .iter()
        .map(|record| (&record.path, record))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use treepo_model::primitives::size::SizePrimitives;

    fn path(s: &str) -> RepoPath {
        RepoPath::new(s.as_bytes()).expect("valid test path")
    }

    fn file(s: &str, bytes: u64) -> PathRecord {
        let mut record = PathRecord::new(path(s), NodeKind::File);
        record.size = SizePrimitives {
            bytes,
            ..SizePrimitives::default()
        };
        record
    }

    fn directory(s: &str) -> PathRecord {
        PathRecord::new(path(s), NodeKind::Directory)
    }

    fn rolled(mut records: Vec<PathRecord>) -> BTreeMap<RepoPath, PathRecord> {
        roll_up(&mut records, WalkOptions::default());
        records
            .into_iter()
            .map(|record| (record.path.clone(), record))
            .collect()
    }

    #[test]
    fn sizes_and_counts_roll_up_the_tree() {
        let tree = rolled(vec![
            directory(""),
            directory("src"),
            file("src/main.rs", 300),
            file("src/lib.rs", 700),
            directory("src/util"),
            file("src/util/mod.rs", 1000),
            file("README.md", 2000),
        ]);

        let root = &tree[&RepoPath::root()];
        assert_eq!(root.size.bytes, 4000);
        assert_eq!(root.structural.child_count, 2); // src, README.md
        assert_eq!(root.structural.descendant_file_count, 4);
        assert_eq!(root.structural.descendant_dir_count, 2);
        assert_eq!(root.structural.max_subtree_depth, 3);

        let src = &tree[&path("src")];
        assert_eq!(src.size.bytes, 2000);
        assert_eq!(src.structural.child_count, 3);
        assert_eq!(src.structural.descendant_file_count, 3);
        assert_eq!(src.structural.max_subtree_depth, 2);
    }

    #[test]
    fn relative_bytes_are_a_share_of_the_parent() {
        let tree = rolled(vec![directory(""), file("a.txt", 250), file("b.txt", 750)]);
        assert_eq!(
            tree[&path("a.txt")].size.relative_bytes,
            Fx::from_ratio(1, 4)
        );
        assert_eq!(
            tree[&path("b.txt")].size.relative_bytes,
            Fx::from_ratio(3, 4)
        );
        // The root is the whole thing.
        assert_eq!(tree[&RepoPath::root()].size.relative_bytes, Fx::ONE);
    }

    /// An empty repository still yields a root (`AC-SKEL-2`).
    #[test]
    fn an_empty_tree_rolls_up_without_dividing_by_zero() {
        let tree = rolled(vec![directory("")]);
        let root = &tree[&RepoPath::root()];
        assert_eq!(root.size.bytes, 0);
        assert_eq!(root.structural.child_count, 0);
        assert_eq!(root.size.relative_bytes, Fx::ONE);
        assert_eq!(root.size.distribution.median, 0);
    }

    #[test]
    fn evenness_ranks_a_balanced_split_above_a_lopsided_one() {
        assert_eq!(evenness(&[]), Fx::ONE);
        assert_eq!(evenness(&[42]), Fx::ONE);
        assert_eq!(evenness(&[50, 50]), Fx::ONE);
        assert_eq!(evenness(&[25, 25, 25, 25]), Fx::ONE);
        // All mass in one child is the floor.
        assert_eq!(evenness(&[100, 0]), Fx::ZERO);
        assert_eq!(evenness(&[100, 0, 0, 0]), Fx::ZERO);
        // And in between, in between.
        let middling = evenness(&[70, 30]);
        assert!(middling > Fx::ZERO && middling < Fx::ONE);
        assert!(evenness(&[90, 10]) < middling);
    }

    /// The two shapes the skew exists to tell apart.
    #[test]
    fn hierarchy_skew_separates_chains_from_fans() {
        // A flat directory of four files: depth 1, four descendants.
        let fan = rolled(vec![
            directory(""),
            file("a", 1),
            file("b", 1),
            file("c", 1),
            file("d", 1),
        ]);
        assert_eq!(fan[&RepoPath::root()].structural.hierarchy_skew, Fx::ONE);

        // A corridor: root/a/b/c/d, one child each.
        let chain = rolled(vec![
            directory(""),
            directory("a"),
            directory("a/b"),
            directory("a/b/c"),
            file("a/b/c/d", 1),
        ]);
        assert_eq!(
            chain[&RepoPath::root()].structural.hierarchy_skew,
            Fx::NEG_ONE
        );
    }

    #[test]
    fn the_branching_histogram_describes_the_whole_subtree() {
        let tree = rolled(vec![
            directory(""),
            directory("wide"),
            file("wide/a", 1),
            file("wide/b", 1),
            file("wide/c", 1),
            directory("narrow"),
            file("narrow/only", 1),
        ]);
        let buckets = tree[&RepoPath::root()].structural.branching.buckets();
        // Four leaves, one node with one child, one with two, one with three.
        assert_eq!(buckets[0], 4);
        assert_eq!(buckets[1], 1);
        assert_eq!(buckets[2], 1);
        assert_eq!(buckets[3], 1);
    }

    #[test]
    fn the_size_distribution_uses_values_that_really_occur() {
        let tree = rolled(vec![
            directory(""),
            file("a", 10),
            file("b", 20),
            file("c", 30),
            file("d", 40),
        ]);
        let distribution = tree[&RepoPath::root()].size.distribution;
        assert_eq!(distribution.min, 10);
        assert_eq!(distribution.max, 40);
        assert_eq!(distribution.median, 20);
        assert_eq!(distribution.mean, 25);
    }

    /// PRD §6, "One enormous file": the outlier is counted so `P7` can clamp it.
    #[test]
    fn large_files_are_counted_against_the_threshold() {
        let mut records = vec![
            directory(""),
            directory("assets"),
            file("assets/texture.png", 60 * 1024 * 1024),
            file("assets/icon.png", 900),
            file("src.rs", 100),
        ];
        roll_up(&mut records, WalkOptions::default());
        let tree: BTreeMap<_, _> = records
            .into_iter()
            .map(|record| (record.path.clone(), record))
            .collect();
        assert_eq!(tree[&path("assets")].size.large_file_count, 1);
        assert_eq!(tree[&RepoPath::root()].size.large_file_count, 1);
        // And the distribution shows the spread rather than only the total.
        let distribution = tree[&path("assets")].size.distribution;
        assert_eq!(distribution.min, 900);
        assert_eq!(distribution.max, 60 * 1024 * 1024);
    }

    /// A threshold that is a knob, not a constant.
    #[test]
    fn the_large_file_threshold_is_configurable() {
        let mut records = vec![directory(""), file("a.bin", 5000)];
        roll_up(
            &mut records,
            WalkOptions {
                large_file_bytes: 1000,
            },
        );
        assert_eq!(records[0].size.large_file_count, 1);
    }

    /// PRD §6: submodules and symlinks are recorded, never descended into.
    #[test]
    fn sealed_nodes_are_leaves() {
        let tree = rolled(vec![
            directory(""),
            PathRecord::new(path("extern/lib"), NodeKind::Submodule),
            directory("extern"),
            PathRecord::new(path("link"), NodeKind::Symlink),
        ]);
        assert!(tree[&path("extern/lib")].structural.is_leaf());
        assert!(tree[&path("link")].structural.is_leaf());
        assert_eq!(tree[&RepoPath::root()].structural.descendant_file_count, 2);
    }

    #[test]
    fn rolling_up_does_not_depend_on_input_order() {
        let forwards = rolled(vec![
            directory(""),
            directory("src"),
            file("src/a", 1),
            file("src/b", 2),
        ]);
        let backwards = rolled(vec![
            file("src/b", 2),
            file("src/a", 1),
            directory("src"),
            directory(""),
        ]);
        assert_eq!(forwards, backwards);
    }
}
