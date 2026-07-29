//! The name a contributor is shown as — `F-ID-3`.
//!
//! > Pseudonyms are deterministic (`N3`) from the normalized identity key: two-word,
//! > pronounceable, drawn from a themed wordlist, with deterministic collision resolution
//! > within a repository. The same contributor yields the same pseudonym on every machine.
//!
//! # Two functions, because "deterministic" and "collision-resolved" pull apart
//!
//! A pseudonym that is a pure function of one key cannot know whether another contributor
//! drew the same pair; a pseudonym that is unique within a repository is a function of the
//! whole contributor set. Those are different things and this module offers both, named so
//! that the difference is visible at the call site:
//!
//! * [`Wordlist::draw`] — the unresolved draw. A pure function of the key. What a caller
//!   holding one key and no repository can have.
//! * [`Wordlist::assign`] — the repository-wide [`Roster`], where collisions are resolved.
//!   What the UI uses.
//!
//! # How collisions resolve, and the one case where a pseudonym moves
//!
//! Keys are assigned **in ascending key order**, and the first claimant of a word pair keeps
//! it. A key whose pair is taken redraws with a salt, up to [`SALTED_ATTEMPTS`] times, and
//! then falls back to its own base pair with a numeric discriminator.
//!
//! Key order is hash order, which matters twice. It is uncorrelated with contribution
//! volume, so `N4` is untouched — assignment order is not a ranking of anybody. And it is a
//! property of the keys alone, so the roster does not depend on the order a caller happened
//! to iterate a manifest in.
//!
//! The stability consequence is worth stating plainly, because it is the one way a
//! pseudonym can change under someone:
//!
//! > A contributor's pseudonym moves only if a **new** contributor appears who both draws
//! > the same word pair *and* sorts earlier by key.
//!
//! With the built-in wordlist that is one chance in thirty-two thousand per contributor
//! added. The wordlist is sized for this rather than for elegance: shrink it and the
//! property degrades quietly, which is why [`Wordlist::pair_count`] is exposed and why the
//! module test asserts a floor on it.
//!
//! # The discriminator exists so that nothing can fail
//!
//! A repository with more contributors than the wordlist has pairs is a real thing — the
//! Linux kernel has some twenty-five thousand — and `F-ID-3` asks for a pseudonym for every
//! contributor, not for as many as fit. Past the salted redraws a contributor keeps its own
//! base pair and takes the first free discriminator, so assignment always terminates and
//! always succeeds. `Ash Willow 2` is not a good name; it is a much better outcome than an
//! error, and at the sizes treepo actually meets it never appears.

use alloc::string::String;
use core::fmt;
use serde::Deserialize;
use treepo_det::{OrderedMap, OrderedSet, Seed};
use treepo_model::identity::AuthorKey;

/// The compiled-in wordlist.
const BUILT_IN_RON: &str = include_str!("../../../assets/wordlists/pseudonyms.ron");

/// The wordlist format this crate understands.
///
/// A pseudonym is derived rather than stored, so editing the wordlist invalidates no
/// manifest — but it does rename every contributor in every repository at once, which is
/// why this is a version rather than a silent read.
pub const WORDLIST_VERSION: u32 = 1;

/// Domain separator for the pseudonym seed.
const DOMAIN: &[u8] = b"treepo/pseudonym/v1";

/// How many salted redraws a colliding key gets before falling back to a discriminator.
///
/// Eight is well past what the built-in wordlist ever needs — at a load factor low enough
/// for the stability property above to hold, a second attempt almost always succeeds. It is
/// a bound rather than a tuning knob: what it protects against is a nearly-full wordlist,
/// where unbounded redrawing would spend a long time probing a space that has no room left.
pub const SALTED_ATTEMPTS: u64 = 8;

/// A contributor's displayed name: two words, and a discriminator that is almost always
/// absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pseudonym<'w> {
    first: &'w str,
    second: &'w str,
    discriminator: u32,
}

