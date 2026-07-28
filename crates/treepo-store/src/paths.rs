//! `F-MAN-2` — the platform application-data root, and the layout beneath it.
//!
//! | Platform | Location |
//! |----------|----------|
//! | Windows | `%LOCALAPPDATA%\treepo\` |
//! | macOS | `~/Library/Application Support/treepo/` |
//! | Linux | `$XDG_DATA_HOME/treepo/`, falling back to `~/.local/share/treepo/` |
//!
//! ```text
//! <root>/
//!   settings.json                 # global settings
//!   repositories/
//!     <identity-hash>/
//!       identity.json             # resolved identity + how it was derived
//!       manifest.bin              # primitives, classifications, annotations
//!       manifest-meta.json        # schema_version, treepo_version, counts
//!       config.json               # per-repository settings
//!       world/                    # committed world state
//!       cache/                    # frame buffers, blame cache, derived render state
//! ```
//!
//! # This module builds paths and nothing else
//!
//! No accessor here creates a directory, stats a path, or opens a file. A [`RepositoryStore`]
//! for an identity treepo has never seen is a perfectly ordinary value naming a directory that
//! does not exist — which is what `F-MAN-8`'s "deleting the store costs time, never data"
//! looks like from this side. Creation belongs with the first write, where atomicity is the
//! question (`F-MAN-7`).
//!
//! # Why `cache/` is a sibling and not a subdirectory
//!
//! `F-MAN-13` will eventually evict above a size cap, and eviction must never touch
//! `manifest.bin`, `manifest-meta.json`, `config.json`, or `world/` — evicting those discards
//! work and silently loses per-repository settings and agent annotations (`F-MAN-12`). The
//! separation is what makes that policy expressible as "delete one directory" later rather
//! than as a per-file rule someone has to keep correct.

use std::path::{Path, PathBuf};
use treepo_model::identity::RepoIdentity;

/// The directory name treepo owns inside the platform application-data location.
const APP_DIR: &str = "treepo";

/// Why the platform application-data root could not be determined.
#[derive(Debug)]
pub enum LayoutError {
    /// The environment variable `F-MAN-2` names for this platform is unset or empty.
    ///
    /// Not a fallback case: guessing a location would put a user's data somewhere they will
    /// never find it, and somewhere a later fixed guess would orphan.
    NoAppData {
        /// The variable that was consulted.
        variable: &'static str,
        /// What the user can do about it.
        hint: &'static str,
    },
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoAppData { variable, hint } => write!(
                f,
                "cannot locate application data: {variable} is unset or empty. {hint}"
            ),
        }
    }
}

impl std::error::Error for LayoutError {}

/// The root of everything treepo stores (`F-MAN-2`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreRoot {
    root: PathBuf,
}

impl StoreRoot {
    /// The platform-conventional location.
    ///
    /// # Errors
    ///
    /// [`LayoutError::NoAppData`] when the platform's variable is unset or empty, or — on
    /// Linux — when `XDG_DATA_HOME` is unusable *and* `HOME` is unset.
    pub fn platform() -> Result<Self, LayoutError> {
        Ok(Self {
            root: platform_data_dir()?.join(APP_DIR),
        })
    }

    /// A root at an explicit path.
    ///
    /// Every test in the workspace uses this. A test that resolved the real application-data
    /// location would be writing into the developer's actual store, which is both a surprise
    /// and a way for one test run to change the next one's answers.
    #[must_use]
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The root directory itself.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// Global settings, shared by every repository.
    #[must_use]
    pub fn settings_file(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    /// The directory holding one subdirectory per known repository.
    ///
    /// `F-MAN-9` enumerates this to show the user everything treepo holds data for.
    #[must_use]
    pub fn repositories_dir(&self) -> PathBuf {
        self.root.join("repositories")
    }

    /// The store for one repository, named by its identity key.
    #[must_use]
    pub fn repository(&self, identity: &RepoIdentity) -> RepositoryStore {
        RepositoryStore {
            dir: self.repositories_dir().join(identity.directory_name()),
        }
    }
}

/// One repository's directory (`F-MAN-2`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryStore {
    dir: PathBuf,
}

