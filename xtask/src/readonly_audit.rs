//! Zero-write verification — `AC-MAN-2`, `AC-EXT-4`.
//!
//! > **AC-MAN-2** — Opening any repository with default settings produces **zero writes** to
//! > the working tree. Verifiable by filesystem tracing, and by `git status` and file mtimes
//! > being unchanged across association, extraction, first Grow, and a full Thrive session.
//!
//! `N1` is the constitutional claim: treepo reads a repository and never writes to it. Most
//! of that is already structural — architecture D3 chose `gix` over subprocess `git` so that
//! no configured hook, pager, alias, or textconv filter can run, and [`treepo_vcs::walk`]
//! reads the HEAD tree rather than the working directory. This command is what notices if
//! any of that stops being true.
//!
//! # What it does
//!
//! For every corpus fixture: take a complete census of the directory, run every extraction
//! pass Phase 1 has, take the census again, and compare. A census records each path's kind,
//! length, content hash, and modification time; a difference in any of them is a write.
//!
//! # The observer must not share code with the observed
//!
//! The census side of this file uses `std::fs` and `treepo_det::Sha256` and nothing else. It
//! deliberately does not use `gix` to read the fixture, does not reuse `treepo-vcs`'s walk,
//! and does not go through any shared helper. An auditor built from the thing it audits
//! cannot see a defect the two have in common — the same argument `tools/corpus` makes for
//! building fixtures with `git` and reading them with `gix`.
//!
//! For the same reason the extraction below calls each pass by name rather than through a
//! pipeline helper. A helper is somewhere a pass could quietly stop being called, and the
//! audit would stay green while auditing less.
//!
//! # `git status`, as an independent oracle
//!
//! `AC-MAN-2` names `git status` specifically, and it can see things a content census cannot
//! express — most usefully, an index whose stat cache no longer matches the working tree,
//! which makes a repository *dirty* without any file's bytes changing. It runs with
//! `GIT_OPTIONAL_LOCKS=0` so that the oracle cannot itself write the index refresh it would
//! otherwise perform, and before the baseline census so that any refresh it does manage is
//! inside the baseline rather than mistaken for extraction's work.
//!
//! `git` is not required. It is a developer-time tool here exactly as it is in `tools/corpus`
//! (`R1` — the product never shells out to it), and the census gates the audit on its own if
//! git is absent.
//!
//! # What is deliberately not checked
//!
//! **Access times.** Reading a file updates `atime` on some mounts and not others; that is
//! what reading *is*, and treating it as a write would make the audit fail for doing its job.
//!
//! **Anything outside the fixture.** treepo writes plenty — to application data (`F-MAN-1`),
//! which is where everything it learns is *supposed* to go. The claim under audit is about
//! the repository, so the census is rooted at the repository.
//!
//! # The detector is tested every run
//!
//! A check that has never failed is not known to work. After the audit reports clean, the
//! command mutates a throwaway directory four ways — a file added, a file removed, content
//! changed at the same length, and a modification time moved with the bytes untouched — and
//! confirms all four are caught. It runs on scratch space under `target/`; no fixture is
//! ever written to by this command.

use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use treepo_det::{Digest, Sha256};

/// A file's recorded modification time.
///
/// `clippy.toml` bans `std::time::SystemTime` outright, so that no wall clock can reach a
/// generated value (`N3`). This is the one place in the workspace that is outside what the
/// ban protects, and the exception is narrowed to this alias rather than taken per use:
/// `AC-MAN-2` names file modification times as the evidence, and all the audit does with one
/// is compare it against a second reading of the same field and discard both.
///
/// Note what stays banned. `SystemTime::now` is a *disallowed method* and this does not
/// reach it, so the audit still cannot depend on when it runs — which is the property `N3`
/// is actually defending.
#[allow(clippy::disallowed_types)]
type Mtime = std::time::SystemTime;

