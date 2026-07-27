//! [`RepoPath`] — a repository-relative path that survives what real repositories contain.
//!
//! Three properties matter here, and `std::path::Path` has none of them.
//!
//! 1. **Bytes, not `String`.** Git stores paths as bytes, and a POSIX filesystem permits any
//!    byte except `/` and NUL. A path that is not valid UTF-8 is not an error to reject —
//!    PRD §6 requires it be preserved exactly and displayed lossily, and `F-INSP-4` promises
//!    inspection shows the raw path. Storing a `String` would force the lossy conversion at
//!    extraction time, destroying the thing inspection is supposed to reveal.
//! 2. **`/` always, on every platform.** `Path` compares and renders with the platform
//!    separator, so the same repository yields different path bytes on Windows. `P2` seeds
//!    generation from path hashes, which makes a platform-dependent path a
//!    platform-dependent *tree* — an `AC-DET-2` failure with a cause nobody would look for.
//! 3. **Byte ordering.** [`Ord`] is a plain byte comparison: not locale-aware, not
//!    case-insensitive, not Unicode-normalizing. All three of those vary by machine, and
//!    `AC-DET-3` requires the walk order be a property of the repository alone.
//!
//! # Case-colliding paths
//!
//! `README.md` and `readme.md` are two distinct `RepoPath`s. That is deliberate: git tracks
//! both, and on a case-insensitive filesystem only one of them exists in the working tree.
//! Collapsing them here would make one vanish; treating them as unrelated would double-count
//! the bytes on disk. [`RepoPath::case_fold_key`] gives the walk what it needs to detect the
//! collision and resolve it deterministically (PRD §6), without this type pretending the two
//! paths are the same.

use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt;

/// Why a byte string is not a usable repository path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathError {
    /// Contains a NUL byte, which no filesystem and no git tree entry permits.
    InteriorNul,
    /// Starts with `/`. Repository paths are relative to the repository root.
    Absolute,
    /// Ends with `/`. Directory-ness is [`NodeKind`](crate::NodeKind), not punctuation.
    TrailingSlash,
    /// Contains `//`, an empty component.
    EmptyComponent,
    /// Contains a `.` or `..` component. Git never emits these and honouring one would let
    /// a crafted path address something outside the repository.
    DotComponent,
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::InteriorNul => "path contains a NUL byte",
            Self::Absolute => "path is absolute; repository paths are root-relative",
            Self::TrailingSlash => "path has a trailing slash",
            Self::EmptyComponent => "path contains an empty component",
            Self::DotComponent => "path contains a `.` or `..` component",
        };
        f.write_str(text)
    }
}

impl core::error::Error for PathError {}

/// A repository-relative path, stored as `/`-separated bytes.
///
/// The empty path is the repository root and is the only path with zero components.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct RepoPath {
    /// Never has a leading or trailing `/`, never contains `//`, `.`, `..`, or NUL.
    raw: Box<[u8]>,
}

impl RepoPath {
    /// The repository root: zero components, depth 0.
    #[must_use]
    pub fn root() -> Self {
        Self {
            raw: Box::default(),
        }
    }

    /// Builds a path from `/`-separated bytes, as git stores them.
    ///
    /// Empty input is the repository [`root`](Self::root).
    ///
    /// # Errors
    ///
    /// Returns [`PathError`] if the bytes are not a well-formed root-relative path. This is
    /// the boundary check: everything past construction may assume the invariants hold.
    pub fn new(bytes: &[u8]) -> Result<Self, PathError> {
        if bytes.is_empty() {
            return Ok(Self::root());
        }
        if bytes.contains(&0) {
            return Err(PathError::InteriorNul);
        }
        if bytes[0] == b'/' {
            return Err(PathError::Absolute);
        }
        if bytes[bytes.len() - 1] == b'/' {
            return Err(PathError::TrailingSlash);
        }
        for component in bytes.split(|&b| b == b'/') {
            if component.is_empty() {
                return Err(PathError::EmptyComponent);
            }
            if component == b"." || component == b".." {
                return Err(PathError::DotComponent);
            }
        }
        Ok(Self { raw: bytes.into() })
    }

