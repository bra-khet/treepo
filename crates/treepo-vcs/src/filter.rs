//! `F-EXT-8` — deciding what counts as the repository's structure.
//!
//! Five rules, and this module implements four of them:
//!
//! 1. **Always skip `.git/` and `.treepo/`.** Unconditional; no override reaches them.
//! 2. **Honour the repository's `.gitignore`.** Satisfied *by construction* rather than by
//!    matching patterns — see below.
//! 3. **Apply a built-in default exclusion set.** `assets/filters/default-exclusions.ron`,
//!    compiled in by [`FilterSet::built_in`].
//! 4. **Detect generated and vendored content via `.gitattributes` `linguist-*` markers.**
//!    In [`lang::Attributes`](crate::lang::Attributes), not here, and deliberately: a marker
//!    changes what a path *is*, not whether it is structure. Vendored code that this module
//!    dropped would be missing from the tree; classified as
//!    [`Generated`](treepo_model::primitives::size::ContentCategory::Generated) it is present
//!    and rendered in the "machined" material `design/feature-system.md` §8.5 describes,
//!    which is what the repository asked for by marking it.
//! 5. **Overridable per repository.** [`FilterOverrides`] from the manifest, applied here.
//!
//! # Why `.gitignore` needs no pattern matching
//!
//! `F-EXT-7` fixes the lens at `local_committed` — structure comes from the HEAD tree, and a
//! git tree contains tracked files and nothing else. Ignored files are absent before this
//! module sees anything.
//!
//! That is not a shortcut, it is the *more correct* reading of rule 2. `.gitignore` does not
//! apply to already-tracked files: commit `config.local`, then add it to `.gitignore`, and
//! git keeps tracking it. Matching ignore patterns against tree paths would drop a file git
//! itself considers part of the repository. Walking the tree gets this right without trying.
//!
//! The exception is a directory with no `.git` at all (PRD §6), where [`walk`](crate::walk)
//! falls back to the filesystem. A `.gitignore` there has no repository to be authoritative
//! for, and [`FilterSet::use_gitignore`] is recorded but currently unenforced on that path —
//! flagged rather than silently ignored, because it is the one place rule 2 is not free.
//!
//! # Matching
//!
//! Gitignore-flavoured, and deliberately a simplification of it:
//!
//! * A pattern without `/` matches a path *component* at any depth — `target` catches
//!   `crates/x/target`.
//! * A pattern with `/` is anchored at the repository root — `docs/_build` catches only that
//!   one.
//! * Either way, matching a prefix of the path excludes everything beneath it, so a rule
//!   naming a directory does not also need a rule for its contents.
//! * `*` and `?` glob within one component; `**` spans components.
//!
//! Matching is case-sensitive, unlike git's `core.ignorecase` on macOS and Windows. `N3`
//! decides this: honouring `core.ignorecase` would make the same repository produce a
//! different tree depending on the platform it was cloned on, which is the exact failure
//! `AC-DET-2` exists to catch. A case-mismatched exclusion is a visible, fixable surprise;
//! a platform-dependent tree is neither.

use gix::bstr::{BStr, ByteSlice};
use gix::glob::wildmatch;
use serde::Deserialize;
use treepo_model::manifest::FilterOverrides;
use treepo_model::path::RepoPath;

/// `*` and `?` stop at a `/`, and `**` spans components. Git's own semantics.
const MATCH_MODE: wildmatch::Mode = wildmatch::Mode::NO_MATCH_SLASH_LITERAL;

/// Components no override can bring back.
///
/// `.git` is treepo's own read boundary and `.treepo` is treepo's own store (`F-MAN-10`);
/// rendering either would be showing the user the instrument rather than the subject.
const ALWAYS_SKIP: [&str; 2] = [".git", ".treepo"];

/// The built-in exclusion set, compiled in from the assets tree.
///
/// `include_str!` rather than a runtime read: `treepo-vcs` has no business resolving asset
/// paths — that is the application shell's job from Phase 5 — and an extraction that fails
/// because an asset file moved would be a poor trade for a file that changes rarely.
/// [`FilterSet::from_ron`] is the extension point for supplying a different set, which is
/// how `F-SET-3`'s per-repository overrides and Phase 10's settings will reach it.
const BUILT_IN_RON: &str = include_str!("../../../assets/filters/default-exclusions.ron");