impl<'w> Pseudonym<'w> {
    /// The first word, as the file stores it — lower case.
    #[must_use]
    pub const fn first(&self) -> &'w str {
        self.first
    }

    /// The second word, as the file stores it — lower case.
    #[must_use]
    pub const fn second(&self) -> &'w str {
        self.second
    }

    /// The disambiguating number, or zero when there is none.
    ///
    /// Nonzero only in a repository with more contributors than the wordlist has pairs. See
    /// the module docs for why it exists at all.
    #[must_use]
    pub const fn discriminator(&self) -> u32 {
        self.discriminator
    }
}

/// Title case, because the words are stored lower case and capitalization is a display
/// decision — a caller wanting them another way has the two words.
impl fmt::Display for Pseudonym<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_capitalized(f, self.first)?;
        f.write_str(" ")?;
        write_capitalized(f, self.second)?;
        if self.discriminator != 0 {
            write!(f, " {}", self.discriminator)?;
        }
        Ok(())
    }
}

fn write_capitalized(f: &mut fmt::Formatter<'_>, word: &str) -> fmt::Result {
    use core::fmt::Write as _;
    let mut chars = word.chars();
    if let Some(first) = chars.next() {
        // `validate` restricts words to ASCII lower case, so this is the whole of the
        // casing problem. Anything else would need a Unicode table, which would then be a
        // determinism input (`N3`).
        f.write_char(first.to_ascii_uppercase())?;
        f.write_str(chars.as_str())?;
    }
    Ok(())
}

/// One assignment: which words, and which discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Draw {
    first: u32,
    second: u32,
    discriminator: u32,
}

/// The two-word source `F-ID-3` draws from.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wordlist {
    /// The format version this file was written for.
    pub version: u32,
    /// Candidates for the first word.
    pub first: alloc::vec::Vec<String>,
    /// Candidates for the second word.
    pub second: alloc::vec::Vec<String>,
}

impl Wordlist {
    /// The compiled-in wordlist.
    ///
    /// # Panics
    ///
    /// If the compiled-in wordlist is malformed, which a unit test in this module rules out.
    #[must_use]
    pub fn built_in() -> Self {
        Self::from_ron(BUILT_IN_RON).expect("built-in wordlist must parse and validate")
    }

    /// Parses and validates a wordlist from RON.
    ///
    /// # Errors
    ///
    /// [`WordlistError::Parse`] if the text is not a well-formed wordlist, or one of the
    /// validation variants if it is well-formed but cannot produce the names `F-ID-3`
    /// describes.
    pub fn from_ron(text: &str) -> Result<Self, WordlistError> {
        let list: Self = ron::from_str(text).map_err(|error| WordlistError::Parse {
            detail: alloc::format!("{error}"),
        })?;
        list.validate()?;
        Ok(list)
    }

    /// Checks the file against what `F-ID-3` needs of it.
    ///
    /// # Errors
    ///
    /// The first rule violated, naming the word or list that violated it.
    pub fn validate(&self) -> Result<(), WordlistError> {
        if self.version != WORDLIST_VERSION {
            return Err(WordlistError::Version {
                found: self.version,
                expected: WORDLIST_VERSION,
            });
        }
        for (name, words) in [("first", &self.first), ("second", &self.second)] {
            if words.is_empty() {
                return Err(WordlistError::Empty { list: name });
            }
            let mut seen: OrderedSet<&str> = OrderedSet::new();
            for (index, word) in words.iter().enumerate() {
                // Pronounceable, per `F-ID-3`, is served by the words being real words —
                // but the *format* has to hold, or a two-word pseudonym stops being two
                // words. Lower-case ASCII letters only: no spaces, no punctuation, and
                // capitalization left to the display.
                if !word.bytes().all(|b| b.is_ascii_lowercase()) {
                    return Err(WordlistError::Word {
                        list: name,
                        index,
                        detail: "lower-case ASCII letters only — casing is a display decision",
                    });
                }
                if !(2..=14).contains(&word.len()) {
                    return Err(WordlistError::Word {
                        list: name,
                        index,
                        detail: "a pseudonym word is between 2 and 14 letters",
                    });
                }
                if !seen.insert(word.as_str()) {
                    return Err(WordlistError::Duplicate {
                        list: name,
                        word: word.clone(),
                    });
                }
            }
        }
        // A word in both lists means `Willow Willow` is reachable, which reads as a mistake
        // rather than as a name.
        let firsts: OrderedSet<&str> = self.first.iter().map(String::as_str).collect();
        for word in &self.second {
            if firsts.contains(word.as_str()) {
                return Err(WordlistError::Shared { word: word.clone() });
            }
        }
        Ok(())
    }

