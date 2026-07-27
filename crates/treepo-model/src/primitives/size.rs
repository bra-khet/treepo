//! Size and composition primitives — `F-EXT-1`, `F-EXT-4`, `design/feature-system.md` §3.2.
//!
//! Bytes, lines, languages, and what kind of content a path holds. `F-MAT-1` turns the
//! category mix into material families and `P7`'s soft clamp reads the outliers, so the
//! distribution matters as much as the total — one 50 MB asset in a directory of source
//! files should not make the directory read as an asset directory (PRD §6, "One enormous
//! file").

use crate::manifest::LanguageId;
use treepo_det::{Fx, OrderedMap};

/// Lines of code broken down as `F-EXT-4` requires.
///
/// `code + comment + blank == total` is an invariant the extractor maintains; nothing here
/// enforces it, because a file that is 100% one category is legitimate and a checked
/// constructor would only push the arithmetic somewhere less visible.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LineCounts {
    /// Every line, including blanks and comments.
    pub total: u64,
    /// Lines carrying code.
    pub code: u64,
    /// Lines that are only a comment.
    pub comment: u64,
    /// Lines that are empty or whitespace.
    pub blank: u64,
}

impl LineCounts {
    /// Sums two breakdowns, for rolling a directory up from its children.
    #[must_use]
    pub const fn merge(self, other: Self) -> Self {
        Self {
            total: self.total.saturating_add(other.total),
            code: self.code.saturating_add(other.code),
            comment: self.comment.saturating_add(other.comment),
            blank: self.blank.saturating_add(other.blank),
        }
    }

    /// Comment density: comment lines over code plus comment lines, in `0..=1`.
    ///
    /// Blanks are excluded from the denominator deliberately — a heavily spaced file is not
    /// a poorly documented one, and including blanks would make formatting style read as a
    /// documentation signal.
    #[must_use]
    pub const fn comment_density(&self) -> Fx {
        let denominator = self.code.saturating_add(self.comment);
        if denominator == 0 {
            return Fx::ZERO;
        }
        Fx::from_ratio(self.comment as i64, denominator as i64)
    }
}

/// What kind of content a path holds — the `code : assets : config : docs : generated :
/// binary` ratio of `F-EXT-4`.
///
/// One category per file, assigned by `treepo-vcs::lang` from extension, `.gitattributes`
/// `linguist-*` markers, and folder context. Directories carry a mix rather than a category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContentCategory {
    /// Source in a recognized programming language.
    Code,
    /// Images, audio, models, fonts — content that is not read as text.
    Asset,
    /// Manifests, lockfiles, CI definitions, dotfiles.
    Config,
    /// Markdown, plain text, and anything under a docs-signalled folder.
    Docs,
    /// Marked `linguist-generated`, or matching a generated-output convention.
    Generated,
    /// Binary content with no more specific category.
    Binary,
    /// Recognized as none of the above.
    Unknown,
}

impl ContentCategory {
    /// Every category, in declaration order. The order of a ratio vector.
    pub const ALL: [Self; 7] = [
        Self::Code,
        Self::Asset,
        Self::Config,
        Self::Docs,
        Self::Generated,
        Self::Binary,
        Self::Unknown,
    ];

    /// Whether treepo will try to count lines in this category.
    ///
    /// `F-EXT-4` wants a LOC breakdown; opening a 50 MB texture to look for newlines is
    /// wasted I/O on the expensive side of `P10`.
    #[must_use]
    pub const fn is_textual(self) -> bool {
        matches!(
            self,
            Self::Code | Self::Config | Self::Docs | Self::Generated
        )
    }
}

/// The spread of file sizes beneath a path — `design/feature-system.md` §3.2.
///
/// Median and p90 rather than a standard deviation: file sizes are heavily skewed, and the
/// question `P7` actually asks is "is one child about to consume the parent's whole budget",
/// which a percentile answers directly and a variance does not.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SizeDistribution {
    /// Smallest file in bytes.
    pub min: u64,
    /// Median file size in bytes.
    pub median: u64,
    /// 90th-percentile file size in bytes.
    pub p90: u64,
    /// Largest file in bytes.
    pub max: u64,
    /// Mean file size in bytes.
    pub mean: u64,
}

/// Bytes, lines, languages, and composition for one path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SizePrimitives {
    /// Bytes for a file; the sum of the subtree for a directory.
    pub bytes: u64,
    /// This path's bytes as a proportion of its parent's, in `0..=1`.
    ///
    /// Stored rather than derived so `P7`'s clamp and `F-MAT-3`'s normalization do not need
    /// the parent record in hand.
    pub relative_bytes: Fx,
    /// Line breakdown, summed over the subtree for a directory.
    pub lines: LineCounts,
    /// Bytes per language, keyed by an id interned in
    /// [`Manifest::languages`](crate::Manifest::languages).
    pub language_bytes: OrderedMap<LanguageId, u64>,
    /// Bytes per content category. Files have one entry; directories have a mix.
    pub category_bytes: OrderedMap<ContentCategory, u64>,
    /// File-size spread beneath this path. All zero for a file.
    pub distribution: SizeDistribution,
    /// Files beneath this path above the large-file threshold (`P7`, PRD §6).
    pub large_file_count: u32,
}

