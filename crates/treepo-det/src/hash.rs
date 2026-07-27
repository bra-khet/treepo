//! Stable hashing, and the seed tree that hangs off it.
//!
//! `P2` says generation is "seeded hierarchically from path hashes, never from wall-clock
//! time or ambient machine state". [`Seed`] is that hierarchy, made into a type.
//!
//! # Why SHA-256 and not something faster
//!
//! Rust's `DefaultHasher` is explicitly not stable across releases, and `#[derive(Hash)]`
//! output depends on it. Anything persisted in a manifest (`F-MAN-1`), used to key a store
//! directory (`F-MAN-3`), or compared between two machines through a shareable package
//! (`F-MAN-11`) needs a hash whose definition is fixed for the life of the product. SHA-256
//! is fixed, universally specified, and testable against published vectors.
//!
//! The cost is irrelevant at the scale it runs: one hash per path during Grow, on the rare
//! and expensive side of `P10`, never in the continuous loop.
//!
//! # Why the seed tree is derived from labels, not drawn from a parent stream
//!
//! It would be simpler to give the RNG a `split()` and let each subtree take its seed from
//! its parent's stream. That would make every seed a function of *traversal order* — so
//! adding one file at the top of a repository would re-seed everything after it, and the
//! whole tree would change shape for a one-line commit.
//!
//! [`Seed::derive`] takes a label instead. A path's seed depends on its path and nothing
//! else. This is what makes `AC-GROW-4` ("adding one file produces a change localized to
//! the affected limb") a property of the design rather than a hope.

use core::fmt;

use crate::rng::ChaCha8Rng;

/// Domain tag mixed into every root seed, so that seeds derived for different purposes can
/// never collide even given identical labels.
const SEED_DOMAIN: &[u8] = b"treepo/seed/v1";

/// Separator byte for label derivation.
const TAG_LABEL: u8 = 0x01;

/// Separator byte for index derivation.
const TAG_INDEX: u8 = 0x02;

/// SHA-256 round constants: the first 32 bits of the fractional parts of the cube roots of
/// the first 64 primes.
#[rustfmt::skip]
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// SHA-256 initial state: the first 32 bits of the fractional parts of the square roots of
/// the first 8 primes.
const H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// A 256-bit hash result.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Digest([u8; 32]);

impl Digest {
    /// The raw bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Consumes the digest, returning its bytes.
    #[must_use]
    pub const fn into_bytes(self) -> [u8; 32] {
        self.0
    }

    /// The first 16 bytes — the width `AuthorKey` uses (architecture, Data Model).
    #[must_use]
    pub const fn truncated_16(&self) -> [u8; 16] {
        let mut out = [0u8; 16];
        let mut i = 0;
        while i < 16 {
            out[i] = self.0[i];
            i += 1;
        }
        out
    }

    /// The leading 64 bits, big-endian — the width `PathRecord::seed` uses.
    #[must_use]
    pub const fn to_u64(&self) -> u64 {
        u64::from_be_bytes([
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5], self.0[6], self.0[7],
        ])
    }
}

impl fmt::Display for Digest {
    /// Lowercase hex, 64 characters.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Digest({self})")
    }
}

/// Incremental SHA-256.
#[derive(Clone)]
pub struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffered: usize,
    total_bytes: u64,
}

impl Sha256 {
    /// A fresh hasher.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: H0,
            buffer: [0; 64],
            buffered: 0,
            total_bytes: 0,
        }
    }

    /// Hashes `input` in one call.
    #[must_use]
    pub fn digest(input: &[u8]) -> Digest {
        let mut hasher = Self::new();
        hasher.update(input);
        hasher.finalize()
    }

    /// Appends more input.
    pub fn update(&mut self, mut input: &[u8]) {
        self.total_bytes = self.total_bytes.wrapping_add(input.len() as u64);

        if self.buffered > 0 {
            let take = (64 - self.buffered).min(input.len());
            self.buffer[self.buffered..self.buffered + take].copy_from_slice(&input[..take]);
            self.buffered += take;
            input = &input[take..];
            if self.buffered == 64 {
                let block = self.buffer;
                compress(&mut self.state, &block);
                self.buffered = 0;
            }
        }

        while input.len() >= 64 {
            let mut block = [0u8; 64];
            block.copy_from_slice(&input[..64]);
            compress(&mut self.state, &block);
            input = &input[64..];
        }

        if !input.is_empty() {
            self.buffer[..input.len()].copy_from_slice(input);
            self.buffered = input.len();
        }
    }

    /// Finishes and returns the digest.
    #[must_use]
    pub fn finalize(mut self) -> Digest {
        let bit_length = self.total_bytes.wrapping_mul(8);

        // 0x80, then zeros, then the 64-bit big-endian length in the last 8 bytes.
        self.update(&[0x80]);
        while self.buffered != 56 {
            self.update(&[0x00]);
        }
        let block = {
            let mut block = self.buffer;
            block[56..].copy_from_slice(&bit_length.to_be_bytes());
            block
        };
        compress(&mut self.state, &block);

        let mut out = [0u8; 32];
        for (chunk, word) in out.chunks_exact_mut(4).zip(self.state) {
            chunk.copy_from_slice(&word.to_be_bytes());
        }
        Digest(out)
    }
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Sha256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sha256")
            .field("bytes_hashed", &self.total_bytes)
            .finish_non_exhaustive()
    }
}

/// The SHA-256 block function.
fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    for (i, chunk) in block.chunks_exact(4).enumerate() {
        w[i] = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
    }

    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;

    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let choose = (e & f) ^ (!e & g);
        let t1 = h
            .wrapping_add(s1)
            .wrapping_add(choose)
            .wrapping_add(K[i])
            .wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let majority = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(majority);

        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }

    for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *slot = slot.wrapping_add(value);
    }
}

