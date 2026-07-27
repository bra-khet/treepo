//! Stable keys: which repository this is, and which contributor a commit belongs to.
//!
//! Neither key carries a name. That is `N9` expressed as a type: real identities live in
//! `treepo-id` behind `IdentityPolicy`, and the crate every generated value flows through
//! has no field to leak them from. A pseudonym and a colour are derived from
//! [`AuthorKey`] alone, which is exactly enough for the visual language to work.

use alloc::string::{String, ToString as _};
use core::fmt;
use treepo_det::hash::Sha256;

/// A git object id — 20 bytes for SHA-1, 32 for SHA-256.
///
/// Both lengths are stored inline rather than as a `Vec`, and the length is carried
/// explicitly so a SHA-256 repository is not silently truncated to something that looks
/// like a valid SHA-1.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommitId {
    bytes: [u8; 32],
    len: u8,
}

impl CommitId {
    /// A SHA-1 object id, as every git repository written to date uses.
    #[must_use]
    pub const fn sha1(bytes: [u8; 20]) -> Self {
        let mut padded = [0u8; 32];
        let mut i = 0;
        while i < 20 {
            padded[i] = bytes[i];
            i += 1;
        }
        Self {
            bytes: padded,
            len: 20,
        }
    }

    /// A SHA-256 object id, for repositories using git's newer object format.
    #[must_use]
    pub const fn sha256(bytes: [u8; 32]) -> Self {
        Self { bytes, len: 32 }
    }

    /// The significant bytes — 20 or 32 of them.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }
}

impl fmt::Display for CommitId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.as_bytes() {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for CommitId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Abbreviated the way git abbreviates, so a debug dump stays readable.
        let hex = self.to_string();
        write!(f, "CommitId({})", &hex[..12.min(hex.len())])
    }
}

/// A contributor, identified by a hash of their mailmap-normalized email.
///
/// 16 bytes, truncated from SHA-256. Collision resistance is not the property being bought
/// here — the key never authenticates anything and the population is at most a few thousand
/// contributors, where 128 bits is far past sufficient. What it buys is that the key is
/// **one-way**: a manifest, an export, or a shared package carries no recoverable email.
///
/// `F-EXT-9` normalizes first: `.mailmap` is applied, then the email is ASCII-lowercased, so
/// one human with three addresses is one key.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AuthorKey([u8; 16]);

impl AuthorKey {
    /// Domain separator. Changing this string re-keys every author in every stored
    /// manifest, which is a `schema_version` bump, not an edit.
    const DOMAIN: &'static [u8] = b"treepo/author/v1";

    /// Derives the key from an email that `.mailmap` has already resolved.
    ///
    /// The email is ASCII-lowercased here so callers cannot forget to (`F-EXT-9`). Full
    /// Unicode case folding is deliberately not applied: it would need a table that becomes
    /// a determinism input, and the local part of an address is case-sensitive by RFC
    /// anyway — ASCII folding matches what mail providers actually do.
    #[must_use]
    pub fn from_email(email: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(Self::DOMAIN);
        hasher.update(&[0x00]);
        for byte in email {
            hasher.update(&[byte.to_ascii_lowercase()]);
        }
        Self(hasher.finalize().truncated_16())
    }

    /// The key for a commit with no usable email at all — an empty or malformed author
    /// field, which real repositories do contain.
    ///
    /// One shared key rather than one per malformed spelling: these are not people, and
    /// giving each its own key would inflate `author_count` with noise.
    #[must_use]
    pub fn unattributed() -> Self {
        Self::from_email(b"")
    }

    /// The raw key bytes, for seeding a pseudonym or a colour.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl fmt::Debug for AuthorKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The first four bytes only. Enough to correlate two lines of a debug dump, and a
        // reminder that this is a key rather than something to render.
        write!(
            f,
            "AuthorKey({:02x}{:02x}{:02x}{:02x}…)",
            self.0[0], self.0[1], self.0[2], self.0[3]
        )
    }
}

/// How a repository's identity was established (`F-MAN-3`).
///
/// Ordered by precedence: a fork of an upstream repository shares its root commit but has
/// its own remote, and tier 1 winning is what keeps the two apart (PRD §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IdentityTier {
    /// Normalized remote URL. Two clones of one remote share a store (`AC-MAN-4`).
    Remote = 1,
    /// Root commit SHA, when there is no remote. Survives a folder move (`AC-MAN-5`).
    RootCommit = 2,
    /// Absolute path hash, when there is no remote and no commit to hash.
    ///
    /// The only tier that does not survive a move, because there is nothing else to key on.
    PathHash = 3,
}

