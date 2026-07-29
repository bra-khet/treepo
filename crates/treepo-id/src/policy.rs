//! The single gate every contributor display passes through — `F-ID-5`, `F-ID-7`, `N9`.
//!
//! > One setting governs both live view and exports (`N9`). It is not separable — an export
//! > cannot reveal what the live view conceals.
//!
//! [`pseudonym`](crate::pseudonym) and [`palette`](crate::palette) are pure functions of an
//! [`AuthorKey`] and know nothing about policy. This module is where the policy lives, and
//! it is deliberately the *only* thing that produces a display for a contributor.
//!
//! # How `F-ID-5` is enforced, rather than promised
//!
//! Three properties, and only the third is a matter of discipline:
//!
//! 1. **[`IdentityView::identify`] takes no policy argument.** The policy is fixed when the
//!    view is constructed, so there is no call site at which an exporter could ask for a
//!    different answer than the renderer got. "Not separable" is the absence of a parameter.
//! 2. **A pseudonymous view holds no real names at all.** [`RealNames`] is stored only by
//!    [`IdentityView::revealed`]; under the default constructor the field is `None` and
//!    there is nothing in the whole structure a name could come out of. `AC-ID-1` therefore
//!    survives a bug in the display path, not merely a correct display path.
//! 3. **Both consumers must be handed the same view.** That one is architectural and lands
//!    with Phase 10's settings, where the view is built once per repository from
//!    `config.json` (`F-ID-6`). Until then it is written down here.
//!
//! # `F-ID-7` is the default, and it is not an error
//!
//! > When the user is not a contributor to the repository — the common case for a cloned
//! > public repository — every contributor including the viewer is pseudonymous.
//!
//! That state is simply `viewer` being `None`, or being a key no contributor matches. It
//! needs no branch of its own and no message. [`IdentityView::viewer_is_a_contributor`]
//! exists so a UI can *word* things correctly, not so anything can go wrong.
//!
//! # What a revealed name costs to obtain, and why that is a feature
//!
//! `treepo-model` carries no names, so [`RealNames`] cannot be read out of a manifest — it
//! has to come from the repository. **A stored manifest therefore cannot be de-anonymized by
//! toggling a setting**, and neither can one shared through `F-MAN-11`. Reveal requires the
//! repository itself, which is the same thing as requiring the access that would let someone
//! run `git log` anyway.

use crate::palette::{AuthorColor, Palette};
use crate::pseudonym::{Pseudonym, Roster};
use alloc::string::String;
use core::fmt;
use treepo_det::OrderedMap;
use treepo_model::identity::AuthorKey;

/// Which identity level a view is showing.
///
/// Not a setting a caller passes around — it is read *off* an [`IdentityView`], which is
/// what makes it impossible to hold one policy and ask for another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityPolicy {
    /// Everyone but the viewer is a pseudonym. The default, and `N9`'s promise.
    Pseudonymous,
    /// Real identities are shown — the per-repository opt-in of `F-ID-6`, behind the
    /// confirmation `design/feature-system.md` §3.4 drafts.
    Revealed,
}

/// Real contributor names, for a revealed view only.
///
/// # This type is the leak, and it is shaped to be hard to spill
///
/// Its [`Debug`] renders a count rather than its contents, for the reason
/// [`AuthorShare`](treepo_model::primitives::AuthorShare)'s renders a bucket: a debug dump,
/// a log line, or a panic message that happens to include a view must not become the
/// disclosure the whole crate exists to prevent. There is no iterator and no accessor —
/// the only way a name leaves is [`Identification::Revealed`], from a view built with
/// [`IdentityView::revealed`].
#[derive(Clone, Default, PartialEq, Eq)]
pub struct RealNames(OrderedMap<AuthorKey, String>);

impl RealNames {
    /// No names.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one contributor's real name.
    pub fn insert(&mut self, key: AuthorKey, name: String) {
        self.0.insert(key, name);
    }

    /// How many contributors have a recorded name.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether nothing is recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn get(&self, key: &AuthorKey) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }
}

impl FromIterator<(AuthorKey, String)> for RealNames {
    fn from_iter<I: IntoIterator<Item = (AuthorKey, String)>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl fmt::Debug for RealNames {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RealNames({} withheld)", self.0.len())
    }
}

