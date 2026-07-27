//! Working-tree dirtiness — `F-THR-4`.
//!
//! > Working-tree dirtiness overlay: untracked, modified, staged, pending-delete, conflicted.
//! > Rendered as transient markers and material over the frozen HEAD structure, visibly
//! > provisional — the next Grow resolves them (`F-EXT-7`).
//!
//! This is the only pass in the crate that reads the working directory. Every other one takes
//! the HEAD tree, which is why `AC-MAN-2` has been trivially true until now and why
//! `cargo xtask readonly-audit` was built before this module was.
//!
//! # It is an overlay, and it cannot become a skeleton
//!
//! `F-EXT-7` is emphatic: the working tree is *not* a second skeleton. That is enforced
//! structurally rather than by discipline, in two ways that are worth stating because both
//! look like ordinary layout decisions until you know what they are for.
//!
//! **[`Dirtiness`] is defined here, in `treepo-vcs`.** [`Manifest`](treepo_model::Manifest)
//! and [`PathRecord`] live in `treepo-model`, which does not depend on this crate — so no
//! manifest field can hold a dirtiness value, because `treepo-model` cannot name the type. A
//! transient signal is unable to reach the durable record even by accident, which also keeps
//! `AC-MAN-1` (regenerate to identical bytes) safe from something that changes when a user
//! saves a file.
//!
//! ```compile_fail
//! // F-EXT-7: dirtiness is not structure, and a PathRecord has nowhere to put it.
//! let _: fn() = || {
//!     let record: treepo_model::manifest::PathRecord = todo!();
//!     let _ = record.dirtiness;
//! };
//! ```
//!
//! A `compile_fail` test passes when the code fails to compile for *any* reason, so it needs
//! a control. The same shape against a field that does exist compiles, which is what makes
//! the failure above mean "there is no such field" rather than "this line was always wrong":
//!
//! ```
//! let _: fn() = || {
//!     let record: treepo_model::manifest::PathRecord = todo!();
//!     let _ = record.folder_signal;
//! };
//! ```
//!
//! **No pass here takes a `&mut PathRecord`.** Compare [`signals::apply`](crate::signals) and
//! [`log_pass::apply`](crate::log_pass), which do, because they are extraction and are
//! *supposed* to write to the record. [`status`] takes no structure at all and
//! [`WorkingTreeStatus::overlay`] borrows it immutably, so there is no signature in this
//! module through which dirtiness could be written into the skeleton.
//!
//! # The overlay attaches, it does not add
//!
//! Most dirty paths are not in the skeleton at all. An untracked file is by definition absent
//! from HEAD, and `F-EXT-8` has already removed `node_modules/` and everything `.gitignore`
//! covers. A path with no record of its own attaches to the nearest ancestor that has one, as
//! [`Overlay::beneath`] rather than [`Overlay::here`] — so a folder can say "something new is
//! under me" without an untracked file growing a limb of its own. That is what "the next Grow
//! resolves them" means in data: until Grow runs, new work is visible but not yet structural.
//!
//! # Nothing here writes
//!
//! `gix`'s status computes an index stat-cache refresh as a side effect and offers to write it
//! back through `Outcome::write_changes`. That call is never made. The cost is that a
//! subsequent status re-does the work the refresh would have saved; the benefit is `N1`, and
//! `readonly-audit` now runs this pass over every fixture and would report the index the
//! moment that changed.
//!
//! # Bounded, cancellable, and never a loop
//!
//! `AC-THR-1` requires Thrive to hold its frame budget with **no repository access whatsoever
//! during steady state**, and `F-THR-6` describes the caller as "a narrowly scoped,
//! cancellable, infrequent refresh". So this is a discrete callable that returns, with
//! [`StatusOptions::max_paths`] bounding the work a pathological tree can demand and
//! [`StatusOptions::should_interrupt`] letting the caller give up mid-read. It never spawns a
//! watcher and never schedules itself.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use treepo_model::identity::CommitId;
use treepo_model::manifest::PathRecord;
use treepo_model::path::RepoPath;

use crate::discover::Target;
use crate::filter::FilterSet;

/// One of the five states `F-THR-4` names.
///
/// A path can be in several at once — staging a change and then editing the file again is
/// `staged` *and* `modified` — so this is what [`Dirtiness::dominant`] reduces to, not how
/// dirtiness is stored.
///
/// **`Ord` here is declaration order, and declaration order is not precedence.** It exists so
/// this can key a `BTreeMap`; sorting by it and taking the maximum would give `Conflicted`
/// only by coincidence and `Untracked` over `Modified` by the same coincidence. The
/// precedence a renderer wants is [`ALL`](Self::ALL), applied by [`Dirtiness::dominant`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DirtyState {
    /// Present on disk, absent from the index.
    Untracked,
    /// Tracked, and the working-tree copy differs from the index.
    Modified,
    /// The index differs from HEAD — a change waiting for a commit.
    Staged,
    /// Gone from the working tree, or staged for removal. The limb is on its way out.
    PendingDelete,
    /// An unresolved merge. The only state that needs the user to do something.
    Conflicted,
}

