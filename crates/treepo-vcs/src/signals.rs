//! `F-EXT-5` — folder conventions as structured records, never booleans.
//!
//! `design/feature-system.md` §3.1 is unusually specific about why:
//!
//! > A `public` folder full of static assets means something different from a `public` folder
//! > full of binaries or from a `public` that is actually the root of a separate package. The
//! > multi-attribute record preserves the information needed for correct later interpretation.
//!
//! So this pass does not answer "is this a source folder". It answers "this folder is named
//! like a source folder, the convention is worth *this* much, its contents argue *this* way
//! about that, and it sits *here* in the tree" — and stores all four, so a later phase that
//! disagrees with the weight can see what produced it.
//!
//! # Everything here is already extracted
//!
//! No blob is read and no path is stat-ed. Every input comes from a record
//! [`walk`](crate::walk) and [`lang`](crate::lang) already filled: category shares from
//! `size.category_bytes`, language concentration from `size.language_bytes`, size share from
//! `size.bytes`. That is what makes this cheap enough to re-run when the dictionary changes,
//! which is the whole reason the dictionary is a file.
//!
//! Test share is the one exception, and it is recomputed from paths rather than stored —
//! classification by name is pure and costs a string compare per file.
//!
//! # Nesting is carried, not applied
//!
//! A `docs` inside `vendor` is somebody else's documentation, and the record says so through
//! [`HierarchyPosition::ancestor_signals`]. It does *not* reach `effective_weight`.
//!
//! The temptation is to damp a nested signal by its ancestor's weight, and the reason not to
//! is that no design document asks for it: `F-MAT-5`'s enrichment is where nesting is
//! supposed to be interpreted, and a compounding rule invented here would bake one guess into
//! the manifest where a later phase could have made a better one with the same information.
//! Carrying the ancestors costs a few bytes and keeps the decision where it belongs.

use crate::lang::Catalogue;
use serde::Deserialize;
use std::collections::BTreeMap;
use treepo_det::Fx;
use treepo_model::manifest::{NodeKind, PathRecord};
use treepo_model::primitives::folder_signal::{ContentModulation, FolderSignal, HierarchyPosition};
use treepo_model::primitives::size::ContentCategory;

/// The compiled-in dictionary. See [`filter`](crate::filter) for why `include_str!`.
const BUILT_IN_RON: &str = include_str!("../../../assets/params/folder-signals.ron");

/// Weights and thresholds in the asset file are per mille; this is `1.0`.
const WHOLE: i64 = 1000;

/// A ratio a rule can test, named as the asset file spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum RatioName {
    /// Share of bytes in source code.
    Code,
    /// Share of bytes in non-code material.
    Asset,
    /// Share of bytes in configuration.
    Config,
    /// Share of bytes in documentation.
    Docs,
    /// Share of bytes in generated or vendored content.
    Generated,
    /// Share of bytes in binary content.
    Binary,
    /// Share of bytes the catalogue could not name.
    Unknown,
    /// Share of bytes in files whose name or path reads as a test.
    TestLike,
    /// The dominant language's share of the subtree's typed bytes.
    LanguageConcentration,
    /// The subtree's bytes as a proportion of the whole repository's.
    SizeShare,
}

/// One condition on the contents, and what it does to the weight.
///
/// Every stated bound must hold for `adjust` to apply. A rule with neither bound always
/// applies, which is a way of writing a flat correction.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Evidence {
    ratio: RatioName,
    #[serde(default)]
    above: Option<u32>,
    #[serde(default)]
    below: Option<u32>,
    adjust: i32,
}

impl Evidence {
    fn applies(&self, measured: &Measured) -> bool {
        let value = measured.get(self.ratio);
        self.above.is_none_or(|bound| value > i64::from(bound))
            && self.below.is_none_or(|bound| value < i64::from(bound))
    }
}

