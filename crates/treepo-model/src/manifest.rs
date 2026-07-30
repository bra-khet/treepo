//! The [`Manifest`] — everything one extraction learned about one repository.
//!
//! Produced by `treepo-vcs`, persisted by `treepo-store` as `manifest.bin` plus a JSON
//! sidecar (architecture D9, E2), and consumed by `treepo-gen`. It is the whole interface
//! between "reading a repository" and "growing a tree": once a manifest exists, generation
//! never touches the repository again, which is what `N6` and `AC-THR-1` require.
//!
//! # Regenerable, and byte-identical when regenerated
//!
//! `F-MAN-8` allows deleting the store at any time, and `AC-MAN-1` requires that
//! regenerating produces identical bytes. Two properties in this module carry that:
//!
//! * **Nothing here reads a clock.** [`reference_time`](Manifest::reference_time) is the
//!   newest commit timestamp in the repository — a property of the repository, so it is the
//!   same tomorrow. See [`temporal`](crate::primitives::temporal).
//! * **Nothing here depends on insertion order except where the walk fixes it.**
//!   [`paths`](Manifest::paths) is sorted, the author table is keyed by hash, and
//!   [`LanguageTable`] ids follow the sorted walk. `AC-DET-3` is what makes that last one
//!   deterministic, and it is called out where the table is declared.

use crate::identity::{AuthorKey, CommitId};
use crate::path::RepoPath;
use crate::primitives::{
    DerivedSignals, FolderSignal, OwnershipPrimitives, SizePrimitives, StructuralPrimitives,
    TemporalPrimitives,
};
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use treepo_det::{OrderedMap, OrderedSet, Seed};

/// The manifest schema version (`F-MAN-6`).
///
/// A stored manifest whose version differs is regenerated rather than parsed. Bump this for
/// any change to what is stored or how a stored value is derived — including a change to
/// [`AuthorKey`]'s domain string, which would silently re-key every contributor.
///
/// * **2** — dropped the per-contributor commit total from [`AuthorEntry`]. See that type.
/// * **1** — the first schema.
pub const SCHEMA_VERSION: u32 = 2;

/// The domain the per-path seed tree hangs from (`P2`).
const SEED_DOMAIN: &[u8] = b"treepo/manifest/v1";

/// What kind of node a path is.
///
/// The last three variants are PRD §6 edge cases given a name, so the extractor cannot
/// quietly treat one as an ordinary directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeKind {
    /// An ordinary file.
    File,
    /// An ordinary directory.
    Directory,
    /// A submodule — rendered as a sealed object, never recursed into in v1.
    Submodule,
    /// A symbolic link — recorded, never followed, so cycles are impossible.
    Symlink,
}

impl NodeKind {
    /// Whether children may exist beneath this node.
    #[must_use]
    pub const fn is_container(self) -> bool {
        matches!(self, Self::Directory)
    }
}

/// An index into [`LanguageTable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LanguageId(u16);

impl LanguageId {
    /// Builds an id. Only [`LanguageTable`] should mint these; public for tests and for
    /// `treepo-store` reading one back.
    #[must_use]
    pub const fn new(index: u16) -> Self {
        Self(index)
    }

    /// The raw index.
    #[must_use]
    pub const fn index(self) -> u16 {
        self.0
    }
}

/// Language names, interned once per manifest.
///
/// At T3 a manifest holds 80,000 path records, most naming two or three languages. Interning
/// turns each of those into two bytes instead of a heap-allocated string.
///
/// # Ids are assigned on first sight, and that is deliberate
///
/// The alternative — sorting names and using the sorted index — renumbers every existing id
/// the moment a new language appears, which would rewrite the whole manifest on an
/// incremental re-extraction that `AC-EXT-2` requires touch only affected paths. Append-only
/// ids keep existing records valid.
///
/// The cost is that ids depend on the order paths are visited. That is deterministic because
/// `AC-DET-3` requires the walk be sorted, so this table inherits its determinism from the
/// walk rather than owning it — worth knowing if `AC-MAN-1` ever fails on the language table
/// alone.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LanguageTable {
    names: Vec<Box<str>>,
    lookup: OrderedMap<Box<str>, LanguageId>,
}