/// Hashes `input` to a `u64` — a stable replacement for `#[derive(Hash)]` wherever the
/// value is persisted or compared across machines.
#[must_use]
pub fn stable_hash_u64(input: &[u8]) -> u64 {
    Sha256::digest(input).to_u64()
}

/// A node in the generation seed tree.
///
/// A seed is a function of the labels used to reach it, and of nothing else — not of the
/// order things were generated in, not of how many siblings came first, and certainly not
/// of the clock. See the [module docs](self) for why that matters more than it looks like
/// it should.
///
/// ```
/// use treepo_det::Seed;
///
/// let repo = Seed::root(b"repository-identity");
/// let src = repo.derive(b"src");
/// let main = src.derive(b"src/main.rs");
///
/// // Reaching the same label the same way gives the same seed, always.
/// assert_eq!(main, repo.derive(b"src").derive(b"src/main.rs"));
/// // Different labels diverge completely.
/// assert_ne!(main, src.derive(b"src/lib.rs"));
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Seed([u8; 32]);

impl Seed {
    /// The root of a seed tree, domain-separated from every other use of this function.
    #[must_use]
    pub fn root(domain: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(SEED_DOMAIN);
        hasher.update(&(domain.len() as u64).to_le_bytes());
        hasher.update(domain);
        Self(hasher.finalize().into_bytes())
    }

    /// Derives a child seed for a named subtree — typically a repository path.
    ///
    /// The label is length-prefixed, so `derive(b"ab").derive(b"c")` and
    /// `derive(b"a").derive(b"bc")` are different seeds.
    #[must_use]
    pub fn derive(&self, label: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(&self.0);
        hasher.update(&[TAG_LABEL]);
        hasher.update(&(label.len() as u64).to_le_bytes());
        hasher.update(label);
        Self(hasher.finalize().into_bytes())
    }

    /// Derives a child seed for a numbered element — the nth branch of a limb, the nth
    /// particle of an emitter.
    #[must_use]
    pub fn derive_index(&self, index: u64) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(&self.0);
        hasher.update(&[TAG_INDEX]);
        hasher.update(&index.to_le_bytes());
        Self(hasher.finalize().into_bytes())
    }

    /// Opens the random stream for this seed.
    #[must_use]
    pub const fn rng(&self) -> ChaCha8Rng {
        ChaCha8Rng::from_seed(self.0)
    }

    /// Reinterprets raw bytes as a seed.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// The raw bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// The leading 64 bits, for storage in a `PathRecord`.
    #[must_use]
    pub const fn to_u64(&self) -> u64 {
        u64::from_be_bytes([
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5], self.0[6], self.0[7],
        ])
    }
}

impl fmt::Debug for Seed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Enough to identify a seed in a log without pretending it is human-meaningful.
        write!(
            f,
            "Seed({:02x}{:02x}{:02x}{:02x}…)",
            self.0[0], self.0[1], self.0[2], self.0[3]
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;

    /// Published SHA-256 vectors. If a transcription slip crept into `K` or `H0`, these
    /// fail immediately rather than producing a plausible wrong hash forever.
    #[test]
    fn matches_published_vectors() {
        assert_eq!(
            Sha256::digest(b"").to_string(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            Sha256::digest(b"abc").to_string(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            Sha256::digest(b"treepo").to_string(),
            "bafcd3dbc62ea0e681f431cdd89160e3c89733e345d7900a0f010734878e9e52"
        );
        assert_eq!(
            Sha256::digest(&vec![b'a'; 1000]).to_string(),
            "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3"
        );
    }

    /// Every split of the same input must produce the same digest — the buffering logic is
    /// the easiest part of a hash to get subtly wrong.
    #[test]
    fn incremental_update_matches_one_shot() {
        let input: alloc::vec::Vec<u8> = (0..500u32).map(|i| (i * 7) as u8).collect();
        let expected = Sha256::digest(&input);
        for chunk in [1usize, 3, 55, 63, 64, 65, 127, 128, 200] {
            let mut hasher = Sha256::new();
            for part in input.chunks(chunk) {
                hasher.update(part);
            }
            assert_eq!(hasher.finalize(), expected, "chunk size {chunk}");
        }
    }

    #[test]
    fn digest_projections() {
        let digest = Sha256::digest(b"abc");
        assert_eq!(digest.to_u64(), 0xba7816bf8f01cfea);
        assert_eq!(&digest.truncated_16()[..4], &[0xba, 0x78, 0x16, 0xbf]);
    }

    #[test]
    fn seeds_depend_on_the_path_and_nothing_else() {
        let root = Seed::root(b"repo");
        assert_eq!(root.derive(b"src"), Seed::root(b"repo").derive(b"src"));
        assert_ne!(root.derive(b"src"), root.derive(b"docs"));
        assert_ne!(root.derive(b"src"), Seed::root(b"other").derive(b"src"));

        // Order of derivation elsewhere in the tree cannot affect this seed.
        let a = root.derive(b"src");
        let _noise = root.derive(b"a").derive(b"b").derive_index(7);
        assert_eq!(a, root.derive(b"src"));
    }

    #[test]
    fn label_lengths_are_unambiguous() {
        let root = Seed::root(b"repo");
        assert_ne!(
            root.derive(b"ab").derive(b"c"),
            root.derive(b"a").derive(b"bc")
        );
        assert_ne!(root.derive(b""), root.derive_index(0));
    }

    #[test]
    fn indexed_derivation_is_distinct_per_index() {
        let root = Seed::root(b"limb");
        let mut seen = alloc::collections::BTreeSet::new();
        for i in 0..256 {
            assert!(seen.insert(root.derive_index(i)), "collision at {i}");
        }
    }
}