    /// How many distinct two-word names this file can produce.
    ///
    /// The number the stability property in the module docs is stated against.
    #[must_use]
    pub fn pair_count(&self) -> u64 {
        self.first.len() as u64 * self.second.len() as u64
    }

    /// The unresolved draw — a pure function of this key and this file.
    ///
    /// Two contributors can receive the same pseudonym from this. [`assign`] is what
    /// resolves that, and what the UI should use.
    ///
    /// [`assign`]: Self::assign
    #[must_use]
    pub fn draw(&self, key: &AuthorKey) -> Pseudonym<'_> {
        self.resolve(self.indices(key, 0))
    }

    /// Assigns every contributor in a repository a distinct pseudonym (`F-ID-3`).
    ///
    /// Duplicate keys collapse; the iterator may be in any order.
    #[must_use]
    pub fn assign<I>(&self, keys: I) -> Roster<'_>
    where
        I: IntoIterator<Item = AuthorKey>,
    {
        let ordered: OrderedSet<AuthorKey> = keys.into_iter().collect();
        let mut taken: OrderedSet<Draw> = OrderedSet::new();
        let mut assigned: OrderedMap<AuthorKey, Draw> = OrderedMap::new();

        for key in &ordered {
            let base = self.indices(key, 0);
            let mut claim = None;
            for salt in 0..SALTED_ATTEMPTS {
                let candidate = if salt == 0 {
                    base
                } else {
                    self.indices(key, salt)
                };
                if taken.insert(candidate) {
                    claim = Some(candidate);
                    break;
                }
            }
            let claim = claim.unwrap_or_else(|| {
                // Bounded by construction: each key inserts exactly one entry, so at most
                // `ordered.len()` discriminators for this pair can already be taken.
                // Starting at 2 because "Ash Willow 1" reads as though there were a
                // numbering scheme, where "Ash Willow" and "Ash Willow 2" reads as the
                // second of two.
                for discriminator in 2..=(ordered.len() as u32 + 2) {
                    let candidate = Draw {
                        discriminator,
                        ..base
                    };
                    if taken.insert(candidate) {
                        return candidate;
                    }
                }
                unreachable!("more discriminators taken for one pair than there are keys")
            });
            assigned.insert(*key, claim);
        }

        Roster {
            wordlist: self,
            assigned,
        }
    }

    /// The word indices this key draws at this salt.
    fn indices(&self, key: &AuthorKey, salt: u64) -> Draw {
        let mut rng = Seed::root(DOMAIN)
            .derive(key.as_bytes())
            .derive_index(salt)
            .rng();
        // Casts are safe: `validate` refuses an empty list, and a list long enough to
        // overflow a u32 would not have parsed.
        Draw {
            first: rng.below_u32(self.first.len() as u32),
            second: rng.below_u32(self.second.len() as u32),
            discriminator: 0,
        }
    }

    fn resolve(&self, draw: Draw) -> Pseudonym<'_> {
        Pseudonym {
            first: &self.first[draw.first as usize],
            second: &self.second[draw.second as usize],
            discriminator: draw.discriminator,
        }
    }
}

/// Every contributor in one repository, each with a distinct pseudonym.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Roster<'w> {
    wordlist: &'w Wordlist,
    assigned: OrderedMap<AuthorKey, Draw>,
}