/// One folder convention.
#[derive(Debug, Clone, Deserialize)]
pub struct SignalEntry {
    /// The canonical signal name, stored on every record that matches.
    pub name: String,
    /// The conventional weight in per mille.
    weight: u32,
    /// Folder names that match this convention, matched ASCII-case-insensitively.
    #[serde(default)]
    names: Vec<String>,
    /// What the convention means. User-facing, for the debug surface (`F-INSP-4`).
    pub meaning: String,
    #[serde(default)]
    evidence: Vec<Evidence>,
}

impl SignalEntry {
    /// The conventional weight, before any evidence.
    #[must_use]
    pub fn default_weight(&self) -> Fx {
        Fx::from_ratio(i64::from(self.weight), WHOLE)
    }

    /// The names that reach this entry.
    #[must_use]
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// The rules that can move this entry's weight.
    #[must_use]
    pub fn evidence(&self) -> &[Evidence] {
        &self.evidence
    }
}

/// The parsed contents of `assets/params/folder-signals.ron`.
#[derive(Debug, Clone, Deserialize)]
struct DictionaryFile {
    version: u32,
    signals: Vec<SignalEntry>,
}

/// The folder-convention dictionary (`F-EXT-5`).
#[derive(Debug, Clone)]
pub struct SignalDictionary {
    entries: Vec<SignalEntry>,
    /// Lowercased folder name to entry index.
    by_name: BTreeMap<Box<str>, usize>,
}

impl SignalDictionary {
    /// The shipped dictionary.
    ///
    /// # Panics
    ///
    /// If the compiled-in asset does not parse — a build-time error this module's tests
    /// catch, not something a repository can trigger.
    #[must_use]
    pub fn built_in() -> Self {
        Self::from_ron(BUILT_IN_RON).expect("built-in folder-signal dictionary must parse")
    }

    /// Parses a dictionary from RON, for a caller supplying its own.
    ///
    /// The text must open with `#![enable(implicit_some)]`, which is what lets a rule write
    /// `above: 600` instead of `above: Some(600)`.
    ///
    /// # Errors
    ///
    /// Returns the RON parse error if `text` is not a valid dictionary.
    pub fn from_ron(text: &str) -> Result<Self, ron::error::SpannedError> {
        let file: DictionaryFile = ron::from_str(text)?;
        let mut by_name = BTreeMap::new();
        for (index, entry) in file.signals.iter().enumerate() {
            // The canonical name always matches, whether or not it is listed again.
            for name in core::iter::once(&entry.name).chain(&entry.names) {
                by_name
                    .entry(name.to_ascii_lowercase().into_boxed_str())
                    .or_insert(index);
            }
        }
        debug_assert_eq!(file.version, 1, "dictionary schema version");
        Ok(Self {
            entries: file.signals,
            by_name,
        })
    }

    /// Every convention in the dictionary.
    #[must_use]
    pub fn entries(&self) -> &[SignalEntry] {
        &self.entries
    }

    /// The convention a folder name matches, if any.
    ///
    /// Case-insensitive, for the same reason [`lang`](crate::lang) folds extensions and
    /// [`filter`](crate::filter) does not: the fold is applied to path bytes treepo already
    /// holds, so `Docs` and `docs` reach the same convention on every platform.
    #[must_use]
    pub fn lookup(&self, folder_name: &[u8]) -> Option<&SignalEntry> {
        let key = String::from_utf8_lossy(folder_name).to_ascii_lowercase();
        self.by_name
            .get(key.as_str())
            .and_then(|&index| self.entries.get(index))
    }
}

/// The ratios one folder's contents produced, in per mille.
///
/// Integers rather than [`Fx`] because every comparison in the asset file is against a per
/// mille bound, and comparing in the unit the file is written in is one fewer conversion to
/// be wrong about.
#[derive(Debug, Clone, Copy, Default)]
struct Measured {
    code: i64,
    asset: i64,
    config: i64,
    docs: i64,
    generated: i64,
    binary: i64,
    unknown: i64,
    test_like: i64,
    language_concentration: i64,
    size_share: i64,
}