impl DirtyState {
    /// Every state, in `dominant` precedence order — most demanding first.
    pub const ALL: [Self; 5] = [
        Self::Conflicted,
        Self::PendingDelete,
        Self::Modified,
        Self::Staged,
        Self::Untracked,
    ];

    /// A short stable name, for diagnostics and for the debug surface `F-THR-8` describes.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Untracked => "untracked",
            Self::Modified => "modified",
            Self::Staged => "staged",
            Self::PendingDelete => "pending-delete",
            Self::Conflicted => "conflicted",
        }
    }
}

/// Everything true of one path at once.
///
/// Five independent flags rather than one enum, because git's states genuinely overlap and
/// collapsing them at the point of measurement would throw away a distinction the renderer
/// might want. [`dominant`](Self::dominant) does the collapsing, at the point of use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Dirtiness {
    /// Present on disk, absent from the index.
    pub untracked: bool,
    /// The working tree differs from the index.
    pub modified: bool,
    /// The index differs from HEAD.
    pub staged: bool,
    /// Deleted from the working tree, or staged for deletion.
    pub pending_delete: bool,
    /// An unresolved merge.
    pub conflicted: bool,
}

impl Dirtiness {
    /// Nothing is going on.
    pub const CLEAN: Self = Self {
        untracked: false,
        modified: false,
        staged: false,
        pending_delete: false,
        conflicted: false,
    };

    /// A value with one state set.
    #[must_use]
    pub const fn just(state: DirtyState) -> Self {
        let mut this = Self::CLEAN;
        match state {
            DirtyState::Untracked => this.untracked = true,
            DirtyState::Modified => this.modified = true,
            DirtyState::Staged => this.staged = true,
            DirtyState::PendingDelete => this.pending_delete = true,
            DirtyState::Conflicted => this.conflicted = true,
        }
        this
    }

    /// Whether anything at all is set.
    #[must_use]
    pub const fn is_clean(self) -> bool {
        !(self.untracked || self.modified || self.staged || self.pending_delete || self.conflicted)
    }

    /// Whether one state is set.
    #[must_use]
    pub const fn has(self, state: DirtyState) -> bool {
        match state {
            DirtyState::Untracked => self.untracked,
            DirtyState::Modified => self.modified,
            DirtyState::Staged => self.staged,
            DirtyState::PendingDelete => self.pending_delete,
            DirtyState::Conflicted => self.conflicted,
        }
    }

    /// Everything set in either.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self {
            untracked: self.untracked || other.untracked,
            modified: self.modified || other.modified,
            staged: self.staged || other.staged,
            pending_delete: self.pending_delete || other.pending_delete,
            conflicted: self.conflicted || other.conflicted,
        }
    }

    /// The one state a renderer should draw, or `None` if the path is clean.
    ///
    /// The order is how far the path has moved from HEAD, with the caveat that a conflict
    /// outranks everything because it is the only state that is *waiting on the user*. A
    /// pending delete beats a modification because losing a limb reads as a bigger event than
    /// changing one, and staged comes last of the tracked states because a staged change is
    /// the most settled thing that is still not a commit.
    ///
    /// **This is a default, not a constraint.** Choosing a marker is a rendering decision and
    /// `F-THR-4`'s overlay belongs to Phase 8; the flags above are the measurement, and a
    /// renderer that wants a different precedence should read them directly rather than
    /// change this.
    #[must_use]
    pub fn dominant(self) -> Option<DirtyState> {
        DirtyState::ALL.into_iter().find(|state| self.has(*state))
    }
}

/// One path the status read had something to say about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirtyPath {
    /// Repository-relative, `/`-separated — the same encoding [`Structure`] uses, so the two
    /// can be joined without a platform-dependent conversion in between.
    pub path: RepoPath,
    /// What is true of it.
    pub state: Dirtiness,
}

/// What one working-tree read learned.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkingTreeStatus {
    /// Every dirty path that survived `F-EXT-8`, sorted by path.
    ///
    /// Sorted because `gix` reports status from worker threads and the arrival order is a
    /// scheduling artefact. `AC-DET-3` names unordered traversal as a determinism leak, and
    /// while this value never reaches a generated tree, a report whose order changes between
    /// identical reads is a bug waiting to be inherited by something that does.
    pub paths: Vec<DirtyPath>,
    /// The commit this was measured against — the manifest's `built_from_commit`, if the two
    /// agree.
    ///
    /// See [`is_current_for`](Self::is_current_for): an overlay drawn over a skeleton built
    /// from a different commit is showing the wrong tree.
    pub head: Option<CommitId>,
    /// Dirty paths `F-EXT-8` rejected — build output, ignored files, the default exclusions.
    ///
    /// Recorded rather than dropped silently, because "my changes are not showing up" has
    /// exactly one likely cause and this is the number that explains it.
    pub filtered: usize,
    /// Paths whose bytes [`RepoPath`] refused. Should be zero; PRD §6 keeps it visible.
    pub unrepresentable: usize,
    /// Whether the read stopped early — at [`StatusOptions::max_paths`] or by interruption.
    ///
    /// A truncated overlay is still worth drawing; it is not still worth trusting as
    /// complete, and the caller is the one who can tell the difference to a user.
    pub truncated: bool,
}