impl<'w> Roster<'w> {
    /// This contributor's pseudonym, or `None` if they were not among the keys assigned.
    #[must_use]
    pub fn get(&self, key: &AuthorKey) -> Option<Pseudonym<'w>> {
        self.assigned
            .get(key)
            .map(|&draw| self.wordlist.resolve(draw))
    }

    /// How many contributors this roster covers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.assigned.len()
    }

    /// Whether the repository had no contributors at all — PRD §6's empty repository.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.assigned.is_empty()
    }

    /// Every contributor, **in key order**.
    ///
    /// Key order is hash order, which is uncorrelated with contribution. Rendering this
    /// sequence produces no ranking to read (`N4`).
    pub fn iter(&self) -> impl Iterator<Item = (&AuthorKey, Pseudonym<'w>)> {
        self.assigned
            .iter()
            .map(|(key, &draw)| (key, self.wordlist.resolve(draw)))
    }
}

/// Why a wordlist was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WordlistError {
    /// The text is not a well-formed wordlist.
    Parse {
        /// The RON parser's message, including its position.
        detail: String,
    },
    /// The wordlist was written for a different format version.
    Version {
        /// The version the file declares.
        found: u32,
        /// The version this build understands.
        expected: u32,
    },
    /// One of the two lists has no words in it.
    Empty {
        /// Which list — `first` or `second`.
        list: &'static str,
    },
    /// A word is not in the form a pseudonym can be built from.
    Word {
        /// Which list it is in.
        list: &'static str,
        /// Its position in that list.
        index: usize,
        /// What the format requires.
        detail: &'static str,
    },
    /// A word appears twice in one list, which is a typo rather than a weighting.
    Duplicate {
        /// Which list.
        list: &'static str,
        /// The repeated word.
        word: String,
    },
    /// A word appears in both lists, which makes `Willow Willow` reachable.
    Shared {
        /// The word in both.
        word: String,
    },
}

impl fmt::Display for WordlistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse { detail } => write!(f, "pseudonym wordlist is not well-formed: {detail}"),
            Self::Version { found, expected } => write!(
                f,
                "pseudonym wordlist declares version {found}, this build reads version {expected}"
            ),
            Self::Empty { list } => write!(
                f,
                "pseudonym wordlist has no `{list}` words; F-ID-3 needs two words per name"
            ),
            Self::Word {
                list,
                index,
                detail,
            } => write!(f, "pseudonym wordlist `{list}` word {index}: {detail}"),
            Self::Duplicate { list, word } => write!(
                f,
                "pseudonym wordlist `{list}` repeats `{word}` — a duplicate is a typo, not a weight"
            ),
            Self::Shared { word } => write!(
                f,
                "pseudonym wordlist has `{word}` in both lists, which makes `{word} {word}` \
                 reachable"
            ),
        }
    }
}

