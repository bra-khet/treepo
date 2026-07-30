//! `F-EXT-2` — every temporal and ownership primitive, from one traversal.
//!
//! > Temporal and ownership primitives derived from **one pass over `git log --numstat`** —
//! > not from `git blame`. A single traversal emitting commit hash, author name, author
//! > email, timestamp, and per-file added/deleted line counts yields `first_commit_age`,
//! > `last_commit_age`, `commit_count`, `churn_rate` across all windows, `recency_heat`,
//! > `modification_burstiness`, `author_count`, `author_distribution`, `dominant_author`,
//! > `bus_factor_proxy`, and `contribution_recency_per_author` in `O(history)` rather than
//! > `O(files × history)`.
//!
//! `RISK-1` is that `git blame` is `O(files × history)` and simply unaffordable — the PRD
//! calls it "the highest-probability, highest-impact risk". This module is the mitigation,
//! and the `RISK-A` spike (`tools/spike-numstat`) measured it before it was written: 21.8 s
//! for 11,870 commits on four threads, extrapolating to ~37 s of `AC-EXT-1`'s 60 s budget
//! at T2.
//!
//! # `git blame` cannot be invoked from here, by construction
//!
//! `F-EXT-3` demotes blame to a deferred pass that runs *after* the first Grow. Rather than
//! trusting nobody calls it, `treepo-vcs` does not enable `gix`'s `blame` feature — so
//! `gix::blame` does not exist in this build and a call to it does not compile. The crate
//! root carries the `compile_fail` test that holds this, along with the control that makes
//! its failure mean something.
//!
//! # Two phases, because the cost is entirely in the second
//!
//! The spike's most useful measurement: walking the whole commit graph took **0.20 s of a
//! 38.65 s run**. Traversal was never the problem; decompressing and diffing blobs is all of
//! it. So phase one walks the graph serially — collecting commits, authors, and the
//! reference time — and phase two diffs across threads.
//!
//! That split is not only a speed trick. Every value accumulated here is a sum, a minimum, a
//! maximum, or a set union, all of which are associative and commutative, so the merged
//! result does not depend on which thread finished first. The spike confirmed byte-identical
//! line counts at 1, 2, 4, 8, and 16 threads. Parallelism costs nothing under `N3`, which is
//! the only reason it is available at all.
//!
//! # Merges are skipped
//!
//! `git log --numstat` emits no diff for a merge commit, and counting one would double-count
//! every line already attributed to the branch being merged. Matching git's default is both
//! correct and a large part of why this is affordable.

use crate::discover::Target;
use crate::filter::FilterSet;
use crate::mailmap::Identities;
use std::collections::BTreeMap;
use treepo_det::{Fx, OrderedMap};
use treepo_model::identity::AuthorKey;
use treepo_model::manifest::{AuthorEntry, AuthorTable, NodeKind, PathRecord};
use treepo_model::path::RepoPath;
use treepo_model::primitives::ownership::OwnershipPrimitives;
use treepo_model::primitives::temporal::{ChurnWindows, TemporalPrimitives};

/// Seconds in a day.
const DAY: i64 = 86_400;

/// Knobs the history pass reads.
#[derive(Debug, Clone, Copy)]
pub struct HistoryOptions {
    /// How many threads diff commits. Clamped to at least one.
    ///
    /// The spike's scaling curve: 1.77× at four threads, 2.03× at eight, and a *regression*
    /// at sixteen — a shared bottleneck, most likely the object database. Four is the
    /// minimum-spec core count and close to where the curve flattens.
    pub threads: usize,
    /// Half-life for `recency_heat`, in days.
    ///
    /// Ninety days: long enough that a quarter of quiet does not read as abandonment, short
    /// enough that last week's work is visibly hotter than last spring's.
    pub recency_half_life_days: i64,
}

impl Default for HistoryOptions {
    fn default() -> Self {
        Self {
            threads: 4,
            recency_half_life_days: 90,
        }
    }
}

/// One path's history.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PathHistory {
    /// When and how often (`F-EXT-2`).
    pub temporal: TemporalPrimitives,
    /// By whom (`F-EXT-2`, `N4`).
    pub ownership: OwnershipPrimitives,
}

