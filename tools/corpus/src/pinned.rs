//! Real repositories, pinned by commit SHA — PRD §3's other half.
//!
//! [`shapes`](crate::shapes) builds synthetic fixtures for the edge cases. This builds nothing:
//! it fetches repositories that already exist and holds them at an exact commit, so that a
//! budget measured today is comparable to one measured a year from now.
//!
//! # Fetching is explicit, and never happens by default
//!
//! `NFR-8` requires the product to work offline with no network dependency in any product
//! path, and this is not a product path — it is the same developer-time position `git` already
//! occupies in this crate. But the distinction only holds if the network call is impossible to
//! trigger by accident, so nothing here fetches unless a caller asks: [`ensure`] reports what
//! is on disk, [`fetch`] is what goes to the network, and only `cargo xtask budget --fetch`
//! calls it.
//!
//! # The SHA is the pin, not the tag
//!
//! A tag can be moved and a branch certainly can. [`fetch`] checks out the recorded commit and
//! [`verify`] re-reads it, so a repository whose history was rewritten fails loudly instead of
//! being measured as though it were the pinned one.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::BuildError;

/// The pin declarations, compiled in.
const PINS_RON: &str = include_str!("pins.ron");

/// A corpus tier, as PRD §3 defines them.
///
/// `T0` has no pinned repository — it is the empty-and-tiny end of the range and the synthetic
/// shapes cover it completely. It is listed so the enum matches the PRD table rather than
/// matching what happens to be pinned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize)]
pub enum Tier {
    /// 0–20 files, under 1k LOC, 0–20 commits, 1 author. A new or empty project.
    T0,
    /// ~1k files, ~50k LOC, ~2k commits, 1–10 authors. A library.
    T1,
    /// ~10k files, ~500k LOC, ~20k commits, 10–100 authors. An application.
    T2,
    /// ~80k files, ~5M LOC, ~200k commits, 100–2k authors. A kernel-scale monorepo.
    T3,
    /// 300k+ files, 20M+ LOC, 1M+ commits. Browser-scale; best-effort only (`F-CORP-1`).
    T4,
}

impl Tier {
    /// Every tier, smallest first.
    pub const ALL: [Self; 5] = [Self::T0, Self::T1, Self::T2, Self::T3, Self::T4];