    /// The raw bytes, exactly as the repository stores them. `F-INSP-4` shows these.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.raw
    }

    /// Whether this is the repository root.
    #[must_use]
    pub fn is_root(&self) -> bool {
        self.raw.is_empty()
    }

    /// Distance from the repository root. The root itself is 0.
    #[must_use]
    pub fn depth(&self) -> u16 {
        if self.is_root() {
            return 0;
        }
        // One more than the number of separators, saturating: a path 65k levels deep is
        // past every budget in PRD §7 and clamping beats wrapping to a shallow depth.
        let separators = self.raw.iter().filter(|&&b| b == b'/').count();
        u16::try_from(separators)
            .unwrap_or(u16::MAX - 1)
            .min(u16::MAX - 1)
            + 1
    }

    /// The components, root-first. Empty for the repository root.
    pub fn components(&self) -> impl Iterator<Item = &[u8]> {
        // `split` on an empty slice yields one empty component; the root has none.
        let raw: &[u8] = if self.is_root() { &[] } else { &self.raw };
        raw.split(|&b| b == b'/')
            .filter(|component| !component.is_empty())
    }

    /// The final component, or `None` for the repository root.
    #[must_use]
    pub fn file_name(&self) -> Option<&[u8]> {
        if self.is_root() {
            return None;
        }
        match self.raw.iter().rposition(|&b| b == b'/') {
            Some(index) => Some(&self.raw[index + 1..]),
            None => Some(&self.raw),
        }
    }

    /// The containing directory, or `None` for the repository root.
    ///
    /// A top-level path's parent is the root, not `None`.
    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        if self.is_root() {
            return None;
        }
        Some(match self.raw.iter().rposition(|&b| b == b'/') {
            Some(index) => Self {
                raw: self.raw[..index].into(),
            },
            None => Self::root(),
        })
    }

    /// Appends one component.
    ///
    /// # Errors
    ///
    /// Returns [`PathError`] if `component` is empty, contains `/` or NUL, or is `.`/`..`.
    pub fn join(&self, component: &[u8]) -> Result<Self, PathError> {
        if component.is_empty() {
            return Err(PathError::EmptyComponent);
        }
        if component.contains(&b'/') {
            return Err(PathError::EmptyComponent);
        }
        let mut raw = Vec::with_capacity(self.raw.len() + 1 + component.len());
        if !self.is_root() {
            raw.extend_from_slice(&self.raw);
            raw.push(b'/');
        }
        raw.extend_from_slice(component);
        Self::new(&raw)
    }

    /// Whether `self` is `ancestor` or lies beneath it. Every path descends from the root.
    #[must_use]
    pub fn starts_with(&self, ancestor: &Self) -> bool {
        if ancestor.is_root() {
            return true;
        }
        if self.raw.len() < ancestor.raw.len() {
            return false;
        }
        if self.raw[..ancestor.raw.len()] != *ancestor.raw {
            return false;
        }
        // Guards against `srcfoo` matching `src`: the boundary must be a separator.
        self.raw.len() == ancestor.raw.len() || self.raw[ancestor.raw.len()] == b'/'
    }

    /// The extension of the final component, without the dot.
    ///
    /// A leading dot is a hidden file, not an extension: `.gitignore` has none. `tar.gz`
    /// yields `gz` — the longest-suffix question belongs to `treepo-vcs::lang`, which has
    /// the dictionary to answer it.
    #[must_use]
    pub fn extension(&self) -> Option<&[u8]> {
        let name = self.file_name()?;
        let dot = name.iter().rposition(|&b| b == b'.')?;
        if dot == 0 || dot + 1 == name.len() {
            return None;
        }
        Some(&name[dot + 1..])
    }

    /// A key equal for paths that collide on a case-insensitive filesystem.
    ///
    /// ASCII-only folding, deliberately. Real case-insensitive filesystems fold far more
    /// than ASCII — HFS+ and NTFS disagree with each other about Unicode, and both drag in a
    /// table that would have to be versioned as a determinism input. ASCII covers the
    /// collisions that actually occur in source trees, needs no table, and behaves
    /// identically everywhere. Non-ASCII collisions read as two distinct paths, which is the
    /// safe direction: PRD §6 forbids a *vanished* node, not a redundant one.
    #[must_use]
    pub fn case_fold_key(&self) -> Box<[u8]> {
        self.raw.iter().map(u8::to_ascii_lowercase).collect()
    }

    /// The path as text, replacing invalid UTF-8 with U+FFFD.
    ///
    /// Borrows when the path is already valid UTF-8, which is nearly always.
    #[must_use]
    pub fn display(&self) -> Cow<'_, str> {
        // No `alloc::string::String::from_utf8_lossy` import needed; the inherent form on
        // `String` is the allocating one and this borrows in the common case.
        alloc::string::String::from_utf8_lossy(&self.raw)
    }

    /// Whether the raw bytes are valid UTF-8.
    ///
    /// The UI shows a lossy name either way; this is what tells it to also offer the raw
    /// bytes (PRD §6, `F-INSP-4`).
    #[must_use]
    pub fn is_utf8(&self) -> bool {
        core::str::from_utf8(&self.raw).is_ok()
    }
}

/// Renders the lossy form. Use [`RepoPath::as_bytes`] when the exact path matters.
impl fmt::Display for RepoPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_root() {
            return f.write_str("<root>");
        }
        f.write_str(&self.display())
    }
}

/// Shows the lossy form and flags a path whose bytes are not valid UTF-8, so a debug dump
/// of a manifest does not silently look like it round-tripped when it did not.
impl fmt::Debug for RepoPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RepoPath({:?}", self.display())?;
        if !self.is_utf8() {
            f.write_str(", non-utf8")?;
        }
        f.write_str(")")
    }
}