/// Everything one history traversal learned.
#[derive(Debug, Default)]
pub struct History {
    /// The newest commit timestamp in the repository, as absolute epoch seconds.
    ///
    /// The anchor every age is measured against. See
    /// [`Manifest::reference_time`](treepo_model::Manifest::reference_time) for why this
    /// belongs to the repository rather than to the clock.
    pub reference_time: i64,
    /// Commits walked.
    pub commit_count: u32,
    /// Merge commits skipped.
    pub merge_count: u32,
    /// Every contributor, keyed by hash (`N4`).
    pub authors: AuthorTable,
    /// Per-path history, including directories and paths deleted before HEAD.
    pub paths: BTreeMap<RepoPath, PathHistory>,
}

/// Why a history pass could not complete.
#[derive(Debug)]
pub enum HistoryError {
    /// `gix` could not read an object or walk the graph.
    Object(String),
    /// A worker thread panicked.
    WorkerPanic,
}

impl std::fmt::Display for HistoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Object(message) => write!(f, "history traversal failed: {message}"),
            Self::WorkerPanic => f.write_str("a history worker thread panicked"),
        }
    }
}

impl std::error::Error for HistoryError {}

/// A unit of diff work: a commit, its parent, when, and by whom.
#[derive(Debug, Clone)]
struct Job {
    commit: gix::ObjectId,
    parent: Option<gix::ObjectId>,
    time: i64,
    author: AuthorKey,
}

/// What one path accumulates while the pass runs.
///
/// Everything derived — churn windows, recency heat, burstiness — comes out of `events` at
/// the end, so the pass itself stores one `(when, how many lines)` pair per commit that
/// touched the path and nothing else. At T2 that is a few hundred thousand pairs.
#[derive(Debug, Clone, Default)]
struct Accumulator {
    /// `(commit time, lines added + removed)`, one entry per commit touching this path.
    events: Vec<(i64, u64)>,
    author_lines: BTreeMap<AuthorKey, u64>,
    author_recency: BTreeMap<AuthorKey, i64>,
}

impl Accumulator {
    /// Merges another worker's accumulator. Every operation is associative — `N3`.
    fn merge(&mut self, other: Self) {
        self.events.extend(other.events);
        for (author, lines) in other.author_lines {
            *self.author_lines.entry(author).or_default() += lines;
        }
        for (author, time) in other.author_recency {
            let entry = self.author_recency.entry(author).or_insert(i64::MIN);
            *entry = (*entry).max(time);
        }
    }
}

