//! `F-EXT-4` — what a file is written in, and how much of it is code.
//!
//! Two halves that meet in [`scan`]:
//!
//! * **Classification**, which needs only the path. [`Catalogue`] maps an extension or a file
//!   name onto a language and a [`ContentCategory`], and `.gitattributes` `linguist-*` markers
//!   override it — the last of the five `F-EXT-8` rules, which [`filter`](crate::filter)
//!   deferred to here because it needs the attribute stack that only this module reads.
//! * **Counting**, which needs the bytes. [`count_lines`] separates code from comment from
//!   blank, and [`count_markers`] finds the TODO/FIXME density of `F-EXT-6`.
//!
//! # Counting comments without parsing
//!
//! `AC-EXT-4` forbids evaluating repository content, so there is no parser here and there
//! never will be — a language plugin that loads project configuration is exactly the code
//! path `N1` exists to close. What is left is a state machine over comment delimiters, and
//! it is wrong in ways worth naming:
//!
//! * **Only a line whose first non-blank content is a comment marker counts as a comment.**
//!   `let url = "https://example.com";` is code, because the `//` is not at the front. This
//!   single rule removes the largest class of false positives at no cost.
//! * **A block comment must open at the start of a line to be tracked across lines.** A
//!   mid-line `/*` is ignored entirely. The alternative — tracking it — means a lone `"/*"`
//!   inside a string literal makes the rest of the file read as one enormous comment. An
//!   undercount of a rare construct beats a whole file misread, and the failure that was
//!   chosen against is the one that would be invisible.
//! * **Nesting is not tracked.** Rust's `/* /* */ */` closes at the first `*/`.
//!
//! The output feeds a comment-density signal that modulates how a limb looks. It is not a
//! coverage report, and the precision it needs is "roughly, is this documented".
//!
//! # `Unknown` is not counted
//!
//! [`ContentCategory::is_textual`] gates line counting, and `Unknown` is not textual — an
//! unrecognized extension contributes bytes but no lines. Opening every unrecognized file to
//! guess whether it is text would trade a large share of the `AC-EXT-1` budget for a guess,
//! and the honest reading of an unfamiliar repository is that treepo did not recognize it.
//! Adding an entry to `assets/languages/languages.ron` is the fix, and it is visible.

use crate::discover::Target;
use crate::walk::{FileSource, Structure};
use gix::glob::wildmatch;
use serde::Deserialize;
use std::collections::BTreeMap;
use treepo_det::Fx;
use treepo_model::manifest::{LanguageTable, NodeKind, PathRecord};
use treepo_model::path::RepoPath;
use treepo_model::primitives::DerivedSignals;
use treepo_model::primitives::size::{ContentCategory, LineCounts};

/// The compiled-in catalogue. See [`filter`](crate::filter) for why `include_str!`.
const BUILT_IN_RON: &str = include_str!("../../../assets/languages/languages.ron");

/// Globs here match a file name, with git's `*`/`?`/`**` semantics.
const MATCH_MODE: wildmatch::Mode = wildmatch::Mode::NO_MATCH_SLASH_LITERAL;

/// Bytes sniffed for a NUL before deciding a file is binary. Git's own window, so a file
/// treepo calls binary is a file `git diff` also refuses to show.
const BINARY_SNIFF_BYTES: usize = 8000;

/// The markers [`count_markers`] looks for, lowercased; matching is ASCII-case-insensitive.
const DEBT_MARKERS: [&[u8]; 2] = [b"todo", b"fixme"];

/// [`ContentCategory`] as it is spelled in the asset file.
///
/// A local mirror because `treepo-model` is `no_std` and derives no `serde` impls — the
/// price of the constraint that keeps the model free of I/O, paid once, here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum CategoryName {
    Code,
    Asset,
    Config,
    Docs,
    Generated,
    Binary,
    Unknown,
}

impl From<CategoryName> for ContentCategory {
    fn from(name: CategoryName) -> Self {
        match name {
            CategoryName::Code => Self::Code,
            CategoryName::Asset => Self::Asset,
            CategoryName::Config => Self::Config,
            CategoryName::Docs => Self::Docs,
            CategoryName::Generated => Self::Generated,
            CategoryName::Binary => Self::Binary,
            CategoryName::Unknown => Self::Unknown,
        }
    }
}

/// One language, as the catalogue describes it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Language {
    /// The name recorded in `Manifest::languages`.
    pub name: String,
    category: CategoryName,
    #[serde(default)]
    extensions: Vec<String>,
    #[serde(default)]
    filenames: Vec<String>,
    #[serde(default)]
    line_comment: Vec<String>,
    #[serde(default)]
    block_comment: Vec<(String, String)>,
}

impl Language {
    /// What kind of content this language is.
    #[must_use]
    pub fn category(&self) -> ContentCategory {
        self.category.into()
    }

    /// Prefixes that make the rest of a line a comment.
    #[must_use]
    pub fn line_comments(&self) -> &[String] {
        &self.line_comment
    }

    /// Delimiter pairs that open and close a multi-line comment.
    #[must_use]
    pub fn block_comments(&self) -> &[(String, String)] {
        &self.block_comment
    }

    /// Whether any comment syntax is defined. JSON, for one, has none.
    #[must_use]
    pub fn has_comments(&self) -> bool {
        !self.line_comment.is_empty() || !self.block_comment.is_empty()
    }
}

/// Extensions carrying a category but no language.
#[derive(Debug, Clone, Deserialize)]
struct CategoryGroup {
    category: CategoryName,
    extensions: Vec<String>,
}

/// The parsed contents of `assets/languages/languages.ron`.
#[derive(Debug, Clone, Deserialize)]
struct CatalogueFile {
    version: u32,
    languages: Vec<Language>,
    categories: Vec<CategoryGroup>,
    test_directories: Vec<String>,
    test_files: Vec<String>,
    generated_files: Vec<String>,
}

/// A `.gitattributes` marker that overrides the catalogue.
///
/// Both map to [`ContentCategory::Generated`]: the category has no `Vendored` variant, and
/// `design/feature-system.md` §8.5 treats generated and vendored content identically — both
/// get the "more uniform, machined" material. Distinguishing them would add a variant no
/// renderer would read differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marker {
    /// `linguist-generated`.
    Generated,
    /// `linguist-vendored`.
    Vendored,
}

/// What one path is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Classification<'a> {
    /// The language, or `None` if the catalogue does not name one.
    pub language: Option<&'a Language>,
    /// The content category, after any `linguist-*` marker is applied.
    pub category: ContentCategory,
    /// Whether the path looks like a test, by directory or by naming convention.
    ///
    /// Independent of category: a test is still `Code`. `F-EXT-6`'s `test_to_source` needs
    /// the distinction, and `F-EXT-5`'s content modulation will want it too.
    pub is_test: bool,
}

impl Classification<'_> {
    /// Whether counting lines in this path is worth the read.
    #[must_use]
    pub fn is_countable(&self) -> bool {
        self.category.is_textual()
    }
}

/// The language and content catalogue (`F-EXT-4`).
#[derive(Debug, Clone)]
pub struct Catalogue {
    languages: Vec<Language>,
    /// Lowercased extension to language index.
    by_extension: BTreeMap<Box<str>, usize>,
    /// Lowercased whole file name to language index.
    by_filename: BTreeMap<Box<str>, usize>,
    /// Lowercased extension to a category with no language.
    bare_extensions: BTreeMap<Box<str>, ContentCategory>,
    test_directories: Vec<String>,
    test_files: Vec<String>,
    generated_files: Vec<String>,
}