/// Read granularity for hashing. Bounds memory over PRD §6's "one enormous file" row, which
/// the corpus builds at a size nobody wants resident.
const CHUNK: usize = 64 * 1024;

/// `3 commits`, `1 commit`. The report is read by people deciding whether to trust it.
fn count(n: u64, noun: &str) -> String {
    if n == 1 {
        format!("{n} {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let only = crate::flag_value(args, "--fixture")?;
    let self_test_only = args.iter().any(|arg| arg == "--self-test");

    if self_test_only {
        let caught = self_test()?;
        println!("detector self-test: {caught} of {MUTATIONS} mutations caught");
        return Ok(());
    }

    println!("readonly-audit — extraction must write nothing (AC-MAN-2, AC-EXT-4)\n");

    let root = corpus::default_root();
    let fixtures = corpus::ensure(&root).map_err(|e| format!("building the corpus: {e}"))?;
    println!("  corpus  {}\n", root.display());

    let git = git_available();
    if !git {
        println!("  note: `git` is not on PATH — the status oracle is skipped\n");
    }

    let mut audited = 0usize;
    let mut extracted = 0usize;
    let mut consulted = 0usize;
    let mut violations: Vec<(String, Difference)> = Vec::new();

    for fixture in &fixtures {
        if only.as_deref().is_some_and(|name| name != fixture.name) {
            continue;
        }
        if !fixture.path.exists() {
            return Err(format!(
                "fixture `{}` is missing from {} — rebuild the corpus",
                fixture.name,
                root.display()
            ));
        }
        audited += 1;

        let bare = is_bare(&fixture.path);
        let status_before = git.then(|| git_status(&fixture.path)).flatten();
        let before = Census::take(&fixture.path)?;

        let outcome = extract(&fixture.path);
        if outcome.is_ok() {
            extracted += 1;
        }

        let after = Census::take(&fixture.path)?;
        let status_after = git.then(|| git_status(&fixture.path)).flatten();
        if status_before.is_some() && status_after.is_some() {
            consulted += 1;
        }

        let mut found = compare(&before, &after);
        if status_before != status_after {
            found.push(Difference {
                path: PathBuf::from("<git status>"),
                change: Change::Status,
                detail: format!(
                    "before:\n{}\nafter:\n{}",
                    status_before
                        .as_deref()
                        .unwrap_or("(unavailable)")
                        .trim_end(),
                    status_after
                        .as_deref()
                        .unwrap_or("(unavailable)")
                        .trim_end(),
                ),
            });
        }

        let verdict = if found.is_empty() { "clean" } else { "WRITES" };
        let summary = match &outcome {
            Ok(report) => format!(
                "extracted {}, {}, {} dirty",
                count(u64::try_from(report.records).unwrap_or(u64::MAX), "path"),
                count(u64::from(report.commits), "commit"),
                report.dirty
            ),
            Err(reason) => format!("no extraction — {reason}"),
        };
        println!(
            "  {:<17} {verdict:<7} {:>5} on disk   {summary}",
            fixture.name,
            before.entries.len()
        );

        for difference in found {
            let scope = if is_git_internal(&difference.path, bare) {
                "repository"
            } else {
                "working tree"
            };
            violations.push((format!("{} [{scope}]", fixture.name), difference));
        }
    }

    if audited == 0 {
        return Err(match only {
            Some(name) => format!("no fixture named `{name}`"),
            None => "no fixtures were audited".to_owned(),
        });
    }

    // A green audit over fixtures that all failed to open is a green audit over nothing. The
    // `bare` and `no-git` shapes are supposed to refuse; every shape refusing is a broken
    // corpus, and `tests/degenerate.rs` is where each one's outcome is actually pinned.
    if extracted == 0 {
        return Err(format!(
            "{audited} fixture(s) audited and not one extracted — the audit proved nothing"
        ));
    }

    if !violations.is_empty() {
        eprintln!();
        for (fixture, difference) in &violations {
            eprintln!(
                "  ! {fixture} {} — {}\n      {}",
                difference.path.display(),
                difference.change.name(),
                difference.detail
            );
        }
        return Err(format!(
            "{} write(s) to a repository treepo only read. N1 is the claim that this cannot \
             happen; a violation here means a pass has started touching the repository, not \
             that the audit needs relaxing.",
            violations.len()
        ));
    }

    println!(
        "\n  {} audited, {extracted} extracted, 0 writes",
        count(u64::try_from(audited).unwrap_or(u64::MAX), "fixture")
    );
    // Printed rather than assumed: a `git status` that quietly stopped being asked would
    // remove an independent oracle without removing anything from the report.
    println!("  `git status` agreed before and after on {consulted} of them");
    let caught = self_test()?;
    println!("  detector self-test: {caught} of {MUTATIONS} mutations caught");
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Extraction — every pass Phase 1 has, named individually on purpose.
// ---------------------------------------------------------------------------------------

/// What extraction produced, only so the audit can show it did something.
struct Extraction {
    records: usize,
    commits: u32,
    dirty: usize,
}

/// Runs the whole Phase 1 pipeline over one repository.
///
/// The ordering is the one [`treepo_vcs`] documents: signals need the content pass, and
/// `apply_history_signals` needs both the content pass and the history pass.
///
/// An error is not an audit failure. PRD §6 has rows whose defined behavior is a refusal —
/// a bare repository is one — and what this command asks is "was anything written", which a
/// refusal answers as well as a success does.
fn extract(root: &Path) -> Result<Extraction, String> {
    use treepo_vcs::lang::{Catalogue, ContentOptions, apply_history_signals};
    use treepo_vcs::{FilterSet, HistoryOptions, SignalDictionary, StatusOptions, WalkOptions};

    let target = treepo_vcs::discover(root).map_err(|e| format!("discover: {e}"))?;
    let filter = FilterSet::built_in();
    let catalogue = Catalogue::built_in();

    let mut structure = treepo_vcs::walk(&target, &filter, WalkOptions::default())
        .map_err(|e| format!("walk: {e}"))?;

    let mut languages = treepo_model::manifest::LanguageTable::new();
    treepo_vcs::scan(
        &target,
        &mut structure,
        &catalogue,
        &mut languages,
        ContentOptions::default(),
    )
    .map_err(|e| format!("scan: {e}"))?;

    treepo_vcs::signals::apply(
        &mut structure.records,
        &SignalDictionary::built_in(),
        &catalogue,
    );

    let history = treepo_vcs::log_pass(&target, &filter, HistoryOptions::default())
        .map_err(|e| format!("log_pass: {e}"))?;
    treepo_vcs::log_pass::apply(&mut structure.records, &history);
    apply_history_signals(&mut structure.records);

    // The reason this command was built before `status` was. Every pass above reads the HEAD
    // tree and could not write to the working directory if it tried; this one opens the
    // working directory, and `gix` computes an index stat-cache refresh while it does —
    // offered through `Outcome::write_changes`, which `treepo_vcs::status` never calls. That
    // restraint is a line of code, and a line of code is exactly the kind of thing that gets
    // "fixed" by someone chasing the performance note in gix's own documentation. The census
    // around this call is what would notice.
    let dirty = treepo_vcs::status(&target, &filter, &StatusOptions::bounded())
        .map_err(|e| format!("status: {e}"))?;

    Ok(Extraction {
        records: structure.records.len(),
        commits: history.commit_count,
        dirty: dirty.paths.len(),
    })
}

// ---------------------------------------------------------------------------------------
// The census.
// ---------------------------------------------------------------------------------------

/// What kind of thing a path was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    File,
    Directory,
    Symlink,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Directory => "directory",
            Self::Symlink => "symlink",
        }
    }
}