/// Walks the repository's history once and derives every temporal and ownership primitive.
///
/// # Errors
///
/// [`HistoryError`] if the commit graph or an object cannot be read.
pub fn log_pass(
    target: &Target,
    filter: &FilterSet,
    options: HistoryOptions,
) -> Result<History, HistoryError> {
    let Target::Repository(repo_target) = target else {
        // No repository, no history. PRD §6: an ordinary path, with a notice.
        return Ok(History::default());
    };
    if repo_target.head.is_none() {
        return Ok(History::default());
    }
    let repo = &repo_target.repo;
    let identities = Identities::load(repo);

    // Commits at a shallow clone's boundary record a parent whose object was never fetched.
    // Git grafts those to parentless and shows the boundary commit as introducing
    // everything; matching that keeps a shallow clone an ordinary path (PRD §6) instead of
    // an extraction that dies on a missing object. Resolved once here so `diff_chunk` stays
    // strict — a missing object anywhere else is real corruption and should still be loud.
    let boundary: std::collections::BTreeSet<gix::ObjectId> = repo
        .shallow_commits()
        .ok()
        .flatten()
        .map(|commits| commits.iter().copied().collect())
        .unwrap_or_default();

    // ---- phase one: the graph -------------------------------------------------------
    let mut jobs: Vec<Job> = Vec::new();
    let mut authors: BTreeMap<AuthorKey, AuthorEntry> = BTreeMap::new();
    let mut reference_time = i64::MIN;
    let mut merge_count = 0u32;
    let mut commit_count = 0u32;

    let head = repo
        .head_id()
        .map_err(|error| HistoryError::Object(error.to_string()))?;
    let walk = head
        .ancestors()
        .all()
        .map_err(|error| HistoryError::Object(error.to_string()))?;

    for info in walk {
        let info = info.map_err(|error| HistoryError::Object(error.to_string()))?;
        let commit = info
            .object()
            .map_err(|error| HistoryError::Object(error.to_string()))?;
        let signature = commit
            .author()
            .map_err(|error| HistoryError::Object(error.to_string()))?;
        let author = identities.key(signature);
        let time = signature
            .time()
            .map(|time| time.seconds)
            .unwrap_or_default();

        reference_time = reference_time.max(time);
        commit_count = commit_count.saturating_add(1);

        // CHANGED: the pass no longer counts commits per contributor (schema 2).
        // WHY: `N4`. The per-author total was read by nothing and was the widest route to a
        // leaderboard in the manifest — see `treepo_model::AuthorEntry`. The repo-wide
        // `commit_count` accumulated above is a different number and stays; so does
        // `TemporalPrimitives::commit_count`, which is per path and is what `F-EXT-2` names.
        let entry = authors.entry(author).or_insert(AuthorEntry {
            recency: i64::MIN,
            is_self: false,
        });
        entry.recency = entry.recency.max(time);

        let parents: Vec<_> = commit.parent_ids().map(|id| id.detach()).collect();
        if parents.len() > 1 {
            merge_count = merge_count.saturating_add(1);
        } else {
            let parent = if boundary.contains(&info.id) {
                None
            } else {
                parents.first().copied()
            };
            jobs.push(Job {
                commit: info.id,
                parent,
                time,
                author,
            });
        }
    }

    if commit_count == 0 {
        return Ok(History::default());
    }

    // ---- phase two: the diffs -------------------------------------------------------
    let threads = options.threads.max(1);
    let shared = repo.clone().into_sync();
    let chunk_size = jobs.len().div_ceil(threads).max(1);
    let mut merged: BTreeMap<RepoPath, Accumulator> = BTreeMap::new();

    std::thread::scope(|scope| -> Result<(), HistoryError> {
        let mut handles = Vec::new();
        for chunk in jobs.chunks(chunk_size) {
            let shared = &shared;
            handles.push(scope.spawn(move || diff_chunk(shared, chunk, filter)));
        }
        for handle in handles {
            let result = handle.join().map_err(|_| HistoryError::WorkerPanic)?;
            for (path, accumulator) in result? {
                merged.entry(path).or_default().merge(accumulator);
            }
        }
        Ok(())
    })?;

    // ---- phase three: derive ---------------------------------------------------------
    let paths = merged
        .into_iter()
        .map(|(path, accumulator)| {
            let history = finalize(accumulator, reference_time, options);
            (path, history)
        })
        .collect();

    let mut table = AuthorTable::new();
    // `F-ID-1`, resolved once here because this is where the author table is built. The
    // resolution itself lives in `self_ident` — it used to be a private helper in this
    // module, which made a named feature invisible to anyone looking for it.
    let self_key = crate::self_ident::key_for(repo, &identities);
    for (key, mut entry) in authors {
        entry.is_self = Some(key) == self_key;
        table.insert(key, entry);
    }

    Ok(History {
        reference_time,
        commit_count,
        merge_count,
        authors: table,
        paths,
    })
}