impl LanguageTable {
    /// An empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the id for `name`, assigning one if this is its first appearance.
    ///
    /// Saturates at [`u16::MAX`] distinct languages, which no repository will reach — the
    /// linguist dictionary lists a few hundred.
    pub fn intern(&mut self, name: &str) -> LanguageId {
        if let Some(&id) = self.lookup.get(name) {
            return id;
        }
        let id = LanguageId(u16::try_from(self.names.len()).unwrap_or(u16::MAX));
        self.names.push(name.into());
        self.lookup.insert(name.into(), id);
        id
    }

    /// The name behind an id.
    #[must_use]
    pub fn name(&self, id: LanguageId) -> Option<&str> {
        self.names.get(id.0 as usize).map(|name| &**name)
    }

    /// The id for a name, without interning it.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<LanguageId> {
        self.lookup.get(name).copied()
    }

    /// How many distinct languages the manifest names.
    #[must_use]
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Whether no language has been interned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

/// One contributor's repository-wide entry.
///
/// The per-path view is [`OwnershipPrimitives`]; this is the roll-up, and the same `N4`
/// discipline applies — see [`ownership`](crate::primitives::ownership) for why
/// [`AuthorShare`](crate::primitives::AuthorShare) cannot be sorted.
/// # Why there is no per-contributor commit count here
///
/// Schema 1 carried one. It was written by `treepo-vcs::log_pass` because the number was in
/// hand while walking the graph, it was read by nothing — `treepo-gen` never touches
/// [`AuthorTable`] — and no requirement asked for it. `F-EXT-2` names `commit_count` *per
/// path*, which is [`TemporalPrimitives::commit_count`], and
/// `design/feature-system.md` §3.4's ownership set names `contribution_recency_per_author`
/// and no count. What it *was* is the widest route to a leaderboard in this crate: collect
/// [`AuthorTable::iter`], sort by it, two lines. `N4` forbids ranking people and `AC-INSP-2`
/// forbids an inspection surface showing a contribution count, so the field had a use only
/// if that use were forbidden.
///
/// **Adding it back needs a requirement that names it, and an answer to what may read it.**
/// A count that only exists because it was cheap to collect is a count a renderer will
/// eventually sort. If a future `F-INSP-1` panel needs to say "how much" about a
/// contributor, `recency` and [`AuthorShare`](crate::primitives::AuthorShare)'s coarse
/// bucket are what `N4` leaves available.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorEntry {
    /// Most recent commit anywhere in the repository, as absolute epoch seconds.
    ///
    /// A timestamp, not a volume: committing recently does not make someone a larger
    /// contributor, and `design/feature-system.md` §3.4 asks for exactly this per author.
    pub recency: i64,
    /// Whether this is the user running treepo (`F-ID-1`).
    ///
    /// The one contributor `N9` permits to be named, and only by their own choice.
    ///
    /// # The one field here that depends on who is looking
    ///
    /// Every other value in a manifest is a property of the repository. This one is
    /// resolved from the local `user.email` (`treepo-vcs::self_ident`), so it is a property
    /// of the *machine*. It reaches no generated value — `treepo-gen` never reads
    /// [`AuthorTable`], so no skeleton and no determinism digest can move with it — but two
    /// consequences follow and are worth expecting rather than debugging:
    ///
    /// * changing `user.email` and re-extracting produces a different `manifest.bin`. That
    ///   is correct, and outside `AC-MAN-1`'s "unchanged repository state".
    /// * a manifest shared through `F-MAN-11` would carry a bit saying "the sender is one of
    ///   these keys", which against a public repository's author list is enough to name
    ///   them. **`package.rs` must clear this**, and that is Phase 10's job.
    pub is_self: bool,
}

/// Every contributor to the repository, keyed by hash.
///
/// A [`BTreeMap`](alloc::collections::BTreeMap) keyed by [`AuthorKey`], so iteration order
/// is hash order and carries no information about contribution. `N4`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthorTable(OrderedMap<AuthorKey, AuthorEntry>);

impl AuthorTable {
    /// An empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts or replaces a contributor's entry.
    pub fn insert(&mut self, key: AuthorKey, entry: AuthorEntry) {
        self.0.insert(key, entry);
    }