/// One themed group of exclusion patterns, with the explanation shown to the user.
#[derive(Debug, Clone, Deserialize)]
pub struct ExclusionGroup {
    /// Stable identifier, used by `F-SET-3` to enable or disable the group.
    pub name: String,
    /// Why these paths are not the repository's structure. User-facing.
    pub reason: String,
    /// The patterns themselves.
    pub patterns: Vec<String>,
}

/// The parsed contents of `assets/filters/default-exclusions.ron`.
#[derive(Debug, Clone, Deserialize)]
pub struct DefaultExclusions {
    /// Format version, so a future shape change is detectable rather than mis-parsed.
    pub version: u32,
    /// The groups, in file order.
    pub groups: Vec<ExclusionGroup>,
}

/// Why a path was excluded, in terms a user can act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision<'a> {
    /// The path is part of the repository's structure.
    Keep,
    /// The path was excluded by a rule.
    Excluded {
        /// The pattern that matched.
        pattern: &'a str,
        /// The group it came from — `"always"` or `"user"` for the two synthetic ones.
        group: &'a str,
        /// The group's user-facing explanation.
        reason: &'a str,
    },
}

impl Decision<'_> {
    /// Whether the path survives filtering.
    #[must_use]
    pub const fn is_kept(&self) -> bool {
        matches!(self, Self::Keep)
    }
}

/// Where a rule came from, for reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Origin {
    /// `.git` and `.treepo`.
    Always,
    /// A group in the default set, by index.
    Default(usize),
    /// The user's own exclusion (`F-SET-3`).
    User,
}

/// One compiled pattern.
#[derive(Debug, Clone)]
struct Rule {
    pattern: String,
    /// Whether the pattern contains a `/` and is therefore anchored at the root.
    anchored: bool,
    origin: Origin,
}

impl Rule {
    fn new(pattern: &str, origin: Origin) -> Self {
        Self {
            pattern: pattern.to_owned(),
            anchored: pattern.contains('/'),
            origin,
        }
    }

    /// Whether this rule matches `path` or any directory above it.
    ///
    /// Matching a prefix is what makes a rule naming a directory also cover its contents,
    /// so the exclusion file does not need a `foo` and a `foo/**` entry for every directory.
    fn matches(&self, path: &RepoPath) -> bool {
        let raw = path.as_bytes();
        let pattern: &BStr = self.pattern.as_bytes().as_bstr();

        if self.anchored {
            // Every prefix ending at a component boundary, plus the whole path.
            return raw
                .iter()
                .enumerate()
                .filter_map(|(index, &byte)| (byte == b'/').then_some(&raw[..index]))
                .chain(core::iter::once(raw))
                .any(|prefix| wildmatch(pattern, prefix.as_bstr(), MATCH_MODE));
        }

        // Unanchored: any single component, at any depth. Matching a component *is*
        // matching a prefix's last segment, so descendants are covered the same way.
        path.components()
            .any(|component| wildmatch(pattern, component.as_bstr(), MATCH_MODE))
    }
}

/// The filtering rules in force for one extraction (`F-EXT-8`).
#[derive(Debug, Clone)]
pub struct FilterSet {
    groups: Vec<ExclusionGroup>,
    rules: Vec<Rule>,
    re_include: Vec<Rule>,
    /// Recorded from the manifest so [`walk`](crate::walk) can report it; see the module
    /// header for where it currently does and does not bite.
    pub use_gitignore: bool,
}

impl FilterSet {
    /// The shipped defaults with no user overrides.
    ///
    /// # Panics
    ///
    /// If the compiled-in exclusion file does not parse. That is a build-time error caught
    /// by this module's tests, not something a user can trigger.
    #[must_use]
    pub fn built_in() -> Self {
        Self::new(&FilterOverrides::default())
    }

    /// The defaults, adjusted by a manifest's overrides.
    ///
    /// # Panics
    ///
    /// If the compiled-in exclusion file does not parse — see [`built_in`](Self::built_in).
    #[must_use]
    pub fn new(overrides: &FilterOverrides) -> Self {
        let defaults = Self::default_exclusions().expect("built-in exclusion set must parse");
        Self::from_parts(defaults, overrides)
    }

    /// Parses an exclusion set from RON, for a caller supplying its own.
    ///
    /// # Errors
    ///
    /// Returns the RON parse error if `text` is not a valid [`DefaultExclusions`].
    pub fn from_ron(
        text: &str,
        overrides: &FilterOverrides,
    ) -> Result<Self, ron::error::SpannedError> {
        Ok(Self::from_parts(ron::from_str(text)?, overrides))
    }

