//! ChaCha8 — the only source of randomness in treepo.
//!
//! `N3`: "No wall-clock input, no unseeded randomness, no machine-specific state anywhere
//! in the generative pipeline."
//!
//! # What is deliberately missing
//!
//! There is no `from_entropy`, no `Default`, no `thread_rng`, and no way to construct a
//! generator except from explicit bytes. If a seed cannot be traced back to a path hash,
//! there is no API here that will produce one.
//!
//! There is also no `split()`. Deriving a child generator from a parent's *stream* would
//! make every downstream seed a function of traversal order; [`Seed::derive`] exists so
//! that it is a function of the path instead. See [`crate::hash`] for why that distinction
//! decides whether a one-line commit re-shapes half the tree.
//!
//! [`Seed::derive`]: crate::Seed::derive
//!
//! # Why ChaCha8 rather than a small PRNG
//!
//! Generation seeds *thousands* of independent streams, one per path, many of them from
//! seeds that differ in a single bit. A small-state PRNG like an LCG or xorshift correlates
//! visibly under that treatment — neighbouring paths would get neighbouring "random"
//! decisions, and the tree would show it as regularity that no repository primitive
//! explains, in direct violation of `P1`.
//!
//! ChaCha8 has no such structure, has been analysed to death, and costs nothing at the
//! rate Grow draws from it (`P10`).

use core::fmt;

use crate::fixed::Fx;

/// "expand 32-byte k", the standard ChaCha initial constants.
const CONSTANTS: [u32; 4] = [0x6170_7865, 0x3320_646e, 0x7962_2d32, 0x6b20_6574];

/// Words in a ChaCha block.
const BLOCK_WORDS: usize = 16;