    /// One contributor's entry.
    #[must_use]
    pub fn get(&self, key: &AuthorKey) -> Option<&AuthorEntry> {
        self.0.get(key)
    }

    /// Every contributor, in key order.
    pub fn iter(&self) -> impl Iterator<Item = (&AuthorKey, &AuthorEntry)> {
        self.0.iter()
    }

    /// `author_count` for the repository.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the repository has no attributed contributors.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The contributor marked as the user running treepo, if any (`F-ID-1`).
    ///
    /// `None` is the common case, not an error: most repositories a user opens are ones
    /// they merely cloned, and every contributor is then pseudonymous
    /// (`design/feature-system.md` §3.4).
    #[must_use]
    pub fn self_author(&self) -> Option<AuthorKey> {
        self.0
            .iter()
            .find(|(_, entry)| entry.is_self)
            .map(|(&key, _)| key)
    }
}

/// Per-repository filtering, persisted so the tree is reproducible (`F-EXT-8` item 5).
///
/// The built-in exclusion set lives in `assets/filters/default-exclusions.ron`; this records
/// the user's departures from it, which is what makes the manifest self-describing. Someone
/// looking at a tree with no `node_modules` limb can tell whether that is the default or a
/// choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterOverrides {
    /// Honour the repository's `.gitignore` (`F-EXT-8` item 2).
    pub use_gitignore: bool,
    /// Apply the built-in exclusion set (`F-EXT-8` item 3).
    pub use_default_exclusions: bool,
    /// Honour `.gitattributes` `linguist-generated` and `linguist-vendored` (item 4).
    pub use_linguist_markers: bool,
    /// Extra paths to exclude, as glob patterns.
    pub extra_exclusions: OrderedSet<Box<str>>,
    /// Paths to keep despite an exclusion rule matching them.
    ///
    /// Applied last, so a user can recover one vendored directory without disabling the
    /// whole default set.
    pub re_inclusions: OrderedSet<Box<str>>,
}

impl Default for FilterOverrides {
    /// `F-EXT-8`'s defaults: every rule on, no user departures.
    fn default() -> Self {
        Self {
            use_gitignore: true,
            use_default_exclusions: true,
            use_linguist_markers: true,
            extra_exclusions: OrderedSet::new(),
            re_inclusions: OrderedSet::new(),
        }
    }
}

impl FilterOverrides {
    /// Whether these are the shipped defaults.
    ///
    /// Drives the "filters are customized" note in the UI, so a surprising tree is
    /// explicable without opening settings.
    #[must_use]
    pub fn is_default(&self) -> bool {
        self.use_gitignore
            && self.use_default_exclusions
            && self.use_linguist_markers
            && self.extra_exclusions.is_empty()
            && self.re_inclusions.is_empty()
    }
}

/// Everything extracted about one path.
///
/// # There is no stored seed
///
/// The architecture's field list has `seed: u64`; [`PathRecord::seed`] derives it instead.
/// A stored seed is a cached copy of a value computed from the path, and a cached copy can
/// disagree with its input — a manifest edited or migrated with a stale seed would grow a
/// differently-shaped limb with nothing to indicate why. Deriving also protects
/// `AC-GROW-4`: because the seed is a function of the path alone, adding a file cannot
/// reseed its siblings, which is the property that keeps a Grow confined to one limb.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathRecord {
    /// Repository-relative path. Unique within a manifest.
    pub path: RepoPath,
    /// File, directory, submodule, or symlink.
    pub kind: NodeKind,
    /// Topology (`F-EXT-1`).
    pub structural: StructuralPrimitives,
    /// Bytes, lines, languages, composition (`F-EXT-1`, `F-EXT-4`).
    pub size: SizePrimitives,
    /// History (`F-EXT-2`). Absolute timestamps only.
    pub temporal: TemporalPrimitives,
    /// Contributors (`F-EXT-2`). Unordered by construction (`N4`).
    pub ownership: OwnershipPrimitives,
    /// Lightweight quality signals (`F-EXT-6`). `None` fields were not measured.
    pub derived: DerivedSignals,
    /// The folder-convention record, when the name matched the dictionary (`F-EXT-5`).
    pub folder_signal: Option<FolderSignal>,
}