impl Measured {
    fn get(&self, ratio: RatioName) -> i64 {
        match ratio {
            RatioName::Code => self.code,
            RatioName::Asset => self.asset,
            RatioName::Config => self.config,
            RatioName::Docs => self.docs,
            RatioName::Generated => self.generated,
            RatioName::Binary => self.binary,
            RatioName::Unknown => self.unknown,
            RatioName::TestLike => self.test_like,
            RatioName::LanguageConcentration => self.language_concentration,
            RatioName::SizeShare => self.size_share,
        }
    }

    fn modulation(&self) -> ContentModulation {
        ContentModulation {
            language_concentration: per_mille(self.language_concentration),
            size_ratio: per_mille(self.size_share),
            binary_ratio: per_mille(self.binary),
            test_like_ratio: per_mille(self.test_like),
            generated_ratio: per_mille(self.generated),
        }
    }
}

/// Attaches a [`FolderSignal`] to every directory whose name matches the dictionary.
///
/// Returns how many signals were recorded. Must run after [`walk`](crate::walk) and
/// [`lang::scan`](crate::lang::scan): the weights are modulated by content categories, and a
/// tree that has not been scanned would modulate every signal the same way — downward,
/// because a folder with no measured content looks like a folder with no matching content.
pub fn apply(
    records: &mut [PathRecord],
    dictionary: &SignalDictionary,
    catalogue: &Catalogue,
) -> usize {
    let repository_bytes = records
        .iter()
        .find(|record| record.path.is_root())
        .map_or(0, |root| root.size.bytes);
    let test_bytes = roll_up_test_bytes(records, catalogue);

    let mut recorded = 0;
    for (index, record) in records.iter_mut().enumerate() {
        if record.kind != NodeKind::Directory || record.path.is_root() {
            continue;
        }
        let Some(name) = record.path.file_name() else {
            continue;
        };
        let Some(entry) = dictionary.lookup(name) else {
            continue;
        };

        let measured = measure(record, test_bytes[index], repository_bytes);
        let adjustment: i64 = entry
            .evidence
            .iter()
            .filter(|rule| rule.applies(&measured))
            .map(|rule| i64::from(rule.adjust))
            .sum();
        let effective = (i64::from(entry.weight) + adjustment).clamp(0, WHOLE);

        record.folder_signal = Some(FolderSignal {
            signal_name: entry.name.as_str().into(),
            default_semantic_weight: entry.default_weight(),
            content_modulation: measured.modulation(),
            effective_weight: per_mille(effective),
            position_in_hierarchy: HierarchyPosition {
                depth: record.path.depth(),
                // Filled in below, once every signal is known.
                ancestor_signals: Box::default(),
            },
        });
        recorded += 1;
    }

    attach_ancestors(records);
    recorded
}

/// Reads one folder's content ratios off the record the earlier passes filled.
fn measure(record: &PathRecord, test_bytes: u64, repository_bytes: u64) -> Measured {
    let categorized: u64 = record.size.category_bytes.values().sum();
    let share = |category: ContentCategory| {
        ratio(
            record
                .size
                .category_bytes
                .get(&category)
                .copied()
                .unwrap_or(0),
            categorized,
        )
    };

    let typed: u64 = record.size.language_bytes.values().sum();
    let dominant = record
        .size
        .language_bytes
        .values()
        .copied()
        .max()
        .unwrap_or(0);

    Measured {
        code: share(ContentCategory::Code),
        asset: share(ContentCategory::Asset),
        config: share(ContentCategory::Config),
        docs: share(ContentCategory::Docs),
        generated: share(ContentCategory::Generated),
        binary: share(ContentCategory::Binary),
        unknown: share(ContentCategory::Unknown),
        test_like: ratio(test_bytes, categorized),
        // A folder with no typed bytes is not concentrated in one language; it has none.
        // Zero rather than the 1000 a `max / max` would give for a single empty entry.
        language_concentration: ratio(dominant, typed),
        size_share: ratio(record.size.bytes, repository_bytes),
    }
}