impl Catalogue {
    /// The shipped catalogue.
    ///
    /// # Panics
    ///
    /// If the compiled-in asset does not parse — a build-time error this module's tests
    /// catch, not something a repository can trigger.
    #[must_use]
    pub fn built_in() -> Self {
        Self::from_ron(BUILT_IN_RON).expect("built-in language catalogue must parse")
    }

    /// Parses a catalogue from RON, for a caller supplying its own.
    ///
    /// # Errors
    ///
    /// Returns the RON parse error if `text` is not a valid catalogue.
    pub fn from_ron(text: &str) -> Result<Self, ron::error::SpannedError> {
        let file: CatalogueFile = ron::from_str(text)?;
        let mut by_extension = BTreeMap::new();
        let mut by_filename = BTreeMap::new();

        for (index, language) in file.languages.iter().enumerate() {
            for extension in &language.extensions {
                // First entry wins, so an earlier language claiming `.h` keeps it. Ordering
                // in the asset file is therefore meaningful, and the tests below pin the
                // couple of cases where two languages really do overlap.
                by_extension.entry(fold(extension)).or_insert(index);
            }
            for name in &language.filenames {
                by_filename.entry(fold(name)).or_insert(index);
            }
        }

        let mut bare_extensions = BTreeMap::new();
        for group in &file.categories {
            for extension in &group.extensions {
                bare_extensions
                    .entry(fold(extension))
                    .or_insert_with(|| group.category.into());
            }
        }

        debug_assert_eq!(file.version, 1, "catalogue schema version");
        Ok(Self {
            languages: file.languages,
            by_extension,
            by_filename,
            bare_extensions,
            test_directories: file.test_directories,
            test_files: file.test_files,
            generated_files: file.generated_files,
        })
    }

    /// Every language in the catalogue.
    #[must_use]
    pub fn languages(&self) -> &[Language] {
        &self.languages
    }

    /// What `path` is, given any `.gitattributes` marker that applies to it.
    ///
    /// Precedence, highest first: an explicit `linguist-*` marker, a generated-file naming
    /// convention, an exact file name, an extension. The marker wins because it is the
    /// repository stating a fact about itself, which beats any guess made from a suffix.
    #[must_use]
    pub fn classify(&self, path: &RepoPath, marker: Option<Marker>) -> Classification<'_> {
        let name = path.file_name().unwrap_or_default();
        let language = self.language_for(path, name);

        let category = if marker.is_some() || self.is_generated_name(name) {
            ContentCategory::Generated
        } else {
            language
                .map(Language::category)
                .or_else(|| self.bare_category(path))
                .unwrap_or(ContentCategory::Unknown)
        };

        Classification {
            language,
            category,
            is_test: self.is_test_path(path, name),
        }
    }

    /// The language for a path, by exact name first and extension second.
    fn language_for(&self, path: &RepoPath, name: &[u8]) -> Option<&Language> {
        let by_name = self.by_filename.get(fold_bytes(name).as_str()).copied();
        let by_extension = || {
            let extension = path.extension()?;
            self.by_extension
                .get(fold_bytes(extension).as_str())
                .copied()
        };
        by_name
            .or_else(by_extension)
            .and_then(|index| self.languages.get(index))
    }

    /// The category for an extension the language list does not claim.
    fn bare_category(&self, path: &RepoPath) -> Option<ContentCategory> {
        let extension = path.extension()?;
        self.bare_extensions
            .get(fold_bytes(extension).as_str())
            .copied()
    }

    /// Whether a file name matches a generated-output convention.
    fn is_generated_name(&self, name: &[u8]) -> bool {
        self.generated_files
            .iter()
            .any(|pattern| matches_name(pattern, name))
    }

    /// Whether a path is test-like, by any directory component or by its own name.
    fn is_test_path(&self, path: &RepoPath, name: &[u8]) -> bool {
        let in_test_directory = path.components().any(|component| {
            let folded = fold_bytes(component);
            self.test_directories
                .iter()
                .any(|directory| directory.as_str() == folded)
        });
        in_test_directory
            || self
                .test_files
                .iter()
                .any(|pattern| matches_name(pattern, name))
    }
}

/// The comment syntax [`count_lines`] works from.
///
/// Borrowed from a [`Language`], or absent for content with a category but no language.
#[derive(Debug, Clone, Copy, Default)]
pub struct CommentSyntax<'a> {
    /// Prefixes that comment out the rest of a line.
    pub line: &'a [String],
    /// Delimiter pairs opening and closing a multi-line comment.
    pub block: &'a [(String, String)],
}

impl<'a> CommentSyntax<'a> {
    /// The syntax a language defines, or the empty syntax for `None`.
    #[must_use]
    pub fn of(language: Option<&'a Language>) -> Self {
        language.map_or(Self::default(), |language| Self {
            line: language.line_comments(),
            block: language.block_comments(),
        })
    }
}

/// Splits `content` into code, comment, and blank lines.
///
/// With no comment syntax every non-blank line is code, which is the right answer for JSON
/// and the only defensible one for a language the catalogue does not describe. See the module
/// header for the three ways this is deliberately approximate.
#[must_use]
pub fn count_lines(content: &[u8], syntax: CommentSyntax<'_>) -> LineCounts {
    let mut counts = LineCounts::default();
    let mut open: Option<&(String, String)> = None;

    for raw in lines_of(content) {
        counts.total += 1;
        let line = raw.trim_ascii();

        if let Some((_, end)) = open {
            counts.comment += 1;
            if contains(line, end.as_bytes()) {
                open = None;
            }
            continue;
        }
        if line.is_empty() {
            counts.blank += 1;
            continue;
        }
        if syntax
            .line
            .iter()
            .any(|prefix| line.starts_with(prefix.as_bytes()))
        {
            counts.comment += 1;
            continue;
        }
        if let Some(pair) = syntax
            .block
            .iter()
            .find(|(start, _)| line.starts_with(start.as_bytes()))
        {
            counts.comment += 1;
            // A one-line `/* ... */` never opens the block state.
            if !contains(&line[pair.0.len()..], pair.1.as_bytes()) {
                open = Some(pair);
            }
            continue;
        }
        counts.code += 1;
    }

    counts
}

/// Counts TODO and FIXME markers, ASCII-case-insensitively, on word boundaries.
///
/// The boundary check is what keeps `MASTODON` and `todos` out of the count while letting
/// `TODO:` and Rust's `todo!()` in — both of those are the marker, one of them compiles.
#[must_use]
pub fn count_markers(content: &[u8]) -> u64 {
    DEBT_MARKERS
        .iter()
        .map(|marker| count_word(content, marker))
        .sum()
}

/// Whether content is binary, by git's rule: a NUL in the first [`BINARY_SNIFF_BYTES`].
#[must_use]
pub fn looks_binary(content: &[u8]) -> bool {
    content
        .iter()
        .take(BINARY_SNIFF_BYTES)
        .any(|&byte| byte == 0)
}

/// Lines of `content`, without their terminators.
///
/// `bstr`'s, not a hand-rolled split: it strips `\r\n` as one terminator, yields nothing for
/// an empty file rather than one empty line, and does not invent a final line after a
/// trailing newline. All three are edge cases a hand-rolled version gets wrong once.
fn lines_of(content: &[u8]) -> impl Iterator<Item = &[u8]> {
    use gix::bstr::ByteSlice as _;
    content.lines()
}