/// Diffs one chunk of commits.
///
/// Each worker gets its own repository handle and its own blob cache, so nothing is shared
/// and nothing needs locking.
fn diff_chunk(
    shared: &gix::ThreadSafeRepository,
    jobs: &[Job],
    filter: &FilterSet,
) -> Result<BTreeMap<RepoPath, Accumulator>, HistoryError> {
    let repo = shared.to_thread_local();
    let mut cache = repo
        .diff_resource_cache_for_tree_diff()
        .map_err(|error| HistoryError::Object(error.to_string()))?;
    let empty_tree = repo.empty_tree();
    let mut out: BTreeMap<RepoPath, Accumulator> = BTreeMap::new();

    for (index, job) in jobs.iter().enumerate() {
        let new_tree = tree_of(&repo, job.commit)?;
        let old_tree = match job.parent {
            Some(id) => tree_of(&repo, id)?,
            None => empty_tree.clone(),
        };

        // One commit's contribution, keyed by path so a commit counts once per path even
        // when it touches several files in the same directory.
        let mut touched: BTreeMap<RepoPath, u64> = BTreeMap::new();

        let mut platform = old_tree
            .changes()
            .map_err(|error| HistoryError::Object(error.to_string()))?;
        // `Tree::changes` picks up the repository's `diff.renames`, which git defaults to
        // on. `F-EXT-2`'s churn is per-path, and a renamed file legitimately reads as a
        // deletion plus an addition to the paths involved — `--no-renames` is the matching
        // baseline. Rename tracking also cost 2.5% in the spike for no benefit here.
        platform.options(|options| {
            options.track_rewrites(None);
        });

        platform
            .for_each_to_obtain_tree(&new_tree, |change| {
                let Ok(path) = RepoPath::new(change.location()) else {
                    // A path the model cannot represent is skipped rather than fatal: one
                    // malformed historical entry should not cost the whole extraction.
                    return Ok::<_, std::convert::Infallible>(std::ops::ControlFlow::Continue(()));
                };
                if !filter.allows(&path) {
                    return Ok(std::ops::ControlFlow::Continue(()));
                }

                let lines = match change.diff(&mut cache) {
                    Ok(mut blob) => match blob.line_counts() {
                        Ok(Some(counts)) => {
                            u64::from(counts.insertions) + u64::from(counts.removals)
                        }
                        // Binary, exactly as `numstat` reports with `-`. Counted as a touch
                        // with no lines: the path changed, and pretending otherwise would
                        // make an asset directory look frozen.
                        Ok(None) | Err(_) => 0,
                    },
                    Err(_) => 0,
                };

                // Every ancestor is touched too, which is what gives a directory its own
                // churn without double-counting a commit that changed two of its files.
                let mut current = Some(path);
                while let Some(step) = current {
                    *touched.entry(step.clone()).or_default() += lines;
                    current = step.parent();
                }
                Ok(std::ops::ControlFlow::Continue(()))
            })
            .map_err(|error| HistoryError::Object(error.to_string()))?;

        for (path, lines) in touched {
            let accumulator = out.entry(path).or_default();
            accumulator.events.push((job.time, lines));
            *accumulator.author_lines.entry(job.author).or_default() += lines;
            let recency = accumulator
                .author_recency
                .entry(job.author)
                .or_insert(i64::MIN);
            *recency = (*recency).max(job.time);
        }

        // Bounded, per `gix`'s own warning that the cache never shrinks on its own.
        if index % 1000 == 999 {
            cache.clear_resource_cache();
        }
    }

    Ok(out)
}

/// A commit's tree, with both failure modes flattened into one error.
fn tree_of(repo: &gix::Repository, id: gix::ObjectId) -> Result<gix::Tree<'_>, HistoryError> {
    repo.find_commit(id)
        .map_err(|error| HistoryError::Object(error.to_string()))?
        .tree()
        .map_err(|error| HistoryError::Object(error.to_string()))
}