/// Bytes in test-like files beneath each record.
///
/// Recomputed rather than carried out of [`lang::scan`](crate::lang::scan): classification by
/// name is pure and cheap, and threading a side table through two passes to save a string
/// compare per file would couple them for nothing.
fn roll_up_test_bytes(records: &[PathRecord], catalogue: &Catalogue) -> Vec<u64> {
    let parents = crate::walk::parent_indices(records);
    let mut bytes = vec![0u64; records.len()];

    for index in (0..records.len()).rev() {
        let record = &records[index];
        if record.kind == NodeKind::File && catalogue.classify(&record.path, None).is_test {
            bytes[index] = bytes[index].saturating_add(record.size.bytes);
        }
        if let Some(parent) = parents[index] {
            bytes[parent] = bytes[parent].saturating_add(bytes[index]);
        }
    }
    bytes
}

/// Records, on each signal, the signalled folders that enclose it.
///
/// A forward pass: records are sorted, so every ancestor is visited before its descendants
/// and the list can be extended rather than rebuilt by walking up.
fn attach_ancestors(records: &mut [PathRecord]) {
    let parents = crate::walk::parent_indices(records);
    // Root-first signal names on the path *to* each record, excluding the record itself.
    let mut chains: Vec<Vec<Box<str>>> = vec![Vec::new(); records.len()];

    for index in 0..records.len() {
        if let Some(parent) = parents[index] {
            let mut chain = chains[parent].clone();
            if let Some(signal) = &records[parent].folder_signal {
                chain.push(signal.signal_name.clone());
            }
            chains[index] = chain;
        }
        if let Some(signal) = &mut records[index].folder_signal {
            signal.position_in_hierarchy.ancestor_signals =
                core::mem::take(&mut chains[index]).clone().into();
            // Put it back: a descendant still needs its ancestors' chain.
            chains[index] = signal.position_in_hierarchy.ancestor_signals.to_vec();
        }
    }
}

/// `part / whole` in per mille, saturating rather than wrapping.
fn ratio(part: u64, whole: u64) -> i64 {
    if whole == 0 {
        return 0;
    }
    let part = u128::from(part) * (WHOLE as u128);
    i64::try_from(part / u128::from(whole)).unwrap_or(WHOLE)
}