/// One path, at one instant.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    kind: Kind,
    len: u64,
    /// `None` where the filesystem does not report one. Two `None`s compare equal, because
    /// the audit cannot hold a filesystem to a time it declines to keep.
    modified: Option<Mtime>,
    /// File bytes, or a symlink's target. `None` for a directory, whose contents are
    /// censused as entries of their own.
    content: Option<Digest>,
}

/// Every path beneath a root, keyed relative to it so two censuses are comparable.
#[derive(Debug)]
struct Census {
    entries: BTreeMap<PathBuf, Entry>,
}

impl Census {
    /// Walks `root` completely, following no symlink.
    ///
    /// `symlink_metadata` throughout: following a link would let the census leave the fixture
    /// entirely, and the `symlinks` shape includes a deliberately broken one.
    fn take(root: &Path) -> Result<Self, String> {
        let mut entries = BTreeMap::new();
        let mut pending = vec![root.to_path_buf()];

        while let Some(directory) = pending.pop() {
            let listing = std::fs::read_dir(&directory)
                .map_err(|e| format!("reading {}: {e}", directory.display()))?;
            for item in listing {
                let item = item.map_err(|e| format!("reading {}: {e}", directory.display()))?;
                let path = item.path();
                let meta = std::fs::symlink_metadata(&path)
                    .map_err(|e| format!("stat {}: {e}", path.display()))?;

                // Symlinks first: `symlink_metadata` reports a link to a directory as
                // neither a directory nor a file on every platform we build on, and asking
                // in this order makes that independent of which.
                let kind = if meta.is_symlink() {
                    Kind::Symlink
                } else if meta.is_dir() {
                    Kind::Directory
                } else {
                    Kind::File
                };

                let content = match kind {
                    Kind::File => Some(hash_file(&path)?),
                    // Lossy on Windows, where a link target is UTF-16 that may not be valid
                    // Unicode. Both censuses are taken the same way on the same machine, so
                    // the lossiness is symmetric and a changed target still changes the hash.
                    Kind::Symlink => Some(Sha256::digest(
                        std::fs::read_link(&path)
                            .map_err(|e| format!("readlink {}: {e}", path.display()))?
                            .to_string_lossy()
                            .as_bytes(),
                    )),
                    Kind::Directory => None,
                };

                let relative = path
                    .strip_prefix(root)
                    .map_err(|_| format!("{} escaped {}", path.display(), root.display()))?
                    .to_path_buf();
                entries.insert(
                    relative,
                    Entry {
                        kind,
                        len: meta.len(),
                        modified: meta.modified().ok(),
                        content,
                    },
                );

                if kind == Kind::Directory {
                    pending.push(path);
                }
            }
        }

        Ok(Self { entries })
    }
}