impl WorkingTreeStatus {
    /// Whether this overlay belongs on a skeleton built from `built_from_commit`.
    ///
    /// If HEAD moved between extraction and this read, someone committed — and
    /// `docs/design/engine-architecture.md` §5 puts "actual new commit / merge" in the column
    /// that triggers a full Grow, not the column Thrive handles. Drawing this overlay anyway
    /// would mark paths on a structure that no longer describes them.
    #[must_use]
    pub fn is_current_for(&self, built_from_commit: Option<CommitId>) -> bool {
        self.head == built_from_commit
    }

    /// Whether anything at all is dirty.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.paths.is_empty()
    }

    /// How many paths are in a given state.
    #[must_use]
    pub fn count(&self, state: DirtyState) -> usize {
        self.paths
            .iter()
            .filter(|entry| entry.state.has(state))
            .count()
    }

    /// Joins this status onto an extracted structure (`F-EXT-7`).
    ///
    /// `records` is borrowed immutably and comes back untouched — the overlay is a value
    /// beside the structure, never a change to it.
    ///
    /// Records must be sorted by path, which is what [`walk`](crate::walk) leaves them as.
    #[must_use]
    pub fn overlay(&self, records: &[PathRecord]) -> Overlay {
        Overlay::build(self, records)
    }
}

/// Dirtiness positioned on an extracted structure.
///
/// Two values per record, and the distinction is the whole point. [`here`](Self::here) is
/// what is true of the path itself; [`beneath`](Self::beneath) is what is true of anything
/// under it. A directory whose only news is an untracked file has an empty `here` and an
/// untracked `beneath` — it has not changed, but something inside it has, and at a zoom level
/// where the file is not drawn that is the only way to see it at all.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Overlay {
    here: Vec<Dirtiness>,
    beneath: Vec<Dirtiness>,
    attached: usize,
    orphaned: usize,
}

impl Overlay {
    fn build(status: &WorkingTreeStatus, records: &[PathRecord]) -> Self {
        // Attachment binary-searches, so unsorted records produce a wrong overlay rather than
        // a loud one. `walk` always sorts, and a debug assertion is where that contract gets
        // checked: the alternative is sorting a copy on every read, which costs an allocation
        // per Thrive refresh to defend against a caller that does not exist.
        debug_assert!(
            records.is_sorted_by(|a, b| a.path <= b.path),
            "Overlay needs records in the sorted order `walk` produces"
        );

        let mut here = vec![Dirtiness::CLEAN; records.len()];
        let mut beneath = vec![Dirtiness::CLEAN; records.len()];
        let mut attached = 0usize;
        let mut orphaned = 0usize;

        for entry in &status.paths {
            match records.binary_search_by(|record| record.path.cmp(&entry.path)) {
                Ok(index) => {
                    here[index] = here[index].union(entry.state);
                    attached += 1;
                }
                Err(_) => {
                    // Not in the skeleton — an untracked file, or one the filter removed
                    // between extraction and now. It marks the nearest ancestor that *is*
                    // there instead of growing a limb of its own.
                    match nearest_ancestor(records, &entry.path) {
                        Some(index) => {
                            beneath[index] = beneath[index].union(entry.state);
                            attached += 1;
                        }
                        // Only reachable if `records` has no root, which `walk` guarantees it
                        // does even for an empty repository (`AC-SKEL-2`). Counted rather
                        // than asserted: a panic in the overlay would take down a Thrive
                        // frame for a signal that is decorative by design.
                        None => orphaned += 1,
                    }
                }
            }
        }

        // Roll `beneath` upward. Records are sorted by path, so a child always precedes its
        // parent's *next* sibling and every child is visited before its parent is read here —
        // reverse order makes one pass enough for any depth.
        let parents = crate::walk::parent_indices(records);
        for index in (0..records.len()).rev() {
            if let Some(parent) = parents[index] {
                beneath[parent] = beneath[parent].union(here[index]).union(beneath[index]);
            }
        }

        Self {
            here,
            beneath,
            attached,
            orphaned,
        }
    }

    /// What is true of the record at `index` itself.
    #[must_use]
    pub fn here(&self, index: usize) -> Dirtiness {
        self.here.get(index).copied().unwrap_or(Dirtiness::CLEAN)
    }

    /// What is true of anything under the record at `index`.
    #[must_use]
    pub fn beneath(&self, index: usize) -> Dirtiness {
        self.beneath.get(index).copied().unwrap_or(Dirtiness::CLEAN)
    }

    /// Both together — what a limb should read as when its contents are not drawn separately.
    #[must_use]
    pub fn combined(&self, index: usize) -> Dirtiness {
        self.here(index).union(self.beneath(index))
    }

    /// How many records this overlay covers. Equal to the slice it was built from.
    #[must_use]
    pub fn len(&self) -> usize {
        self.here.len()
    }