/// Whether `haystack` contains `needle`.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    use gix::bstr::ByteSlice as _;
    haystack.find(needle).is_some()
}

/// Occurrences of `needle` in `haystack` that are not part of a longer word.
fn count_word(haystack: &[u8], needle: &[u8]) -> u64 {
    let mut count = 0;
    let mut index = 0;
    while index + needle.len() <= haystack.len() {
        let candidate = &haystack[index..index + needle.len()];
        let bounded = candidate.eq_ignore_ascii_case(needle)
            && !is_word_byte(index.checked_sub(1).map(|before| haystack[before]))
            && !is_word_byte(haystack.get(index + needle.len()).copied());
        if bounded {
            count += 1;
            index += needle.len();
        } else {
            index += 1;
        }
    }
    count
}

fn is_word_byte(byte: Option<u8>) -> bool {
    byte.is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

/// Whether a file-name glob matches. Case-sensitive: `*Test.java` and `Cargo.lock` spell
/// their casing on purpose, unlike the extension lookup where case carries no information.
fn matches_name(pattern: &str, name: &[u8]) -> bool {
    use gix::bstr::ByteSlice as _;
    wildmatch(pattern.as_bytes().as_bstr(), name.as_bstr(), MATCH_MODE)
}

fn fold(text: &str) -> Box<str> {
    text.to_ascii_lowercase().into_boxed_str()
}

// ---------------------------------------------------------------------------------------
// `.gitattributes` — the fifth `F-EXT-8` rule
// ---------------------------------------------------------------------------------------

/// The `linguist-*` markers a repository declares about its own files (`F-EXT-8` rule 4).
///
/// # Read from the tree, not from the attribute stack
///
/// `gix` can resolve attributes properly, but doing so needs the index and can reach into
/// the working directory — and [`walk`](crate::walk) advertises that the HEAD-tree path
/// touches neither. Two attributes do not justify giving that up, so this reads
/// `.gitattributes` blobs out of the tree that was already walked.
///
/// What that costs is the parts of the format nobody uses for linguist markers: macros,
/// `info/attributes`, and the user's global attributes file. What it keeps is the property
/// that extraction of a committed tree depends on nothing outside that tree — which is also
/// what makes it reproducible on another machine (`AC-DET-2`).
#[derive(Debug, Clone, Default)]
pub struct Attributes {
    /// Shallowest directory first, then file order, so a later match is a more specific one.
    rules: Vec<AttributeRule>,
    files: usize,
}

/// One `.gitattributes` line that mentions a marker.
#[derive(Debug, Clone)]
struct AttributeRule {
    /// The directory the file lives in; the pattern is relative to it.
    directory: RepoPath,
    pattern: gix::glob::Pattern,
    generated: Option<bool>,
    vendored: Option<bool>,
}

impl AttributeRule {
    fn matches(&self, path: &RepoPath) -> bool {
        use gix::bstr::ByteSlice as _;
        let raw = path.as_bytes();
        let relative = if self.directory.is_root() {
            raw
        } else {
            let prefix = self.directory.as_bytes();
            // Must be strictly beneath, on a component boundary: `srcx/a` is not in `src`.
            if raw.len() <= prefix.len() || !raw.starts_with(prefix) || raw[prefix.len()] != b'/' {
                return false;
            }
            &raw[prefix.len() + 1..]
        };
        let basename = relative
            .iter()
            .rposition(|&byte| byte == b'/')
            .map(|at| at + 1);
        self.pattern.matches_repo_relative_path(
            relative.as_bstr(),
            basename,
            Some(false),
            gix::glob::pattern::Case::Sensitive,
            MATCH_MODE,
        )
    }
}

impl Attributes {
    /// No attributes at all.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Adds the rules from one `.gitattributes`, living in `directory`.
    ///
    /// Call shallowest-first: within one attribute, the last matching rule wins, which is
    /// git's own precedence and makes a deeper file override a shallower one for free.
    pub fn add(&mut self, directory: &RepoPath, content: &[u8]) {
        self.files += 1;
        for line in gix::attrs::parse(content) {
            let Ok((kind, assignments, _line)) = line else {
                // A malformed line is skipped, not fatal. `F-ASSOC-7`: a repository treepo
                // cannot fully parse is still a repository treepo renders.
                continue;
            };
            let gix::attrs::parse::Kind::Pattern(pattern) = kind else {
                continue; // A macro definition assigns nothing to a path.
            };

            let mut generated = None;
            let mut vendored = None;
            for assignment in assignments.flatten() {
                let value = match assignment.state {
                    gix::attrs::StateRef::Set => Some(true),
                    gix::attrs::StateRef::Unset => Some(false),
                    // `linguist-generated=true` is the spelling GitHub documents, and
                    // `=false` is how a repository opts a single file back out.
                    gix::attrs::StateRef::Value(value) => Some(value.as_bstr() != "false"),
                    gix::attrs::StateRef::Unspecified => None,
                };
                match assignment.name.as_str() {
                    "linguist-generated" => generated = value,
                    "linguist-vendored" => vendored = value,
                    _ => {}
                }
            }

            if generated.is_some() || vendored.is_some() {
                self.rules.push(AttributeRule {
                    directory: directory.clone(),
                    pattern,
                    generated,
                    vendored,
                });
            }
        }
    }

    /// The marker in force for `path`, if any.
    #[must_use]
    pub fn marker_for(&self, path: &RepoPath) -> Option<Marker> {
        let mut generated = false;
        let mut vendored = false;
        for rule in &self.rules {
            if !rule.matches(path) {
                continue;
            }
            if let Some(value) = rule.generated {
                generated = value;
            }
            if let Some(value) = rule.vendored {
                vendored = value;
            }
        }
        match (generated, vendored) {
            (true, _) => Some(Marker::Generated),
            (false, true) => Some(Marker::Vendored),
            (false, false) => None,
        }
    }

    /// How many `.gitattributes` files contributed.
    #[must_use]
    pub fn file_count(&self) -> usize {
        self.files
    }

    /// Whether any rule was found.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

// ---------------------------------------------------------------------------------------
// The content pass
// ---------------------------------------------------------------------------------------

/// Seconds in a day, for `doc_staleness_days`.
const DAY: i64 = 86_400;

/// Knobs for the content pass.
#[derive(Debug, Clone, Copy)]
pub struct ContentOptions {
    /// Files larger than this are classified but not read.
    ///
    /// The guard is against a pathological blob, not against ordinary large files: it is
    /// four times [`LARGE_FILE_BYTES`](crate::walk::LARGE_FILE_BYTES), so a file already
    /// flagged as an outlier is still counted. A file above it contributes its bytes and its
    /// category — both of which are known without reading — and no lines, and
    /// [`ScanReport::too_large`] says how many.
    pub max_scan_bytes: u64,
    /// Bytes above which a file counts toward `large_file_debt`.
    ///
    /// Defaults to [`LARGE_FILE_BYTES`](crate::walk::LARGE_FILE_BYTES); pass whatever
    /// [`WalkOptions`](crate::walk::WalkOptions) was given, or the debt and the count will
    /// describe different sets of files.
    pub large_file_bytes: u64,
}

impl Default for ContentOptions {
    fn default() -> Self {
        Self {
            max_scan_bytes: 4 * crate::walk::LARGE_FILE_BYTES,
            large_file_bytes: crate::walk::LARGE_FILE_BYTES,
        }
    }
}

/// What one content pass did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScanReport {
    /// Files whose bytes were read and counted.
    pub scanned: usize,
    /// Files skipped by [`ContentOptions::max_scan_bytes`].
    pub too_large: usize,
    /// Files the catalogue called textual whose bytes contained a NUL.
    pub reclassified_binary: usize,
    /// `.gitattributes` files that contributed a `linguist-*` rule.
    pub attribute_files: usize,
}

/// Why a content pass could not complete.
#[derive(Debug)]
pub enum ScanError {
    /// A blob the tree referenced could not be read.
    Object(String),
    /// The filesystem refused, on the no-repository path.
    Io(std::io::Error),
}

impl std::fmt::Display for ScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Object(message) => write!(f, "could not read a git object: {message}"),
            Self::Io(source) => write!(f, "content scan failed: {source}"),
        }
    }
}