impl RepositoryStore {
    /// The directory itself. Removing it is what `F-MAN-9`'s purge does.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The resolved identity and how it was derived — human-readable by design (`N2`).
    #[must_use]
    pub fn identity_file(&self) -> PathBuf {
        self.dir.join("identity.json")
    }

    /// The manifest: canonical binary, per architecture E2.
    #[must_use]
    pub fn manifest_file(&self) -> PathBuf {
        self.dir.join("manifest.bin")
    }

    /// The manifest's readable sidecar: `schema_version`, `treepo_version`, counts
    /// (`F-MAN-6`).
    #[must_use]
    pub fn manifest_meta_file(&self) -> PathBuf {
        self.dir.join("manifest-meta.json")
    }

    /// Per-repository settings: identity level, filter overrides.
    #[must_use]
    pub fn config_file(&self) -> PathBuf {
        self.dir.join("config.json")
    }

    /// Committed world state.
    #[must_use]
    pub fn world_dir(&self) -> PathBuf {
        self.dir.join("world")
    }

    /// Regenerable derived state: frame buffers, blame cache, render state.
    ///
    /// The only directory `F-MAN-13`'s eventual eviction policy may touch.
    #[must_use]
    pub fn cache_dir(&self) -> PathBuf {
        self.dir.join("cache")
    }
}

/// The platform application-data directory that treepo's own directory sits inside.
#[cfg(windows)]
fn platform_data_dir() -> Result<PathBuf, LayoutError> {
    non_empty("LOCALAPPDATA").ok_or(LayoutError::NoAppData {
        variable: "LOCALAPPDATA",
        hint: "This is set by Windows for every interactive user; a service or container \
               account may need it set explicitly.",
    })
}

#[cfg(target_os = "macos")]
fn platform_data_dir() -> Result<PathBuf, LayoutError> {
    home()
        .map(|home| home.join("Library").join("Application Support"))
        .ok_or(LayoutError::NoAppData {
            variable: "HOME",
            hint: "Set HOME to the account's home directory.",
        })
}

#[cfg(not(any(windows, target_os = "macos")))]
fn platform_data_dir() -> Result<PathBuf, LayoutError> {
    // XDG requires an absolute path and says a relative one must be ignored, so a stray
    // `XDG_DATA_HOME=.` falls through to the documented default rather than scattering
    // stores across whatever directory treepo happened to launch from.
    if let Some(xdg) = non_empty("XDG_DATA_HOME")
        && xdg.is_absolute()
    {
        return Ok(xdg);
    }
    home()
        .map(|home| home.join(".local").join("share"))
        .ok_or(LayoutError::NoAppData {
            variable: "XDG_DATA_HOME or HOME",
            hint: "Set one of them; XDG_DATA_HOME must be an absolute path.",
        })
}

#[cfg(not(windows))]
fn home() -> Option<PathBuf> {
    non_empty("HOME")
}

/// An environment variable's value, treating empty as unset.
fn non_empty(variable: &str) -> Option<PathBuf> {
    usable(std::env::var_os(variable))
}

