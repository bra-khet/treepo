//! One shape per row of PRD §6, `F-CORP-2`, and `F-CORP-3`.
//!
//! Each entry names the requirement it exists for. A shape with no test is dead weight and a
//! PRD row with no shape is an untested claim, so the two lists are meant to be read side by
//! side — `tests/degenerate.rs` walks this list.
//!
//! # What is not here, and why
//!
//! * **T2 and T3.** PRD §3 specifies real public repositories pinned by commit SHA, not
//!   synthetic ones — a generated 20,000-commit repository would have the wrong *shape*, and
//!   the budgets in §7 are about real history. Pinning and caching those is separate work.
//! * **Symlinks on Windows, and non-UTF-8 names anywhere but Linux.** Symlinks need a
//!   permission Windows does not grant by default; non-UTF-8 names are rejected outright by
//!   macOS, whose filesystems require valid UTF-8 where Linux permits any byte. The shapes
//!   are built where they can be and skipped where they cannot, which [`Platforms`] records
//!   rather than hides.

use crate::{BuildError, Builder};
use std::path::Path;

/// Where a shape can be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platforms {
    /// Everywhere.
    All,
    /// Unix only — symlinks need either a permission or a filesystem Windows lacks.
    UnixOnly,
    /// Linux only — filenames that are not valid UTF-8.
    ///
    /// Not merely "not Windows": macOS rejects them at the syscall with `EILSEQ`, because
    /// APFS and HFS+ require valid UTF-8 where Linux permits any byte but `/` and NUL. The
    /// distinction is invisible until CI runs on all three, which is what it is for.
    LinuxOnly,
}

impl Platforms {
    /// Whether this shape can be built on the current platform.
    #[must_use]
    pub const fn available(self) -> bool {
        match self {
            Self::All => true,
            Self::UnixOnly => cfg!(unix),
            Self::LinuxOnly => cfg!(target_os = "linux"),
        }
    }
}

/// One fixture definition.
#[derive(Clone, Copy)]
pub struct Shape {
    /// Stable name and directory name.
    pub name: &'static str,
    /// The PRD row or requirement this exists for.
    pub covers: &'static str,
    /// Where it can be built.
    pub platforms: Platforms,
    /// Builds it.
    pub build: fn(&Path, &str) -> Result<(), BuildError>,
}

impl std::fmt::Debug for Shape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shape")
            .field("name", &self.name)
            .field("covers", &self.covers)
            .field("platforms", &self.platforms)
            .finish()
    }
}

/// Every shape, in a stable order.
#[must_use]
pub fn all_shapes() -> &'static [Shape] {
    &[
        Shape {
            name: "empty",
            covers: "PRD §6 Empty repository; AC-SKEL-2; F-MAN-3 tier 3",
            platforms: Platforms::All,
            build: build_empty,
        },
        Shape {
            name: "no-git",
            covers: "PRD §6 No .git; AC-ASSOC-3",
            platforms: Platforms::All,
            build: build_no_git,
        },
        Shape {
            name: "single-file",
            covers: "PRD §6 Single file",
            platforms: Platforms::All,
            build: build_single_file,
        },
        Shape {
            name: "single-author",
            covers: "PRD §6 Single author; F-CORP-2",
            platforms: Platforms::All,
            build: build_single_author,
        },
        Shape {
            name: "many-authors",
            covers: "PRD §6 1000+ authors; F-CORP-2",
            platforms: Platforms::All,
            build: build_many_authors,
        },
        Shape {
            name: "mailmap",
            covers: "F-EXT-9; AC-EXT-3; F-CORP-2",
            platforms: Platforms::All,
            build: build_mailmap,
        },
        Shape {
            name: "deep-nesting",
            covers: "PRD §6 Deep nesting >15; F-CORP-2",
            platforms: Platforms::All,
            build: build_deep_nesting,
        },
        Shape {
            name: "huge-file",
            covers: "PRD §6 One enormous file; P7; F-CORP-2",
            platforms: Platforms::All,
            build: build_huge_file,
        },
        Shape {
            name: "case-collision",
            covers: "PRD §6 Case-colliding paths",
            platforms: Platforms::All,
            build: build_case_collision,
        },
        Shape {
            name: "excluded-content",
            covers: "F-EXT-8; PRD §6 gitignored and vendored content",
            platforms: Platforms::All,
            build: build_excluded_content,
        },
        Shape {
            name: "detached-head",
            covers: "PRD §6 Detached HEAD",
            platforms: Platforms::All,
            build: build_detached_head,
        },
        Shape {
            name: "bare",
            covers: "PRD §6 Bare repository; F-ASSOC-2",
            platforms: Platforms::All,
            build: build_bare,
        },
        Shape {
            name: "shallow",
            covers: "PRD §6 Shallow clone; F-CORP-2; F-ASSOC-2",
            platforms: Platforms::All,
            build: build_shallow,
        },
        Shape {
            name: "no-remote",
            covers: "PRD §6 No remote; F-CORP-3; F-MAN-3 tier 2",
            platforms: Platforms::All,
            build: build_no_remote,
        },
        Shape {
            name: "multi-remote",
            covers: "PRD §6 Multiple remotes, no origin; F-CORP-3; F-MAN-3 tier 1",
            platforms: Platforms::All,
            build: build_multi_remote,
        },
        Shape {
            name: "symlinks",
            covers: "PRD §6 Symlinks",
            platforms: Platforms::UnixOnly,
            build: build_symlinks,
        },
        Shape {
            name: "non-utf8",
            covers: "PRD §6 Non-UTF8 paths; F-INSP-4",
            platforms: Platforms::LinuxOnly,
            build: build_non_utf8,
        },
    ]
}