/// SHA-256 of a file, read in chunks rather than slurped.
fn hash_file(path: &Path) -> Result<Digest, String> {
    let mut file =
        std::fs::File::open(path).map_err(|e| format!("opening {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; CHUNK];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| format!("reading {}: {e}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize())
}

// ---------------------------------------------------------------------------------------
// The comparison.
// ---------------------------------------------------------------------------------------

/// How a path differed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Change {
    Added,
    Removed,
    Kind,
    Size,
    Content,
    Mtime,
    Status,
}

impl Change {
    fn name(self) -> &'static str {
        match self {
            Self::Added => "created",
            Self::Removed => "deleted",
            Self::Kind => "changed kind",
            Self::Size => "changed size",
            Self::Content => "changed content",
            Self::Mtime => "modification time moved",
            Self::Status => "`git status` changed",
        }
    }
}

/// One difference between two censuses.
#[derive(Debug)]
struct Difference {
    path: PathBuf,
    change: Change,
    detail: String,
}

/// Every difference, in path order.
///
/// One difference per path, most specific first: a rewritten file reports its content rather
/// than the modification time that necessarily moved with it, so the report names the cause
/// and not its shadow.
fn compare(before: &Census, after: &Census) -> Vec<Difference> {
    let mut differences = Vec::new();

    for (path, old) in &before.entries {
        match after.entries.get(path) {
            None => differences.push(Difference {
                path: path.clone(),
                change: Change::Removed,
                detail: format!("a {} that extraction did not leave behind", old.kind.name()),
            }),
            Some(new) => {
                if let Some(change) = difference_between(old, new) {
                    differences.push(Difference {
                        path: path.clone(),
                        change: change.0,
                        detail: change.1,
                    });
                }
            }
        }
    }

    for (path, new) in &after.entries {
        if !before.entries.contains_key(path) {
            differences.push(Difference {
                path: path.clone(),
                change: Change::Added,
                detail: format!("a {} of {} bytes", new.kind.name(), new.len),
            });
        }
    }

    differences.sort_by(|a, b| a.path.cmp(&b.path));
    differences
}