/// Turns raw events into the primitives `F-EXT-2` names.
fn finalize(accumulator: Accumulator, reference_time: i64, options: HistoryOptions) -> PathHistory {
    let Accumulator {
        mut events,
        author_lines,
        author_recency,
    } = accumulator;
    // By time, then by line count so equal timestamps have a fixed order. `sort_by`, not
    // `sort_unstable_by`: the latter's handling of equal elements is unspecified across Rust
    // versions, and this ordering reaches `burstiness`.
    events.sort();

    let first_commit_time = events.first().map(|&(time, _)| time);
    let last_commit_time = events.last().map(|&(time, _)| time);
    let commit_count = u32::try_from(events.len()).unwrap_or(u32::MAX);

    let within = |days: i64| -> u64 {
        events
            .iter()
            .filter(|&&(time, _)| reference_time.saturating_sub(time) <= days * DAY)
            .map(|&(_, lines)| lines)
            .sum()
    };
    let churn = ChurnWindows {
        days_30: within(30),
        days_90: within(90),
        days_365: within(365),
        lifetime: events.iter().map(|&(_, lines)| lines).sum(),
    };

    let temporal = TemporalPrimitives {
        first_commit_time,
        last_commit_time,
        commit_count,
        churn,
        recency_heat: recency_heat(&events, reference_time, options.recency_half_life_days),
        burstiness: burstiness(&events),
        // Needs a line count to be a ratio of (`F-EXT-4`); see the field's own docs.
        stability: None,
    };

    let counts: OrderedMap<AuthorKey, u64> = author_lines.into_iter().collect();
    let recency: OrderedMap<AuthorKey, i64> = author_recency.into_iter().collect();
    PathHistory {
        temporal,
        ownership: OwnershipPrimitives::from_line_counts(&counts, recency),
    }
}

/// `recency_heat` — the share of a path's lifetime churn that is recent, in `0..=1`.
///
/// Exponentially weighted, with the decay computed by integer halving and linear
/// interpolation rather than `exp`. `N3` forbids the platform `libm` here for the same
/// reason `F-SKEL-6` replaced `sin`: its results are not guaranteed bit-identical across
/// machines, and this value reaches generated geometry.
///
/// Normalizing by lifetime churn rather than by an absolute line count is what keeps the
/// result comparable between a busy monorepo file and a small library's: it answers "how
/// much of this path's life happened recently", which is the question `F-MAT-4`'s
/// age/recency gradient asks.
fn recency_heat(events: &[(i64, u64)], reference_time: i64, half_life_days: i64) -> Fx {
    let total: u64 = events.iter().map(|&(_, lines)| lines).sum();
    if total == 0 || half_life_days <= 0 {
        return Fx::ZERO;
    }
    // Weighted in parts per million so the accumulation stays in integers.
    let weighted: u128 = events
        .iter()
        .map(|&(time, lines)| {
            let age_days = reference_time.saturating_sub(time).max(0) / DAY;
            u128::from(decay_ppm(age_days, half_life_days)) * u128::from(lines)
        })
        .sum();
    let ppm = (weighted / u128::from(total)).min(1_000_000) as i64;
    Fx::from_ratio(ppm, 1_000_000)
}

/// `2^(-age/half_life)`, in parts per million, by halving and linear interpolation.
fn decay_ppm(age_days: i64, half_life_days: i64) -> u32 {
    if age_days <= 0 {
        return 1_000_000;
    }
    let halvings = age_days / half_life_days;
    // Past twenty half-lives the weight is under one part per million — indistinguishable
    // from zero at this precision, and the shift would eventually overflow.
    if halvings >= 20 {
        return 0;
    }
    let high = 1_000_000u64 >> halvings;
    let low = high / 2;
    let progress = age_days % half_life_days;
    (high - (high - low) * progress as u64 / half_life_days as u64) as u32
}