    /// The tier's name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::T0 => "T0",
            Self::T1 => "T1",
            Self::T2 => "T2",
            Self::T3 => "T3",
            Self::T4 => "T4",
        }
    }

    /// The tier's nominal file count at HEAD and commit count, straight from PRD §3.
    ///
    /// One figure each, as the table gives them. The bands below are derived from these
    /// rather than written separately, so revising the PRD means changing one number.
    #[must_use]
    pub const fn nominal(self) -> (u64, u64) {
        match self {
            Self::T0 => (20, 20),
            Self::T1 => (1_000, 2_000),
            Self::T2 => (10_000, 20_000),
            Self::T3 => (80_000, 200_000),
            Self::T4 => (300_000, 1_000_000),
        }
    }

    /// The band of file counts that reads as this tier.
    ///
    /// The split between two tiers sits at the **geometric mean** of their nominal figures,
    /// not at a multiple of either. The tiers are roughly a decade apart, so any fixed
    /// multiple either leaves gaps — a 200-file repository belonging to no tier — or lets two
    /// tiers claim the same repository. A geometric split tiles exactly: every count lands in
    /// one tier, and the boundary sits where a count is equally far from both in the ratio
    /// terms the table is written in.
    #[must_use]
    pub const fn files(self) -> (u64, u64) {
        match self {
            Self::T0 => (0, split(Self::T0.nominal().0, Self::T1.nominal().0)),
            Self::T1 => (
                split(Self::T0.nominal().0, Self::T1.nominal().0),
                split(Self::T1.nominal().0, Self::T2.nominal().0),
            ),
            Self::T2 => (
                split(Self::T1.nominal().0, Self::T2.nominal().0),
                split(Self::T2.nominal().0, Self::T3.nominal().0),
            ),
            Self::T3 => (
                split(Self::T2.nominal().0, Self::T3.nominal().0),
                split(Self::T3.nominal().0, Self::T4.nominal().0),
            ),
            Self::T4 => (split(Self::T3.nominal().0, Self::T4.nominal().0), u64::MAX),
        }
    }

    /// The band of commit counts that reads as this tier. Split the same way as [`files`].
    ///
    /// [`files`]: Self::files
    #[must_use]
    pub const fn commits(self) -> (u64, u64) {
        match self {
            Self::T0 => (0, split(Self::T0.nominal().1, Self::T1.nominal().1)),
            Self::T1 => (
                split(Self::T0.nominal().1, Self::T1.nominal().1),
                split(Self::T1.nominal().1, Self::T2.nominal().1),
            ),
            Self::T2 => (
                split(Self::T1.nominal().1, Self::T2.nominal().1),
                split(Self::T2.nominal().1, Self::T3.nominal().1),
            ),
            Self::T3 => (
                split(Self::T2.nominal().1, Self::T3.nominal().1),
                split(Self::T3.nominal().1, Self::T4.nominal().1),
            ),
            Self::T4 => (split(Self::T3.nominal().1, Self::T4.nominal().1), u64::MAX),
        }
    }

    /// The §7 full-extraction target and hard ceiling, in seconds.
    ///
    /// `None` for `T0`, which §7 does not give a row, and for `T4`, whose row is
    /// "unbounded; cancellable, progress-reporting, warned in advance" — a budget number for
    /// T4 would be inventing a requirement the PRD deliberately declined to state.
    #[must_use]
    pub const fn budget_seconds(self) -> Option<(u64, u64)> {
        match self {
            Self::T1 => Some((10, 30)),
            Self::T2 => Some((60, 180)),
            Self::T3 => Some((600, 1_800)),
            Self::T0 | Self::T4 => None,
        }
    }
}

/// The boundary between two tiers: the geometric mean of their nominal figures.
const fn split(lower: u64, upper: u64) -> u64 {
    (lower * upper).isqrt()
}

/// One pinned repository.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Pin {
    /// Stable short name; also its directory name in the cache.
    pub name: String,
    /// The tier this pin is meant to represent.
    pub tier: Tier,
    /// Why it was chosen, in a sentence.
    pub what: String,
    /// Where to fetch it from.
    pub url: String,
    /// The human-readable name of the pinned point. Never trusted for fetching.
    pub tag: String,
    /// The commit the measurement is taken at. This is the pin.
    pub commit: String,
}

/// Every pin, in declaration order.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Pins {
    /// Schema version.
    pub version: u32,
    /// The repositories.
    pub repositories: Vec<Pin>,
}

impl Pins {
    /// The pins compiled into this build.
    ///
    /// # Panics
    ///
    /// If `pins.ron` does not parse. It is compiled in, so that is a build-time defect rather
    /// than anything a user can cause.
    #[must_use]
    pub fn built_in() -> Self {
        ron::from_str(PINS_RON).expect("pins.ron is compiled in and must parse")
    }

    /// The pin with this name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Pin> {
        self.repositories.iter().find(|pin| pin.name == name)
    }
}

/// Where pinned repositories are cached: `target/corpus-pinned`.
///
/// Beside `target/corpus` and under `target/` for the same reason — a build artifact, not
/// something to check in. These are much larger, which makes it more true rather than less.
#[must_use]
pub fn default_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is `<root>/tools/corpus`.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map_or_else(
            || PathBuf::from("target/corpus-pinned"),
            |root| root.join("target/corpus-pinned"),
        )
}

/// What is on disk for a pin, without touching the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Presence {
    /// Present and sitting at the pinned commit.
    Pinned,
    /// Present but at a different commit — a stale or hand-edited cache.
    Drifted {
        /// What HEAD actually says.
        found: String,
    },
    /// The directory is there but is not a usable repository.
    Broken,
    /// Not fetched.
    Absent,
}