    /// Whether it covers no records at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.here.is_empty()
    }

    /// Dirty paths that found a record to attach to.
    #[must_use]
    pub fn attached(&self) -> usize {
        self.attached
    }

    /// Dirty paths that found nothing at all — always zero for a structure from
    /// [`walk`](crate::walk), which always contains the root.
    #[must_use]
    pub fn orphaned(&self) -> usize {
        self.orphaned
    }
}

/// The deepest record that is a strict ancestor of `path`.
///
/// Walks the path upward, binary-searching for each parent in turn, so the cost is bounded by
/// **depth** rather than by record count — `O(depth · log n)`.
///
/// The obvious alternative is to take the insertion point for `path` and scan backwards for
/// the first record that is a prefix of it. That is correct and much worse: ancestors sort
/// before all of their own descendants, so an untracked file inside a directory holding fifty
/// thousand entries scans past every one of them. With `max_paths` dirty paths that is
/// quadratic in the size of the repository, on the pass that has the tightest time budget in
/// the crate (`AC-THR-2`). Paths are shallow — PRD §6 treats depth over fifteen as the
/// notable case — so climbing is cheap and does not care how wide the tree is.
fn nearest_ancestor(records: &[PathRecord], path: &RepoPath) -> Option<usize> {
    let mut candidate = path.parent();
    while let Some(ancestor) = candidate {
        if let Ok(index) = records.binary_search_by(|record| record.path.cmp(&ancestor)) {
            return Some(index);
        }
        candidate = ancestor.parent();
    }
    None
}

/// How much work a status read may do.
#[derive(Debug, Clone, Default)]
pub struct StatusOptions {
    /// Stop after this many dirty paths. `0` means no limit.
    ///
    /// A repository with a hundred thousand untracked files is a supported path, not an error
    /// — but `AC-THR-2` gives the overlay two seconds and an unbounded read cannot promise
    /// that. Beyond a few thousand markers the overlay has stopped being legible anyway, so
    /// the cap costs nothing a user could see. [`WorkingTreeStatus::truncated`] says when it
    /// bound.
    pub max_paths: usize,
    /// A flag the caller can set to abandon the read (`F-THR-6` — "cancellable").
    ///
    /// Passed to `gix`, so it is honoured inside the directory walk rather than only between
    /// items. A cancelled read returns what it had, marked
    /// [`truncated`](WorkingTreeStatus::truncated), rather than an error: the caller cancelled
    /// on purpose and does not need to be told about it twice.
    pub should_interrupt: Option<Arc<AtomicBool>>,
}

impl StatusOptions {
    /// The default cap, chosen to bound the work rather than to describe a real repository.
    pub const DEFAULT_MAX_PATHS: usize = 10_000;

    /// A bounded read with no cancellation flag.
    #[must_use]
    pub fn bounded() -> Self {
        Self {
            max_paths: Self::DEFAULT_MAX_PATHS,
            should_interrupt: None,
        }
    }
}

/// Why a status read could not complete.
#[derive(Debug)]
pub enum StatusError {
    /// `gix` refused to start the read.
    Start(String),
    /// The read failed partway through.
    Read(String),
}

impl std::fmt::Display for StatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Start(why) => write!(f, "could not read working-tree status: {why}"),
            Self::Read(why) => write!(f, "working-tree status failed partway through: {why}"),
        }
    }
}

impl std::error::Error for StatusError {}

/// Reads working-tree dirtiness (`F-THR-4`).
///
/// Takes no [`Structure`](crate::walk::Structure), on purpose. Joining to the skeleton is
/// [`WorkingTreeStatus::overlay`]'s job and happens afterwards, so "is this repository dirty
/// at all?" is answerable without a walk — which is what makes it usable as the cheap check
/// before deciding whether a Grow is worth triggering.
///
/// Pass the same [`FilterSet`] the walk used, or paths the skeleton does not contain will be
/// reported as dirty.
///
/// A [`Target::PlainDirectory`] has no index to be dirty against and yields an empty status
/// rather than an error — PRD §6 makes a directory with no `.git` an ordinary path.
///
/// # Errors
///
/// [`StatusError`] if `gix` cannot start or finish the read. A read stopped by
/// [`StatusOptions::should_interrupt`] or by the path cap is *not* an error; it comes back
/// with [`truncated`](WorkingTreeStatus::truncated) set.
pub fn status(
    target: &Target,
    filter: &FilterSet,
    options: &StatusOptions,
) -> Result<WorkingTreeStatus, StatusError> {
    let Some(repo) = target.repository() else {
        return Ok(WorkingTreeStatus::default());
    };
    read(repo, filter, options)
}