/// How one contributor is shown.
///
/// The *variant* carries what kind of identity it is, so a consumer can act on the kind
/// without inspecting a policy. `F-ID-8` and `AC-EXP-2` are the motivating case: an exporter
/// writing file metadata can refuse anything that is not [`Pseudonymous`] with a `match`
/// rather than with a flag it has to remember to check.
///
/// [`Pseudonymous`]: Self::Pseudonymous
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Identification<'a> {
    /// The person running treepo (`F-ID-1`).
    ///
    /// **Carries no name, deliberately.** The PRD permits showing the viewer's own name —
    /// `AC-ID-1` protects contributors "other than the user" — and not showing it buys
    /// something real: a shared tree, an export, or a screenshot says "You" where the viewer
    /// appears, so publishing one does not announce who made it. The viewer already knows
    /// their own name, so nothing is lost. `treepo-vcs::self_ident` reads `user.name` only
    /// for `.mailmap` resolution and discards it for the same reason.
    Yourself,
    /// A stable pseudonym — the default for everyone else (`F-ID-2`, `F-ID-3`).
    Pseudonymous(Pseudonym<'a>),
    /// A real identity, reachable only from [`IdentityView::revealed`] (`F-ID-6`).
    Revealed {
        /// The contributor's name as the repository records it.
        name: &'a str,
    },
}

/// `You`, the pseudonym, or the real name — the rendering a UI wants by default.
impl fmt::Display for Identification<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Yourself => f.write_str("You"),
            Self::Pseudonymous(pseudonym) => write!(f, "{pseudonym}"),
            Self::Revealed { name } => f.write_str(name),
        }
    }
}

/// Everything needed to show a repository's contributors, at one identity level.
///
/// Built once per repository. See the module docs for why there is no per-call policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityView<'w> {
    roster: Roster<'w>,
    palette: &'w Palette,
    viewer: Option<AuthorKey>,
    /// `Some` exactly when the policy is [`IdentityPolicy::Revealed`]. The two are the same
    /// fact, so they cannot disagree.
    revealed: Option<RealNames>,
}

impl<'w> IdentityView<'w> {
    /// The default view: everyone but the viewer is a pseudonym (`N9`, `F-ID-2`).
    ///
    /// `viewer` is `treepo_vcs::self_identity(...)`'s key, or the manifest's
    /// [`self_author`](treepo_model::manifest::AuthorTable::self_author). `None` — and a key
    /// that matches no contributor — are both `F-ID-7`'s ordinary state.
    #[must_use]
    pub fn pseudonymous(
        roster: Roster<'w>,
        palette: &'w Palette,
        viewer: Option<AuthorKey>,
    ) -> Self {
        Self {
            roster,
            palette,
            viewer,
            revealed: None,
        }
    }

    /// The opted-in view: real names where they are known (`F-ID-6`).
    ///
    /// Constructing one is the whole of the reveal. There is no setter, so a view cannot
    /// change level under a consumer holding it — `AC-ID-4`'s "live view and subsequent
    /// exports together" is served by both being handed a newly built view, never by one of
    /// them being mutated.
    #[must_use]
    pub fn revealed(
        roster: Roster<'w>,
        palette: &'w Palette,
        viewer: Option<AuthorKey>,
        names: RealNames,
    ) -> Self {
        Self {
            roster,
            palette,
            viewer,
            revealed: Some(names),
        }
    }

    /// Which identity level this view shows.
    #[must_use]
    pub fn policy(&self) -> IdentityPolicy {
        match self.revealed {
            None => IdentityPolicy::Pseudonymous,
            Some(_) => IdentityPolicy::Revealed,
        }
    }

    /// The viewer's key, if the machine has a git identity configured (`F-ID-1`).
    #[must_use]
    pub fn viewer(&self) -> Option<AuthorKey> {
        self.viewer
    }

    /// Whether the viewer has actually contributed here (`F-ID-7`).
    ///
    /// `false` is the common case and not an error — it is what happens every time someone
    /// opens a repository they cloned. A UI may use it to word a panel; nothing else should
    /// branch on it.
    #[must_use]
    pub fn viewer_is_a_contributor(&self) -> bool {
        self.viewer
            .is_some_and(|key| self.roster.get(&key).is_some())
    }