/// Reports what is on disk for `pin`. Never goes to the network.
#[must_use]
pub fn ensure(root: &Path, pin: &Pin) -> Presence {
    let path = root.join(&pin.name);
    if !path.join(".git").exists() {
        return if path.exists() {
            Presence::Broken
        } else {
            Presence::Absent
        };
    }
    match head_of(&path) {
        None => Presence::Broken,
        Some(found) if found == pin.commit => Presence::Pinned,
        Some(found) => Presence::Drifted { found },
    }
}

/// Fetches `pin` into `root` and checks out its commit. **This is the network call.**
///
/// Idempotent: a repository already at the pinned commit is left alone, and one that merely
/// lacks the commit is fetched into rather than re-cloned.
///
/// The clone is deliberately complete — no `--depth`, no `--filter=blob:none`. A shallow or
/// partial clone would make the budget meaningless twice over: extraction would be measuring a
/// truncated history, and a blobless clone would fetch objects *during* the measured pass,
/// putting network latency inside a figure that is supposed to be about local I/O.
///
/// # Errors
///
/// [`BuildError`] if git is missing, the fetch fails, or the repository does not have the
/// pinned commit.
pub fn fetch(root: &Path, pin: &Pin) -> Result<PathBuf, BuildError> {
    let path = root.join(&pin.name);
    if ensure(root, pin) == Presence::Pinned {
        return Ok(path);
    }
    std::fs::create_dir_all(root)?;

    if !path.join(".git").exists() {
        if path.exists() {
            std::fs::remove_dir_all(&path)?;
        }
        // `--no-checkout`, because the very next thing is a checkout of a specific commit and
        // materialising the default branch first would write every file twice.
        git(
            root,
            &["clone", "--quiet", "--no-checkout", &pin.url],
            Some(&path),
        )?;
    }

    // The commit may predate or postdate whatever the clone brought down.
    if !has_commit(&path, &pin.commit) {
        git(&path, &["fetch", "--quiet", "origin", &pin.commit], None).or_else(|_| {
            // Some servers refuse a fetch-by-SHA. Falling back to everything is slower and
            // always works.
            git(&path, &["fetch", "--quiet", "--tags", "origin"], None)
        })?;
    }
    if !has_commit(&path, &pin.commit) {
        return Err(BuildError::Git {
            args: vec!["fetch".to_owned(), pin.commit.clone()],
            stderr: format!(
                "`{}` does not have commit {} (tagged {}). The pin is wrong, or this history \
                 was rewritten — either way the measurement would not be the one it claims.",
                pin.url, pin.commit, pin.tag
            ),
        });
    }

    git(
        &path,
        &["checkout", "--quiet", "--detach", &pin.commit],
        None,
    )?;
    verify(&path, pin)?;
    Ok(path)
}

/// Confirms a fetched repository is at the pinned commit.
///
/// # Errors
///
/// [`BuildError::Git`] if it is not.
pub fn verify(path: &Path, pin: &Pin) -> Result<(), BuildError> {
    match head_of(path) {
        Some(found) if found == pin.commit => Ok(()),
        found => Err(BuildError::Git {
            args: vec!["rev-parse".to_owned(), "HEAD".to_owned()],
            stderr: format!(
                "`{}` is at {} but the pin is {}. A budget measured here would not be \
                 comparable to any other.",
                pin.name,
                found.as_deref().unwrap_or("an unreadable HEAD"),
                pin.commit
            ),
        }),
    }
}

fn head_of(path: &Path) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "HEAD"])
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn has_commit(path: &Path, commit: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["cat-file", "-e", &format!("{commit}^{{commit}}")])
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .is_ok_and(|out| out.status.success())
}

