//! The on-disk shape of a manifest — plain data, and the only thing serde sees.
//!
//! Every type here mirrors one in `treepo-model` as integers, strings and vectors. See the
//! parent module for why the model itself does not derive serde; the short version is that
//! `Manifest` reaches into `treepo-det`, which is asserted to have zero dependencies, and that
//! a schema whose only purpose is to be a schema is the thing `SCHEMA_VERSION` should describe.
//!
//! # Field order is the encoding
//!
//! postcard writes fields in declaration order with no names, so **reordering a field here is
//! a format change** even though it compiles and looks harmless. So is changing a type, and so
//! is adding one. `the_encoding_has_a_golden_digest` in the parent module is what notices;
//! `SCHEMA_VERSION` is what makes an old file regenerate rather than misparse.
//!
//! # Fixed-point crosses as bits
//!
//! `Fx` is Q32.32 and its raw `i64` *is* the value — storing the bits is exact, and there is
//! no float anywhere in the path (`N3`). The same goes for `AuthorShare`, which stores its
//! parts-per-million.
//!
//! # Exhaustiveness
//!
//! Each `From` below destructures its source, so a new field in `treepo-model` stops this file
//! compiling instead of quietly not being persisted. Four types cannot be destructured because
//! their fields are private — [`OwnershipPrimitives`], [`BranchingHistogram`], [`DepthProfile`]
//! and the two tables — and each is marked where it appears. Those are the four places a new
//! field could slip through, and each has a round-trip test in `treepo-model` next to it.

use serde::{Deserialize, Serialize};
use treepo_det::{Fx, OrderedMap, OrderedSet, Seed};
use treepo_model::identity::{AuthorKey, CommitId};
use treepo_model::manifest::{
    AuthorEntry, AuthorTable, FilterOverrides, LanguageId, LanguageTable, Manifest, NodeKind,
    PathRecord,
};
use treepo_model::path::RepoPath;
use treepo_model::primitives::size::{ContentCategory, SizeDistribution};
use treepo_model::primitives::{
    AuthorShare, BalanceScore, BranchingHistogram, ChurnWindows, ContentModulation, DepthProfile,
    DerivedSignals, FolderSignal, HierarchyPosition, LineCounts, OwnershipPrimitives,
    SizePrimitives, StructuralPrimitives, TemporalPrimitives,
};