/// A repository initialized but never committed to.
fn build_empty(root: &Path, name: &str) -> Result<(), BuildError> {
    Builder::init(root.to_path_buf(), name)?;
    Ok(())
}

/// A directory of files with no repository at all.
fn build_no_git(root: &Path, name: &str) -> Result<(), BuildError> {
    let builder = Builder::plain(root.to_path_buf(), name)?;
    builder.write_source("main.py", 40)?;
    builder.write_source("lib/helpers.py", 25)?;
    builder.write("README.md", b"# A folder, not a repository\n")?;
    Ok(())
}

/// One file, one commit. Minimal but valid structure.
fn build_single_file(root: &Path, name: &str) -> Result<(), BuildError> {
    let mut builder = Builder::init(root.to_path_buf(), name)?;
    builder.write("only.txt", b"the whole repository\n")?;
    builder.commit("the only commit")?;
    Ok(())
}

/// A small library with real history, all by one person.
fn build_single_author(root: &Path, name: &str) -> Result<(), BuildError> {
    let mut builder = Builder::init(root.to_path_buf(), name)?;
    builder.write("README.md", b"# single-author\n")?;
    builder.write_source("src/lib.rs", 60)?;
    builder.commit("initial")?;

    for step in 0..8 {
        builder.write_source(&format!("src/module_{step}.rs"), 30 + step * 5)?;
        builder.commit(&format!("add module {step}"))?;
    }
    // Churn on one file, so recency heat and burstiness have something to separate.
    for step in 0..4 {
        builder.write_source("src/lib.rs", 60 + step * 20)?;
        builder.commit(&format!("grow lib {step}"))?;
    }
    Ok(())
}

/// Many distinct contributors on one file, for palette and mosaic pressure.
///
/// Sixty rather than the thousand `F-CORP-2` names: the property under test is that shares
/// stay proportional and the minimum quota does not fragment a limb, and sixty exercises it
/// at a hundredth of the build time. A true 1000-author fixture belongs with the T2/T3
/// pinned repositories.
fn build_many_authors(root: &Path, name: &str) -> Result<(), BuildError> {
    let mut builder = Builder::init(root.to_path_buf(), name)?;
    builder.write("shared.txt", b"line 0\n")?;
    builder.commit("initial")?;

    for index in 0..60 {
        let mut text = String::new();
        for line in 0..=index {
            text.push_str(&format!("line {line}\n"));
        }
        builder.write("shared.txt", text.as_bytes())?;
        builder.commit_as(
            &format!("contribution {index}"),
            &format!("Contributor {index}"),
            &format!("person{index}@example.invalid"),
        )?;
    }
    Ok(())
}

/// One human, three addresses, and a `.mailmap` that says so (`AC-EXT-3`).
fn build_mailmap(root: &Path, name: &str) -> Result<(), BuildError> {
    let mut builder = Builder::init(root.to_path_buf(), name)?;
    builder.write(
        ".mailmap",
        b"Ada Lovelace <ada@example.invalid> <ada@work.example.invalid>\n\
          Ada Lovelace <ada@example.invalid> <a.lovelace@old.example.invalid>\n",
    )?;
    builder.write_source("src/engine.rs", 40)?;
    builder.commit_as("initial", "Ada Lovelace", "ada@example.invalid")?;

    builder.write_source("src/engine.rs", 60)?;
    builder.commit_as("from the office", "Ada L", "ada@work.example.invalid")?;

    builder.write_source("src/engine.rs", 80)?;
    builder.commit_as(
        "from years ago",
        "A. Lovelace",
        "a.lovelace@old.example.invalid",
    )?;

    // One genuinely different person, so the collapse is visible against a control.
    builder.write_source("src/other.rs", 20)?;
    builder.commit_as("someone else entirely", "Bob", "bob@example.invalid")?;
    Ok(())
}