fn read(
    repo: &gix::Repository,
    filter: &FilterSet,
    options: &StatusOptions,
) -> Result<WorkingTreeStatus, StatusError> {
    use gix::status::UntrackedFiles;

    let head = repo
        .head_id()
        .ok()
        .and_then(|id| crate::discover::to_commit_id(&id));

    let mut platform = repo
        .status(gix::progress::Discard)
        .map_err(|e| StatusError::Start(e.to_string()))?
        // Collapsed is git's own default for `--untracked-files=normal`, and it is the right
        // one here for a second reason: an untracked directory's contents are not in the
        // skeleton, so every file inside would attach to the same ancestor and render as one
        // marker regardless. Reporting them individually would cost a full walk of, say, a
        // freshly unpacked `node_modules` to produce a result identical to reporting the
        // directory once.
        .untracked_files(UntrackedFiles::Collapsed)
        // Rename tracking off. A rename is a delete plus an add until the next Grow decides
        // otherwise, and paying `gix`'s similarity comparison inside the Thrive-adjacent
        // budget to relabel two markers as one is not a trade `AC-THR-2` can afford.
        .index_worktree_rewrites(None);

    if let Some(flag) = &options.should_interrupt {
        platform = platform.should_interrupt_owned(Arc::clone(flag));
    }

    // A repository with no commits has no HEAD tree to diff the index against, so the
    // tree-to-index half is skipped rather than allowed to fail. Everything is untracked or
    // staged-as-addition there anyway, which the index-to-worktree half already reports.
    let mut collected = Collected::default();
    if head.is_some() {
        let mut iter = platform
            .into_iter(None)
            .map_err(|e| StatusError::Start(e.to_string()))?;
        drain(&mut iter, filter, options, &mut collected)?;
    } else {
        // The index-to-worktree iterator yields the narrower item type; `gix` converts it
        // into the wider one, so `classify` stays a single function with a single match and
        // cannot drift between the two paths.
        // `Box::new` on the error only to keep the closure's `Result` small — `gix`'s status
        // error is 240 bytes and `clippy::result_large_err` is right that carrying it by
        // value through a hot iterator is waste, even though this one is never taken.
        let mut iter = platform
            .into_index_worktree_iter(None)
            .map_err(|e| StatusError::Start(e.to_string()))?
            .map(|item| item.map(gix::status::Item::from).map_err(Box::new));
        drain(&mut iter, filter, options, &mut collected)?;
    }

    Ok(collected.finish(head))
}

/// Dirtiness accumulated per path, before it is flattened into a sorted list.
#[derive(Default)]
struct Collected {
    by_path: std::collections::BTreeMap<RepoPath, Dirtiness>,
    filtered: usize,
    unrepresentable: usize,
    truncated: bool,
}

impl Collected {
    /// Records one observation, or explains why it was dropped.
    fn note(&mut self, raw: &[u8], state: Dirtiness, filter: &FilterSet, max: usize) {
        if state.is_clean() {
            return;
        }
        let Ok(path) = RepoPath::new(raw) else {
            self.unrepresentable += 1;
            return;
        };
        // `F-EXT-8` decides what counts as structure, and it has to decide the same way here
        // as it did during the walk. A dirty `target/` that lit up the trunk on every build
        // would make the overlay unusable in exactly the repositories people are working in.
        if !filter.allows(&path) {
            self.filtered += 1;
            return;
        }
        if max > 0 && self.by_path.len() >= max && !self.by_path.contains_key(&path) {
            self.truncated = true;
            return;
        }
        let slot = self.by_path.entry(path).or_default();
        *slot = slot.union(state);
    }

    fn finish(self, head: Option<CommitId>) -> WorkingTreeStatus {
        WorkingTreeStatus {
            // BTreeMap iteration is already in `RepoPath` order, which is byte-wise — the
            // same order `walk` sorts records into, so `Overlay`'s binary search is valid.
            paths: self
                .by_path
                .into_iter()
                .map(|(path, state)| DirtyPath { path, state })
                .collect(),
            head,
            filtered: self.filtered,
            unrepresentable: self.unrepresentable,
            truncated: self.truncated,
        }
    }
}

/// Pulls every item out of a status iterator and classifies it.
fn drain<I, E>(
    iter: &mut I,
    filter: &FilterSet,
    options: &StatusOptions,
    into: &mut Collected,
) -> Result<(), StatusError>
where
    I: Iterator<Item = Result<gix::status::Item, E>>,
    E: std::fmt::Display,
{
    for item in iter {
        match item {
            Ok(item) => classify(&item, filter, options.max_paths, into),
            Err(error) => return Err(StatusError::Read(error.to_string())),
        }
        if into.truncated {
            break;
        }
        if options
            .should_interrupt
            .as_ref()
            .is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Relaxed))
        {
            into.truncated = true;
            break;
        }
    }
    Ok(())
}