fn difference_between(old: &Entry, new: &Entry) -> Option<(Change, String)> {
    if old.kind != new.kind {
        return Some((
            Change::Kind,
            format!("{} became {}", old.kind.name(), new.kind.name()),
        ));
    }
    if old.len != new.len {
        return Some((
            Change::Size,
            format!("{} bytes became {}", old.len, new.len),
        ));
    }
    if old.content != new.content {
        return Some((
            Change::Content,
            format!(
                "{} bytes rewritten in place ({} -> {})",
                new.len,
                old.content
                    .map_or_else(|| "-".to_owned(), |d| d.to_string()),
                new.content
                    .map_or_else(|| "-".to_owned(), |d| d.to_string()),
            ),
        ));
    }
    if old.modified != new.modified {
        return Some((
            Change::Mtime,
            "the bytes are identical, so this is a write that happened to rewrite the same \
             content, or a touch"
                .to_owned(),
        ));
    }
    None
}

/// Whether a path belongs to git's own storage rather than to the working tree.
///
/// `AC-MAN-2` is about the working tree; `N1` is about the repository. The audit fails on
/// either, and separates them so a violation says which claim broke — a rewritten
/// `.git/index` and a rewritten source file are very different findings.
fn is_git_internal(path: &Path, bare: bool) -> bool {
    bare || path
        .components()
        .next()
        .is_some_and(|first| first.as_os_str() == ".git")
}

/// A repository with no working tree: no `.git` directory, but git's own layout at the root.
fn is_bare(root: &Path) -> bool {
    !root.join(".git").exists() && root.join("HEAD").is_file()
}