/// Twenty levels of nesting — aggregation must engage without a stack overflow.
fn build_deep_nesting(root: &Path, name: &str) -> Result<(), BuildError> {
    let mut builder = Builder::init(root.to_path_buf(), name)?;
    let mut path = String::from("deep");
    for level in 0..20 {
        path.push_str(&format!("/level{level:02}"));
        builder.write_source(&format!("{path}/file.rs"), 5)?;
    }
    builder.write_source("shallow.rs", 10)?;
    builder.commit("a corridor")?;
    Ok(())
}

/// One file far larger than its siblings (`P7`'s soft clamp, PRD §6).
///
/// Eight megabytes rather than the fifty `F-CORP-2` names: the property under test is that
/// one file does not consume its parent's whole budget, which any sufficiently extreme
/// outlier exercises, and fifty megabytes in every clone of this repository's fixtures is a
/// poor trade for the same assertion.
fn build_huge_file(root: &Path, name: &str) -> Result<(), BuildError> {
    let mut builder = Builder::init(root.to_path_buf(), name)?;
    builder.write_source("src/small.rs", 30)?;
    builder.write_source("src/also_small.rs", 25)?;
    // Incompressible-ish content, so the pack does not make it disappear.
    let mut rng = treepo_det::Seed::root(b"treepo/corpus/huge").rng();
    let mut bulk = vec![0u8; 8 * 1024 * 1024];
    rng.fill_bytes(&mut bulk);
    builder.write("assets/enormous.bin", &bulk)?;
    builder.commit("a small project with one enormous asset")?;
    Ok(())
}

/// Two paths differing only in case — both tracked, neither may vanish.
fn build_case_collision(root: &Path, name: &str) -> Result<(), BuildError> {
    let mut builder = Builder::init(root.to_path_buf(), name)?;
    builder.write("Readme.md", b"one\n")?;
    builder.commit("first spelling")?;

    // On a case-insensitive filesystem the second write would land on the first file, so
    // the index is edited directly. This is the only way to *build* the collision that real
    // repositories acquire when contributors on different platforms disagree.
    let hash = {
        let path = root.join("readme-alt-content");
        std::fs::write(&path, b"two\n")?;
        let out = builder.git(&["hash-object", "-w", "readme-alt-content"])?;
        std::fs::remove_file(&path)?;
        out.trim().to_owned()
    };
    builder.git(&[
        "update-index",
        "--add",
        "--cacheinfo",
        &format!("100644,{hash},README.md"),
    ])?;
    // Not `commit`: `git add --all` would see the injected entry has no file in the working
    // tree on a case-sensitive filesystem and stage its removal, undoing the fixture.
    builder.commit_staged(
        "second spelling, differing only in case",
        "Corpus Builder",
        "corpus@treepo.invalid",
    )?;
    Ok(())
}

/// Gitignored output, a vendored directory, and a tracked dotfile (`F-EXT-8`).
fn build_excluded_content(root: &Path, name: &str) -> Result<(), BuildError> {
    let mut builder = Builder::init(root.to_path_buf(), name)?;
    builder.write(".gitignore", b"/build/\n*.log\n")?;
    builder.write(".gitattributes", b"vendor/** linguist-vendored\n")?;
    builder.write_source("src/main.rs", 40)?;
    builder.write_source("vendor/thirdparty/big.js", 200)?;
    builder.write_source("node_modules/left-pad/index.js", 15)?;
    // Present on disk, never tracked — the walk must not see these at all.
    builder.write("build/output.o", b"binary-ish\n")?;
    builder.write("debug.log", b"noise\n")?;
    builder.commit("a project with dependencies and output")?;

    // A file tracked *before* it was ignored, which git keeps tracking. The reason
    // `treepo-vcs::filter` reads the tree rather than matching ignore patterns.
    builder.write("legacy.log", b"tracked despite the pattern\n")?;
    builder.git(&["add", "--force", "legacy.log"])?;
    builder.commit("track a file the ignore file also matches")?;
    Ok(())
}

