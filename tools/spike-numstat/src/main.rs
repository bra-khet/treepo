//! RISK-A spike — can `gix` do `F-EXT-2`'s job inside the budget?
//!
//! The architecture's risk register:
//!
//! > **RISK-A — `gix` maturity for the `F-EXT-2` single-pass traversal.** `gix` has no
//! > direct `--numstat` equivalent; per-file line counts require assembling blob diffs over
//! > the commit graph. This is the performance linchpin for RISK-1's mitigation.
//!
//! `RISK-1` says `git blame` is unaffordable, and the whole extraction design rests on
//! deriving ownership and churn from one `O(history)` pass instead. If that pass cannot be
//! built on `gix` at a workable speed, the T2 and T3 budgets in PRD §7 are unreachable and
//! the decision to use `gix` at all (architecture D3) has to be reopened — which costs `R1`
//! (a consumer machine has no `git` binary) and widens `N1` (subprocess `git` honours
//! repository config that can execute programs).
//!
//! So this measures one thing: **wall-clock time to walk a real commit graph and emit
//! per-file added/deleted line counts**, with a correctness check against `git log
//! --numstat` itself.
//!
//! It is deliberately not the extraction layer. It collects only what is needed to prove
//! the traversal is viable and to show the shape of the cost.
//!
//! ```text
//! cargo run --release -p spike-numstat -- <repo> [--threads N] [--limit N] [--dump f]
//! ```
//!
//! # Why the work is split in two phases
//!
//! Measuring `--no-counts` showed the commit-graph walk costs 0.2s of a 39s run. Effectively
//! all of the cost is per-blob diffing, and every commit's diff is independent of every
//! other's — so phase 1 collects the commit list serially and phase 2 diffs them across
//! threads.
//!
//! That split is not just a benchmark trick: summing integer line counts is associative, so
//! the merged result does not depend on which thread finished first. Parallelism here costs
//! nothing under `N3`, which is the only reason it is available as a mitigation at all.

use std::collections::BTreeMap;
use std::path::PathBuf;

// Measurement code needs a clock. The N3 ban on `Instant` protects the *generative*
// pipeline from wall-clock input; a benchmark that cannot read a clock cannot benchmark.
// This crate is deleted once the spike is recorded, and never feeds a generated value.
#[allow(clippy::disallowed_types, clippy::disallowed_methods)]
mod clock {
    pub(crate) struct Timer(std::time::Instant);

    impl Timer {
        pub(crate) fn start() -> Self {
            Self(std::time::Instant::now())
        }
        pub(crate) fn seconds(&self) -> f64 {
            self.0.elapsed().as_secs_f64()
        }
    }
}

use clock::Timer;

/// What one worker accumulates; merged by summing, which is order-independent.
#[derive(Default)]
struct Totals {
    changes: usize,
    binary_changes: usize,
    insertions: usize,
    deletions: usize,
    per_path: BTreeMap<String, (usize, usize, usize)>,
    rows: Vec<(String, usize, usize)>,
}

impl Totals {
    fn merge(&mut self, other: Totals) {
        self.changes += other.changes;
        self.binary_changes += other.binary_changes;
        self.insertions += other.insertions;
        self.deletions += other.deletions;
        for (path, (touches, added, removed)) in other.per_path {
            let entry = self.per_path.entry(path).or_default();
            entry.0 += touches;
            entry.1 += added;
            entry.2 += removed;
        }
        self.rows.extend(other.rows);
    }
}