    /// The parsed built-in exclusion set.
    ///
    /// # Errors
    ///
    /// Returns the RON parse error. Reachable only if the compiled-in asset is malformed.
    pub fn default_exclusions() -> Result<DefaultExclusions, ron::error::SpannedError> {
        ron::from_str(BUILT_IN_RON)
    }

    fn from_parts(defaults: DefaultExclusions, overrides: &FilterOverrides) -> Self {
        let mut rules = Vec::new();
        let mut groups = Vec::new();

        for component in ALWAYS_SKIP {
            rules.push(Rule::new(component, Origin::Always));
        }

        if overrides.use_default_exclusions {
            for (index, group) in defaults.groups.iter().enumerate() {
                for pattern in &group.patterns {
                    rules.push(Rule::new(pattern, Origin::Default(index)));
                }
            }
            groups = defaults.groups;
        }

        for pattern in &overrides.extra_exclusions {
            rules.push(Rule::new(pattern, Origin::User));
        }

        let re_include = overrides
            .re_inclusions
            .iter()
            .map(|pattern| Rule::new(pattern, Origin::User))
            .collect();

        Self {
            groups,
            rules,
            re_include,
            use_gitignore: overrides.use_gitignore,
        }
    }

    /// Whether `path` is part of the repository's structure, and if not, which rule said so.
    ///
    /// Re-inclusions are applied last and beat every rule but [`ALWAYS_SKIP`]. Git cannot do
    /// that — a negation inside an ignored directory has no effect, because git never
    /// descends into it — but treepo walks a tree it already holds, so recovering one
    /// vendored directory does not require disabling the whole group it matched.
    #[must_use]
    pub fn decide(&self, path: &RepoPath) -> Decision<'_> {
        let Some(rule) = self.rules.iter().find(|rule| rule.matches(path)) else {
            return Decision::Keep;
        };

        if rule.origin != Origin::Always && self.re_include.iter().any(|r| r.matches(path)) {
            return Decision::Keep;
        }

        let (group, reason) = match rule.origin {
            Origin::Always => (
                "always",
                "treepo's own metadata, never part of a repository's structure",
            ),
            Origin::User => ("user", "excluded by this repository's settings"),
            Origin::Default(index) => self
                .groups
                .get(index)
                .map_or(("default", "excluded by the built-in default set"), |g| {
                    (g.name.as_str(), g.reason.as_str())
                }),
        };