    /// How this contributor is shown.
    ///
    /// `None` for a key this repository has no contributor for. That is a caller error
    /// rather than an ordinary case — every key a renderer holds comes from the manifest the
    /// roster was built from — and it is an `Option` rather than a fallback pseudonym
    /// because an unrostered draw could collide with a rostered one, which is exactly what
    /// [`Wordlist::assign`](crate::pseudonym::Wordlist::assign) exists to prevent.
    ///
    /// The viewer is answered before the roster is consulted, so `identify` on your own key
    /// is [`Identification::Yourself`] whether or not you have committed here.
    #[must_use]
    pub fn identify(&self, key: &AuthorKey) -> Option<Identification<'_>> {
        if self.viewer == Some(*key) {
            return Some(Identification::Yourself);
        }
        let pseudonym = self.roster.get(key)?;
        // A revealed view with no recorded name for someone falls back to their pseudonym.
        // Reveal shows what is known; a gap in the names is not a reason to show nothing.
        match self.revealed.as_ref().and_then(|names| names.get(key)) {
            Some(name) => Some(Identification::Revealed { name }),
            None => Some(Identification::Pseudonymous(pseudonym)),
        }
    }

    /// This contributor's colour (`F-ID-4`).
    ///
    /// **Unaffected by the policy**, and that is the design rather than an omission: the
    /// colour is seeded from the key, so a mosaic looks identical whether or not names are
    /// showing. Reveal changes what a label says, not what the tree looks like.
    #[must_use]
    pub fn color_of(&self, key: &AuthorKey) -> AuthorColor {
        self.palette.color_of(key)
    }

    /// Every contributor, in key order (`N4` — hash order carries no ranking).
    pub fn contributors(&self) -> impl Iterator<Item = (&AuthorKey, Identification<'_>)> {
        self.roster.iter().map(move |(key, pseudonym)| {
            let identification = if self.viewer == Some(*key) {
                Identification::Yourself
            } else {
                match self.revealed.as_ref().and_then(|names| names.get(key)) {
                    Some(name) => Identification::Revealed { name },
                    None => Identification::Pseudonymous(pseudonym),
                }
            };
            (key, identification)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pseudonym::Wordlist;
    use alloc::string::ToString as _;
    use alloc::vec::Vec;

    fn author(n: u32) -> AuthorKey {
        AuthorKey::from_email(alloc::format!("contributor-{n}@example.invalid").as_bytes())
    }

    fn names() -> RealNames {
        (0..4u32)
            .map(|n| (author(n), alloc::format!("Real Person {n}")))
            .collect()
    }

    struct Fixture {
        wordlist: Wordlist,
        palette: Palette,
        keys: Vec<AuthorKey>,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                wordlist: Wordlist::built_in(),
                palette: Palette::built_in(),
                keys: (0..4u32).map(author).collect(),
            }
        }

        fn view(&self, viewer: Option<AuthorKey>) -> IdentityView<'_> {
            IdentityView::pseudonymous(
                self.wordlist.assign(self.keys.iter().copied()),
                &self.palette,
                viewer,
            )
        }

        fn revealed_view(&self, viewer: Option<AuthorKey>) -> IdentityView<'_> {
            IdentityView::revealed(
                self.wordlist.assign(self.keys.iter().copied()),
                &self.palette,
                viewer,
                names(),
            )
        }
    }

    /// `AC-ID-1`, at the gate. Under the default policy no contributor resolves to a real
    /// identity — and cannot, because the view holds none.
    #[test]
    fn the_default_policy_never_produces_a_real_name() {
        let fixture = Fixture::new();
        let view = fixture.view(Some(author(0)));
        assert_eq!(view.policy(), IdentityPolicy::Pseudonymous);

        for (_, identification) in view.contributors() {
            assert!(
                !matches!(identification, Identification::Revealed { .. }),
                "a pseudonymous view produced a real identity"
            );
        }
        for key in &fixture.keys {
            let rendered = view.identify(key).expect("a contributor").to_string();
            assert!(!rendered.contains("Real Person"), "{rendered}");
        }
    }

    /// `F-ID-1`: the viewer is themselves, and is rendered as `You` rather than as a name.
    #[test]
    fn the_viewer_is_yourself_and_carries_no_name() {
        let fixture = Fixture::new();
        let view = fixture.view(Some(author(1)));
        assert_eq!(view.identify(&author(1)), Some(Identification::Yourself));
        assert_eq!(view.identify(&author(1)).unwrap().to_string(), "You");
        assert!(view.viewer_is_a_contributor());

        // Everyone else is a pseudonym, including under reveal — see the next test.
        assert!(matches!(
            view.identify(&author(2)),
            Some(Identification::Pseudonymous(_))
        ));
    }

    /// `F-ID-7`: nobody configured, or configured but not a contributor. Both are the
    /// ordinary state — every contributor including the viewer is pseudonymous.
    #[test]
    fn a_viewer_who_is_not_a_contributor_is_the_ordinary_state() {
        let fixture = Fixture::new();

        let anonymous = fixture.view(None);
        assert!(!anonymous.viewer_is_a_contributor());
        assert!(anonymous.identify(&author(0)).is_some());
        for (_, identification) in anonymous.contributors() {
            assert!(matches!(identification, Identification::Pseudonymous(_)));
        }

        // Configured, but has never committed here — the common case for a cloned
        // repository. Still not a contributor, and nothing errors.
        let stranger = fixture.view(Some(author(99)));
        assert!(!stranger.viewer_is_a_contributor());
        assert_eq!(
            stranger.identify(&author(99)),
            Some(Identification::Yourself)
        );
        assert!(stranger.contributors().count() == fixture.keys.len());
    }

    #[test]
    fn a_revealed_view_shows_names_for_everyone_but_the_viewer() {
        let fixture = Fixture::new();
        let view = fixture.revealed_view(Some(author(0)));
        assert_eq!(view.policy(), IdentityPolicy::Revealed);

        assert_eq!(view.identify(&author(0)), Some(Identification::Yourself));
        assert_eq!(
            view.identify(&author(2)),
            Some(Identification::Revealed {
                name: "Real Person 2"
            })
        );
        assert_eq!(
            view.identify(&author(2)).unwrap().to_string(),
            "Real Person 2"
        );
    }

    /// Reveal shows what is known. A contributor with no recorded name keeps their
    /// pseudonym rather than rendering as nothing.
    #[test]
    fn a_revealed_view_falls_back_to_the_pseudonym_for_an_unnamed_contributor() {
        let wordlist = Wordlist::built_in();
        let palette = Palette::built_in();
        let keys: Vec<AuthorKey> = (0..4u32).map(author).collect();
        let mut partial = RealNames::new();
        partial.insert(author(1), "Real Person 1".to_string());

        let view = IdentityView::revealed(
            wordlist.assign(keys.iter().copied()),
            &palette,
            None,
            partial,
        );
        assert!(matches!(
            view.identify(&author(1)),
            Some(Identification::Revealed { .. })
        ));
        assert!(matches!(
            view.identify(&author(3)),
            Some(Identification::Pseudonymous(_))
        ));
    }

    /// `F-ID-4`: reveal changes labels, not the tree. A mosaic must not repaint itself when
    /// someone toggles a privacy setting.
    #[test]
    fn revealing_does_not_change_a_single_colour() {
        let fixture = Fixture::new();
        let hidden = fixture.view(Some(author(0)));
        let shown = fixture.revealed_view(Some(author(0)));
        for key in &fixture.keys {
            assert_eq!(hidden.color_of(key), shown.color_of(key));
        }
    }

    /// A key from outside the repository is a caller error, not a contributor to invent a
    /// name for.
    #[test]
    fn an_unrostered_key_has_no_identification() {
        let fixture = Fixture::new();
        assert!(fixture.view(None).identify(&author(4_242)).is_none());
    }

    /// `N9`: a debug dump of a revealed view must not be the disclosure.
    #[test]
    fn debug_output_withholds_the_names_it_holds() {
        let fixture = Fixture::new();
        let rendered = alloc::format!("{:?}", fixture.revealed_view(None));
        assert!(rendered.contains("RealNames(4 withheld)"), "{rendered}");
        assert!(!rendered.contains("Real Person"), "{rendered}");
    }
}