/// Runs git in `cwd`, optionally appending a destination path argument.
fn git(cwd: &Path, args: &[&str], destination: Option<&Path>) -> Result<(), BuildError> {
    let mut command = Command::new("git");
    command.current_dir(cwd).args(args);
    if let Some(destination) = destination {
        command.arg(destination);
    }
    // `GIT_TERMINAL_PROMPT=0` matters more here than anywhere else in this crate: a private
    // or moved URL would otherwise block on a credential prompt forever, in a command a
    // developer may well have left running.
    command.env("GIT_CONFIG_NOSYSTEM", "1");
    command.env("GIT_TERMINAL_PROMPT", "0");
    let output = command.output().map_err(BuildError::GitMissing)?;
    if output.status.success() {
        return Ok(());
    }
    Err(BuildError::Git {
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The compiled-in pins must parse, and must be internally coherent. Nothing here touches
    /// the network — this is the cheap check that runs in CI, where fetching gigabytes does
    /// not.
    #[test]
    fn the_pins_parse_and_are_well_formed() {
        let pins = Pins::built_in();
        assert_eq!(pins.version, 1);
        assert!(!pins.repositories.is_empty());

        for pin in &pins.repositories {
            assert_eq!(
                pin.commit.len(),
                40,
                "`{}` is pinned to `{}`, which is not a full SHA-1 — an abbreviated commit \
                 can become ambiguous as a repository grows",
                pin.name,
                pin.commit
            );
            assert!(
                pin.commit.bytes().all(|b| b.is_ascii_hexdigit()),
                "`{}` has a non-hex commit",
                pin.name
            );
            assert!(
                pin.commit.bytes().all(|b| !b.is_ascii_uppercase()),
                "`{}` has an upper-case commit; `git rev-parse` prints lower-case and the \
                 comparison is by string",
                pin.name
            );
            assert!(
                pin.url.starts_with("https://"),
                "`{}` is not https",
                pin.name
            );
            assert!(!pin.tag.is_empty() && !pin.what.is_empty());
        }
    }

    /// PRD §3 requires a real repository for T2 upward. T0 and T1 are allowed to be synthetic;
    /// the tiers with budgets are not.
    #[test]
    fn every_tier_with_a_budget_has_a_pin() {
        let pins = Pins::built_in();
        for tier in [Tier::T1, Tier::T2, Tier::T3] {
            assert!(
                pins.repositories.iter().any(|pin| pin.tier == tier),
                "no pinned repository for {} — its §7 budget could not be measured",
                tier.name()
            );
        }
    }

    #[test]
    fn names_are_unique_because_they_are_directory_names() {
        let pins = Pins::built_in();
        let mut names: Vec<&str> = pins.repositories.iter().map(|p| p.name.as_str()).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "two pins share a cache directory");
    }

    /// The bands are what `budget` checks a pin against, so a gap or an inversion between them
    /// would let a repository belong to no tier or to the wrong one.
    #[test]
    fn the_tier_bands_are_ordered_and_overlap() {
        for tier in Tier::ALL {
            let (lo, hi) = tier.files();
            assert!(lo < hi, "{} has an inverted file band", tier.name());
            let (lo, hi) = tier.commits();
            assert!(lo < hi, "{} has an inverted commit band", tier.name());
        }
        // The bands tile exactly: one tier's ceiling is the next one's floor, so no count
        // belongs to two tiers and none belongs to neither. This is the assertion that
        // caught the first attempt, where a fixed multiple around each nominal figure left
        // a 60-to-333-file hole that no tier claimed.
        for pair in Tier::ALL.windows(2) {
            assert_eq!(
                pair[0].files().1,
                pair[1].files().0,
                "files do not tile between {} and {}",
                pair[0].name(),
                pair[1].name()
            );
            assert_eq!(
                pair[0].commits().1,
                pair[1].commits().0,
                "commits do not tile between {} and {}",
                pair[0].name(),
                pair[1].name()
            );
        }
    }

    #[test]
    fn absent_is_reported_without_a_network_call() {
        let pins = Pins::built_in();
        let pin = &pins.repositories[0];
        let empty = std::env::temp_dir().join("treepo-pins-absent-check");
        let _ = std::fs::remove_dir_all(&empty);
        assert_eq!(ensure(&empty, pin), Presence::Absent);
    }
}