fn classify(item: &gix::status::Item, filter: &FilterSet, max: usize, into: &mut Collected) {
    use gix::status::Item;
    use gix::status::index_worktree::Item as WorktreeItem;
    // `gix::status::plumbing` is `gix-status` itself. Reached through `gix` rather than
    // depended on directly, so the version stays pinned by the one dependency that matters.
    use gix::status::plumbing::index_as_worktree::{Change, EntryStatus};

    match item {
        Item::IndexWorktree(WorktreeItem::Modification {
            rela_path, status, ..
        }) => {
            let state = match status {
                EntryStatus::Conflict { .. } => Dirtiness::just(DirtyState::Conflicted),
                EntryStatus::Change(Change::Removed) => Dirtiness::just(DirtyState::PendingDelete),
                EntryStatus::Change(_) => Dirtiness::just(DirtyState::Modified),
                // `IntentToAdd` is `git add --intent-to-add`: an index entry promising a file
                // that is not in the object database yet. Untracked is what it is.
                EntryStatus::IntentToAdd => Dirtiness::just(DirtyState::Untracked),
                // **Not a change.** `NeedsUpdate` means the file is identical and only the
                // index's cached stat is stale — the hint `gix` offers so a future read can
                // skip the comparison. Treating it as a modification would light up most of
                // a repository the first time the overlay ran after a checkout, which is
                // both wrong and the single most plausible way to make this feature look
                // broken.
                EntryStatus::NeedsUpdate(_) => Dirtiness::CLEAN,
            };
            into.note(rela_path.as_ref(), state, filter, max);
        }

        Item::IndexWorktree(WorktreeItem::DirectoryContents { entry, .. }) => {
            // Ignored, pruned, and tracked entries all arrive here too. Only the untracked
            // ones are news.
            if entry.status == gix::dir::entry::Status::Untracked {
                into.note(
                    entry.rela_path.as_ref(),
                    Dirtiness::just(DirtyState::Untracked),
                    filter,
                    max,
                );
            }
        }

        // Rewrite tracking is disabled above, so this is unreachable in practice. Handled
        // rather than ignored because "unreachable given a setting" is one edit away from
        // being wrong, and the honest reading of a rename is exactly its two halves.
        Item::IndexWorktree(WorktreeItem::Rewrite {
            source,
            dirwalk_entry,
            ..
        }) => {
            into.note(
                source.rela_path(),
                Dirtiness::just(DirtyState::PendingDelete),
                filter,
                max,
            );
            into.note(
                dirwalk_entry.rela_path.as_ref(),
                Dirtiness::just(DirtyState::Untracked),
                filter,
                max,
            );
        }

        // The index against HEAD: everything here is staged by definition. A staged deletion
        // is also pending — the limb is going, it is just already recorded as going.
        Item::TreeIndex(change) => {
            let (location, extra) = match change {
                gix::diff::index::Change::Deletion { location, .. } => {
                    (location, Dirtiness::just(DirtyState::PendingDelete))
                }
                gix::diff::index::Change::Addition { location, .. }
                | gix::diff::index::Change::Modification { location, .. }
                | gix::diff::index::Change::Rewrite { location, .. } => {
                    (location, Dirtiness::CLEAN)
                }
            };
            into.note(
                location.as_ref(),
                Dirtiness::just(DirtyState::Staged).union(extra),
                filter,
                max,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use treepo_model::manifest::NodeKind;

    fn path(s: &str) -> RepoPath {
        RepoPath::new(s.as_bytes()).expect("valid path")
    }

    /// A skeleton, in the sorted order `walk` leaves behind.
    fn skeleton(paths: &[&str]) -> Vec<PathRecord> {
        let mut records: Vec<PathRecord> = core::iter::once(RepoPath::root())
            .chain(paths.iter().map(|p| path(p)))
            .map(|p| {
                let kind = if p.as_bytes().ends_with(b".rs") || p.as_bytes().ends_with(b".md") {
                    NodeKind::File
                } else {
                    NodeKind::Directory
                };
                PathRecord::new(p, kind)
            })
            .collect();
        records.sort_by(|a, b| a.path.cmp(&b.path));
        records
    }

    fn dirty(entries: &[(&str, Dirtiness)]) -> WorkingTreeStatus {
        let mut paths: Vec<DirtyPath> = entries
            .iter()
            .map(|(p, state)| DirtyPath {
                path: path(p),
                state: *state,
            })
            .collect();
        paths.sort_by(|a, b| a.path.cmp(&b.path));
        WorkingTreeStatus {
            paths,
            ..Default::default()
        }
    }

    fn index_of(records: &[PathRecord], p: &str) -> usize {
        records
            .iter()
            .position(|record| record.path == path(p))
            .expect("record present")
    }

    #[test]
    fn states_do_not_collapse_into_one_another() {
        let both = Dirtiness::just(DirtyState::Staged).union(Dirtiness::just(DirtyState::Modified));
        assert!(both.staged && both.modified);
        // Staging a change and then editing the file again is genuinely both, and the point
        // of five flags rather than one enum is that neither erases the other.
        assert_eq!(both.dominant(), Some(DirtyState::Modified));
        assert_eq!(
            Dirtiness::just(DirtyState::Staged).dominant(),
            Some(DirtyState::Staged)
        );
        assert_eq!(Dirtiness::CLEAN.dominant(), None);
        assert!(Dirtiness::CLEAN.is_clean());
    }

    /// The precedence is a claim about what a viewer should see first.
    #[test]
    fn a_conflict_outranks_everything_it_is_mixed_with() {
        let mut everything = Dirtiness::CLEAN;
        for state in DirtyState::ALL {
            everything = everything.union(Dirtiness::just(state));
        }
        assert_eq!(everything.dominant(), Some(DirtyState::Conflicted));
        assert!(DirtyState::ALL.iter().all(|s| everything.has(*s)));
    }

    /// `F-EXT-7`: a dirty path in the skeleton marks itself.
    #[test]
    fn a_modified_tracked_file_marks_its_own_record() {
        let records = skeleton(&["src", "src/main.rs"]);
        let overlay =
            dirty(&[("src/main.rs", Dirtiness::just(DirtyState::Modified))]).overlay(&records);

        let file = index_of(&records, "src/main.rs");
        assert!(overlay.here(file).modified);
        assert!(
            !overlay.beneath(file).modified,
            "a file has nothing under it"
        );
        assert_eq!(overlay.attached(), 1);
        assert_eq!(overlay.orphaned(), 0);
    }

    /// `F-EXT-7`: an untracked file does not grow a limb. It marks the limb that holds it.
    #[test]
    fn an_untracked_file_attaches_to_its_nearest_present_ancestor() {
        let records = skeleton(&["src", "src/main.rs"]);
        // `src/new.rs` is not in the skeleton — it is untracked, so it is not in HEAD.
        let overlay =
            dirty(&[("src/new.rs", Dirtiness::just(DirtyState::Untracked))]).overlay(&records);

        let dir = index_of(&records, "src");
        assert!(
            overlay.here(dir).is_clean(),
            "the directory itself has not changed"
        );
        assert!(overlay.beneath(dir).untracked, "but something new is in it");
        assert_eq!(overlay.attached(), 1);
        assert_eq!(records.len(), 3, "and no record was added");
    }

    /// The nearest ancestor is the *deepest* one, not the alphabetically previous record.
    #[test]
    fn attachment_finds_the_deepest_ancestor_not_the_preceding_record() {
        let records = skeleton(&["src", "src/a.rs", "src/deep", "src/deep/kept.rs"]);
        // `src/a.rs` sorts immediately before `src/deep/new.rs` and is not an ancestor of it.
        let overlay =
            dirty(&[("src/deep/new.rs", Dirtiness::just(DirtyState::Untracked))]).overlay(&records);

        assert!(overlay.beneath(index_of(&records, "src/deep")).untracked);
        assert!(
            overlay.here(index_of(&records, "src/a.rs")).is_clean(),
            "the preceding record is not an ancestor and must not be marked"
        );
        assert_eq!(overlay.attached(), 1);
    }

    /// A limb has to show that something under it is happening, or the overlay is invisible
    /// at any zoom level where files are not drawn individually.
    #[test]
    fn dirtiness_rolls_up_to_every_ancestor() {
        let records = skeleton(&["crates", "crates/a", "crates/a/src", "crates/a/src/lib.rs"]);
        let overlay = dirty(&[("crates/a/src/lib.rs", Dirtiness::just(DirtyState::Modified))])
            .overlay(&records);

        for ancestor in ["", "crates", "crates/a", "crates/a/src"] {
            let index = records
                .iter()
                .position(|r| r.path.as_bytes() == ancestor.as_bytes())
                .expect("ancestor present");
            assert!(
                overlay.beneath(index).modified,
                "`{ancestor}` should see the change under it"
            );
            assert!(
                overlay.here(index).is_clean(),
                "`{ancestor}` did not itself change"
            );
        }
        assert!(overlay.combined(0).modified, "the root reads as dirty");
    }

    /// Different states under one limb accumulate rather than replace.
    #[test]
    fn a_limb_carries_every_state_beneath_it() {
        let records = skeleton(&["src", "src/a.rs", "src/b.rs"]);
        let overlay = dirty(&[
            ("src/a.rs", Dirtiness::just(DirtyState::Modified)),
            ("src/b.rs", Dirtiness::just(DirtyState::PendingDelete)),
            ("src/untracked.rs", Dirtiness::just(DirtyState::Untracked)),
        ])
        .overlay(&records);

        let dir = index_of(&records, "src");
        let beneath = overlay.beneath(dir);
        assert!(beneath.modified && beneath.pending_delete && beneath.untracked);
        assert_eq!(beneath.dominant(), Some(DirtyState::PendingDelete));
    }

    /// The root is always in the skeleton (`AC-SKEL-2`), so nothing can fail to attach.
    #[test]
    fn everything_attaches_because_the_root_is_always_there() {
        let records = skeleton(&[]);
        let overlay = dirty(&[
            ("wherever/it/is.rs", Dirtiness::just(DirtyState::Untracked)),
            ("top.md", Dirtiness::just(DirtyState::Untracked)),
        ])
        .overlay(&records);

        assert_eq!(overlay.orphaned(), 0);
        assert_eq!(overlay.attached(), 2);
        assert!(overlay.beneath(0).untracked);
    }

    /// An overlay against a structure built from a different commit is describing a tree that
    /// no longer exists. Committing is a Grow trigger, not a Thrive one.
    #[test]
    fn an_overlay_knows_which_head_it_was_measured_against() {
        let head = CommitId::sha1([1u8; 20]);
        let moved = CommitId::sha1([2u8; 20]);
        let status = WorkingTreeStatus {
            head: Some(head),
            ..Default::default()
        };

        assert!(status.is_current_for(Some(head)));
        assert!(!status.is_current_for(Some(moved)));
        assert!(
            !status.is_current_for(None),
            "a skeleton from no commit is not the one this was read against"
        );
    }

    #[test]
    fn counting_reports_paths_not_flags() {
        let status = dirty(&[
            ("a.rs", Dirtiness::just(DirtyState::Modified)),
            (
                "b.rs",
                Dirtiness::just(DirtyState::Modified).union(Dirtiness::just(DirtyState::Staged)),
            ),
        ]);
        assert_eq!(status.count(DirtyState::Modified), 2);
        assert_eq!(status.count(DirtyState::Staged), 1);
        assert_eq!(status.count(DirtyState::Conflicted), 0);
        assert!(!status.is_clean());
    }

    /// `F-EXT-8` has to decide the same way here as it did during the walk, or the overlay
    /// lights up build output the skeleton does not contain.
    #[test]
    fn the_filter_rejects_the_same_paths_the_walk_did() {
        let filter = FilterSet::built_in();
        let mut collected = Collected::default();
        let untracked = Dirtiness::just(DirtyState::Untracked);

        collected.note(b"src/real.rs", untracked, &filter, 0);
        collected.note(b"target/debug/build.rs", untracked, &filter, 0);
        collected.note(b"node_modules/left-pad/index.js", untracked, &filter, 0);

        let status = collected.finish(None);
        assert_eq!(status.paths.len(), 1);
        assert_eq!(status.paths[0].path, path("src/real.rs"));
        assert_eq!(status.filtered, 2, "and it says how many it dropped");
    }

    /// A repository with a hundred thousand untracked files is a supported path, and
    /// `AC-THR-2` still gives the overlay two seconds.
    #[test]
    fn the_path_cap_bounds_the_read_and_says_so() {
        let filter = FilterSet::built_in();
        let mut collected = Collected::default();
        for index in 0..10u32 {
            collected.note(
                format!("src/file{index}.rs").as_bytes(),
                Dirtiness::just(DirtyState::Untracked),
                &filter,
                3,
            );
        }

        let status = collected.finish(None);
        assert_eq!(status.paths.len(), 3);
        assert!(status.truncated);

        // A path already collected still accumulates after the cap — dropping a second
        // observation of a path we are already reporting would describe it wrongly rather
        // than merely incompletely.
        let mut collected = Collected::default();
        collected.note(b"a.rs", Dirtiness::just(DirtyState::Staged), &filter, 1);
        collected.note(b"a.rs", Dirtiness::just(DirtyState::Modified), &filter, 1);
        let status = collected.finish(None);
        assert_eq!(status.paths.len(), 1);
        assert!(status.paths[0].state.staged && status.paths[0].state.modified);
    }

    /// PRD §6 keeps unrepresentable paths visible rather than silently dropped.
    ///
    /// The `..` case is the one that matters. [`RepoPath`] rejects a dot component because
    /// honouring one would let a path address something outside the repository, and a status
    /// item is one of the few places a path arrives having been shaped by what is on disk.
    /// It must be refused here rather than resolved, and refusing it must not take the read
    /// down with it.
    #[test]
    fn a_path_repopath_refuses_is_counted_not_swallowed() {
        let filter = FilterSet::built_in();
        let mut collected = Collected::default();
        let untracked = Dirtiness::just(DirtyState::Untracked);

        collected.note(b"../outside/secrets.env", untracked, &filter, 0);
        collected.note(b"src//empty.rs", untracked, &filter, 0);
        collected.note(b"nul\0byte.rs", untracked, &filter, 0);
        collected.note(b"ok.rs", untracked, &filter, 0);

        let status = collected.finish(None);
        assert_eq!(status.unrepresentable, 3);
        assert_eq!(status.paths.len(), 1);
        assert_eq!(status.paths[0].path, path("ok.rs"));
    }

    /// Output order must not depend on which worker thread reported what (`AC-DET-3`).
    #[test]
    fn the_report_is_sorted_whatever_order_it_arrived_in() {
        let filter = FilterSet::built_in();
        let mut collected = Collected::default();
        for name in ["z.rs", "a.rs", "m/deep.rs", "b.rs"] {
            collected.note(
                name.as_bytes(),
                Dirtiness::just(DirtyState::Modified),
                &filter,
                0,
            );
        }
        let status = collected.finish(None);
        let order: Vec<&str> = status
            .paths
            .iter()
            .map(|entry| entry.path.display().into_owned().leak() as &str)
            .collect();
        assert_eq!(order, ["a.rs", "b.rs", "m/deep.rs", "z.rs"]);
    }

    /// A clean observation is not an entry. `NeedsUpdate` arrives as one, and if it were
    /// recorded the overlay would light up most of a repository after any checkout.
    #[test]
    fn a_clean_observation_never_becomes_an_entry() {
        let filter = FilterSet::built_in();
        let mut collected = Collected::default();
        collected.note(b"unchanged.rs", Dirtiness::CLEAN, &filter, 0);
        assert!(collected.finish(None).is_clean());
    }
}
