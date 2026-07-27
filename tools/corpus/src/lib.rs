//! The reference corpus — `F-CORP-2`, `F-CORP-3`, and every PRD §6 edge case.
//!
//! > The corpus is a build artifact: synthetic fixtures for T0–T1 generated
//! > deterministically in CI, plus real public repositories pinned by commit SHA for T2–T4.
//!
//! This crate is the first half. Each fixture is a real git repository built from scratch
//! into a scratch directory, and building it twice produces the same commit hashes — which
//! is what lets a test assert on an exact tree rather than on "something plausible".
//!
//! # Fixtures are built by `git`, not by `gix`
//!
//! `R1` says the *product* cannot assume a `git` binary exists, and architecture D3 chose
//! `gix` for exactly that reason. It does not say a developer-time fixture builder cannot
//! use one, and there is a positive argument for it: building fixtures with the same library
//! that reads them would make the tests circular. A `.mailmap` fixture that `gix` writes and
//! `gix` reads proves the two agree, not that either is right.
//!
//! So the fixtures are built by git and read by `treepo-vcs`, and a disagreement between
//! them is a real finding. This crate is never linked into the product; `cargo xtask
//! dep-guard` does not police `tools/`, and nothing under `crates/` depends on it.
//!
//! # Determinism
//!
//! Every commit fixes `GIT_AUTHOR_DATE`, `GIT_COMMITTER_DATE`, and both name/email pairs, so
//! commit hashes are a function of the fixture definition alone. Generated file content
//! comes from [`treepo_det::ChaCha8Rng`] seeded from the fixture's name, so a file's bytes
//! depend on where it sits and not on when it was made.
//!
//! Repository configuration is set explicitly rather than inherited — a developer whose
//! global `core.autocrlf` is on would otherwise build different blobs, and therefore
//! different commit hashes, than CI does.

use std::path::{Path, PathBuf};
use std::process::Command;
use treepo_det::Seed;

pub mod shapes;

pub use shapes::{Shape, all_shapes};

/// The fixed instant every fixture's first commit is dated.
///
/// 2021-01-01T00:00:00Z. Fixtures date commits forward from here, so "days ago" in a test is
/// relative to the fixture's own newest commit rather than to today — the same discipline
/// [`Manifest::reference_time`](treepo_model) applies to real repositories.
pub const EPOCH: i64 = 1_609_459_200;

/// Seconds between consecutive commits in a fixture, unless a shape says otherwise.
pub const COMMIT_INTERVAL: i64 = 86_400;

/// A built fixture.
#[derive(Debug, Clone)]
pub struct Fixture {
    /// The shape's stable name; also its directory name.
    pub name: &'static str,
    /// Where it was built.
    pub path: PathBuf,
}

/// Why a fixture could not be built.
#[derive(Debug)]
pub enum BuildError {
    /// `git` is not on `PATH`.
    ///
    /// Only ever a developer-machine problem: the product never shells out to git, and CI
    /// images all have it.
    GitMissing(std::io::Error),
    /// A git command failed.
    Git {
        /// The arguments passed.
        args: Vec<String>,
        /// What git printed.
        stderr: String,
    },
    /// The filesystem refused.
    Io(std::io::Error),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GitMissing(source) => write!(
                f,
                "`git` is required to build fixtures (the product never uses it): {source}"
            ),
            Self::Git { args, stderr } => write!(f, "git {} failed: {stderr}", args.join(" ")),
            Self::Io(source) => write!(f, "{source}"),
        }
    }
}

impl std::error::Error for BuildError {}

impl From<std::io::Error> for BuildError {
    fn from(source: std::io::Error) -> Self {
        Self::Io(source)
    }
}

/// A repository under construction.
///
/// Every method is deliberately small and explicit; a fixture reads as a list of what the
/// repository contains, which is what makes it reviewable against the PRD row it exists for.
#[derive(Debug)]
pub struct Builder {
    root: PathBuf,
    clock: i64,
    seed: Seed,
}