impl PathRecord {
    /// A record for a path nothing has been measured about yet.
    #[must_use]
    pub fn new(path: RepoPath, kind: NodeKind) -> Self {
        let depth = path.depth();
        Self {
            path,
            kind,
            structural: StructuralPrimitives::leaf(depth),
            size: SizePrimitives::default(),
            temporal: TemporalPrimitives::default(),
            ownership: OwnershipPrimitives::default(),
            derived: DerivedSignals::default(),
            folder_signal: None,
        }
    }

    /// This path's generator seed, derived from `root` (`P2`).
    ///
    /// The full path is the label, so `src/main.rs` and `docs/main.rs` diverge completely
    /// rather than sharing whatever `main.rs` would produce.
    #[must_use]
    pub fn seed(&self, root: &Seed) -> Seed {
        root.derive(self.path.as_bytes())
    }
}

/// Everything one extraction learned about one repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// Schema version this manifest was written under (`F-MAN-6`).
    pub schema_version: u32,
    /// The treepo version that wrote it, for diagnostics.
    pub treepo_version: String,
    /// The commit the structure was extracted from — HEAD at extraction time (`F-EXT-7`).
    ///
    /// `None` for a repository with no commits or no `.git` (PRD §6).
    pub built_from_commit: Option<CommitId>,
    /// The newest commit timestamp in the repository, as absolute epoch seconds.
    ///
    /// **The anchor every age is measured against**, and the reason this crate never reads a
    /// clock. It is a property of the repository, so regenerating the manifest tomorrow
    /// produces the same ages and therefore the same tree (`AC-MAN-1`, `AC-DET-1`). Zero
    /// for a repository with no history.
    pub reference_time: i64,
    /// Whether the repository is a shallow clone (PRD §6).
    ///
    /// A shallow clone yields a history-less tree, which looks like a defect rather than a
    /// truncation. `F-ASSOC-2` warns explicitly and offers to unshallow; silently growing
    /// an ageless tree is called out in the PRD as a defect, so this flag is not optional
    /// metadata.
    pub is_shallow: bool,
    /// The root of the per-path seed tree (`P2`).
    ///
    /// Stored rather than derived so a manifest is self-contained and the seeding is
    /// inspectable. What it is derived *from* is `treepo-store`'s decision in Phase 2 —
    /// root commit and repository identity behave differently for a fork, which is a
    /// product question rather than a model one.
    pub root_seed: Seed,
    /// Every path, sorted by [`RepoPath`]'s byte order.
    ///
    /// Sorted rather than a map: lookup is a binary search, iteration is in tree order for
    /// free, and the serialized form has no ordering ambiguity to resolve (`AC-MAN-1`).
    /// Maintained by [`Manifest::set_paths`].
    paths: Vec<PathRecord>,
    /// Every contributor, keyed by hash (`N4`).
    pub authors: AuthorTable,
    /// Language names interned by [`SizePrimitives`].
    pub languages: LanguageTable,
    /// Filtering in force when this manifest was built (`F-EXT-8`).
    pub filters: FilterOverrides,
}