impl SizePrimitives {
    /// The dominant language by bytes, or `None` for a tie or an empty path.
    ///
    /// A tie yields `None` for the same reason
    /// [`dominant_author`](super::ownership::OwnershipPrimitives::dominant_author) does:
    /// breaking it by key order would be stable but arbitrary, and the choice would flip on
    /// the next commit.
    #[must_use]
    pub fn dominant_language(&self) -> Option<LanguageId> {
        let mut best: Option<(LanguageId, u64)> = None;
        let mut tied = false;
        for (&language, &bytes) in &self.language_bytes {
            match best {
                Some((_, best_bytes)) if bytes > best_bytes => {
                    best = Some((language, bytes));
                    tied = false;
                }
                Some((_, best_bytes)) if bytes == best_bytes => tied = true,
                Some(_) => {}
                None => best = Some((language, bytes)),
            }
        }
        if tied {
            None
        } else {
            best.map(|(language, _)| language)
        }
    }

    /// One category's share of this path's bytes, in `0..=1`.
    #[must_use]
    pub fn category_ratio(&self, category: ContentCategory) -> Fx {
        let total: u64 = self.category_bytes.values().copied().sum();
        if total == 0 {
            return Fx::ZERO;
        }
        let part = self.category_bytes.get(&category).copied().unwrap_or(0);
        Fx::from_ratio(part as i64, total as i64)
    }

    /// How many distinct languages appear beneath this path.
    #[must_use]
    pub fn language_count(&self) -> u32 {
        u32::try_from(self.language_bytes.len()).unwrap_or(u32::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comment_density_excludes_blank_lines() {
        let counts = LineCounts {
            total: 100,
            code: 60,
            comment: 20,
            blank: 20,
        };
        // 20 / (60 + 20), not 20 / 100.
        assert_eq!(counts.comment_density(), Fx::from_ratio(1, 4));
    }

    #[test]
    fn comment_density_of_an_empty_file_is_zero_not_a_panic() {
        assert_eq!(LineCounts::default().comment_density(), Fx::ZERO);
        let blanks_only = LineCounts {
            total: 5,
            blank: 5,
            ..LineCounts::default()
        };
        assert_eq!(blanks_only.comment_density(), Fx::ZERO);
    }

    #[test]
    fn merging_line_counts_sums_every_field() {
        let a = LineCounts {
            total: 10,
            code: 6,
            comment: 2,
            blank: 2,
        };
        let b = LineCounts {
            total: 5,
            code: 5,
            comment: 0,
            blank: 0,
        };
        let merged = a.merge(b);
        assert_eq!(merged.total, 15);
        assert_eq!(merged.code, 11);
        assert_eq!(merged.code + merged.comment + merged.blank, merged.total);
    }

    #[test]
    fn only_textual_categories_are_worth_counting_lines_in() {
        assert!(ContentCategory::Code.is_textual());
        assert!(ContentCategory::Docs.is_textual());
        assert!(!ContentCategory::Asset.is_textual());
        assert!(!ContentCategory::Binary.is_textual());
    }

    fn sized(entries: &[(ContentCategory, u64)]) -> SizePrimitives {
        SizePrimitives {
            category_bytes: entries.iter().copied().collect(),
            ..SizePrimitives::default()
        }
    }

    #[test]
    fn category_ratios_are_proportions_of_the_path() {
        let s = sized(&[(ContentCategory::Code, 750), (ContentCategory::Docs, 250)]);
        assert_eq!(
            s.category_ratio(ContentCategory::Code),
            Fx::from_ratio(3, 4)
        );
        assert_eq!(
            s.category_ratio(ContentCategory::Docs),
            Fx::from_ratio(1, 4)
        );
        assert_eq!(s.category_ratio(ContentCategory::Asset), Fx::ZERO);
        assert_eq!(sized(&[]).category_ratio(ContentCategory::Code), Fx::ZERO);
    }

    #[test]
    fn dominant_language_needs_a_clear_winner() {
        let clear = SizePrimitives {
            language_bytes: [(LanguageId::new(0), 900), (LanguageId::new(1), 100)]
                .into_iter()
                .collect(),
            ..SizePrimitives::default()
        };
        assert_eq!(clear.dominant_language(), Some(LanguageId::new(0)));
        assert_eq!(clear.language_count(), 2);

        let tied = SizePrimitives {
            language_bytes: [(LanguageId::new(0), 500), (LanguageId::new(1), 500)]
                .into_iter()
                .collect(),
            ..SizePrimitives::default()
        };
        assert_eq!(tied.dominant_language(), None);
        assert_eq!(SizePrimitives::default().dominant_language(), None);
    }
}
