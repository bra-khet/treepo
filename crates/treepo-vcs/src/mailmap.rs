//! `F-EXT-9` — one human is one contributor.
//!
//! > Author identity normalization honors the repository's `.mailmap` when present, then
//! > falls back to case-normalized email as the identity key. One human with three email
//! > addresses must be one contributor (this materially affects `author_distribution` and
//! > every downstream color assignment).
//!
//! The downstream part is why this is `P0` rather than a nicety. `F-ID-3` seeds a pseudonym
//! from the identity key and `F-ID-4` seeds a colour from it, so an unnormalized repository
//! does not merely miscount contributors — it renders one person as three differently
//! coloured strangers scattered across the same limb.
//!
//! # No name ever leaves this module
//!
//! `.mailmap` maps names as well as addresses, and `gix` resolves both. Only the address is
//! read here, and only to hash it: [`AuthorKey`] has no name field, and `N9` makes the model
//! layer structurally incapable of carrying one. Names live in `treepo-id` behind
//! `IdentityPolicy`, which is the single gate `F-ID-5` requires for the live view and every
//! export alike.

use gix::actor::SignatureRef;
use gix::bstr::ByteSlice as _;
use treepo_model::identity::AuthorKey;

/// The repository's identity normalization (`F-EXT-9`).
#[derive(Debug, Default)]
pub struct Identities {
    mailmap: gix::mailmap::Snapshot,
}

impl Identities {
    /// Loads `.mailmap` from the repository, if it has one.
    ///
    /// A missing or malformed `.mailmap` yields an empty snapshot rather than an error:
    /// most repositories have none, and that is the ordinary case rather than a degraded
    /// one. `AC-EXT-3` is the observable difference — the same repository without the file
    /// reports a higher `author_count`.
    #[must_use]
    pub fn load(repo: &gix::Repository) -> Self {
        Self {
            mailmap: repo.open_mailmap(),
        }
    }

    /// No normalization — email case folding only.
    ///
    /// The fallback `F-EXT-9` specifies, and what a directory with no repository gets.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// The identity key for a commit's author.
    ///
    /// `.mailmap` is applied first, then [`AuthorKey::from_email`] folds ASCII case. A
    /// commit with no usable address collapses to [`AuthorKey::unattributed`] rather than
    /// minting a key per malformed spelling — those are not people, and one key per spelling
    /// would inflate `author_count` with noise.
    #[must_use]
    pub fn key(&self, signature: SignatureRef<'_>) -> AuthorKey {
        let resolved = self.mailmap.resolve_cow(signature);
        let email = resolved.email.trim();
        if email.is_empty() {
            return AuthorKey::unattributed();
        }
        AuthorKey::from_email(email)
    }

    /// Whether the repository supplied any mapping at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.mailmap.iter().next().is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gix::bstr::ByteSlice;

    fn signature<'a>(name: &'a str, email: &'a str) -> SignatureRef<'a> {
        SignatureRef {
            name: name.as_bytes().as_bstr(),
            email: email.as_bytes().as_bstr(),
            time: "0 +0000",
        }
    }

    #[test]
    fn without_a_mailmap_only_case_is_folded() {
        let identities = Identities::none();
        assert!(identities.is_empty());
        let lower = identities.key(signature("Ada", "ada@example.com"));
        let upper = identities.key(signature("Ada Lovelace", "Ada@Example.COM"));
        let other = identities.key(signature("Ada", "ada@work.example.com"));
        // The name is not part of the key; the address is, case-insensitively.
        assert_eq!(lower, upper);
        assert_ne!(lower, other);
    }

    /// `F-EXT-9`: a commit with no usable address is one contributor, not many.
    #[test]
    fn unusable_addresses_collapse_to_one_key() {
        let identities = Identities::none();
        let empty = identities.key(signature("nobody", ""));
        let blank = identities.key(signature("someone else", "   "));
        assert_eq!(empty, AuthorKey::unattributed());
        assert_eq!(blank, AuthorKey::unattributed());
    }

    /// The mapping `AC-EXT-3` is about: three addresses, one human, one key.
    #[test]
    fn a_mailmap_collapses_aliases() {
        let text = b"Ada Lovelace <ada@example.com> <ada@work.example.com>\n\
                     Ada Lovelace <ada@example.com> <a.lovelace@old.example.com>\n";
        let identities = Identities {
            mailmap: gix::mailmap::Snapshot::from_bytes(text),
        };
        assert!(!identities.is_empty());

        let canonical = identities.key(signature("Ada", "ada@example.com"));
        let work = identities.key(signature("Ada L", "ada@work.example.com"));
        let old = identities.key(signature("A. Lovelace", "a.lovelace@old.example.com"));
        assert_eq!(canonical, work);
        assert_eq!(canonical, old);

        // Someone genuinely different stays different.
        assert_ne!(
            canonical,
            identities.key(signature("Bob", "bob@example.com"))
        );
    }

    /// The same repository read without its `.mailmap` reports more contributors —
    /// `AC-EXT-3` stated as a property rather than a fixture.
    #[test]
    fn dropping_the_mailmap_raises_the_author_count() {
        let text = b"Ada <ada@example.com> <ada@work.example.com>\n";
        let mapped = Identities {
            mailmap: gix::mailmap::Snapshot::from_bytes(text),
        };
        let unmapped = Identities::none();

        let addresses = ["ada@example.com", "ada@work.example.com"];
        let distinct = |identities: &Identities| {
            addresses
                .iter()
                .map(|address| identities.key(signature("Ada", address)))
                .collect::<std::collections::BTreeSet<_>>()
                .len()
        };
        assert_eq!(distinct(&mapped), 1);
        assert_eq!(distinct(&unmapped), 2);
    }
}