// ---------------------------------------------------------------------------------------
// The git oracle.
// ---------------------------------------------------------------------------------------

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// `git status --porcelain`, or `None` where there is no working tree to ask about.
///
/// `GIT_OPTIONAL_LOCKS=0` is what keeps the oracle from writing: without it `git status`
/// refreshes the index when its stat cache is stale, which is a write into the very
/// directory this command exists to prove nobody writes to.
///
/// The `.git` check is not redundant with `-C`. Git searches *upward* from the directory it
/// is given, and the corpus is built under `target/`, inside treepo's own working tree — so
/// without this, the `no-git` and `bare` shapes would be answered about treepo. An oracle
/// pointed at the wrong repository agrees with itself perfectly and means nothing.
fn git_status(root: &Path) -> Option<String> {
    if !root.join(".git").is_dir() {
        return None;
    }
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--porcelain"])
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

// ---------------------------------------------------------------------------------------
// Proving the detector detects.
// ---------------------------------------------------------------------------------------

/// How many ways the self-test breaks a directory.
const MUTATIONS: usize = 4;

/// Mutates a scratch directory four ways and confirms the census catches each one.
///
/// The interesting case is the fourth. A write that restores the previous length and content
/// still moves the modification time, and a write that restores the modification time still
/// changes the content — so the two checks between them leave no shape of write invisible,
/// and this is what holds that true. The third mutation keeps the length deliberately, so it
/// can only be caught by the hash.
fn self_test() -> Result<usize, String> {
    self_test_in(&crate::workspace_root().join("target/readonly-audit-selftest"))
}

/// The self-test, against a caller-chosen scratch directory.
///
/// Split out only so the unit test below cannot collide with a `cargo xtask readonly-audit`
/// running beside it — two processes clearing the same directory would fail for a reason
/// that has nothing to do with what either was checking.
fn self_test_in(root: &Path) -> Result<usize, String> {
    if root.exists() {
        std::fs::remove_dir_all(root).map_err(|e| format!("clearing scratch space: {e}"))?;
    }
    let nested = root.join("nested");
    std::fs::create_dir_all(&nested).map_err(|e| format!("creating scratch space: {e}"))?;

    let write = |name: &str, bytes: &[u8]| -> Result<PathBuf, String> {
        let path = root.join(name);
        std::fs::write(&path, bytes).map_err(|e| format!("writing {name}: {e}"))?;
        Ok(path)
    };

    let doomed = write("removed.txt", b"this file is about to go")?;
    let rewritten = write("rewritten.txt", b"aaaaaaaaaaaa")?;
    let touched = write("touched.txt", b"these bytes do not change")?;
    write("untouched.txt", b"nor do these")?;
    std::fs::write(nested.join("deep.txt"), b"nested and untouched")
        .map_err(|e| format!("writing nested file: {e}"))?;

    let before = Census::take(root)?;

    std::fs::remove_file(&doomed).map_err(|e| format!("removing: {e}"))?;
    write("added.txt", b"new")?;
    // Same length as what it replaces, so size cannot be what catches it.
    std::fs::write(&rewritten, b"bbbbbbbbbbbb").map_err(|e| format!("rewriting: {e}"))?;
    std::fs::File::options()
        .write(true)
        .open(&touched)
        .map_err(|e| format!("opening to touch: {e}"))?
        .set_modified(Mtime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000))
        .map_err(|e| format!("setting a modification time: {e}"))?;

    let after = Census::take(root)?;
    let found = compare(&before, &after);

    let expected = [
        ("added.txt", Change::Added),
        ("removed.txt", Change::Removed),
        ("rewritten.txt", Change::Content),
        ("touched.txt", Change::Mtime),
    ];
    let mut caught = 0usize;
    for (name, change) in expected {
        let hit = found
            .iter()
            .find(|difference| difference.path == Path::new(name));
        match hit {
            Some(difference) if difference.change == change => caught += 1,
            Some(difference) => {
                return Err(format!(
                    "the detector saw {name} as `{}` rather than `{}` — the audit's report \
                     would name the wrong cause",
                    difference.change.name(),
                    change.name()
                ));
            }
            None => {
                return Err(format!(
                    "the detector did not notice {name} ({}). Every green run of this \
                     command rests on it noticing, so this is a failure of the audit itself \
                     and not of the code under audit.",
                    change.name()
                ));
            }
        }
    }

    if found.len() != MUTATIONS {
        return Err(format!(
            "the detector reported {} differences for {MUTATIONS} mutations — something \
             unmutated was called a write, and the audit would cry wolf: {:?}",
            found.len(),
            found.iter().map(|d| &d.path).collect::<Vec<_>>()
        ));
    }

    std::fs::remove_dir_all(root).map_err(|e| format!("clearing scratch space: {e}"))?;
    Ok(caught)
}

#[cfg(test)]
mod tests {
    use super::{MUTATIONS, self_test_in};

    /// The detector self-test runs on every `readonly-audit` invocation, which is where it
    /// matters. It is also a `cargo test` case so that breaking the detector fails the
    /// ordinary test suite, rather than waiting for someone to run the audit and be told a
    /// green result cannot be trusted.
    #[test]
    fn the_detector_notices_every_shape_of_write() {
        let root = crate::workspace_root().join("target/readonly-audit-selftest-cargo");
        assert_eq!(self_test_in(&root).expect("the self-test runs"), MUTATIONS);
    }
}