        Decision::Excluded {
            pattern: &rule.pattern,
            group,
            reason,
        }
    }

    /// Whether `path` survives filtering.
    #[must_use]
    pub fn allows(&self, path: &RepoPath) -> bool {
        self.decide(path).is_kept()
    }

    /// The default groups in force, for the settings surface (`F-SET-3`).
    #[must_use]
    pub fn groups(&self) -> &[ExclusionGroup] {
        &self.groups
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(s: &str) -> RepoPath {
        RepoPath::new(s.as_bytes()).expect("valid test path")
    }

    fn excluded_by(set: &FilterSet, s: &str) -> Option<String> {
        match set.decide(&path(s)) {
            Decision::Keep => None,
            Decision::Excluded { group, .. } => Some(group.to_owned()),
        }
    }

    /// The asset is compiled in, so a malformed edit must fail here rather than at runtime.
    #[test]
    fn the_built_in_exclusion_set_parses() {
        let defaults = FilterSet::default_exclusions().expect("built-in set parses");
        assert_eq!(defaults.version, 1);
        assert!(!defaults.groups.is_empty());
        for group in &defaults.groups {
            assert!(!group.name.is_empty(), "every group needs a name");
            assert!(
                !group.reason.is_empty(),
                "every group needs a user-facing reason"
            );
            assert!(!group.patterns.is_empty(), "empty group {}", group.name);
        }
    }

    #[test]
    fn treepo_and_git_metadata_are_always_skipped() {
        let set = FilterSet::built_in();
        assert_eq!(excluded_by(&set, ".git"), Some("always".into()));
        assert_eq!(excluded_by(&set, ".git/config"), Some("always".into()));
        assert_eq!(
            excluded_by(&set, ".treepo/manifest.bin"),
            Some("always".into())
        );
        assert_eq!(excluded_by(&set, "src/.git/hooks"), Some("always".into()));
    }

    #[test]
    fn unanchored_patterns_match_at_any_depth() {
        let set = FilterSet::built_in();
        assert!(!set.allows(&path("node_modules")));
        assert!(!set.allows(&path("node_modules/left-pad/index.js")));
        assert!(!set.allows(&path("packages/ui/node_modules/react/index.js")));
        assert!(!set.allows(&path("crates/treepo-det/target/debug/x")));
        // A component that merely contains the pattern is not a match.
        assert!(set.allows(&path("src/node_modules_helper.rs")));
        assert!(set.allows(&path("src/targets.rs")));
    }

    #[test]
    fn anchored_patterns_stay_at_the_root() {
        let set = FilterSet::built_in();
        assert!(!set.allows(&path("docs/_build")));
        assert!(!set.allows(&path("docs/_build/html/index.html")));
        // Same name, not at the root: kept.
        assert!(set.allows(&path("vendor/project/docs/_build/index.html")));
    }

    #[test]
    fn globs_match_within_a_component() {
        let set = FilterSet::built_in();
        assert!(!set.allows(&path("web/app.min.js")));
        assert!(!set.allows(&path("treepo.egg-info/PKG-INFO")));
        assert!(!set.allows(&path("cmake-build-debug/CMakeCache.txt")));
        assert!(set.allows(&path("web/app.js")));
    }

    /// A surprising absence must be explicable without reading source.
    #[test]
    fn exclusions_report_the_group_and_the_reason() {
        let set = FilterSet::built_in();
        let Decision::Excluded {
            pattern,
            group,
            reason,
        } = set.decide(&path("web/node_modules/x"))
        else {
            panic!("node_modules should be excluded");
        };
        assert_eq!(pattern, "node_modules");
        assert_eq!(group, "package-managers");
        assert!(reason.contains("package manager"));
    }

    #[test]
    fn disabling_the_default_set_keeps_only_the_unconditional_rules() {
        let overrides = FilterOverrides {
            use_default_exclusions: false,
            ..FilterOverrides::default()
        };
        let set = FilterSet::new(&overrides);
        assert!(set.allows(&path("node_modules/react/index.js")));
        assert!(set.allows(&path("target/debug/x")));
        // Still not these.
        assert!(!set.allows(&path(".git/config")));
        assert!(set.groups().is_empty());
    }

    #[test]
    fn user_exclusions_apply_on_top_of_the_defaults() {
        let mut overrides = FilterOverrides::default();
        overrides.extra_exclusions.insert("fixtures".into());
        let set = FilterSet::new(&overrides);
        assert_eq!(
            excluded_by(&set, "tests/fixtures/big.json"),
            Some("user".into())
        );
        assert!(set.allows(&path("tests/cases/big.json")));
    }

    /// Git cannot re-include inside an excluded directory; treepo can, because it walks a
    /// tree it already holds.
    #[test]
    fn re_inclusions_beat_exclusions_but_not_the_unconditional_rules() {
        let mut overrides = FilterOverrides::default();
        overrides
            .re_inclusions
            .insert("vendor/bundle/ours/**".into());
        let set = FilterSet::new(&overrides);

        assert!(!set.allows(&path("vendor/bundle/theirs/x.rb")));
        assert!(set.allows(&path("vendor/bundle/ours/x.rb")));
        // .git is not recoverable by any override.
        overrides.re_inclusions.insert(".git/**".into());
        let set = FilterSet::new(&overrides);
        assert!(!set.allows(&path(".git/config")));
    }

    /// `N3`: the same repository must filter identically on every platform.
    #[test]
    fn matching_is_case_sensitive() {
        let set = FilterSet::built_in();
        assert!(!set.allows(&path("node_modules/x")));
        assert!(set.allows(&path("Node_Modules/x")));
    }

    #[test]
    fn a_custom_exclusion_set_can_be_supplied() {
        let text = r#"(
            version: 1,
            groups: [(name: "test", reason: "because", patterns: ["scratch"])],
        )"#;
        let set = FilterSet::from_ron(text, &FilterOverrides::default()).expect("parses");
        assert_eq!(excluded_by(&set, "a/scratch/b"), Some("test".into()));
        // The built-in groups are not also present.
        assert!(set.allows(&path("node_modules/x")));
        assert!(FilterSet::from_ron("(nonsense", &FilterOverrides::default()).is_err());
    }

    #[test]
    fn the_repository_root_is_never_excluded() {
        assert!(FilterSet::built_in().allows(&RepoPath::root()));
    }
}