/// The rule, separated from the lookup so it can be tested without mutating the process
/// environment — which `set_var` is `unsafe` for in edition 2024, and which this crate
/// forbids outright.
///
/// Empty means unset for all three of these variables: it is what a shell leaves behind when
/// one is cleared, and joining `treepo` onto an empty path yields a relative directory that
/// would follow the working directory around.
fn usable(value: Option<std::ffi::OsString>) -> Option<PathBuf> {
    value.filter(|value| !value.is_empty()).map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use treepo_model::identity::IdentityTier;

    fn identity() -> RepoIdentity {
        RepoIdentity::new(
            IdentityTier::Remote,
            "github.com/bra-khet/treepo".to_string(),
            0,
        )
    }

    #[test]
    fn the_layout_matches_f_man_2() {
        let root = StoreRoot::at("/data/treepo");
        let store = root.repository(&identity());

        assert_eq!(
            root.settings_file(),
            Path::new("/data/treepo/settings.json")
        );
        assert_eq!(
            root.repositories_dir(),
            Path::new("/data/treepo/repositories")
        );

        let dir = root.repositories_dir().join(identity().directory_name());
        assert_eq!(store.dir(), dir);
        assert_eq!(store.identity_file(), dir.join("identity.json"));
        assert_eq!(store.manifest_file(), dir.join("manifest.bin"));
        assert_eq!(store.manifest_meta_file(), dir.join("manifest-meta.json"));
        assert_eq!(store.config_file(), dir.join("config.json"));
        assert_eq!(store.world_dir(), dir.join("world"));
        assert_eq!(store.cache_dir(), dir.join("cache"));
    }

    /// The directory name is the identity, so the same repository is the same directory
    /// wherever it is opened from (`AC-MAN-4`, `AC-MAN-5`).
    #[test]
    fn the_directory_name_is_the_identity_key() {
        let root = StoreRoot::at("/data/treepo");
        let store = root.repository(&identity());
        let name = store
            .dir()
            .file_name()
            .and_then(|name| name.to_str())
            .expect("a directory name");

        assert_eq!(name, identity().directory_name());
        assert_eq!(name.len(), 64, "SHA-256 as lowercase hex");
        assert!(
            name.bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        );
    }

    /// `resolved_at` is housekeeping and must not move a repository's data.
    #[test]
    fn reopening_later_addresses_the_same_directory() {
        let root = StoreRoot::at("/data/treepo");
        let first = RepoIdentity::new(IdentityTier::RootCommit, "abc123".to_string(), 0);
        let later = RepoIdentity::new(
            IdentityTier::RootCommit,
            "abc123".to_string(),
            1_777_000_000,
        );
        assert_eq!(root.repository(&first), root.repository(&later));
    }

    /// `F-MAN-13` may evict `cache/` and must never reach anything beside it.
    #[test]
    fn evictable_state_is_separable_from_durable_state() {
        let root = StoreRoot::at("/data/treepo");
        let store = root.repository(&identity());
        let cache = store.cache_dir();
        for durable in [
            store.identity_file(),
            store.manifest_file(),
            store.manifest_meta_file(),
            store.config_file(),
            store.world_dir(),
        ] {
            assert!(
                !durable.starts_with(&cache),
                "{} must survive cache eviction",
                durable.display()
            );
        }
    }

    /// Building a path must not create one — the whole module is addressing.
    #[test]
    fn addressing_a_store_touches_no_filesystem() {
        let root = StoreRoot::at(std::env::temp_dir().join("treepo-paths-must-not-exist-4f1c8a"));
        let store = root.repository(&identity());
        let _ = store.manifest_file();
        let _ = store.world_dir();
        let _ = root.settings_file();
        assert!(!root.path().exists(), "no accessor may create anything");
    }

    /// Whatever the platform, the root is treepo's own directory rather than the bare
    /// application-data location — one `treepo`, not zero and not two.
    #[test]
    fn the_platform_root_is_named_and_absolute() {
        let Ok(root) = StoreRoot::platform() else {
            // A CI container with no HOME is a legitimate environment; the error path is
            // covered by `non_empty` treating empty as unset.
            return;
        };
        assert_eq!(
            root.path().file_name().and_then(|n| n.to_str()),
            Some("treepo")
        );
        assert!(root.path().is_absolute());
        assert_ne!(
            root.path().parent().and_then(|p| p.file_name()),
            Some(std::ffi::OsStr::new("treepo")),
            "the app-data root already ends in `treepo`; nesting it again is the F-MAN-2 \
             table read twice"
        );
    }

    #[test]
    fn an_empty_variable_is_an_unset_variable() {
        use std::ffi::OsString;

        assert_eq!(usable(None), None);
        assert_eq!(usable(Some(OsString::new())), None);
        assert_eq!(
            usable(Some(OsString::from("/somewhere"))),
            Some(PathBuf::from("/somewhere"))
        );
    }
}