/// HEAD pointing at a commit rather than a branch.
fn build_detached_head(root: &Path, name: &str) -> Result<(), BuildError> {
    let mut builder = Builder::init(root.to_path_buf(), name)?;
    builder.write_source("src/main.rs", 20)?;
    builder.commit("first")?;
    builder.write_source("src/main.rs", 40)?;
    builder.commit("second")?;
    builder.git(&["checkout", "--quiet", "--detach", "HEAD~1"])?;
    Ok(())
}

/// A bare repository — rejected at association.
fn build_bare(root: &Path, name: &str) -> Result<(), BuildError> {
    // Built as a normal repository, then cloned bare, because a bare repository has no
    // working tree to write files into.
    let source = root.with_file_name(format!("{name}-source"));
    let mut builder = Builder::init(source.clone(), name)?;
    builder.write_source("src/main.rs", 20)?;
    builder.commit("first")?;

    if root.exists() {
        std::fs::remove_dir_all(root)?;
    }
    std::fs::create_dir_all(root)?;
    let holder = Builder::plain(root.to_path_buf(), name)?;
    holder.git(&[
        "clone",
        "--quiet",
        "--bare",
        source.to_str().unwrap_or_default(),
        ".",
    ])?;
    Ok(())
}

/// A `--depth 1` clone, whose history is truncated.
///
/// PRD §6 is emphatic about this row: "Silently producing a history-less tree is a defect —
/// this is common and would otherwise look like a bug."
fn build_shallow(root: &Path, name: &str) -> Result<(), BuildError> {
    let source = root.with_file_name(format!("{name}-source"));
    let mut builder = Builder::init(source.clone(), name)?;
    for step in 0..5 {
        builder.write_source("src/main.rs", 20 + step * 10)?;
        builder.commit(&format!("commit {step}"))?;
    }

    if root.exists() {
        std::fs::remove_dir_all(root)?;
    }
    std::fs::create_dir_all(root)?;
    let holder = Builder::plain(root.to_path_buf(), name)?;
    // `file://` rather than a plain path: git only honours `--depth` over a transport.
    let url = format!(
        "file:///{}",
        source.display().to_string().replace('\\', "/")
    );
    holder.git(&["clone", "--quiet", "--depth", "1", &url, "."])?;
    Ok(())
}

/// History, no remote configured (`F-MAN-3` tier 2).
fn build_no_remote(root: &Path, name: &str) -> Result<(), BuildError> {
    let mut builder = Builder::init(root.to_path_buf(), name)?;
    builder.write_source("src/main.rs", 30)?;
    builder.commit("first")?;
    Ok(())
}

/// Two remotes, neither called `origin` (`F-MAN-3` tier 1, alphabetically first).
fn build_multi_remote(root: &Path, name: &str) -> Result<(), BuildError> {
    let mut builder = Builder::init(root.to_path_buf(), name)?;
    builder.write_source("src/main.rs", 30)?;
    builder.commit("first")?;
    // Never fetched from; the URLs exist to be read.
    builder.git(&[
        "remote",
        "add",
        "upstream",
        "https://example.invalid/upstream.git",
    ])?;
    builder.git(&[
        "remote",
        "add",
        "backup",
        "https://example.invalid/backup.git",
    ])?;
    Ok(())
}

/// A symlink, which must be recorded and never followed.
fn build_symlinks(root: &Path, name: &str) -> Result<(), BuildError> {
    let mut builder = Builder::init(root.to_path_buf(), name)?;
    builder.write_source("real/target.rs", 20)?;
    // A symlink to its own parent: following it would loop forever.
    builder.git(&["config", "core.symlinks", "true"])?;
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("real", root.join("link-to-real"))?;
        std::os::unix::fs::symlink("..", root.join("real/loop"))?;
    }
    builder.commit("a link and a loop")?;
    Ok(())
}

/// A filename whose bytes are not valid UTF-8 (`F-INSP-4`).
fn build_non_utf8(root: &Path, name: &str) -> Result<(), BuildError> {
    let mut builder = Builder::init(root.to_path_buf(), name)?;
    builder.write_source("src/ok.rs", 10)?;
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt as _;
        let raw = std::ffi::OsStr::from_bytes(b"src/caf\xe9.rs");
        std::fs::write(root.join(raw), b"// latin-1 name\n")?;
    }
    builder.commit("a name that is not utf-8")?;
    Ok(())
}