/// The stable name of a repository's store directory (`F-MAN-3`).
///
/// Resolution lives in `treepo-store`; this is the shape it produces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoIdentity {
    /// SHA-256 of the tier's source value. Names the store directory.
    pub key: [u8; 32],
    /// Which rule produced the key.
    pub tier: IdentityTier,
    /// The normalized remote URL, root commit hex, or absolute path the key was built from.
    ///
    /// Kept so `identity.json` is inspectable (`N2`) and so `F-MAN-5` can offer to relink a
    /// store when a remote URL changes upstream.
    pub source_value: String,
    /// Wall-clock seconds when resolution ran.
    ///
    /// **The one clock reading in this crate, and it never reaches generation.** It is
    /// store housekeeping — how `F-MAN-9`'s browser shows a last-opened time. Nothing in
    /// `treepo-gen` or `treepo-grow` may read it, or `AC-DET-1` is lost.
    pub resolved_at: u64,
}

impl RepoIdentity {
    /// Domain separator, as for [`AuthorKey`].
    const DOMAIN: &'static [u8] = b"treepo/repo/v1";

    /// Builds an identity by hashing `source_value` under its tier.
    ///
    /// The tier is part of the hash input, so an identical string cannot collide across
    /// tiers — a path that happens to look like a remote URL stays a distinct repository.
    #[must_use]
    pub fn new(tier: IdentityTier, source_value: String, resolved_at: u64) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(Self::DOMAIN);
        hasher.update(&[tier as u8]);
        hasher.update(source_value.as_bytes());
        Self {
            key: hasher.finalize().into_bytes(),
            tier,
            source_value,
            resolved_at,
        }
    }

    /// The store directory name: the key as lowercase hex.
    #[must_use]
    pub fn directory_name(&self) -> String {
        let mut name = String::with_capacity(64);
        for byte in self.key {
            // `core::fmt::Write` is in scope via the trait import below.
            use core::fmt::Write as _;
            let _ = write!(name, "{byte:02x}");
        }
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_id_renders_full_hex_and_debug_abbreviates() {
        let id = CommitId::sha1([
            0xf3, 0x32, 0xd8, 0x8a, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99,
            0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
        ]);
        assert_eq!(id.to_string(), "f332d88a00112233445566778899aabbccddeeff");
        assert_eq!(id.as_bytes().len(), 20);
        assert_eq!(alloc::format!("{id:?}"), "CommitId(f332d88a0011)");
    }

    /// A SHA-256 id must not be mistakable for a SHA-1 one.
    #[test]
    fn commit_id_lengths_are_distinct() {
        let short = CommitId::sha1([0xab; 20]);
        let long = CommitId::sha256([0xab; 32]);
        assert_ne!(short, long);
        assert_eq!(short.as_bytes().len(), 20);
        assert_eq!(long.as_bytes().len(), 32);
    }

    /// `F-EXT-9`: one human with three spellings is one contributor.
    #[test]
    fn author_keys_fold_ascii_case() {
        let lower = AuthorKey::from_email(b"ada@example.com");
        let upper = AuthorKey::from_email(b"Ada@Example.COM");
        let other = AuthorKey::from_email(b"ada@example.org");
        assert_eq!(lower, upper);
        assert_ne!(lower, other);
    }

    /// The key must not be reversible into the address it came from.
    #[test]
    fn author_key_reveals_nothing() {
        let key = AuthorKey::from_email(b"ada@example.com");
        let bytes = key.as_bytes();
        assert_eq!(bytes.len(), 16);
        assert!(!bytes.windows(3).any(|w| w == b"ada"));
        // Debug output is a correlation aid, not an identity.
        let rendered = alloc::format!("{key:?}");
        assert!(rendered.starts_with("AuthorKey("));
        assert!(!rendered.contains("example"));
    }

    #[test]
    fn unattributed_is_stable_and_distinct() {
        assert_eq!(AuthorKey::unattributed(), AuthorKey::unattributed());
        assert_ne!(AuthorKey::unattributed(), AuthorKey::from_email(b" "));
    }

    /// PRD §6, "Fork of an upstream repository": same string, different tier, different
    /// repository.
    #[test]
    fn identity_tiers_do_not_collide_on_equal_source_values() {
        let value = "https://example.com/repo.git".to_string();
        let remote = RepoIdentity::new(IdentityTier::Remote, value.clone(), 0);
        let path = RepoIdentity::new(IdentityTier::PathHash, value, 0);
        assert_ne!(remote.key, path.key);
    }

    /// `resolved_at` is metadata; it must not change the store a repository lands in.
    #[test]
    fn identity_key_ignores_the_clock() {
        let a = RepoIdentity::new(IdentityTier::Remote, "x".to_string(), 0);
        let b = RepoIdentity::new(IdentityTier::Remote, "x".to_string(), 1_777_000_000);
        assert_eq!(a.key, b.key);
        assert_eq!(a.directory_name(), b.directory_name());
        assert_eq!(a.directory_name().len(), 64);
    }

    #[test]
    fn tier_precedence_is_ordered() {
        assert!(IdentityTier::Remote < IdentityTier::RootCommit);
        assert!(IdentityTier::RootCommit < IdentityTier::PathHash);
    }
}