/// A unit of diff work: a commit and the parent to diff it against.
struct Job {
    commit: gix::ObjectId,
    parent: Option<gix::ObjectId>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut repo_path: Option<PathBuf> = None;
    let mut limit = usize::MAX;
    let mut dump: Option<PathBuf> = None;
    let mut counts = true;
    let mut threads = 1usize;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--limit" => limit = args.next().ok_or("--limit needs a value")?.parse()?,
            "--dump" => dump = Some(args.next().ok_or("--dump needs a value")?.into()),
            "--threads" => threads = args.next().ok_or("--threads needs a value")?.parse()?,
            // Isolates traversal cost from blob-diff cost.
            "--no-counts" => counts = false,
            other => repo_path = Some(other.into()),
        }
    }
    let repo_path = repo_path.ok_or("usage: spike-numstat <repo> [--threads N] [--limit N]")?;
    let threads = threads.max(1);

    let repo = gix::open(&repo_path)?;

    // ---- phase 1: walk the commit graph -------------------------------------------
    let walk = Timer::start();
    let mut jobs: Vec<Job> = Vec::new();
    let mut authors: BTreeMap<String, usize> = BTreeMap::new();
    let mut commits = 0usize;
    let mut merges = 0usize;

    for info in repo.head_id()?.ancestors().all()? {
        let info = info?;
        let commit = info.object()?;
        *authors
            .entry(commit.author()?.email.to_string().to_lowercase())
            .or_default() += 1;
        commits += 1;

        let parents: Vec<_> = commit.parent_ids().map(|id| id.detach()).collect();

        // `git log --numstat` emits no diff for a merge, and counting one would
        // double-count every line already attributed to the branch being merged. Matching
        // git's default is both correct and a large part of why this is affordable.
        if parents.len() > 1 {
            merges += 1;
        } else {
            jobs.push(Job {
                commit: info.id,
                parent: parents.first().copied(),
            });
        }

        if commits >= limit {
            break;
        }
    }
    let walk_seconds = walk.seconds();

    println!("--- traversal ---");
    println!("  commits walked     {commits}");
    println!("  merges skipped     {merges}");
    println!("  distinct authors   {}", authors.len());
    println!("  graph walk         {walk_seconds:.2}s");

    if !counts {
        return Ok(());
    }

    // ---- phase 2: diff every job --------------------------------------------------
    let diff = Timer::start();
    let shared = repo.into_sync();
    let chunk_size = jobs.len().div_ceil(threads);
    let collect_rows = dump.is_some();
    let mut totals = Totals::default();

    std::thread::scope(|scope| -> Result<(), Box<dyn std::error::Error>> {
        let mut handles = Vec::new();
        for chunk in jobs.chunks(chunk_size.max(1)) {
            let shared = &shared;
            handles.push(scope.spawn(move || process(shared, chunk, collect_rows)));
        }
        for handle in handles {
            let result = handle.join().map_err(|_| "worker panicked")?;
            totals.merge(result.map_err(|e| e.to_string())?);
        }
        Ok(())
    })?;

    let diff_seconds = diff.seconds();
    let elapsed = walk_seconds + diff_seconds;

    println!("\n--- diffs ---");
    println!("  threads            {threads}");
    println!("  file changes       {}", totals.changes);
    println!("  binary changes     {}", totals.binary_changes);
    println!("  paths touched      {}", totals.per_path.len());
    println!("  insertions         {}", totals.insertions);
    println!("  deletions          {}", totals.deletions);

    println!("\n--- cost ---");
    println!("  graph walk         {walk_seconds:.2}s");
    println!("  blob diffs         {diff_seconds:.2}s");
    println!("  total              {elapsed:.2}s");
    if commits > 0 {
        println!(
            "  per commit         {:.3}ms",
            elapsed * 1000.0 / commits as f64
        );
    }

    if let Some(path) = dump {
        let mut rows = totals.rows;
        rows.sort();
        let text: String = rows
            .iter()
            .map(|(sha, added, removed)| format!("{sha}\t{added}\t{removed}\n"))
            .collect();
        std::fs::write(&path, text)?;
        println!("\nwrote {} commit rows to {}", rows.len(), path.display());
    }

    Ok(())
}

/// Diff one chunk of commits. Each worker gets its own repository handle and its own blob
/// resource cache, so nothing is shared and nothing needs locking.
fn process(
    shared: &gix::ThreadSafeRepository,
    jobs: &[Job],
    collect_rows: bool,
) -> Result<Totals, Box<dyn std::error::Error + Send + Sync>> {
    let repo = shared.to_thread_local();
    let mut blob_cache = repo.diff_resource_cache_for_tree_diff()?;
    let empty_tree = repo.empty_tree();
    let mut totals = Totals::default();

    for (index, job) in jobs.iter().enumerate() {
        let new_tree = repo.find_commit(job.commit)?.tree()?;
        let old_tree = match job.parent {
            Some(id) => repo.find_commit(id)?.tree()?,
            None => empty_tree.clone(),
        };

        let mut commit_insertions = 0usize;
        let mut commit_deletions = 0usize;

        let mut platform = old_tree.changes()?;

        // Rename tracking is not wanted here and `Tree::changes()` picks up the
        // repository's `diff.renames`, which git defaults to on. F-EXT-2's churn is
        // per-path, and a renamed file legitimately reads as a deletion plus an addition
        // to the paths involved. `git log --numstat --no-renames` is the matching baseline.
        platform.options(|options| {
            options.track_rewrites(None);
        });

        platform.for_each_to_obtain_tree(&new_tree, |change| {
            let location = change.location().to_string();
            totals.changes += 1;

            let (added, removed) = match change.diff(&mut blob_cache) {
                Ok(mut blob) => match blob.line_counts() {
                    Ok(Some(stats)) => (stats.insertions as usize, stats.removals as usize),
                    // Binary, exactly as `numstat` reports with `-`.
                    Ok(None) => {
                        totals.binary_changes += 1;
                        (0, 0)
                    }
                    Err(_) => (0, 0),
                },
                Err(_) => (0, 0),
            };

            commit_insertions += added;
            commit_deletions += removed;

            let entry = totals.per_path.entry(location).or_default();
            entry.0 += 1;
            entry.1 += added;
            entry.2 += removed;

            Ok::<_, std::convert::Infallible>(std::ops::ControlFlow::Continue(()))
        })?;

        totals.insertions += commit_insertions;
        totals.deletions += commit_deletions;

        if collect_rows {
            totals
                .rows
                .push((job.commit.to_string(), commit_insertions, commit_deletions));
        }

        // Bound the cache, per gix's own warning that it never shrinks on its own.
        if index % 1000 == 999 {
            blob_cache.clear_resource_cache();
        }
    }

    Ok(totals)
}