/// The ChaCha quarter-round.
const fn quarter_round(state: &mut [u32; BLOCK_WORDS], a: usize, b: usize, c: usize, d: usize) {
    state[a] = state[a].wrapping_add(state[b]);
    state[d] = (state[d] ^ state[a]).rotate_left(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_left(12);
    state[a] = state[a].wrapping_add(state[b]);
    state[d] = (state[d] ^ state[a]).rotate_left(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_left(7);
}

/// The ChaCha block function over `ROUNDS` rounds.
///
/// Generic over the round count only so that the tests can drive it at 20 rounds and check
/// the published RFC 8439 vector — that vector validates the quarter-round, the round
/// pattern, the state layout, and the word ordering all at once, which is considerably
/// stronger evidence than a self-generated ChaCha8 vector alone.
const fn block<const ROUNDS: usize>(key: &[u32; 8], tail: [u32; 4]) -> [u32; BLOCK_WORDS] {
    let initial = [
        CONSTANTS[0],
        CONSTANTS[1],
        CONSTANTS[2],
        CONSTANTS[3],
        key[0],
        key[1],
        key[2],
        key[3],
        key[4],
        key[5],
        key[6],
        key[7],
        tail[0],
        tail[1],
        tail[2],
        tail[3],
    ];

    let mut state = initial;
    let mut round = 0;
    while round < ROUNDS / 2 {
        // Column round.
        quarter_round(&mut state, 0, 4, 8, 12);
        quarter_round(&mut state, 1, 5, 9, 13);
        quarter_round(&mut state, 2, 6, 10, 14);
        quarter_round(&mut state, 3, 7, 11, 15);
        // Diagonal round.
        quarter_round(&mut state, 0, 5, 10, 15);
        quarter_round(&mut state, 1, 6, 11, 12);
        quarter_round(&mut state, 2, 7, 8, 13);
        quarter_round(&mut state, 3, 4, 9, 14);
        round += 1;
    }

    let mut out = [0u32; BLOCK_WORDS];
    let mut i = 0;
    while i < BLOCK_WORDS {
        out[i] = state[i].wrapping_add(initial[i]);
        i += 1;
    }
    out
}

/// A ChaCha8 random number generator.
///
/// Construct one from a [`Seed`](crate::Seed) — `seed.rng()` — or directly from 32 bytes.
/// A generator with the same seed produces the same sequence on every platform, forever.
#[derive(Clone)]
pub struct ChaCha8Rng {
    key: [u32; 8],
    counter: u64,
    stream: u64,
    block: [u32; BLOCK_WORDS],
    /// Index of the next unconsumed word; `BLOCK_WORDS` means "refill before reading".
    index: usize,
}

impl ChaCha8Rng {
    /// Builds a generator from 32 explicit seed bytes.
    #[must_use]
    pub const fn from_seed(seed: [u8; 32]) -> Self {
        let mut key = [0u32; 8];
        let mut i = 0;
        while i < 8 {
            key[i] = u32::from_le_bytes([
                seed[i * 4],
                seed[i * 4 + 1],
                seed[i * 4 + 2],
                seed[i * 4 + 3],
            ]);
            i += 1;
        }
        Self {
            key,
            counter: 0,
            stream: 0,
            block: [0; BLOCK_WORDS],
            index: BLOCK_WORDS,
        }
    }

    /// Builds a generator from a `u64`.
    ///
    /// Convenient for tests and for `PathRecord::seed`. Prefer a full
    /// [`Seed`](crate::Seed) where one is available — low-entropy seeds are perfectly safe
    /// with ChaCha, but a `u64` cannot carry the domain separation that a seed tree does.
    #[must_use]
    pub const fn from_u64(seed: u64) -> Self {
        let bytes = seed.to_le_bytes();
        Self::from_seed([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ])
    }

    /// Selects an independent stream from the same seed, rewinding to its start.
    ///
    /// Two streams of one seed are as independent as two different seeds. This is how
    /// parallel work stays deterministic without threading a counter through it: give each
    /// worker a stream index, and the result does not depend on which worker ran first.
    #[must_use]
    pub const fn with_stream(mut self, stream: u64) -> Self {
        self.stream = stream;
        self.counter = 0;
        self.index = BLOCK_WORDS;
        self
    }

    /// The stream index this generator is drawing from.
    #[must_use]
    pub const fn stream(&self) -> u64 {
        self.stream
    }

    /// Generates the next block and rewinds the read cursor to its start.
    fn refill(&mut self) {
        let tail = [
            self.counter as u32,
            (self.counter >> 32) as u32,
            self.stream as u32,
            (self.stream >> 32) as u32,
        ];
        self.block = block::<8>(&self.key, tail);
        self.counter = self.counter.wrapping_add(1);
        self.index = 0;
    }

    /// The next 32 bits.
    pub fn next_u32(&mut self) -> u32 {
        if self.index >= BLOCK_WORDS {
            self.refill();
        }
        let word = self.block[self.index];
        self.index += 1;
        word
    }

    /// The next 64 bits.
    pub fn next_u64(&mut self) -> u64 {
        let low = u64::from(self.next_u32());
        let high = u64::from(self.next_u32());
        low | (high << 32)
    }

    /// Fills a byte slice.
    pub fn fill_bytes(&mut self, out: &mut [u8]) {
        let mut chunks = out.chunks_exact_mut(4);
        for chunk in &mut chunks {
            chunk.copy_from_slice(&self.next_u32().to_le_bytes());
        }
        let tail = chunks.into_remainder();
        if !tail.is_empty() {
            let bytes = self.next_u32().to_le_bytes();
            tail.copy_from_slice(&bytes[..tail.len()]);
        }
    }

    /// A uniform value in `0..bound`.
    ///
    /// Unbiased. The obvious `next_u32() % bound` is not: it favours the low end of the
    /// range by an amount that grows with `bound`, which would show up as a systematic lean
    /// in branch angles — a visible artefact with no primitive behind it (`P1`). This uses
    /// Lemire's multiply-shift with rejection, which is exact and takes the rejection branch
    /// vanishingly rarely.
    ///
    /// # Panics
    ///
    /// If `bound` is zero.
    pub fn below_u32(&mut self, bound: u32) -> u32 {
        assert!(bound != 0, "ChaCha8Rng::below_u32: empty range");
        let mut product = u64::from(self.next_u32()) * u64::from(bound);
        let mut low = product as u32;
        if low < bound {
            let threshold = bound.wrapping_neg() % bound;
            while low < threshold {
                product = u64::from(self.next_u32()) * u64::from(bound);
                low = product as u32;
            }
        }
        (product >> 32) as u32
    }

    /// A uniform value in `lo..hi`.
    ///
    /// # Panics
    ///
    /// If `hi <= lo`.
    pub fn range_i32(&mut self, lo: i32, hi: i32) -> i32 {
        assert!(hi > lo, "ChaCha8Rng::range_i32: empty range");
        let span = (i64::from(hi) - i64::from(lo)) as u32;
        lo.wrapping_add_unsigned(self.below_u32(span))
    }

    /// A uniform value in `[0, 1)`.
    ///
    /// Every one of the 2³² representable fractions is equally likely — this is a direct
    /// reinterpretation of 32 random bits as the fractional part of an [`Fx`], with no
    /// division and nothing to round.
    pub fn unit_fx(&mut self) -> Fx {
        Fx::from_bits(i64::from(self.next_u32()))
    }

    /// A uniform value in `[lo, hi)`.
    pub fn range_fx(&mut self, lo: Fx, hi: Fx) -> Fx {
        lo + (hi - lo) * self.unit_fx()
    }

    /// A uniform value in `[-1, 1)`.
    ///
    /// The shape most generative parameters want: a signed perturbation around a nominal
    /// value (`F-SKEL-4`'s noise term, for instance).
    pub fn signed_unit_fx(&mut self) -> Fx {
        Fx::from_bits(i64::from(self.next_u32()) * 2 - Fx::ONE.to_bits())
    }

    /// `true` with probability `numerator / denominator`.
    ///
    /// # Panics
    ///
    /// If `denominator` is zero.
    pub fn chance(&mut self, numerator: u32, denominator: u32) -> bool {
        assert!(denominator != 0, "ChaCha8Rng::chance: zero denominator");
        self.below_u32(denominator) < numerator
    }
}

impl fmt::Debug for ChaCha8Rng {
    /// Deliberately omits the key. A generator's identity is its seed, and a seed printed
    /// into a log is a seed someone will copy into code instead of deriving it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChaCha8Rng")
            .field("stream", &self.stream)
            .field("block", &self.counter)
            .field("word", &self.index)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    fn unhex(text: &str) -> Vec<u8> {
        text.as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let digit = |c: u8| (c as char).to_digit(16).expect("hex digit") as u8;
                digit(pair[0]) << 4 | digit(pair[1])
            })
            .collect()
    }

    fn words_to_bytes(words: [u32; BLOCK_WORDS]) -> Vec<u8> {
        words.iter().flat_map(|w| w.to_le_bytes()).collect()
    }

    /// RFC 8439 §2.3.2. Twenty rounds, so it does not exercise treepo's round count — it
    /// exercises everything else, against a vector nobody in this repository invented.
    #[test]
    fn core_matches_rfc8439_chacha20_block() {
        let key: [u32; 8] = core::array::from_fn(|i| {
            u32::from_le_bytes([
                (i * 4) as u8,
                (i * 4 + 1) as u8,
                (i * 4 + 2) as u8,
                (i * 4 + 3) as u8,
            ])
        });
        // counter = 1, nonce = 00:00:00:09:00:00:00:4a:00:00:00:00
        let tail = [1, 0x0900_0000, 0x4a00_0000, 0x0000_0000];
        assert_eq!(
            words_to_bytes(block::<20>(&key, tail)),
            unhex(
                "10f1e7e4d13b5915500fdd1fa32071c4c7d1f4c733c068030422aa9ac3d46c4e\
                 d2826446079faa0914c2d705d98b02a2b5129cd1de164eb9cbd083e8a2503c4e"
            )
        );
    }

    /// The generator's own output, pinned. These bytes are what `AC-DET-2` compares across
    /// platforms; a change here changes every tree.
    #[test]
    fn keystream_is_pinned() {
        let mut rng = ChaCha8Rng::from_seed([0u8; 32]);
        let mut bytes = vec![0u8; 128];
        rng.fill_bytes(&mut bytes);
        assert_eq!(
            bytes,
            unhex(
                "3e00ef2f895f40d67f5bb8e81f09a5a12c840ec3ce9a7f3b181be188ef711a1e\
                 984ce172b9216f419f445367456d5619314a42a3da86b001387bfdb80e0cfe42\
                 d2aefa0deaa5c151bf0adb6c01f2a5adc0fd581259f9a2aadcf20f8fd566a26b\
                 5032ec38bbc5da98ee0c6f568b872a65a08abf251deb21bb4b56e5d8821e68aa"
            )
        );

        let key: [u8; 32] = core::array::from_fn(|i| (i * 7 + 3) as u8);
        let mut rng = ChaCha8Rng::from_seed(key);
        let mut bytes = vec![0u8; 64];
        rng.fill_bytes(&mut bytes);
        assert_eq!(
            bytes,
            unhex(
                "0378d61ad1dd45c5509aaf18f95d308fe81ecff41efbd8c4fd3dcef33d13f689\
                 7cc9448241a94e2d4243715444e3e3eeb28c8f5e1ce326b3b6780ec35c9fd9c8"
            )
        );
    }

    #[test]
    fn word_reads_agree_with_byte_reads() {
        // `fill_bytes` must produce exactly the little-endian concatenation of `next_u32`,
        // including across the 64-byte block boundary and through the partial-chunk tail.
        let mut by_word = ChaCha8Rng::from_seed([9u8; 32]);
        let expected: Vec<u8> = (0..20)
            .flat_map(|_| by_word.next_u32().to_le_bytes())
            .collect();

        let mut by_byte = ChaCha8Rng::from_seed([9u8; 32]);
        let mut buffer = [0u8; 67];
        by_byte.fill_bytes(&mut buffer);
        assert_eq!(&buffer[..], &expected[..67]);
    }

    #[test]
    fn same_seed_same_sequence() {
        let draw = |seed: u64| {
            let mut rng = ChaCha8Rng::from_u64(seed);
            (0..64).map(|_| rng.next_u64()).collect::<Vec<_>>()
        };
        assert_eq!(draw(42), draw(42));
        assert_ne!(draw(42), draw(43));
    }

    #[test]
    fn streams_are_independent() {
        let base = ChaCha8Rng::from_u64(1);
        let draw = |stream: u64| {
            let mut rng = base.clone().with_stream(stream);
            (0..32).map(|_| rng.next_u64()).collect::<Vec<_>>()
        };
        assert_ne!(draw(0), draw(1));
        assert_eq!(draw(7), draw(7));
    }

    #[test]
    fn adjacent_seeds_do_not_correlate() {
        // The property a small-state PRNG fails: seeds differing in one bit must produce
        // unrelated streams, because path hashes of sibling directories are arbitrary.
        let first: Vec<u32> = (0..64u64)
            .map(|i| ChaCha8Rng::from_u64(i).next_u32())
            .collect();
        let mut sorted = first.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), first.len(), "duplicate first draws");

        // Top bits should be roughly balanced rather than marching in step.
        let high = first.iter().filter(|w| **w >= 1 << 31).count();
        assert!(
            (16..=48).contains(&high),
            "{high} of 64 draws had the high bit set"
        );
    }

    #[test]
    fn bounded_draws_stay_in_range_and_cover_it() {
        let mut rng = ChaCha8Rng::from_u64(7);
        for bound in [1u32, 2, 3, 7, 10, 1000, u32::MAX] {
            for _ in 0..200 {
                assert!(rng.below_u32(bound) < bound, "bound {bound}");
            }
        }

        let mut seen = [false; 6];
        for _ in 0..500 {
            seen[rng.below_u32(6) as usize] = true;
        }
        assert!(
            seen.iter().all(|hit| *hit),
            "die roll never covered all faces"
        );
    }

    #[test]
    fn bounded_draws_are_not_visibly_biased() {
        // 60_000 draws over 6 buckets: each should land near 10_000. A modulo-biased
        // generator over this bound would skew the low buckets measurably.
        let mut rng = ChaCha8Rng::from_u64(99);
        let mut counts = [0u32; 6];
        for _ in 0..60_000 {
            counts[rng.below_u32(6) as usize] += 1;
        }
        for (face, count) in counts.iter().enumerate() {
            assert!(
                (9_400..=10_600).contains(count),
                "face {face} came up {count} times"
            );
        }
    }

    #[test]
    fn ranges() {
        let mut rng = ChaCha8Rng::from_u64(3);
        for _ in 0..500 {
            let v = rng.range_i32(-10, 10);
            assert!((-10..10).contains(&v), "{v}");

            let unit = rng.unit_fx();
            assert!(unit >= Fx::ZERO && unit < Fx::ONE, "{unit}");

            let signed = rng.signed_unit_fx();
            assert!(signed >= Fx::NEG_ONE && signed < Fx::ONE, "{signed}");

            let scaled = rng.range_fx(Fx::from_int(2), Fx::from_int(5));
            assert!(
                scaled >= Fx::from_int(2) && scaled < Fx::from_int(5),
                "{scaled}"
            );
        }
    }

    #[test]
    fn chance_is_proportional() {
        let mut rng = ChaCha8Rng::from_u64(11);
        let hits = (0..10_000).filter(|_| rng.chance(1, 4)).count();
        assert!((2_300..=2_700).contains(&hits), "{hits} hits in 10000");
        assert!(!rng.chance(0, 4));
        assert!(rng.chance(4, 4));
    }
}