impl std::error::Error for ScanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
            Self::Object(_) => None,
        }
    }
}

impl From<std::io::Error> for ScanError {
    fn from(source: std::io::Error) -> Self {
        Self::Io(source)
    }
}

/// Per-path totals the manifest does not store, accumulated during the rollup.
#[derive(Debug, Clone, Copy, Default)]
struct Totals {
    /// Bytes in test-like `Code` files. Source bytes are `Code` bytes minus these.
    test_code_bytes: u64,
    /// TODO and FIXME markers found.
    debt_markers: u64,
    /// Bytes in files at or above the large-file threshold.
    large_bytes: u64,
    /// Files whose content was actually read.
    counted_files: u32,
}

impl Totals {
    fn absorb(&mut self, child: Self) {
        self.test_code_bytes = self.test_code_bytes.saturating_add(child.test_code_bytes);
        self.debt_markers = self.debt_markers.saturating_add(child.debt_markers);
        self.large_bytes = self.large_bytes.saturating_add(child.large_bytes);
        self.counted_files = self.counted_files.saturating_add(child.counted_files);
    }
}

/// Fills in language, lines, and content categories for a walked structure (`F-EXT-4`).
///
/// Reads every countable file once, rolls the result up the tree, and derives the signals of
/// `F-EXT-6` that do not need history. Two of those do —
/// [`apply_history_signals`] fills them, and must run after both this and
/// [`log_pass::apply`](crate::log_pass::apply).
///
/// # Errors
///
/// [`ScanError`] if a blob or a file cannot be read.
pub fn scan(
    target: &Target,
    structure: &mut Structure,
    catalogue: &Catalogue,
    languages: &mut LanguageTable,
    options: ContentOptions,
) -> Result<ScanReport, ScanError> {
    let repo = target.repository();
    let Structure {
        records, sources, ..
    } = structure;

    let attributes = load_attributes(repo, records, sources)?;
    let mut report = ScanReport {
        attribute_files: attributes.file_count(),
        ..ScanReport::default()
    };
    let mut totals = vec![Totals::default(); records.len()];

    for index in 0..records.len() {
        if records[index].kind != NodeKind::File {
            continue;
        }
        let path = records[index].path.clone();
        let bytes = records[index].size.bytes;
        let mut class = catalogue.classify(&path, attributes.marker_for(&path));

        let mut lines = LineCounts::default();
        let mut markers = 0;
        let mut counted = false;

        if class.is_countable() {
            if bytes > options.max_scan_bytes {
                report.too_large += 1;
            } else if let Some(source) = sources.get(&path) {
                let content = read_content(source, repo)?;
                if looks_binary(&content) {
                    // The suffix said text and the bytes disagree. The bytes are the
                    // evidence; a `.txt` full of NULs is not documentation.
                    class.category = ContentCategory::Binary;
                    report.reclassified_binary += 1;
                } else {
                    lines = count_lines(&content, CommentSyntax::of(class.language));
                    markers = count_markers(&content);
                    counted = true;
                    report.scanned += 1;
                }
            }
        }

        totals[index] = Totals {
            test_code_bytes: u64::from(class.is_test && class.category == ContentCategory::Code)
                * bytes,
            debt_markers: markers,
            large_bytes: u64::from(bytes >= options.large_file_bytes) * bytes,
            counted_files: u32::from(counted),
        };

        let record = &mut records[index];
        record.size.lines = lines;
        record.size.category_bytes = core::iter::once((class.category, bytes)).collect();
        // Language only where the category is textual: a reclassified binary is not written
        // in the language its suffix claimed, and a texture is not written in one at all.
        record.size.language_bytes = match class.language.filter(|_| class.category.is_textual()) {
            Some(language) => core::iter::once((languages.intern(&language.name), bytes)).collect(),
            None => Default::default(),
        };
    }

    roll_up_content(records, &mut totals);
    derive_signals(records, &totals);
    Ok(report)
}

/// Reads every `.gitattributes` in the structure, shallowest first.
fn load_attributes(
    repo: Option<&gix::Repository>,
    records: &[PathRecord],
    sources: &BTreeMap<RepoPath, FileSource>,
) -> Result<Attributes, ScanError> {
    let mut found: Vec<&RepoPath> = records
        .iter()
        .filter(|record| record.kind == NodeKind::File)
        .filter(|record| record.path.file_name() == Some(b".gitattributes"))
        .map(|record| &record.path)
        .collect();
    // Depth first, then path, so `add`'s "last match wins" resolves to "deepest wins".
    found.sort_by_key(|path| (path.depth(), *path));

    let mut attributes = Attributes::none();
    for path in found {
        let Some(source) = sources.get(path) else {
            continue;
        };
        let directory = path.parent().unwrap_or_else(RepoPath::root);
        attributes.add(&directory, &read_content(source, repo)?);
    }
    Ok(attributes)
}

/// One file's bytes, from wherever the walk found it.
fn read_content(source: &FileSource, repo: Option<&gix::Repository>) -> Result<Vec<u8>, ScanError> {
    match source {
        FileSource::Blob(oid) => {
            let repo = repo.ok_or_else(|| {
                ScanError::Object("a blob source with no repository to read it from".into())
            })?;
            let object = repo
                .find_object(*oid)
                .map_err(|error| ScanError::Object(error.to_string()))?;
            Ok(object.data.clone())
        }
        FileSource::Disk(path) => Ok(std::fs::read(path)?),
    }
}

/// Sums lines, languages, categories, and the side totals into every ancestor.
///
/// Records are sorted, so a child's index always exceeds its parent's; walking backwards
/// therefore finishes a subtree before reaching the directory that contains it. Same
/// argument as [`walk::roll_up`](crate::walk), and the same parent index.
fn roll_up_content(records: &mut [PathRecord], totals: &mut [Totals]) {
    let parents = crate::walk::parent_indices(records);
    for index in (0..records.len()).rev() {
        let Some(parent) = parents[index] else {
            continue;
        };
        let (lines, languages, categories) = {
            let record = &records[index];
            (
                record.size.lines,
                record.size.language_bytes.clone(),
                record.size.category_bytes.clone(),
            )
        };
        let into = &mut records[parent];
        into.size.lines = into.size.lines.merge(lines);
        for (language, bytes) in languages {
            *into.size.language_bytes.entry(language).or_insert(0) += bytes;
        }
        for (category, bytes) in categories {
            *into.size.category_bytes.entry(category).or_insert(0) += bytes;
        }
        let child = totals[index];
        totals[parent].absorb(child);
    }
}