/// `modification_burstiness` — how concentrated a path's commits are in time, in `0..=1`.
///
/// The Goh–Barabási coefficient `(σ − μ) / (σ + μ)` over inter-commit intervals, mapped from
/// its natural `−1..=1` onto `0..=1`. Zero is perfectly regular, one is every commit in the
/// same instant, and a random (Poisson) history sits near a half.
///
/// Computed entirely in `i128` and converted only at the end. Intervals run to hundreds of
/// millions of seconds, and their squares overflow a Q32.32 fixed-point value long before
/// the arithmetic is done — doing this in [`Fx`] would silently saturate on any repository
/// older than a few years.
///
/// # Preconditions
///
/// `events` must be sorted by time, which [`finalize`] guarantees. Unsorted input yields
/// negative intervals; they clamp to zero rather than corrupting the arithmetic, but the
/// result would be meaningless, so debug builds assert instead.
fn burstiness(events: &[(i64, u64)]) -> Fx {
    debug_assert!(
        events.windows(2).all(|pair| pair[0].0 <= pair[1].0),
        "burstiness requires time-sorted events"
    );
    if events.len() < 3 {
        return Fx::ZERO;
    }
    let intervals: Vec<i128> = events
        .windows(2)
        .map(|pair| i128::from(pair[1].0 - pair[0].0).max(0))
        .collect();

    let count = intervals.len() as i128;
    let sum: i128 = intervals.iter().sum();
    if sum == 0 {
        // Every commit at the same instant: maximally bursty by definition.
        return Fx::ONE;
    }
    let sum_squares: i128 = intervals.iter().map(|&interval| interval * interval).sum();

    // n²σ² = n·Σx² − (Σx)², non-negative by Cauchy–Schwarz.
    let scaled_variance = (count * sum_squares - sum * sum).max(0);
    let scaled_sigma = (scaled_variance as u128).isqrt() as i128; // = n·σ
    let scaled_mu = sum; // = n·μ

    // B = (σ − μ)/(σ + μ), and the n cancels.
    let coefficient = ((scaled_sigma - scaled_mu) * 1_000_000) / (scaled_sigma + scaled_mu);
    let mapped = (coefficient + 1_000_000) / 2;
    Fx::from_ratio(mapped.clamp(0, 1_000_000) as i64, 1_000_000)
}