impl Manifest {
    /// An empty manifest for a repository nothing has been read from yet.
    #[must_use]
    pub fn new(treepo_version: String, root_seed: Seed) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            treepo_version,
            built_from_commit: None,
            reference_time: 0,
            is_shallow: false,
            root_seed,
            paths: Vec::new(),
            authors: AuthorTable::new(),
            languages: LanguageTable::new(),
            filters: FilterOverrides::default(),
        }
    }

    /// The root of a seed tree for a repository, domain-separated (`P2`).
    ///
    /// `source` is whatever Phase 2 decides identifies the repository; passing it through
    /// this function is what keeps the seed tree separate from every other use of
    /// [`Seed`] in treepo.
    #[must_use]
    pub fn root_seed_for(source: &[u8]) -> Seed {
        Seed::root(SEED_DOMAIN).derive(source)
    }

    /// Replaces the path records, sorting them.
    ///
    /// The only way to populate [`paths`](Self::paths), so the sorted invariant that
    /// [`path`](Self::path) binary-searches against cannot be broken from outside.
    /// `sort_by` rather than `sort_unstable_by`: paths are unique, so the two agree, but
    /// the unstable sort's behaviour on equal elements is unspecified across Rust versions
    /// and this crate does not use it anywhere output depends on.
    pub fn set_paths(&mut self, mut paths: Vec<PathRecord>) {
        paths.sort_by(|a, b| a.path.cmp(&b.path));
        self.paths = paths;
    }

    /// Every path record, in sorted order.
    #[must_use]
    pub fn paths(&self) -> &[PathRecord] {
        &self.paths
    }

    /// The record for one path.
    #[must_use]
    pub fn path(&self, path: &RepoPath) -> Option<&PathRecord> {
        self.paths
            .binary_search_by(|record| record.path.cmp(path))
            .ok()
            .map(|index| &self.paths[index])
    }

    /// The records directly beneath `parent`.
    ///
    /// Byte order does not group a directory's children contiguously — `src/a` and
    /// `src/a.rs` interleave with `src/a/b` — so this filters rather than slicing. Callers
    /// walking the whole tree should iterate [`paths`](Self::paths) once instead.
    pub fn children<'a>(&'a self, parent: &'a RepoPath) -> impl Iterator<Item = &'a PathRecord> {
        self.paths
            .iter()
            .filter(move |record| record.path.parent().as_ref() == Some(parent))
    }

    /// The generator seed for a path (`P2`).
    #[must_use]
    pub fn seed_for(&self, path: &RepoPath) -> Seed {
        self.root_seed.derive(path.as_bytes())
    }

    /// Whether the repository had any commit history to extract.
    ///
    /// False is an ordinary path, not a failure: PRD §6 requires a tree from filesystem
    /// primitives alone, with an explicit notice that age, churn, and ownership are
    /// unavailable.
    #[must_use]
    pub const fn has_history(&self) -> bool {
        self.built_from_commit.is_some()
    }

    /// Whether a stored manifest can be read, or must be regenerated (`F-MAN-6`).
    #[must_use]
    pub const fn is_current_schema(&self) -> bool {
        self.schema_version == SCHEMA_VERSION
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString as _;
    use alloc::vec;

    fn path(s: &str) -> RepoPath {
        RepoPath::new(s.as_bytes()).expect("valid test path")
    }

    fn manifest() -> Manifest {
        Manifest::new("0.1.0".to_string(), Manifest::root_seed_for(b"test"))
    }

    #[test]
    fn paths_are_sorted_regardless_of_insertion_order() {
        let mut m = manifest();
        m.set_paths(vec![
            PathRecord::new(path("src/main.rs"), NodeKind::File),
            PathRecord::new(path("README.md"), NodeKind::File),
            PathRecord::new(path("src"), NodeKind::Directory),
        ]);
        let order: Vec<_> = m.paths().iter().map(|r| r.path.to_string()).collect();
        assert_eq!(order, ["README.md", "src", "src/main.rs"]);
    }

    #[test]
    fn lookup_finds_records_and_misses_cleanly() {
        let mut m = manifest();
        m.set_paths(vec![
            PathRecord::new(path("src"), NodeKind::Directory),
            PathRecord::new(path("src/main.rs"), NodeKind::File),
        ]);
        assert_eq!(
            m.path(&path("src")).map(|r| r.kind),
            Some(NodeKind::Directory)
        );
        assert_eq!(
            m.path(&path("src/main.rs")).map(|r| r.kind),
            Some(NodeKind::File)
        );
        assert!(m.path(&path("nope")).is_none());
    }

    /// Byte order interleaves `src/a.rs` between `src` and `src/a/b`, which is why
    /// `children` filters instead of slicing a range.
    #[test]
    fn children_are_found_across_an_interleaved_ordering() {
        let mut m = manifest();
        m.set_paths(vec![
            PathRecord::new(path("src"), NodeKind::Directory),
            PathRecord::new(path("src/a"), NodeKind::Directory),
            PathRecord::new(path("src/a/deep.rs"), NodeKind::File),
            PathRecord::new(path("src/a.rs"), NodeKind::File),
        ]);
        let names: Vec<_> = m
            .children(&path("src"))
            .map(|r| r.path.to_string())
            .collect();
        assert_eq!(names, ["src/a", "src/a.rs"]);
        let deep: Vec<_> = m
            .children(&path("src/a"))
            .map(|r| r.path.to_string())
            .collect();
        assert_eq!(deep, ["src/a/deep.rs"]);
    }

    /// `P2`: a path's seed is a function of its path, so a sibling appearing changes
    /// nothing — the property `AC-GROW-4` needs.
    #[test]
    fn seeds_are_a_function_of_the_path_alone() {
        let mut m = manifest();
        m.set_paths(vec![PathRecord::new(path("src/main.rs"), NodeKind::File)]);
        let before = m.seed_for(&path("src/main.rs"));

        m.set_paths(vec![
            PathRecord::new(path("src/aaa.rs"), NodeKind::File),
            PathRecord::new(path("src/main.rs"), NodeKind::File),
            PathRecord::new(path("src/zzz.rs"), NodeKind::File),
        ]);
        assert_eq!(m.seed_for(&path("src/main.rs")), before);

        // And the record derives the same seed as the manifest.
        let record = m.path(&path("src/main.rs")).unwrap();
        assert_eq!(record.seed(&m.root_seed), before);
        // Same file name, different directory, different seed.
        assert_ne!(m.seed_for(&path("docs/main.rs")), before);
    }

    #[test]
    fn root_seeds_are_domain_separated_by_source() {
        assert_ne!(Manifest::root_seed_for(b"a"), Manifest::root_seed_for(b"b"));
        assert_eq!(Manifest::root_seed_for(b"a"), Manifest::root_seed_for(b"a"));
    }

    #[test]
    fn language_ids_are_stable_as_the_table_grows() {
        let mut table = LanguageTable::new();
        let rust = table.intern("Rust");
        let toml = table.intern("TOML");
        assert_eq!(table.intern("Rust"), rust);
        assert_ne!(rust, toml);
        // A new language appends; existing ids do not move (`AC-EXT-2`).
        let wgsl = table.intern("WGSL");
        assert_eq!(table.intern("Rust"), rust);
        assert_eq!(table.name(rust), Some("Rust"));
        assert_eq!(table.name(wgsl), Some("WGSL"));
        assert_eq!(table.get("Rust"), Some(rust));
        assert_eq!(table.get("Zig"), None);
        assert_eq!(table.len(), 3);
    }

    #[test]
    fn filter_defaults_are_every_rule_on() {
        let filters = FilterOverrides::default();
        assert!(filters.is_default());
        assert!(filters.use_gitignore);

        let mut customized = FilterOverrides::default();
        customized.extra_exclusions.insert("fixtures/**".into());
        assert!(!customized.is_default());
    }

    #[test]
    fn the_author_table_iterates_in_key_order() {
        let mut authors = AuthorTable::new();
        let entry = |is_self| AuthorEntry {
            recency: 0,
            is_self,
        };
        let a = AuthorKey::from_email(b"a@example.com");
        let b = AuthorKey::from_email(b"b@example.com");
        authors.insert(a, entry(false));
        authors.insert(b, entry(true));

        let keys: Vec<_> = authors.iter().map(|(&k, _)| k).collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
        assert_eq!(authors.len(), 2);
        assert_eq!(authors.self_author(), Some(b));
    }

    /// PRD §6, "No `.git`": a history-less repository is an ordinary manifest.
    #[test]
    fn a_manifest_without_history_is_valid() {
        let m = manifest();
        assert!(!m.has_history());
        assert_eq!(m.reference_time, 0);
        assert!(m.authors.is_empty());
        assert!(m.is_current_schema());
        assert!(!m.is_shallow);
        assert!(m.authors.self_author().is_none());
    }

    #[test]
    fn a_new_record_starts_at_its_own_depth() {
        let record = PathRecord::new(path("a/b/c.rs"), NodeKind::File);
        assert_eq!(record.structural.depth, 3);
        assert!(record.structural.is_leaf());
        assert!(!record.temporal.has_history());
        assert!(record.ownership.is_empty());
        assert!(record.folder_signal.is_none());
    }

    #[test]
    fn only_directories_hold_children() {
        assert!(NodeKind::Directory.is_container());
        assert!(!NodeKind::File.is_container());
        // PRD §6: a submodule is a sealed object, not a directory to recurse into.
        assert!(!NodeKind::Submodule.is_container());
        assert!(!NodeKind::Symlink.is_container());
    }
}