/// Per mille as fixed point.
fn per_mille(value: i64) -> Fx {
    Fx::from_ratio(value, WHOLE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use treepo_model::path::RepoPath;
    use treepo_model::primitives::size::SizePrimitives;

    fn path(s: &str) -> RepoPath {
        RepoPath::new(s.as_bytes()).expect("valid test path")
    }

    fn dictionary() -> SignalDictionary {
        SignalDictionary::built_in()
    }

    fn directory(p: &str) -> PathRecord {
        PathRecord::new(path(p), NodeKind::Directory)
    }

    /// A file as `lang::scan` would have left it: one category, and one language when the
    /// category is textual. Setting only the category would leave every folder reading as
    /// having no language at all, which is a real state but not the common one.
    fn file(p: &str, bytes: u64, category: ContentCategory) -> PathRecord {
        let mut record = PathRecord::new(path(p), NodeKind::File);
        record.size = SizePrimitives {
            bytes,
            category_bytes: core::iter::once((category, bytes)).collect(),
            language_bytes: if category.is_textual() {
                core::iter::once((treepo_model::manifest::LanguageId::new(0), bytes)).collect()
            } else {
                Default::default()
            },
            ..SizePrimitives::default()
        };
        record
    }

    /// Sorts, rolls bytes and categories up, and applies the dictionary — the state `apply`
    /// sees for real, after `walk` and `lang::scan`.
    fn signalled(mut records: Vec<PathRecord>) -> BTreeMap<RepoPath, PathRecord> {
        records.sort_by(|a, b| a.path.cmp(&b.path));
        let parents = crate::walk::parent_indices(&records);
        for index in (0..records.len()).rev() {
            let Some(parent) = parents[index] else {
                continue;
            };
            let (bytes, categories, languages) = {
                let record = &records[index];
                (
                    record.size.bytes,
                    record.size.category_bytes.clone(),
                    record.size.language_bytes.clone(),
                )
            };
            records[parent].size.bytes += bytes;
            for (category, count) in categories {
                *records[parent]
                    .size
                    .category_bytes
                    .entry(category)
                    .or_insert(0) += count;
            }
            for (language, count) in languages {
                *records[parent]
                    .size
                    .language_bytes
                    .entry(language)
                    .or_insert(0) += count;
            }
        }
        apply(&mut records, &dictionary(), &Catalogue::built_in());
        records
            .into_iter()
            .map(|record| (record.path.clone(), record))
            .collect()
    }

    fn signal_of<'a>(tree: &'a BTreeMap<RepoPath, PathRecord>, p: &str) -> &'a FolderSignal {
        tree[&path(p)]
            .folder_signal
            .as_ref()
            .unwrap_or_else(|| panic!("{p} should carry a signal"))
    }

    /// The asset is compiled in, so a malformed edit must fail here, not at runtime.
    #[test]
    fn the_built_in_dictionary_parses() {
        let dictionary = dictionary();
        assert!(!dictionary.entries().is_empty());
        for entry in dictionary.entries() {
            assert!(!entry.name.is_empty(), "every signal needs a name");
            assert!(
                !entry.meaning.is_empty(),
                "{} needs a user-facing meaning",
                entry.name
            );
            assert!(
                entry.weight <= u32::try_from(WHOLE).expect("fits"),
                "{} weighs more than 1.0",
                entry.name
            );
            for rule in entry.evidence() {
                assert!(
                    rule.above.is_some() || rule.below.is_some(),
                    "{} has an unconditional rule",
                    entry.name
                );
                assert!(
                    rule.adjust != 0,
                    "{} has a rule that does nothing",
                    entry.name
                );
            }
        }
    }

    /// Test-likeness is decided partly by directory name, so a `TestLike` rule on a signal
    /// whose own name is a test-directory name always fires and can never contradict. The
    /// first draft of the dictionary had exactly that on `tests`, and the rule meant to
    /// catch a `tests` folder with no tests in it was dead on arrival.
    #[test]
    fn no_signal_tests_a_ratio_its_own_name_determines() {
        let catalogue = Catalogue::built_in();
        let test_directories: Vec<&str> = catalogue
            .test_directories()
            .iter()
            .map(String::as_str)
            .collect();

        for entry in dictionary().entries() {
            let self_confirming = core::iter::once(entry.name.as_str())
                .chain(entry.names().iter().map(String::as_str))
                .any(|name| test_directories.contains(&name));
            if !self_confirming {
                continue;
            }
            for rule in entry.evidence() {
                assert_ne!(
                    rule.ratio,
                    RatioName::TestLike,
                    "`{}` matches a test-directory name, so a TestLike rule confirms itself",
                    entry.name
                );
            }
        }
    }

    /// Every name the design section lists must reach a convention.
    #[test]
    fn the_names_the_design_names_are_all_covered() {
        let dictionary = dictionary();
        for name in [
            "public", "src", "lib", "internal", "vendor", "assets", "docs", "scripts", "test",
            "tests", "examples", "pkg", "cmd", "crates",
        ] {
            assert!(
                dictionary.lookup(name.as_bytes()).is_some(),
                "{name} is named in design/feature-system.md §3.1"
            );
        }
        assert!(dictionary.lookup(b"nonsense").is_none());
    }

    #[test]
    fn folder_names_match_case_insensitively() {
        let dictionary = dictionary();
        let lower = dictionary.lookup(b"docs").expect("docs");
        let upper = dictionary.lookup(b"Docs").expect("Docs");
        assert_eq!(lower.name, upper.name);
        assert_eq!(
            dictionary.lookup(b"SRC").map(|e| e.name.as_str()),
            Some("src")
        );
    }

    #[test]
    fn only_named_directories_carry_a_signal() {
        let tree = signalled(vec![
            directory(""),
            directory("src"),
            file("src/main.rs", 100, ContentCategory::Code),
            directory("whatever"),
            file("whatever/a.rs", 100, ContentCategory::Code),
        ]);
        assert!(tree[&path("src")].folder_signal.is_some());
        assert!(tree[&path("whatever")].folder_signal.is_none());
        // A file is not a folder, and the root has no name to match.
        assert!(tree[&path("src/main.rs")].folder_signal.is_none());
        assert!(tree[&RepoPath::root()].folder_signal.is_none());
    }

    /// The case `design/feature-system.md` §3.1 uses to argue against booleans.
    #[test]
    fn one_name_gets_three_different_weights_from_its_contents() {
        let assets = signalled(vec![
            directory(""),
            directory("public"),
            file("public/logo.png", 1000, ContentCategory::Asset),
        ]);
        let binaries = signalled(vec![
            directory(""),
            directory("public"),
            file("public/blob.bin", 1000, ContentCategory::Binary),
        ]);
        let package = signalled(vec![
            directory(""),
            directory("public"),
            file("public/index.ts", 1000, ContentCategory::Code),
        ]);

        let assets = signal_of(&assets, "public");
        let binaries = signal_of(&binaries, "public");
        let package = signal_of(&package, "public");

        // Same name, same conventional weight, three different effective weights.
        assert_eq!(assets.signal_name, binaries.signal_name);
        assert_eq!(
            assets.default_semantic_weight,
            binaries.default_semantic_weight
        );
        assert!(assets.effective_weight > package.effective_weight);
        assert!(package.effective_weight > binaries.effective_weight);

        // And the delta says which way the contents argued.
        assert!(assets.modulation_delta() > Fx::ZERO);
        assert!(binaries.modulation_delta() < Fx::ZERO);
    }

    #[test]
    fn a_src_with_no_code_in_it_loses_most_of_its_weight() {
        let real = signalled(vec![
            directory(""),
            directory("src"),
            file("src/main.rs", 1000, ContentCategory::Code),
        ]);
        let hollow = signalled(vec![
            directory(""),
            directory("src"),
            file("src/notes.md", 1000, ContentCategory::Docs),
        ]);
        assert!(signal_of(&real, "src").modulation_delta() > Fx::ZERO);
        let lost = signal_of(&hollow, "src");
        assert!(lost.modulation_delta() < Fx::ZERO);
        assert!(lost.effective_weight < Fx::from_ratio(6, 10));
        // Still recorded, and still named `src` — the name is data, not a verdict.
        assert_eq!(&*lost.signal_name, "src");
    }

    #[test]
    fn the_modulation_that_produced_a_weight_is_stored_with_it() {
        let tree = signalled(vec![
            directory(""),
            directory("assets"),
            file("assets/a.png", 750, ContentCategory::Asset),
            file("assets/b.rs", 250, ContentCategory::Code),
        ]);
        let signal = signal_of(&tree, "assets");
        // Three quarters of the repository's bytes are under it.
        assert_eq!(signal.content_modulation.size_ratio, Fx::ONE);
        assert_eq!(signal.content_modulation.binary_ratio, Fx::ZERO);
        assert!(signal.effective_weight > signal.default_semantic_weight);
    }

    #[test]
    fn a_tests_folder_is_confirmed_by_code_not_by_its_own_name() {
        let real = signalled(vec![
            directory(""),
            directory("tests"),
            file("tests/walk.rs", 1000, ContentCategory::Code),
        ]);
        let hollow = signalled(vec![
            directory(""),
            directory("spec"),
            file("spec/notes.md", 1000, ContentCategory::Docs),
        ]);
        assert!(signal_of(&real, "tests").modulation_delta() > Fx::ZERO);

        // `spec` reaches the same convention under its canonical name, and loses weight
        // despite being 100% "test-like" — which it is only because of its own name.
        let hollow = signal_of(&hollow, "spec");
        assert_eq!(&*hollow.signal_name, "tests");
        assert_eq!(hollow.content_modulation.test_like_ratio, Fx::ONE);
        assert!(hollow.modulation_delta() < Fx::ZERO);
    }

    /// The non-circular use of the same ratio.
    #[test]
    fn a_src_that_is_mostly_tests_loses_weight() {
        let ordinary = signalled(vec![
            directory(""),
            directory("src"),
            file("src/main.rs", 1000, ContentCategory::Code),
        ]);
        let test_tree = signalled(vec![
            directory(""),
            directory("src"),
            directory("src/tests"),
            file("src/tests/a.rs", 1000, ContentCategory::Code),
        ]);
        assert!(
            signal_of(&test_tree, "src").effective_weight
                < signal_of(&ordinary, "src").effective_weight
        );
    }

    /// `HierarchyPosition` is the whole point of the record for `F-MAT-5`.
    #[test]
    fn enclosing_signals_travel_with_the_record() {
        let tree = signalled(vec![
            directory(""),
            directory("vendor"),
            directory("vendor/thing"),
            directory("vendor/thing/docs"),
            file("vendor/thing/docs/readme.md", 100, ContentCategory::Docs),
            directory("docs"),
            file("docs/PRD.md", 100, ContentCategory::Docs),
        ]);

        let ours = signal_of(&tree, "docs");
        assert!(!ours.is_nested());
        assert_eq!(ours.position_in_hierarchy.depth, 1);

        let theirs = signal_of(&tree, "vendor/thing/docs");
        assert!(theirs.is_nested());
        assert!(theirs.position_in_hierarchy.is_within("vendor"));
        assert!(!ours.position_in_hierarchy.is_within("vendor"));
        assert_eq!(theirs.position_in_hierarchy.depth, 3);
        // Unsignalled folders on the way do not appear, so the list stays short.
        assert_eq!(
            &*theirs.position_in_hierarchy.ancestor_signals,
            ["vendor".into()]
        );

        // Nesting is recorded, not applied: same contents, same weight.
        assert_eq!(ours.effective_weight, theirs.effective_weight);
    }

    #[test]
    fn ancestor_signals_are_root_first_and_can_stack() {
        let tree = signalled(vec![
            directory(""),
            directory("vendor"),
            directory("vendor/src"),
            directory("vendor/src/tests"),
            file("vendor/src/tests/a.rs", 100, ContentCategory::Code),
        ]);
        let deep = signal_of(&tree, "vendor/src/tests");
        assert_eq!(
            &*deep.position_in_hierarchy.ancestor_signals,
            ["vendor".into(), "src".into()]
        );
    }

    #[test]
    fn language_concentration_is_zero_when_nothing_is_typed() {
        let tree = signalled(vec![
            directory(""),
            directory("assets"),
            file("assets/a.png", 100, ContentCategory::Asset),
        ]);
        assert_eq!(
            signal_of(&tree, "assets")
                .content_modulation
                .language_concentration,
            Fx::ZERO
        );
    }

    #[test]
    fn an_unscanned_tree_still_produces_records_rather_than_failing() {
        // No categories anywhere — what `apply` would see if `lang::scan` had not run.
        let mut records = vec![directory(""), directory("src")];
        let recorded = apply(&mut records, &dictionary(), &Catalogue::built_in());
        assert_eq!(recorded, 1);
        let signal = records[1].folder_signal.as_ref().expect("recorded");
        // Every ratio reads zero, so the contradicting rules fire. That is why `apply` is
        // documented as running after the content pass.
        assert!(signal.effective_weight < signal.default_semantic_weight);
    }

    #[test]
    fn a_custom_dictionary_can_be_supplied() {
        let text = r#"#![enable(implicit_some)]
        (
            version: 1,
            signals: [(
                name: "toybox",
                weight: 400,
                names: ["toys"],
                meaning: "somewhere to put things",
                evidence: [(ratio: Code, above: 500, adjust: 200)],
            )],
        )"#;
        let dictionary = SignalDictionary::from_ron(text).expect("parses");
        assert_eq!(
            dictionary.lookup(b"toys").map(|e| e.name.as_str()),
            Some("toybox")
        );
        // The canonical name matches even when it is not repeated in `names`.
        assert!(dictionary.lookup(b"toybox").is_some());
        assert!(dictionary.lookup(b"src").is_none());
        assert!(SignalDictionary::from_ron("(nonsense").is_err());
    }
}