/// Fills `BalanceScore::kind` and the history-free half of [`DerivedSignals`].
fn derive_signals(records: &mut [PathRecord], totals: &[Totals]) {
    for (record, total) in records.iter_mut().zip(totals) {
        let category_bytes: Vec<u64> = ContentCategory::ALL
            .iter()
            .map(|category| {
                record
                    .size
                    .category_bytes
                    .get(category)
                    .copied()
                    .unwrap_or(0)
            })
            .collect();
        let all_bytes: u64 = category_bytes.iter().sum();

        // Over all seven slots, not only the ones present: a directory of nothing but code
        // is concentrated in one category, which is what a kind-imbalance means. Because a
        // real directory rarely holds more than three categories, values cluster low — but
        // consistently, which is what makes them comparable between paths.
        record.structural.balance.kind =
            (record.kind == NodeKind::Directory).then(|| crate::walk::evenness(&category_bytes));

        let code = record
            .size
            .category_bytes
            .get(&ContentCategory::Code)
            .copied()
            .unwrap_or(0);
        let source = code.saturating_sub(total.test_code_bytes);
        let generated = record
            .size
            .category_bytes
            .get(&ContentCategory::Generated)
            .copied()
            .unwrap_or(0);

        record.derived = DerivedSignals {
            comment_density: (total.counted_files > 0).then(|| record.size.lines.comment_density()),
            // No source bytes is no denominator. A directory holding only tests is a real
            // shape, but "infinitely many tests per line of source" is not a measurement.
            test_to_source: (source > 0).then(|| ratio(total.test_code_bytes, source)),
            todo_density: (total.counted_files > 0 && record.size.lines.code > 0).then(|| {
                Fx::from_ratio(
                    i64::try_from(total.debt_markers.saturating_mul(1000)).unwrap_or(i64::MAX),
                    i64::try_from(record.size.lines.code).unwrap_or(i64::MAX),
                )
            }),
            // Both of these are byte ratios, so they are measurable for anything with bytes
            // — including a path whose content was never opened.
            generated_debt: (all_bytes > 0).then(|| ratio(generated, all_bytes)),
            large_file_debt: (record.size.bytes > 0)
                .then(|| ratio(total.large_bytes, record.size.bytes)),
            // Needs commit times; see `apply_history_signals`.
            doc_staleness_days: None,
        };
    }
}

/// Fills the two signals that need both a content pass and a history pass.
///
/// Must run after [`scan`] and after [`log_pass::apply`](crate::log_pass::apply). Neither
/// `stability` nor `doc_staleness_days` can be computed from one of them alone: the first
/// divides churn in lines by a line count, and the second compares two commit times selected
/// by content category.
pub fn apply_history_signals(records: &mut [PathRecord]) {
    let parents = crate::walk::parent_indices(records);
    let count = records.len();

    // Newest commit touching a `Docs` / `Code` path in each subtree.
    let mut docs_time: Vec<Option<i64>> = vec![None; count];
    let mut code_time: Vec<Option<i64>> = vec![None; count];

    for index in (0..count).rev() {
        let record = &records[index];
        if record.kind == NodeKind::File {
            // A file has exactly one category, so reading it back beats carrying it.
            let category = record.size.category_bytes.keys().next().copied();
            let touched = record.temporal.last_commit_time;
            match category {
                Some(ContentCategory::Docs) => docs_time[index] = touched,
                Some(ContentCategory::Code) => code_time[index] = touched,
                _ => {}
            }
        }
        if let Some(parent) = parents[index] {
            docs_time[parent] = newer(docs_time[parent], docs_time[index]);
            code_time[parent] = newer(code_time[parent], code_time[index]);
        }
    }

    for (index, record) in records.iter_mut().enumerate() {
        // Positive means the docs are older than the code they accompany.
        record.derived.doc_staleness_days = docs_time[index]
            .zip(code_time[index])
            .map(|(docs, code)| (code - docs) / DAY);

        // Churn is measured in lines, so the denominator has to be lines. The 90-day window
        // is the same one `HistoryOptions::recency_half_life_days` uses — a path that reads
        // as hot there should read as unstable here, and two different windows would let
        // those disagree.
        record.temporal.stability = (record.size.lines.total > 0).then(|| {
            let churn = ratio(record.temporal.churn.days_90, record.size.lines.total);
            Fx::ONE.sub(churn.min(Fx::ONE))
        });
    }
}

fn newer(a: Option<i64>, b: Option<i64>) -> Option<i64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (found, None) | (None, found) => found,
    }
}

/// `part / whole` as fixed point, saturating rather than wrapping on absurd inputs.
fn ratio(part: u64, whole: u64) -> Fx {
    if whole == 0 {
        return Fx::ZERO;
    }
    Fx::from_ratio(
        i64::try_from(part).unwrap_or(i64::MAX),
        i64::try_from(whole).unwrap_or(i64::MAX),
    )
}