impl Builder {
    /// Creates an empty directory with no `.git` — PRD §6's "No `.git`" row.
    ///
    /// # Errors
    ///
    /// [`BuildError::Io`] if the directory cannot be created.
    pub fn plain(root: PathBuf, name: &str) -> Result<Self, BuildError> {
        if root.exists() {
            std::fs::remove_dir_all(&root)?;
        }
        std::fs::create_dir_all(&root)?;
        Ok(Self {
            root,
            clock: EPOCH,
            seed: Seed::root(b"treepo/corpus/v1").derive(name.as_bytes()),
        })
    }

    /// Creates an initialized repository with no commits — PRD §6's "no commits" row.
    ///
    /// # Errors
    ///
    /// [`BuildError`] if the directory cannot be created or `git init` fails.
    pub fn init(root: PathBuf, name: &str) -> Result<Self, BuildError> {
        let builder = Self::plain(root, name)?;
        // `main` explicitly: git's default branch name depends on the developer's
        // `init.defaultBranch`, and a fixture that changes name between machines is not a
        // fixture.
        builder.git(&["init", "--quiet", "--initial-branch=main"])?;
        // Configuration that would otherwise be inherited from the developer's global file
        // and change the blobs, and therefore the commit hashes.
        builder.git(&["config", "core.autocrlf", "false"])?;
        builder.git(&["config", "core.eol", "lf"])?;
        builder.git(&["config", "commit.gpgsign", "false"])?;
        builder.git(&["config", "diff.renames", "false"])?;
        builder.git(&["config", "user.name", "Corpus Builder"])?;
        builder.git(&["config", "user.email", "corpus@treepo.invalid"])?;
        Ok(builder)
    }