impl core::str::FromStr for RepoPath {
    type Err = PathError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn path(s: &str) -> RepoPath {
        RepoPath::new(s.as_bytes()).expect("valid test path")
    }

    #[test]
    fn root_is_the_empty_path() {
        let root = RepoPath::root();
        assert!(root.is_root());
        assert_eq!(root.depth(), 0);
        assert_eq!(root.file_name(), None);
        assert_eq!(root.parent(), None);
        assert_eq!(root.components().count(), 0);
        assert_eq!(RepoPath::new(b"").unwrap(), root);
    }

    #[test]
    fn depth_counts_components() {
        assert_eq!(path("src").depth(), 1);
        assert_eq!(path("src/main.rs").depth(), 2);
        assert_eq!(path("a/b/c/d/e").depth(), 5);
    }

    #[test]
    fn parent_walks_to_root_and_stops() {
        let mut current = Some(path("a/b/c"));
        let mut seen = vec![];
        while let Some(p) = current {
            seen.push(p.to_string());
            current = p.parent();
        }
        assert_eq!(seen, ["a/b/c", "a/b", "a", "<root>"]);
    }

    #[test]
    fn malformed_paths_are_rejected_at_the_boundary() {
        assert_eq!(RepoPath::new(b"/abs"), Err(PathError::Absolute));
        assert_eq!(RepoPath::new(b"trailing/"), Err(PathError::TrailingSlash));
        assert_eq!(RepoPath::new(b"a//b"), Err(PathError::EmptyComponent));
        assert_eq!(RepoPath::new(b"a/./b"), Err(PathError::DotComponent));
        assert_eq!(RepoPath::new(b"a/../b"), Err(PathError::DotComponent));
        assert_eq!(RepoPath::new(b"a\0b"), Err(PathError::InteriorNul));
    }

    #[test]
    fn dotfiles_are_not_dot_components() {
        assert_eq!(path(".gitignore").file_name(), Some(&b".gitignore"[..]));
        assert_eq!(path("a/..b").depth(), 2);
    }

    /// PRD §6: non-UTF8 paths keep their bytes and gain a lossy display name.
    #[test]
    fn non_utf8_paths_survive_intact() {
        let raw = b"src/caf\xe9.rs";
        let p = RepoPath::new(raw).unwrap();
        assert_eq!(p.as_bytes(), raw);
        assert!(!p.is_utf8());
        assert_eq!(p.display(), "src/caf\u{fffd}.rs");
        assert_eq!(p.file_name(), Some(&b"caf\xe9.rs"[..]));
        assert_eq!(p.extension(), Some(&b"rs"[..]));
    }

    /// The whole reason this type is not `std::path::Path`: ordering must not vary.
    #[test]
    fn ordering_is_byte_order() {
        let mut paths = [
            path("src/z.rs"),
            path("Src/a.rs"),
            path("src/a.rs"),
            path("src-gen/a.rs"),
        ];
        paths.sort();
        let rendered: Vec<_> = paths.iter().map(RepoPath::to_string).collect();
        // 'S' < 's' and '-' (0x2d) < '/' (0x2f). Both are byte facts, not locale facts.
        assert_eq!(
            rendered,
            ["Src/a.rs", "src-gen/a.rs", "src/a.rs", "src/z.rs"]
        );
    }

    /// PRD §6: case-colliding paths stay distinct, but are detectable.
    #[test]
    fn case_collisions_are_detectable_without_being_merged() {
        let upper = path("README.md");
        let lower = path("readme.md");
        assert_ne!(upper, lower);
        assert_eq!(upper.case_fold_key(), lower.case_fold_key());
    }

    #[test]
    fn starts_with_respects_component_boundaries() {
        let file = path("src/main.rs");
        assert!(file.starts_with(&path("src")));
        assert!(file.starts_with(&RepoPath::root()));
        assert!(file.starts_with(&file));
        assert!(!file.starts_with(&path("src-gen")));
        assert!(!path("srcfoo/x").starts_with(&path("src")));
    }

    #[test]
    fn join_builds_from_the_root_down() {
        let built = RepoPath::root()
            .join(b"src")
            .unwrap()
            .join(b"main.rs")
            .unwrap();
        assert_eq!(built, path("src/main.rs"));
        assert_eq!(
            RepoPath::root().join(b"a/b"),
            Err(PathError::EmptyComponent)
        );
        assert_eq!(RepoPath::root().join(b".."), Err(PathError::DotComponent));
    }

    #[test]
    fn extension_ignores_leading_and_trailing_dots() {
        assert_eq!(path("src/main.rs").extension(), Some(&b"rs"[..]));
        assert_eq!(path("archive.tar.gz").extension(), Some(&b"gz"[..]));
        assert_eq!(path(".gitignore").extension(), None);
        assert_eq!(path("Makefile").extension(), None);
        assert_eq!(path("trailing.").extension(), None);
    }
}