/// Path bytes as a lowercased lookup key. Lossy for non-UTF-8, which cannot match a
/// catalogue entry anyway — every extension in the asset file is ASCII.
fn fold_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(s: &str) -> RepoPath {
        RepoPath::new(s.as_bytes()).expect("valid test path")
    }

    fn catalogue() -> Catalogue {
        Catalogue::built_in()
    }

    fn classify<'a>(catalogue: &'a Catalogue, s: &str) -> Classification<'a> {
        catalogue.classify(&path(s), None)
    }

    /// The asset is compiled in, so a malformed edit must fail here, not at runtime.
    #[test]
    fn the_built_in_catalogue_parses() {
        let catalogue = catalogue();
        assert!(!catalogue.languages().is_empty());
        for language in catalogue.languages() {
            assert!(!language.name.is_empty(), "every language needs a name");
            assert!(
                !language.extensions.is_empty() || !language.filenames.is_empty(),
                "{} matches nothing",
                language.name
            );
            for (start, end) in language.block_comments() {
                assert!(
                    !start.is_empty() && !end.is_empty(),
                    "{} has an empty block delimiter",
                    language.name
                );
            }
        }
    }

    #[test]
    fn extensions_and_file_names_both_reach_a_language() {
        let catalogue = catalogue();
        assert_eq!(
            classify(&catalogue, "src/main.rs").language.unwrap().name,
            "Rust"
        );
        assert_eq!(
            classify(&catalogue, "Makefile").language.unwrap().name,
            "Make"
        );
        assert_eq!(
            classify(&catalogue, "README").language.unwrap().name,
            "Plain text"
        );
        // A leading dot is not a suffix separator, so this is a name match.
        assert_eq!(
            classify(&catalogue, ".gitignore").language.unwrap().name,
            "Git config"
        );
    }

    /// Case folding here is determinism-preserving, unlike in the exclusion set.
    #[test]
    fn extension_matching_ignores_ascii_case() {
        let catalogue = catalogue();
        assert_eq!(
            classify(&catalogue, "art/Logo.PNG").category,
            ContentCategory::Asset
        );
        assert_eq!(
            classify(&catalogue, "art/logo.png").category,
            ContentCategory::Asset
        );
        assert_eq!(
            classify(&catalogue, "MAKEFILE").language.unwrap().name,
            "Make"
        );
    }

    #[test]
    fn every_category_is_reachable_from_a_real_path() {
        let catalogue = catalogue();
        assert_eq!(
            classify(&catalogue, "src/lib.rs").category,
            ContentCategory::Code
        );
        assert_eq!(
            classify(&catalogue, "art/tree.png").category,
            ContentCategory::Asset
        );
        assert_eq!(
            classify(&catalogue, "Cargo.toml").category,
            ContentCategory::Config
        );
        assert_eq!(
            classify(&catalogue, "docs/PRD.md").category,
            ContentCategory::Docs
        );
        assert_eq!(
            classify(&catalogue, "Cargo.lock").category,
            ContentCategory::Generated
        );
        assert_eq!(
            classify(&catalogue, "bin/tool.exe").category,
            ContentCategory::Binary
        );
        assert_eq!(
            classify(&catalogue, "mystery.qqq").category,
            ContentCategory::Unknown
        );
    }

    /// An unrecognized file is not guessed at, and not counted.
    #[test]
    fn unknown_content_is_not_countable() {
        let catalogue = catalogue();
        let unknown = classify(&catalogue, "mystery.qqq");
        assert!(unknown.language.is_none());
        assert!(!unknown.is_countable());
        assert!(classify(&catalogue, "src/lib.rs").is_countable());
        assert!(!classify(&catalogue, "art/tree.png").is_countable());
    }

    /// The repository's own statement beats any guess from a suffix.
    #[test]
    fn a_linguist_marker_overrides_the_extension() {
        let catalogue = catalogue();
        let marked = catalogue.classify(&path("src/parser.rs"), Some(Marker::Generated));
        assert_eq!(marked.category, ContentCategory::Generated);
        // The language survives — a generated Rust file is still Rust, and still countable.
        assert_eq!(marked.language.unwrap().name, "Rust");
        assert!(marked.is_countable());

        let vendored = catalogue.classify(&path("third_party/lib.js"), Some(Marker::Vendored));
        assert_eq!(vendored.category, ContentCategory::Generated);
    }

    #[test]
    fn generated_conventions_apply_without_a_marker() {
        let catalogue = catalogue();
        for generated in [
            "api/service.pb.go",
            "gen/schema_pb2.py",
            "lib/model.freezed.dart",
            "web/bundle.min.js",
            "go.sum",
            "Cargo.lock",
        ] {
            assert_eq!(
                classify(&catalogue, generated).category,
                ContentCategory::Generated,
                "{generated}"
            );
        }
        assert_eq!(
            classify(&catalogue, "api/service.go").category,
            ContentCategory::Code
        );
    }

    #[test]
    fn test_paths_are_recognized_by_directory_and_by_name() {
        let catalogue = catalogue();
        assert!(classify(&catalogue, "tests/walk_self.rs").is_test);
        assert!(classify(&catalogue, "crates/x/tests/a.rs").is_test);
        assert!(classify(&catalogue, "pkg/thing_test.go").is_test);
        assert!(classify(&catalogue, "src/parser.test.ts").is_test);
        assert!(classify(&catalogue, "java/FooTest.java").is_test);
        // Being a test does not change what it is.
        assert_eq!(
            classify(&catalogue, "tests/walk_self.rs").category,
            ContentCategory::Code
        );
        assert!(!classify(&catalogue, "src/latest.rs").is_test);
        assert!(!classify(&catalogue, "src/protest.go").is_test);
    }

    fn rust(content: &str) -> LineCounts {
        let catalogue = catalogue();
        let language = classify(&catalogue, "a.rs").language.unwrap().clone();
        count_lines(content.as_bytes(), CommentSyntax::of(Some(&language)))
    }

    #[test]
    fn the_three_line_kinds_add_up_to_the_total() {
        let counts = rust("fn main() {}\n\n// a comment\n   \nlet x = 1;\n");
        assert_eq!(counts.total, 5);
        assert_eq!(counts.code, 2);
        assert_eq!(counts.comment, 1);
        assert_eq!(counts.blank, 2);
        assert_eq!(counts.code + counts.comment + counts.blank, counts.total);
    }

    #[test]
    fn block_comments_span_lines_and_close_once() {
        let counts = rust("/* one\n * two\n */\nfn main() {}\n");
        assert_eq!(counts.comment, 3);
        assert_eq!(counts.code, 1);

        // Opened and closed on one line does not swallow what follows.
        let inline = rust("/* short */\nfn main() {}\n");
        assert_eq!(inline.comment, 1);
        assert_eq!(inline.code, 1);
    }

    /// The failure the module header chose against: one `"/*"` in a string must not make the
    /// rest of the file read as a comment.
    #[test]
    fn a_block_delimiter_inside_a_string_does_not_open_a_comment() {
        let counts = rust("let a = \"/*\";\nlet b = 1;\nlet c = 2;\n");
        assert_eq!(counts.code, 3);
        assert_eq!(counts.comment, 0);
    }

    /// The other half of the same rule: a trailing `//` is code with a note on it.
    #[test]
    fn a_line_comment_after_code_leaves_the_line_as_code() {
        let counts = rust("let url = \"https://example.com\"; // fetch it\n");
        assert_eq!(counts.code, 1);
        assert_eq!(counts.comment, 0);
    }

    #[test]
    fn without_comment_syntax_every_non_blank_line_is_code() {
        let counts = count_lines(b"{\n  \"a\": 1\n}\n", CommentSyntax::default());
        assert_eq!(counts.code, 3);
        assert_eq!(counts.comment, 0);
    }

    #[test]
    fn line_endings_and_empty_files_are_handled() {
        assert_eq!(count_lines(b"", CommentSyntax::default()).total, 0);
        // A trailing newline does not invent a final empty line; a missing one does not lose
        // the last line.
        assert_eq!(count_lines(b"a\nb\n", CommentSyntax::default()).total, 2);
        assert_eq!(count_lines(b"a\nb", CommentSyntax::default()).total, 2);
        // CRLF, which is what a Windows checkout of a text file looks like.
        let crlf = count_lines(b"a\r\n\r\nb\r\n", CommentSyntax::default());
        assert_eq!(crlf.total, 3);
        assert_eq!(crlf.code, 2);
        assert_eq!(crlf.blank, 1);
    }

    #[test]
    fn debt_markers_need_word_boundaries() {
        assert_eq!(count_markers(b"// TODO: fix this\n"), 1);
        assert_eq!(count_markers(b"// todo: lowercase counts\n"), 1);
        assert_eq!(count_markers(b"unimplemented!(); todo!()\n"), 1);
        assert_eq!(count_markers(b"FIXME and TODO on one line\n"), 2);
        // Not markers.
        assert_eq!(count_markers(b"a MASTODON appeared\n"), 0);
        assert_eq!(count_markers(b"the todos are listed below\n"), 0);
        assert_eq!(count_markers(b"my_todo_list = []\n"), 0);
    }

    #[test]
    fn binary_content_is_recognized_by_a_nul() {
        assert!(looks_binary(b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR"));
        assert!(!looks_binary(b"fn main() {}\n"));
        assert!(!looks_binary(b""));
        // Past the sniff window, a NUL is not looked for.
        let mut late = vec![b'a'; BINARY_SNIFF_BYTES];
        late.push(0);
        assert!(!looks_binary(&late));
    }

    // ---- `.gitattributes` ----

    fn attributes(entries: &[(&str, &str)]) -> Attributes {
        let mut attributes = Attributes::none();
        for (directory, content) in entries {
            let directory = if directory.is_empty() {
                RepoPath::root()
            } else {
                path(directory)
            };
            attributes.add(&directory, content.as_bytes());
        }
        attributes
    }

    #[test]
    fn linguist_markers_are_read_from_gitattributes() {
        let set = attributes(&[(
            "",
            "*.pb.go linguist-generated\nvendor/** linguist-vendored\n*.rs text\n",
        )]);
        assert_eq!(
            set.marker_for(&path("api/x.pb.go")),
            Some(Marker::Generated)
        );
        assert_eq!(
            set.marker_for(&path("vendor/lib/a.c")),
            Some(Marker::Vendored)
        );
        // An attribute that is not a linguist marker contributes no rule.
        assert_eq!(set.marker_for(&path("src/main.rs")), None);
        assert!(!set.is_empty());
        assert_eq!(set.file_count(), 1);
    }

    /// The two spellings GitHub documents, including the opt-out.
    #[test]
    fn a_marker_can_be_set_by_value_and_unset_again() {
        let set = attributes(&[(
            "",
            "generated/* linguist-generated=true\ngenerated/keep.rs linguist-generated=false\n",
        )]);
        assert_eq!(
            set.marker_for(&path("generated/a.rs")),
            Some(Marker::Generated)
        );
        assert_eq!(set.marker_for(&path("generated/keep.rs")), None);

        let unset = attributes(&[(
            "",
            "*.rs linguist-generated\nsrc/*.rs -linguist-generated\n",
        )]);
        assert_eq!(unset.marker_for(&path("build.rs")), Some(Marker::Generated));
        assert_eq!(unset.marker_for(&path("src/main.rs")), None);
    }

    /// A `.gitattributes` speaks only for its own directory and below.
    #[test]
    fn attribute_scope_is_the_directory_it_lives_in() {
        let set = attributes(&[("vendor", "*.js linguist-vendored\n")]);
        assert_eq!(set.marker_for(&path("vendor/a.js")), Some(Marker::Vendored));
        // A slash-free pattern matches the basename at any depth below the file — git's own
        // rule, and the reason this defers to `gix`'s matcher instead of reusing the simpler
        // one in `filter.rs`.
        assert_eq!(
            set.marker_for(&path("vendor/deep/a.js")),
            Some(Marker::Vendored)
        );
        assert_eq!(set.marker_for(&path("src/a.js")), None);
        // A directory that merely starts with the same bytes is not inside it.
        assert_eq!(set.marker_for(&path("vendored/a.js")), None);
    }

    /// Git's precedence: the deeper file wins, because it is added last.
    #[test]
    fn a_deeper_gitattributes_overrides_a_shallower_one() {
        let set = attributes(&[
            ("", "**/*.rs linguist-generated\n"),
            ("src", "*.rs -linguist-generated\n"),
        ]);
        assert_eq!(set.marker_for(&path("build/x.rs")), Some(Marker::Generated));
        assert_eq!(set.marker_for(&path("src/main.rs")), None);
    }

    #[test]
    fn a_malformed_attributes_line_is_skipped_not_fatal() {
        let set = attributes(&[("", "# a comment\n\n!bad\n*.rs linguist-generated\n")]);
        assert_eq!(set.marker_for(&path("a.rs")), Some(Marker::Generated));
    }

    // ---- rollup and derived signals ----

    use treepo_model::primitives::size::SizePrimitives;
    use treepo_model::primitives::temporal::ChurnWindows;

    /// Builds a file record already carrying what `scan`'s per-file phase would have set.
    fn scanned(
        p: &str,
        bytes: u64,
        category: ContentCategory,
        lines: LineCounts,
    ) -> (PathRecord, Totals) {
        let mut record = PathRecord::new(path(p), NodeKind::File);
        record.size = SizePrimitives {
            bytes,
            lines,
            category_bytes: core::iter::once((category, bytes)).collect(),
            ..SizePrimitives::default()
        };
        let totals = Totals {
            counted_files: u32::from(category.is_textual()),
            ..Totals::default()
        };
        (record, totals)
    }

    fn code_lines(total: u64, comment: u64) -> LineCounts {
        LineCounts {
            total,
            code: total - comment,
            comment,
            blank: 0,
        }
    }

    /// Sorts and rolls up bytes, standing in for what `walk` has already done when `scan`
    /// runs for real. Without it every directory would have zero bytes and the byte-ratio
    /// signals would have no denominator.
    fn prepare(entries: Vec<(PathRecord, Totals)>) -> (Vec<PathRecord>, Vec<Totals>) {
        let mut order: Vec<usize> = (0..entries.len()).collect();
        order.sort_by(|&a, &b| entries[a].0.path.cmp(&entries[b].0.path));
        let mut records: Vec<PathRecord> = order.iter().map(|&i| entries[i].0.clone()).collect();
        let totals: Vec<Totals> = order.iter().map(|&i| entries[i].1).collect();

        let parents = crate::walk::parent_indices(&records);
        for index in (0..records.len()).rev() {
            if let Some(parent) = parents[index] {
                let bytes = records[index].size.bytes;
                records[parent].size.bytes += bytes;
            }
        }
        (records, totals)
    }

    /// Runs the half of `scan` that needs no repository.
    fn finish(entries: Vec<(PathRecord, Totals)>) -> BTreeMap<RepoPath, PathRecord> {
        let (mut records, mut totals) = prepare(entries);
        roll_up_content(&mut records, &mut totals);
        derive_signals(&mut records, &totals);
        records
            .into_iter()
            .map(|record| (record.path.clone(), record))
            .collect()
    }

    /// Two derivations of the same rational need not land on the same fixed-point bits:
    /// `1 − 1/10` and `9/10` differ by one ULP in Q32.32. `N3` requires the same input to
    /// give the same output, not that fixed point agree with the reals, so a test about
    /// meaning uses a tolerance and a test about determinism does not.
    fn assert_close(actual: Fx, expected: Fx) {
        let slack = Fx::from_ratio(1, 1_000_000);
        assert!(
            actual.sub(expected).abs() <= slack,
            "{actual:?} is not within {slack:?} of {expected:?}"
        );
    }

    fn directory(p: &str) -> (PathRecord, Totals) {
        (
            PathRecord::new(path(p), NodeKind::Directory),
            Totals::default(),
        )
    }

    #[test]
    fn lines_and_categories_roll_up_the_tree() {
        let tree = finish(vec![
            directory(""),
            directory("src"),
            scanned("src/a.rs", 100, ContentCategory::Code, code_lines(10, 2)),
            scanned("src/b.rs", 200, ContentCategory::Code, code_lines(20, 4)),
            scanned("README.md", 50, ContentCategory::Docs, code_lines(5, 0)),
        ]);

        let root = &tree[&RepoPath::root()];
        assert_eq!(root.size.lines.total, 35);
        assert_eq!(root.size.lines.comment, 6);
        assert_eq!(
            root.size.category_bytes.get(&ContentCategory::Code),
            Some(&300)
        );
        assert_eq!(
            root.size.category_bytes.get(&ContentCategory::Docs),
            Some(&50)
        );
        // And the subtree totals are the subtree's, not the whole repository's.
        assert_eq!(tree[&path("src")].size.lines.total, 30);
    }

    /// A pure-code directory is concentrated in one category; a mixed one is not.
    #[test]
    fn kind_balance_reads_the_spread_across_categories() {
        let pure = finish(vec![
            directory(""),
            scanned("a.rs", 100, ContentCategory::Code, code_lines(10, 0)),
            scanned("b.rs", 100, ContentCategory::Code, code_lines(10, 0)),
        ]);
        let mixed = finish(vec![
            directory(""),
            scanned("a.rs", 100, ContentCategory::Code, code_lines(10, 0)),
            scanned("a.png", 100, ContentCategory::Asset, LineCounts::default()),
            scanned("a.toml", 100, ContentCategory::Config, code_lines(10, 0)),
        ]);

        let pure_kind = pure[&RepoPath::root()].structural.balance.kind.unwrap();
        let mixed_kind = mixed[&RepoPath::root()].structural.balance.kind.unwrap();
        assert_eq!(pure_kind, Fx::ZERO);
        assert!(mixed_kind > pure_kind);
        // A file has no distribution of kinds to be balanced about.
        assert_eq!(pure[&path("a.rs")].structural.balance.kind, None);
    }

    #[test]
    fn derived_signals_are_measured_only_where_there_is_a_denominator() {
        let tree = finish(vec![
            directory(""),
            scanned(
                "art.png",
                500,
                ContentCategory::Asset,
                LineCounts::default(),
            ),
        ]);
        let root = &tree[&RepoPath::root()];
        // Nothing textual was read, so no line-based signal was measured.
        assert_eq!(root.derived.comment_density, None);
        assert_eq!(root.derived.todo_density, None);
        assert_eq!(root.derived.test_to_source, None);
        // Byte ratios need no read, and are measured.
        assert_eq!(root.derived.generated_debt, Some(Fx::ZERO));
        assert!(root.derived.is_measured());
    }

    #[test]
    fn test_to_source_divides_test_bytes_by_source_bytes() {
        let mut entries = vec![
            directory(""),
            scanned("src/lib.rs", 400, ContentCategory::Code, code_lines(40, 0)),
            directory("src"),
            directory("tests"),
            scanned("tests/a.rs", 100, ContentCategory::Code, code_lines(10, 0)),
        ];
        // What `scan` would have recorded for a test-like `Code` file.
        entries[4].1.test_code_bytes = 100;
        let tree = finish(entries);
        assert_eq!(
            tree[&RepoPath::root()].derived.test_to_source,
            Some(Fx::from_ratio(1, 4))
        );
    }

    #[test]
    fn todo_density_is_markers_per_thousand_code_lines() {
        let mut entries = vec![
            directory(""),
            scanned("a.rs", 100, ContentCategory::Code, code_lines(500, 0)),
        ];
        entries[1].1.debt_markers = 5;
        let tree = finish(entries);
        // 5 markers over 500 code lines is 10 per thousand.
        assert_eq!(
            tree[&RepoPath::root()].derived.todo_density,
            Some(Fx::from_int(10))
        );
    }

    #[test]
    fn generated_and_large_file_debt_are_byte_shares() {
        let mut entries = vec![
            directory(""),
            scanned("a.rs", 250, ContentCategory::Code, code_lines(10, 0)),
            scanned(
                "Cargo.lock",
                750,
                ContentCategory::Generated,
                code_lines(80, 0),
            ),
        ];
        entries[2].1.large_bytes = 750;
        let tree = finish(entries);
        let root = &tree[&RepoPath::root()];
        assert_eq!(root.derived.generated_debt, Some(Fx::from_ratio(3, 4)));
        assert_eq!(root.derived.large_file_debt, Some(Fx::from_ratio(3, 4)));
    }

    // ---- history-dependent signals ----

    fn with_history(
        entries: Vec<(PathRecord, Totals)>,
        history: &[(&str, i64, u64)],
    ) -> BTreeMap<RepoPath, PathRecord> {
        let (mut records, mut totals) = prepare(entries);
        for (path_text, last_commit, churn_90) in history {
            let target = path(path_text);
            let record = records
                .iter_mut()
                .find(|record| record.path == target)
                .expect("history for a path in the tree");
            record.temporal.last_commit_time = Some(*last_commit);
            record.temporal.churn = ChurnWindows {
                days_90: *churn_90,
                ..ChurnWindows::default()
            };
        }

        roll_up_content(&mut records, &mut totals);
        derive_signals(&mut records, &totals);
        apply_history_signals(&mut records);
        records
            .into_iter()
            .map(|record| (record.path.clone(), record))
            .collect()
    }

    #[test]
    fn stability_is_the_inverse_of_recent_churn_over_size() {
        let tree = with_history(
            vec![
                directory(""),
                scanned("calm.rs", 100, ContentCategory::Code, code_lines(100, 0)),
                scanned(
                    "churning.rs",
                    100,
                    ContentCategory::Code,
                    code_lines(100, 0),
                ),
                scanned(
                    "art.png",
                    100,
                    ContentCategory::Asset,
                    LineCounts::default(),
                ),
            ],
            &[("calm.rs", 0, 10), ("churning.rs", 0, 400)],
        );

        // 10 lines churned against 100 lines held.
        assert_close(
            tree[&path("calm.rs")].temporal.stability.unwrap(),
            Fx::from_ratio(9, 10),
        );
        // Churning faster than its own size floors at zero rather than going negative.
        assert_eq!(
            tree[&path("churning.rs")].temporal.stability,
            Some(Fx::ZERO)
        );
        // No lines is no denominator, which stays unmeasured rather than reading as stable.
        assert_eq!(tree[&path("art.png")].temporal.stability, None);
    }

    #[test]
    fn doc_staleness_compares_the_newest_docs_against_the_newest_code() {
        let day = 86_400;
        let tree = with_history(
            vec![
                directory(""),
                scanned("src.rs", 100, ContentCategory::Code, code_lines(10, 0)),
                scanned("README.md", 100, ContentCategory::Docs, code_lines(10, 0)),
            ],
            &[("src.rs", 30 * day, 0), ("README.md", 10 * day, 0)],
        );
        // Code touched 20 days after the docs.
        assert_eq!(tree[&RepoPath::root()].derived.doc_staleness_days, Some(20));
        // A file on its own has no counterpart to be stale against.
        assert_eq!(tree[&path("README.md")].derived.doc_staleness_days, None);
    }

    /// Fresher docs are a negative number, not a clamped zero.
    #[test]
    fn doc_staleness_is_signed() {
        let day = 86_400;
        let tree = with_history(
            vec![
                directory(""),
                scanned("src.rs", 100, ContentCategory::Code, code_lines(10, 0)),
                scanned("README.md", 100, ContentCategory::Docs, code_lines(10, 0)),
            ],
            &[("src.rs", 10 * day, 0), ("README.md", 30 * day, 0)],
        );
        assert_eq!(
            tree[&RepoPath::root()].derived.doc_staleness_days,
            Some(-20)
        );
    }

    #[test]
    fn a_custom_catalogue_can_be_supplied() {
        let text = r#"(
            version: 1,
            languages: [(name: "Toy", category: Code, extensions: ["toy"], line_comment: ["!"])],
            categories: [(category: Asset, extensions: ["blob"])],
            test_directories: ["t"],
            test_files: [],
            generated_files: [],
        )"#;
        let catalogue = Catalogue::from_ron(text).expect("parses");
        assert_eq!(classify(&catalogue, "a.toy").language.unwrap().name, "Toy");
        assert_eq!(
            classify(&catalogue, "a.blob").category,
            ContentCategory::Asset
        );
        assert!(classify(&catalogue, "t/a.toy").is_test);
        // The built-in languages are not also present.
        assert!(classify(&catalogue, "a.rs").language.is_none());
        assert!(Catalogue::from_ron("(nonsense").is_err());
    }
}