/// Merges history into the records a [`walk`](crate::walk) produced.
///
/// Records with no history keep their defaults, which is correct for a file added to the
/// working tree but never committed, and for every path when there is no repository at all.
pub fn apply(records: &mut [PathRecord], history: &History) {
    for record in records.iter_mut() {
        let Some(path_history) = history.paths.get(&record.path) else {
            continue;
        };
        record.temporal = path_history.temporal;
        record.ownership = path_history.ownership.clone();
        // A directory's commit count is the number of commits that touched *anything*
        // beneath it, which the pass already de-duplicates per commit.
        debug_assert!(
            record.kind != NodeKind::File || record.temporal.commit_count > 0,
            "a file with history must have at least one commit"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REFERENCE: i64 = 1_800_000_000;

    fn at(days_ago: i64, lines: u64) -> (i64, u64) {
        (REFERENCE - days_ago * DAY, lines)
    }

    #[test]
    fn decay_halves_every_half_life() {
        assert_eq!(decay_ppm(0, 90), 1_000_000);
        assert_eq!(decay_ppm(90, 90), 500_000);
        assert_eq!(decay_ppm(180, 90), 250_000);
        assert_eq!(decay_ppm(360, 90), 62_500);
        // Interpolated inside a half-life, and monotonically decreasing.
        assert!(decay_ppm(45, 90) < 1_000_000 && decay_ppm(45, 90) > 500_000);
        for day in 1..400 {
            assert!(decay_ppm(day, 90) <= decay_ppm(day - 1, 90), "day {day}");
        }
        // Far enough back is indistinguishable from never.
        assert_eq!(decay_ppm(100_000, 90), 0);
    }

    #[test]
    fn recency_heat_rewards_recent_work() {
        let hot = recency_heat(&[at(1, 100)], REFERENCE, 90);
        let cold = recency_heat(&[at(900, 100)], REFERENCE, 90);
        assert!(hot > Fx::from_ratio(9, 10), "yesterday should be hot");
        assert!(cold < Fx::from_ratio(1, 100), "three years ago should not");

        // Half the churn recent, half ancient, lands in between.
        let mixed = recency_heat(&[at(0, 100), at(900, 100)], REFERENCE, 90);
        assert!(mixed > cold && mixed < hot);
        // Nothing to weigh is zero, not a division trap.
        assert_eq!(recency_heat(&[], REFERENCE, 90), Fx::ZERO);
        assert_eq!(recency_heat(&[at(1, 0)], REFERENCE, 90), Fx::ZERO);
    }

    #[test]
    fn burstiness_separates_regular_from_clustered_histories() {
        // One commit every 30 days: perfectly regular. Oldest first, as `finalize` sorts.
        let regular: Vec<_> = (0..10).rev().map(|i| at(i * 30, 10)).collect();
        let steady = burstiness(&regular);
        assert_eq!(
            steady,
            Fx::ZERO,
            "evenly spaced commits are not bursty at all"
        );

        // Ten commits in one week, then nothing for three years.
        let mut clustered: Vec<_> = (0..9).rev().map(|i| at(900 + i, 10)).collect();
        clustered.push(at(0, 10));
        let spiky = burstiness(&clustered);
        assert!(spiky > steady);
        assert!(spiky > Fx::from_ratio(1, 2), "a long gap is bursty");

        // Too few intervals to say anything.
        assert_eq!(burstiness(&[at(0, 1)]), Fx::ZERO);
        assert_eq!(burstiness(&[at(1, 1), at(0, 1)]), Fx::ZERO);
        // Everything at once.
        assert_eq!(burstiness(&[at(5, 1), at(5, 1), at(5, 1)]), Fx::ONE);
    }

    /// The arithmetic that would overflow in fixed point: a decade-old repository.
    #[test]
    fn burstiness_survives_a_decade_of_intervals() {
        let decade: Vec<_> = (0..40).rev().map(|i| at(i * 90, 10)).collect();
        let value = burstiness(&decade);
        assert!(
            value >= Fx::ZERO && value <= Fx::ONE,
            "{value:?} out of range"
        );
    }

    #[test]
    fn churn_windows_bucket_by_age_against_the_reference() {
        let accumulator = Accumulator {
            events: vec![at(1, 10), at(60, 20), at(200, 40), at(900, 80)],
            ..Accumulator::default()
        };
        let history = finalize(accumulator, REFERENCE, HistoryOptions::default());
        assert_eq!(history.temporal.churn.days_30, 10);
        assert_eq!(history.temporal.churn.days_90, 30);
        assert_eq!(history.temporal.churn.days_365, 70);
        assert_eq!(history.temporal.churn.lifetime, 150);
        assert_eq!(history.temporal.commit_count, 4);
        assert_eq!(
            history.temporal.first_commit_time,
            Some(REFERENCE - 900 * DAY)
        );
        assert_eq!(history.temporal.last_commit_time, Some(REFERENCE - DAY));
    }

    #[test]
    fn ownership_comes_out_of_the_line_counts() {
        let ada = AuthorKey::from_email(b"ada@example.com");
        let bob = AuthorKey::from_email(b"bob@example.com");
        let accumulator = Accumulator {
            events: vec![at(1, 100)],
            author_lines: [(ada, 900), (bob, 100)].into_iter().collect(),
            author_recency: [(ada, REFERENCE), (bob, REFERENCE - 500 * DAY)]
                .into_iter()
                .collect(),
        };
        let history = finalize(accumulator, REFERENCE, HistoryOptions::default());
        assert_eq!(history.ownership.author_count(), 2);
        assert_eq!(history.ownership.dominant_author(), Some(ada));
        assert_eq!(history.ownership.bus_factor_proxy(), 1);
        assert_eq!(history.ownership.share_of(&ada).to_ppm(), 900_000);
    }

    /// `N3`: merging two workers' results must not depend on which finished first.
    #[test]
    fn accumulators_merge_associatively() {
        let ada = AuthorKey::from_email(b"ada@example.com");
        let make = |time, lines| Accumulator {
            events: vec![(time, lines)],
            author_lines: [(ada, lines)].into_iter().collect(),
            author_recency: [(ada, time)].into_iter().collect(),
        };

        let mut forwards = make(100, 5);
        forwards.merge(make(200, 7));
        let mut backwards = make(200, 7);
        backwards.merge(make(100, 5));

        let options = HistoryOptions::default();
        assert_eq!(
            finalize(forwards, REFERENCE, options),
            finalize(backwards, REFERENCE, options)
        );
    }

    #[test]
    fn a_repository_without_history_yields_an_empty_pass() {
        let history = History::default();
        assert_eq!(history.commit_count, 0);
        assert_eq!(history.reference_time, 0);
        assert!(history.paths.is_empty());
        assert!(history.authors.is_empty());
    }
}
