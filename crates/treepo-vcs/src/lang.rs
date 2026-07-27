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

use gix::glob::wildmatch;
use serde::Deserialize;
use std::collections::BTreeMap;
use treepo_model::path::RepoPath;
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