    /// Where the fixture lives.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Writes a file, creating parent directories.
    ///
    /// # Errors
    ///
    /// [`BuildError::Io`] if the write fails.
    pub fn write(&self, relative: &str, contents: &[u8]) -> Result<(), BuildError> {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, contents)?;
        Ok(())
    }

    /// Writes a file of `lines` plausible source lines, seeded from its own path.
    ///
    /// Content is generated rather than fixed so file sizes vary the way a real repository's
    /// do, and seeded from the path so the same path always holds the same bytes.
    ///
    /// # Errors
    ///
    /// [`BuildError::Io`] if the write fails.
    pub fn write_source(&self, relative: &str, lines: usize) -> Result<(), BuildError> {
        let mut rng = self.seed.derive(relative.as_bytes()).rng();
        let mut text = String::new();
        for index in 0..lines {
            match rng.below_u32(10) {
                0 => text.push('\n'),
                1 => text.push_str("// a comment about the code below\n"),
                _ => {
                    let width = 8 + rng.below_u32(40) as usize;
                    text.push_str("    let value_");
                    text.push_str(&index.to_string());
                    text.push_str(" = ");
                    text.push_str(&"x".repeat(width));
                    text.push_str(";\n");
                }
            }
        }
        self.write(relative, text.as_bytes())
    }

    /// Stages everything and commits, advancing the fixture clock.
    ///
    /// # Errors
    ///
    /// [`BuildError`] if a git command fails.
    pub fn commit(&mut self, message: &str) -> Result<(), BuildError> {
        self.commit_as(message, "Corpus Builder", "corpus@treepo.invalid")
    }

    /// Stages everything and commits under a named identity (`F-EXT-9`, `AC-EXT-3`).
    ///
    /// # Errors
    ///
    /// [`BuildError`] if a git command fails.
    pub fn commit_as(&mut self, message: &str, name: &str, email: &str) -> Result<(), BuildError> {
        self.git(&["add", "--all"])?;
        self.commit_staged(message, name, email)
    }

    /// Commits what is already staged, without touching the index.
    ///
    /// For fixtures whose index deliberately disagrees with the working tree — the
    /// case-collision shape builds an index entry that a case-insensitive filesystem cannot
    /// materialize, and `git add --all` on a case-*sensitive* one would stage its deletion.
    ///
    /// # Errors
    ///
    /// [`BuildError`] if the commit fails.
    pub fn commit_staged(
        &mut self,
        message: &str,
        name: &str,
        email: &str,
    ) -> Result<(), BuildError> {
        let date = format!("{} +0000", self.clock);
        self.git_env(
            &["commit", "--quiet", "--allow-empty", "-m", message],
            &[
                ("GIT_AUTHOR_NAME", name),
                ("GIT_AUTHOR_EMAIL", email),
                ("GIT_AUTHOR_DATE", &date),
                ("GIT_COMMITTER_NAME", name),
                ("GIT_COMMITTER_EMAIL", email),
                ("GIT_COMMITTER_DATE", &date),
            ],
        )?;
        self.clock += COMMIT_INTERVAL;
        Ok(())
    }

    /// Runs a git command in the fixture.
    ///
    /// # Errors
    ///
    /// [`BuildError`] if git is missing or the command fails.
    pub fn git(&self, args: &[&str]) -> Result<String, BuildError> {
        self.git_env(args, &[])
    }

    /// Runs a git command with extra environment.
    ///
    /// # Errors
    ///
    /// [`BuildError`] if git is missing or the command fails.
    pub fn git_env(&self, args: &[&str], env: &[(&str, &str)]) -> Result<String, BuildError> {
        let mut command = Command::new("git");
        command.arg("-C").arg(&self.root).args(args);
        // Keeps a developer's global config out of the fixture entirely.
        command.env("GIT_CONFIG_NOSYSTEM", "1");
        command.env("HOME", &self.root);
        command.env("GIT_TERMINAL_PROMPT", "0");
        for (key, value) in env {
            command.env(key, value);
        }
        let output = command.output().map_err(BuildError::GitMissing)?;
        if !output.status.success() {
            return Err(BuildError::Git {
                args: args.iter().map(|arg| (*arg).to_owned()).collect(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

/// Where fixtures are built unless a caller says otherwise: `target/corpus`.
///
/// Under `target/` because the fixtures are a build artifact (PRD §3), reproducible from
/// this crate rather than checked in — several are megabytes, and one of them is a
/// deliberate case collision that a case-insensitive checkout could not represent anyway.
#[must_use]
pub fn default_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is `<root>/tools/corpus`.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map_or_else(
            || PathBuf::from("target/corpus"),
            |root| root.join("target/corpus"),
        )
}

/// Builds every shape available on this platform into `root`, replacing what is there.
///
/// # Errors
///
/// [`BuildError`] if any fixture fails to build.
pub fn build_all(root: &Path) -> Result<Vec<Fixture>, BuildError> {
    std::fs::create_dir_all(root)?;
    let mut built = Vec::new();
    for shape in all_shapes() {
        if !shape.platforms.available() {
            continue;
        }
        built.push(build(root, *shape)?);
    }
    Ok(built)
}

/// Builds one shape into `root`.
///
/// # Errors
///
/// [`BuildError`] if the fixture fails to build.
pub fn build(root: &Path, shape: Shape) -> Result<Fixture, BuildError> {
    let path = root.join(shape.name);
    (shape.build)(&path, shape.name)?;
    Ok(Fixture {
        name: shape.name,
        path,
    })
}

/// Builds the corpus if it is not already present, and returns it.
///
/// Tests call this rather than [`build_all`] so a run does not pay to rebuild fixtures that
/// have not changed. The stamp file records the shape list, so adding a shape rebuilds.
///
/// # Errors
///
/// [`BuildError`] if the corpus needs building and cannot be built.
pub fn ensure(root: &Path) -> Result<Vec<Fixture>, BuildError> {
    let stamp = root.join("CORPUS-STAMP");
    let expected: String = all_shapes()
        .iter()
        .map(|shape| shape.name)
        .collect::<Vec<_>>()
        .join("\n");

    if std::fs::read_to_string(&stamp).is_ok_and(|found| found == expected) {
        return Ok(all_shapes()
            .iter()
            .map(|shape| Fixture {
                name: shape.name,
                path: root.join(shape.name),
            })
            .collect());
    }

    let built = build_all(root)?;
    std::fs::write(&stamp, expected)?;
    Ok(built)
}