impl core::error::Error for WordlistError {}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString as _;
    use alloc::vec::Vec;

    /// Distinct keys from distinct numbers.
    ///
    /// Spelled as an address rather than as raw bytes on purpose: [`AuthorKey::from_email`]
    /// ASCII-lowercases its input (`F-EXT-9`), so `n.to_le_bytes()` maps `0x41` and `0x61`
    /// to one contributor and a "400 keys" fixture is quietly 348. The first draft of this
    /// module did exactly that, and two tests failed with an off-by-13% that looked like a
    /// collision bug in `assign`.
    fn author(n: u32) -> AuthorKey {
        AuthorKey::from_email(alloc::format!("contributor-{n}@example.invalid").as_bytes())
    }

    fn authors(count: u32) -> Vec<AuthorKey> {
        (0..count).map(author).collect()
    }

    #[test]
    fn the_built_in_wordlist_parses_and_validates() {
        let list = Wordlist::built_in();
        assert_eq!(list.version, WORDLIST_VERSION);
        assert_eq!(list.first.len(), 128);
        assert_eq!(list.second.len(), 128);
    }

    /// The stability property in the module docs is a claim about this number. If the
    /// wordlist is ever trimmed, the claim has to be re-derived rather than silently
    /// weakened.
    #[test]
    fn the_wordlist_is_large_enough_for_the_stability_claim() {
        assert!(
            Wordlist::built_in().pair_count() >= 16_384,
            "the one-in-thirty-two-thousand figure in the module docs assumes this floor"
        );
    }

    /// `F-ID-3`: "The same contributor yields the same pseudonym on every machine." Within
    /// one machine that reduces to the draw being a function of the key and nothing else.
    #[test]
    fn a_draw_is_a_function_of_the_key_alone() {
        let list = Wordlist::built_in();
        let key = author(11);
        assert_eq!(list.draw(&key), list.draw(&key));
        assert_ne!(list.draw(&key), list.draw(&author(12)));
    }

    /// A wordlist small enough that collisions are certain.
    ///
    /// Sixteen pairs against a dozen contributors, so most keys have to redraw. Anything
    /// testing *collision resolution* against the built-in wordlist is testing the branch
    /// that never runs — see `assignment_does_not_depend_on_the_order_the_keys_arrive_in`
    /// for what that cost.
    fn crowded_wordlist() -> Wordlist {
        Wordlist::from_ron(
            r#"(
                version: 1,
                first: ["ashen", "birchen", "cedarn", "dawnlit"],
                second: ["fern", "glade", "hollow", "larch"],
            )"#,
        )
        .expect("test wordlist")
    }

    /// Whether resolution actually had to do anything — an assigned name differing from the
    /// key's own unresolved draw.
    fn resolution_happened(list: &Wordlist, roster: &Roster<'_>, keys: &[AuthorKey]) -> bool {
        keys.iter()
            .any(|key| roster.get(key) != Some(list.draw(key)))
    }

    #[test]
    fn every_contributor_in_a_roster_has_a_distinct_pseudonym() {
        // Both wordlists: the built-in one for the ordinary case, and the crowded one so the
        // uniqueness claim is also tested where it is actually under pressure.
        for (list, count) in [(Wordlist::built_in(), 400u32), (crowded_wordlist(), 12)] {
            let keys = authors(count);
            let roster = list.assign(keys.clone());
            assert_eq!(roster.len(), keys.len());

            let mut rendered: Vec<String> = roster.iter().map(|(_, p)| p.to_string()).collect();
            assert_eq!(rendered.len(), keys.len());
            rendered.sort();
            rendered.dedup();
            assert_eq!(rendered.len(), keys.len(), "two contributors share a name");
        }
    }

    /// The roster must be a function of the key *set*. A caller iterating a manifest in a
    /// different order — or a future caller iterating a different collection — must not get
    /// different names.
    ///
    /// # This test was vacuous once, and the sabotage found it
    ///
    /// The first version used the built-in wordlist and 200 keys, which collide never. With
    /// no collision there is nothing for the resolution order to decide, so replacing the
    /// key-ordered walk with the caller's arrival order **passed** — the test asserted a
    /// property it could not observe. Same failure shape as `readonly-audit`'s "an oracle
    /// pointed at the wrong repository agrees with itself perfectly".
    ///
    /// It now runs on [`crowded_wordlist`] and refuses to proceed unless resolution actually
    /// fired, so a fixture that stops forcing collisions fails loudly instead of quietly
    /// testing nothing.
    #[test]
    fn assignment_does_not_depend_on_the_order_the_keys_arrive_in() {
        let list = crowded_wordlist();
        let keys = authors(12);
        let forward = list.assign(keys.clone());
        assert!(
            resolution_happened(&list, &forward, &keys),
            "the fixture no longer forces a collision — this test would prove nothing"
        );

        let mut reversed = keys.clone();
        reversed.reverse();
        let backward = list.assign(reversed);
        for key in &keys {
            assert_eq!(forward.get(key), backward.get(key), "{key:?} moved");
        }

        // A duplicate key is one contributor, not two — and specifically it must not consume
        // a second word pair, which would push its own second occurrence into a redraw.
        let mut doubled = keys.clone();
        doubled.extend(keys.iter().copied());
        let twice = list.assign(doubled);
        assert_eq!(twice.len(), keys.len());
        for key in &keys {
            assert_eq!(forward.get(key), twice.get(key), "{key:?} moved");
        }
    }

    /// The stability claim, exercised: a contributor arriving does not rename the others.
    ///
    /// This holds because the new key does not draw anybody's pair. It is deterministic on
    /// fixed keys — if a future wordlist edit makes these keys collide, this test fails and
    /// the claim in the module docs is what needs re-checking.
    #[test]
    fn adding_a_contributor_leaves_the_others_alone() {
        let list = Wordlist::built_in();
        let existing = authors(120);
        let before = list.assign(existing.clone());

        let mut grown = existing.clone();
        grown.push(author(9_999));
        let after = list.assign(grown);

        for key in &existing {
            assert_eq!(before.get(key), after.get(key), "{key:?} was renamed");
        }
        assert!(after.get(&author(9_999)).is_some());
    }

    /// The collision machinery, forced. One word in each list means every contributor draws
    /// the same pair, so this exercises the salted redraws exhausting and the discriminator
    /// taking over — the path the built-in wordlist never reaches.
    #[test]
    fn contributors_who_cannot_be_told_apart_by_words_are_told_apart_anyway() {
        let list =
            Wordlist::from_ron(r#"(version: 1, first: ["ash"], second: ["willow"])"#).unwrap();
        assert_eq!(list.pair_count(), 1);

        let keys = authors(4);
        let roster = list.assign(keys.clone());

        let mut rendered: Vec<String> = keys
            .iter()
            .map(|k| roster.get(k).expect("assigned").to_string())
            .collect();
        rendered.sort();
        assert_eq!(
            rendered,
            ["Ash Willow", "Ash Willow 2", "Ash Willow 3", "Ash Willow 4"]
        );
    }

    #[test]
    fn display_is_title_cased_and_hides_an_absent_discriminator() {
        let list = Wordlist::built_in();
        let name = list.draw(&author(3));
        let rendered = name.to_string();
        assert_eq!(name.discriminator(), 0);
        assert!(!rendered.ends_with(char::is_numeric), "{rendered}");
        assert_eq!(
            rendered,
            alloc::format!(
                "{}{} {}{}",
                name.first()[..1].to_ascii_uppercase(),
                &name.first()[1..],
                name.second()[..1].to_ascii_uppercase(),
                &name.second()[1..],
            )
        );
    }

    /// PRD §6, "Empty repository": no contributors is an ordinary state.
    #[test]
    fn a_repository_with_no_contributors_gets_an_empty_roster() {
        let list = Wordlist::built_in();
        let roster = list.assign(Vec::new());
        assert!(roster.is_empty());
        assert_eq!(roster.len(), 0);
        assert!(roster.get(&author(1)).is_none());
    }

    #[test]
    fn each_rule_refuses_the_edit_that_breaks_it() {
        let base = Wordlist::built_in();

        let mut wrong_version = base.clone();
        wrong_version.version = WORDLIST_VERSION + 1;
        assert!(matches!(
            wrong_version.validate(),
            Err(WordlistError::Version { .. })
        ));

        let mut empty = base.clone();
        empty.second.clear();
        assert!(matches!(
            empty.validate(),
            Err(WordlistError::Empty { list: "second" })
        ));

        let mut spaced = base.clone();
        spaced.first[0] = "two words".to_string();
        assert!(matches!(
            spaced.validate(),
            Err(WordlistError::Word {
                list: "first",
                index: 0,
                ..
            })
        ));

        let mut shouted = base.clone();
        shouted.first[1] = "Amber".to_string();
        assert!(matches!(
            shouted.validate(),
            Err(WordlistError::Word { list: "first", .. })
        ));

        let mut repeated = base.clone();
        repeated.second[1] = repeated.second[0].clone();
        assert!(matches!(
            repeated.validate(),
            Err(WordlistError::Duplicate { list: "second", .. })
        ));

        let mut shared = base.clone();
        shared.second[0] = shared.first[0].clone();
        assert!(matches!(
            shared.validate(),
            Err(WordlistError::Shared { .. })
        ));
    }

    #[test]
    fn an_unknown_field_is_refused_rather_than_ignored() {
        let text = r#"(version: 1, first: ["ash"], second: ["willow"], theme: "forest")"#;
        assert!(matches!(
            Wordlist::from_ron(text),
            Err(WordlistError::Parse { .. })
        ));
    }
}