/// A manifest as stored.
///
/// `schema_version` is absent on purpose: it lives in the file header, where it can be read
/// without decoding this. See the parent module.
#[derive(Debug, Serialize, Deserialize)]
pub struct StoredManifest {
    treepo_version: String,
    built_from_commit: Option<Vec<u8>>,
    reference_time: i64,
    is_shallow: bool,
    root_seed: [u8; 32],
    /// Language names in id order, so ids are positions and are never stored per record.
    languages: Vec<String>,
    authors: Vec<StoredAuthor>,
    filters: StoredFilters,
    paths: Vec<StoredPath>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredAuthor {
    key: [u8; 16],
    recency: i64,
    commit_count: u32,
    is_self: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredFilters {
    use_gitignore: bool,
    use_default_exclusions: bool,
    use_linguist_markers: bool,
    extra_exclusions: Vec<String>,
    re_inclusions: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredPath {
    /// Repository path bytes, not a `String` — `RepoPath` is bytes precisely because a path
    /// need not be UTF-8 (PRD §6, `F-INSP-4`).
    path: Vec<u8>,
    kind: u8,
    structural: StoredStructural,
    size: StoredSize,
    temporal: StoredTemporal,
    ownership: StoredOwnership,
    derived: StoredDerived,
    folder_signal: Option<StoredFolderSignal>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredStructural {
    depth: u16,
    child_count: u32,
    descendant_file_count: u32,
    descendant_dir_count: u32,
    max_subtree_depth: u16,
    branching: [u32; BranchingHistogram::BUCKETS],
    depth_profile: Vec<u32>,
    balance_size: i64,
    balance_depth: i64,
    balance_kind: Option<i64>,
    hierarchy_skew: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredSize {
    bytes: u64,
    relative_bytes: i64,
    lines: [u64; 4],
    language_bytes: Vec<(u16, u64)>,
    category_bytes: Vec<(u8, u64)>,
    distribution: [u64; 5],
    large_file_count: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredTemporal {
    first_commit_time: Option<i64>,
    last_commit_time: Option<i64>,
    commit_count: u32,
    churn: [u64; 4],
    recency_heat: i64,
    burstiness: i64,
    stability: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredOwnership {
    shares: Vec<([u8; 16], u32)>,
    recency: Vec<([u8; 16], i64)>,
    dominant: Option<[u8; 16]>,
    bus_factor: u16,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredDerived {
    comment_density: Option<i64>,
    test_to_source: Option<i64>,
    todo_density: Option<i64>,
    doc_staleness_days: Option<i64>,
    generated_debt: Option<i64>,
    large_file_debt: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredFolderSignal {
    signal_name: String,
    default_semantic_weight: i64,
    modulation: [i64; 5],
    effective_weight: i64,
    depth: u16,
    ancestor_signals: Vec<String>,
}

// ---------------------------------------------------------------------------------------
// Model → stored.
// ---------------------------------------------------------------------------------------

impl From<&Manifest> for StoredManifest {
    fn from(manifest: &Manifest) -> Self {
        // `Manifest` has private `paths`, so this is accessor-based rather than destructured.
        // Every other field is public and is named below, which is the same coverage by a
        // different route — a new public field would be unused here and nothing would say so,
        // which is why `a_fully_populated_manifest_round_trips_unchanged` sets all of them.
        Self {
            treepo_version: manifest.treepo_version.clone(),
            built_from_commit: manifest.built_from_commit.map(|id| id.as_bytes().to_vec()),
            reference_time: manifest.reference_time,
            is_shallow: manifest.is_shallow,
            root_seed: *manifest.root_seed.as_bytes(),
            languages: language_names(&manifest.languages),
            authors: authors_of(&manifest.authors),
            filters: (&manifest.filters).into(),
            paths: manifest.paths().iter().map(StoredPath::from).collect(),
        }
    }
}

/// Names in id order, which is what makes the id a position and not a stored number.
///
/// `LanguageTable`'s fields are private; ids are dense from zero by construction, so walking
/// `0..len` recovers every name in the order `intern` assigned them.
fn language_names(table: &LanguageTable) -> Vec<String> {
    (0..table.len())
        .map(|index| {
            let id = LanguageId::new(u16::try_from(index).unwrap_or(u16::MAX));
            table.name(id).unwrap_or_default().to_string()
        })
        .collect()
}

/// `AuthorTable`'s field is private; `iter` yields key order, which is the stored order.
fn authors_of(table: &AuthorTable) -> Vec<StoredAuthor> {
    table
        .iter()
        .map(|(key, entry)| {
            let AuthorEntry {
                recency,
                commit_count,
                is_self,
            } = entry;
            StoredAuthor {
                key: *key.as_bytes(),
                recency: *recency,
                commit_count: *commit_count,
                is_self: *is_self,
            }
        })
        .collect()
}

impl From<&FilterOverrides> for StoredFilters {
    fn from(filters: &FilterOverrides) -> Self {
        let FilterOverrides {
            use_gitignore,
            use_default_exclusions,
            use_linguist_markers,
            extra_exclusions,
            re_inclusions,
        } = filters;
        Self {
            use_gitignore: *use_gitignore,
            use_default_exclusions: *use_default_exclusions,
            use_linguist_markers: *use_linguist_markers,
            extra_exclusions: strings(extra_exclusions),
            re_inclusions: strings(re_inclusions),
        }
    }
}

fn strings(set: &OrderedSet<Box<str>>) -> Vec<String> {
    set.iter().map(|value| value.to_string()).collect()
}

impl From<&PathRecord> for StoredPath {
    fn from(record: &PathRecord) -> Self {
        let PathRecord {
            path,
            kind,
            structural,
            size,
            temporal,
            ownership,
            derived,
            folder_signal,
        } = record;
        Self {
            path: path.as_bytes().to_vec(),
            kind: node_kind_code(*kind),
            structural: structural.into(),
            size: size.into(),
            temporal: temporal.into(),
            ownership: ownership.into(),
            derived: derived.into(),
            folder_signal: folder_signal.as_ref().map(StoredFolderSignal::from),
        }
    }
}

impl From<&StructuralPrimitives> for StoredStructural {
    fn from(value: &StructuralPrimitives) -> Self {
        let StructuralPrimitives {
            depth,
            child_count,
            descendant_file_count,
            descendant_dir_count,
            max_subtree_depth,
            branching,
            depth_profile,
            balance,
            hierarchy_skew,
        } = value;
        let BalanceScore {
            size,
            depth: balance_depth,
            kind,
        } = balance;
        Self {
            depth: *depth,
            child_count: *child_count,
            descendant_file_count: *descendant_file_count,
            descendant_dir_count: *descendant_dir_count,
            max_subtree_depth: *max_subtree_depth,
            // Private fields; accessors are the only route.
            branching: *branching.buckets(),
            depth_profile: depth_profile.levels().to_vec(),
            balance_size: size.to_bits(),
            balance_depth: balance_depth.to_bits(),
            balance_kind: kind.map(Fx::to_bits),
            hierarchy_skew: hierarchy_skew.to_bits(),
        }
    }
}

impl From<&SizePrimitives> for StoredSize {
    fn from(value: &SizePrimitives) -> Self {
        let SizePrimitives {
            bytes,
            relative_bytes,
            lines,
            language_bytes,
            category_bytes,
            distribution,
            large_file_count,
        } = value;
        let LineCounts {
            total,
            code,
            comment,
            blank,
        } = lines;
        let SizeDistribution {
            min,
            median,
            p90,
            max,
            mean,
        } = distribution;
        Self {
            bytes: *bytes,
            relative_bytes: relative_bytes.to_bits(),
            lines: [*total, *code, *comment, *blank],
            language_bytes: language_bytes
                .iter()
                .map(|(id, bytes)| (id.index(), *bytes))
                .collect(),
            category_bytes: category_bytes
                .iter()
                .map(|(category, bytes)| (category_code(*category), *bytes))
                .collect(),
            distribution: [*min, *median, *p90, *max, *mean],
            large_file_count: *large_file_count,
        }
    }
}

impl From<&TemporalPrimitives> for StoredTemporal {
    fn from(value: &TemporalPrimitives) -> Self {
        let TemporalPrimitives {
            first_commit_time,
            last_commit_time,
            commit_count,
            churn,
            recency_heat,
            burstiness,
            stability,
        } = value;
        let ChurnWindows {
            days_30,
            days_90,
            days_365,
            lifetime,
        } = churn;
        Self {
            first_commit_time: *first_commit_time,
            last_commit_time: *last_commit_time,
            commit_count: *commit_count,
            churn: [*days_30, *days_90, *days_365, *lifetime],
            recency_heat: recency_heat.to_bits(),
            burstiness: burstiness.to_bits(),
            stability: stability.map(Fx::to_bits),
        }
    }
}

impl From<&OwnershipPrimitives> for StoredOwnership {
    /// Accessor-based: every field of `OwnershipPrimitives` is private, because
    /// `from_line_counts` derives `dominant` and `bus_factor` and they must not be settable
    /// independently. `ownership_survives_a_round_trip_through_its_parts` in `treepo-model`
    /// is what holds this pair of conversions to being exact.
    fn from(value: &OwnershipPrimitives) -> Self {
        Self {
            shares: value
                .shares()
                .map(|(key, share)| (*key.as_bytes(), share.to_ppm()))
                .collect(),
            recency: value
                .recency()
                .map(|(key, at)| (*key.as_bytes(), *at))
                .collect(),
            dominant: value.dominant_author().map(|key| *key.as_bytes()),
            bus_factor: value.bus_factor_proxy(),
        }
    }
}

impl From<&DerivedSignals> for StoredDerived {
    fn from(value: &DerivedSignals) -> Self {
        let DerivedSignals {
            comment_density,
            test_to_source,
            todo_density,
            doc_staleness_days,
            generated_debt,
            large_file_debt,
        } = value;
        Self {
            comment_density: comment_density.map(Fx::to_bits),
            test_to_source: test_to_source.map(Fx::to_bits),
            todo_density: todo_density.map(Fx::to_bits),
            doc_staleness_days: *doc_staleness_days,
            generated_debt: generated_debt.map(Fx::to_bits),
            large_file_debt: large_file_debt.map(Fx::to_bits),
        }
    }
}

impl From<&FolderSignal> for StoredFolderSignal {
    fn from(value: &FolderSignal) -> Self {
        let FolderSignal {
            signal_name,
            default_semantic_weight,
            content_modulation,
            effective_weight,
            position_in_hierarchy,
        } = value;
        let ContentModulation {
            language_concentration,
            size_ratio,
            binary_ratio,
            test_like_ratio,
            generated_ratio,
        } = content_modulation;
        let HierarchyPosition {
            depth,
            ancestor_signals,
        } = position_in_hierarchy;
        Self {
            signal_name: signal_name.to_string(),
            default_semantic_weight: default_semantic_weight.to_bits(),
            modulation: [
                language_concentration.to_bits(),
                size_ratio.to_bits(),
                binary_ratio.to_bits(),
                test_like_ratio.to_bits(),
                generated_ratio.to_bits(),
            ],
            effective_weight: effective_weight.to_bits(),
            depth: *depth,
            ancestor_signals: ancestor_signals.iter().map(|s| s.to_string()).collect(),
        }
    }
}

// ---------------------------------------------------------------------------------------
// Stored → model.
// ---------------------------------------------------------------------------------------

impl StoredManifest {
    /// Rebuilds the manifest, with `schema_version` supplied by the file header.
    ///
    /// # Errors
    ///
    /// A description of the first value the model rejects. Every one of them means the file
    /// decoded but is not a manifest this build can use, which is a regeneration rather than
    /// a crash (`F-MAN-6`).
    pub fn into_manifest(self, schema_version: u32) -> Result<Manifest, String> {
        let mut manifest = Manifest::new(self.treepo_version, Seed::from_bytes(self.root_seed));
        manifest.schema_version = schema_version;
        manifest.built_from_commit = self.built_from_commit.map(commit_id).transpose()?;
        manifest.reference_time = self.reference_time;
        manifest.is_shallow = self.is_shallow;

        // Interning in stored order reassigns exactly the ids the names were written under.
        let mut languages = LanguageTable::new();
        for name in &self.languages {
            languages.intern(name);
        }
        manifest.languages = languages;

        let mut authors = AuthorTable::new();
        for author in self.authors {
            authors.insert(
                AuthorKey::from_bytes(author.key),
                AuthorEntry {
                    recency: author.recency,
                    commit_count: author.commit_count,
                    is_self: author.is_self,
                },
            );
        }
        manifest.authors = authors;
        manifest.filters = self.filters.into();

        let mut records = Vec::with_capacity(self.paths.len());
        for path in self.paths {
            records.push(path.into_record()?);
        }
        manifest.set_paths(records);
        Ok(manifest)
    }
}

fn commit_id(bytes: Vec<u8>) -> Result<CommitId, String> {
    match bytes.len() {
        20 => Ok(CommitId::sha1(bytes.try_into().unwrap_or([0; 20]))),
        32 => Ok(CommitId::sha256(bytes.try_into().unwrap_or([0; 32]))),
        other => Err(format!("commit id is {other} bytes, not 20 or 32")),
    }
}

impl From<StoredFilters> for FilterOverrides {
    fn from(value: StoredFilters) -> Self {
        Self {
            use_gitignore: value.use_gitignore,
            use_default_exclusions: value.use_default_exclusions,
            use_linguist_markers: value.use_linguist_markers,
            extra_exclusions: value.extra_exclusions.into_iter().map(Into::into).collect(),
            re_inclusions: value.re_inclusions.into_iter().map(Into::into).collect(),
        }
    }
}

impl StoredPath {
    fn into_record(self) -> Result<PathRecord, String> {
        let path = RepoPath::new(&self.path)
            .map_err(|error| format!("stored path is not usable: {error}"))?;
        let mut record = PathRecord::new(path, node_kind(self.kind)?);
        record.structural = self.structural.into();
        record.size = self.size.into_primitives()?;
        record.temporal = self.temporal.into();
        record.ownership = self.ownership.into();
        record.derived = self.derived.into();
        record.folder_signal = self.folder_signal.map(Into::into);
        Ok(record)
    }
}

impl From<StoredStructural> for StructuralPrimitives {
    fn from(value: StoredStructural) -> Self {
        Self {
            depth: value.depth,
            child_count: value.child_count,
            descendant_file_count: value.descendant_file_count,
            descendant_dir_count: value.descendant_dir_count,
            max_subtree_depth: value.max_subtree_depth,
            branching: BranchingHistogram::from_buckets(value.branching),
            depth_profile: DepthProfile::from_levels(value.depth_profile),
            balance: BalanceScore {
                size: Fx::from_bits(value.balance_size),
                depth: Fx::from_bits(value.balance_depth),
                kind: value.balance_kind.map(Fx::from_bits),
            },
            hierarchy_skew: Fx::from_bits(value.hierarchy_skew),
        }
    }
}

impl StoredSize {
    fn into_primitives(self) -> Result<SizePrimitives, String> {
        let mut category_bytes = OrderedMap::new();
        for (code, bytes) in self.category_bytes {
            category_bytes.insert(category(code)?, bytes);
        }
        Ok(SizePrimitives {
            bytes: self.bytes,
            relative_bytes: Fx::from_bits(self.relative_bytes),
            lines: LineCounts {
                total: self.lines[0],
                code: self.lines[1],
                comment: self.lines[2],
                blank: self.lines[3],
            },
            language_bytes: self
                .language_bytes
                .into_iter()
                .map(|(index, bytes)| (LanguageId::new(index), bytes))
                .collect(),
            category_bytes,
            distribution: SizeDistribution {
                min: self.distribution[0],
                median: self.distribution[1],
                p90: self.distribution[2],
                max: self.distribution[3],
                mean: self.distribution[4],
            },
            large_file_count: self.large_file_count,
        })
    }
}

impl From<StoredTemporal> for TemporalPrimitives {
    fn from(value: StoredTemporal) -> Self {
        Self {
            first_commit_time: value.first_commit_time,
            last_commit_time: value.last_commit_time,
            commit_count: value.commit_count,
            churn: ChurnWindows {
                days_30: value.churn[0],
                days_90: value.churn[1],
                days_365: value.churn[2],
                lifetime: value.churn[3],
            },
            recency_heat: Fx::from_bits(value.recency_heat),
            burstiness: Fx::from_bits(value.burstiness),
            stability: value.stability.map(Fx::from_bits),
        }
    }
}

impl From<StoredOwnership> for OwnershipPrimitives {
    fn from(value: StoredOwnership) -> Self {
        Self::from_stored(
            value
                .shares
                .into_iter()
                .map(|(key, ppm)| (AuthorKey::from_bytes(key), AuthorShare::from_ppm(ppm)))
                .collect(),
            value
                .recency
                .into_iter()
                .map(|(key, at)| (AuthorKey::from_bytes(key), at))
                .collect(),
            value.dominant.map(AuthorKey::from_bytes),
            value.bus_factor,
        )
    }
}

impl From<StoredDerived> for DerivedSignals {
    fn from(value: StoredDerived) -> Self {
        Self {
            comment_density: value.comment_density.map(Fx::from_bits),
            test_to_source: value.test_to_source.map(Fx::from_bits),
            todo_density: value.todo_density.map(Fx::from_bits),
            doc_staleness_days: value.doc_staleness_days,
            generated_debt: value.generated_debt.map(Fx::from_bits),
            large_file_debt: value.large_file_debt.map(Fx::from_bits),
        }
    }
}

impl From<StoredFolderSignal> for FolderSignal {
    fn from(value: StoredFolderSignal) -> Self {
        Self {
            signal_name: value.signal_name.into(),
            default_semantic_weight: Fx::from_bits(value.default_semantic_weight),
            content_modulation: ContentModulation {
                language_concentration: Fx::from_bits(value.modulation[0]),
                size_ratio: Fx::from_bits(value.modulation[1]),
                binary_ratio: Fx::from_bits(value.modulation[2]),
                test_like_ratio: Fx::from_bits(value.modulation[3]),
                generated_ratio: Fx::from_bits(value.modulation[4]),
            },
            effective_weight: Fx::from_bits(value.effective_weight),
            position_in_hierarchy: HierarchyPosition {
                depth: value.depth,
                ancestor_signals: value.ancestor_signals.into_iter().map(Into::into).collect(),
            },
        }
    }
}

// ---------------------------------------------------------------------------------------
// Enum discriminants.
//
// Written out rather than cast from the enum, so that reordering a variant in `treepo-model`
// cannot silently renumber every stored record. Each match is exhaustive, so *adding* a
// variant stops this file compiling — which is the moment to decide its code and bump
// `SCHEMA_VERSION`.
// ---------------------------------------------------------------------------------------

const fn node_kind_code(kind: NodeKind) -> u8 {
    match kind {
        NodeKind::File => 0,
        NodeKind::Directory => 1,
        NodeKind::Submodule => 2,
        NodeKind::Symlink => 3,
    }
}

fn node_kind(code: u8) -> Result<NodeKind, String> {
    match code {
        0 => Ok(NodeKind::File),
        1 => Ok(NodeKind::Directory),
        2 => Ok(NodeKind::Submodule),
        3 => Ok(NodeKind::Symlink),
        other => Err(format!("unknown node kind {other}")),
    }
}

const fn category_code(category: ContentCategory) -> u8 {
    match category {
        ContentCategory::Code => 0,
        ContentCategory::Asset => 1,
        ContentCategory::Config => 2,
        ContentCategory::Docs => 3,
        ContentCategory::Generated => 4,
        ContentCategory::Binary => 5,
        ContentCategory::Unknown => 6,
    }
}

fn category(code: u8) -> Result<ContentCategory, String> {
    match code {
        0 => Ok(ContentCategory::Code),
        1 => Ok(ContentCategory::Asset),
        2 => Ok(ContentCategory::Config),
        3 => Ok(ContentCategory::Docs),
        4 => Ok(ContentCategory::Generated),
        5 => Ok(ContentCategory::Binary),
        6 => Ok(ContentCategory::Unknown),
        other => Err(format!("unknown content category {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every code must be distinct and must round-trip, or a stored record changes meaning.
    #[test]
    fn enum_codes_are_stable_and_distinct() {
        let mut kinds = Vec::new();
        for kind in [
            NodeKind::File,
            NodeKind::Directory,
            NodeKind::Submodule,
            NodeKind::Symlink,
        ] {
            let code = node_kind_code(kind);
            assert_eq!(node_kind(code), Ok(kind));
            assert!(!kinds.contains(&code), "duplicate node-kind code {code}");
            kinds.push(code);
        }
        assert!(node_kind(4).is_err());

        let mut codes = Vec::new();
        for value in ContentCategory::ALL {
            let code = category_code(value);
            assert_eq!(category(code), Ok(value));
            assert!(!codes.contains(&code), "duplicate category code {code}");
            codes.push(code);
        }
        assert!(category(7).is_err());
    }

    /// The stored language list is positional, so its order *is* the id assignment.
    #[test]
    fn language_ids_are_positions_in_the_stored_list() {
        let mut table = LanguageTable::new();
        let rust = table.intern("Rust");
        let wgsl = table.intern("WGSL");
        assert_eq!(language_names(&table), ["Rust", "WGSL"]);

        let mut rebuilt = LanguageTable::new();
        for name in language_names(&table) {
            rebuilt.intern(&name);
        }
        assert_eq!(rebuilt.get("Rust"), Some(rust));
        assert_eq!(rebuilt.get("WGSL"), Some(wgsl));
        assert_eq!(rebuilt.len(), table.len());
    }

    /// A path that is not valid UTF-8 must survive, and a stored path the model rejects must
    /// be reported rather than panicked on (PRD §6, `F-INSP-4`).
    #[test]
    fn path_bytes_are_preserved_and_bad_ones_are_reported() {
        let raw = b"src/\xff\xfe.rs";
        let record = PathRecord::new(
            RepoPath::new(raw).expect("bytes are a path"),
            NodeKind::File,
        );
        let stored = StoredPath::from(&record);
        assert_eq!(stored.path, raw);
        assert_eq!(stored.into_record().expect("back again"), record);

        let broken = StoredPath {
            path: b"has\0nul".to_vec(),
            ..StoredPath::from(&record)
        };
        let error = broken.into_record().expect_err("NUL is not a path byte");
        assert!(error.contains("not usable"), "{error}");
    }
}
