//! Fixed-point scalar and angle types.
//!
//! Architecture D2 confines fixed-point to "angles and turtle state, where the trig problem
//! actually lives" — deliberately not to every calculation in the system, because
//! conversion noise everywhere costs more than it buys.
//!
//! Two types live here:
//!
//! * [`Fx`] — a Q32.32 signed scalar. Roughly ±2.1 billion with a resolution of 2⁻³²
//!   (about 2.3 × 10⁻¹⁰), which is finer than an `f32` anywhere above 1.0 and, unlike an
//!   `f32`, has the same resolution at every magnitude.
//! * [`Angle`] — a binary angle. A full turn is exactly 2³², so angle arithmetic wraps at
//!   a full turn for free and can never drift out of range no matter how many turtle
//!   rotations accumulate.
//!
//! # Why saturating, not wrapping or checked
//!
//! Rust's `+` panics on overflow in debug builds and wraps in release builds. For a type
//! whose entire job is producing identical results everywhere, a value that differs
//! between `cargo test` and `cargo build --release` is precisely the bug class this crate
//! exists to eliminate. Every operation here saturates instead: same answer in both
//! profiles, and a clamped extreme rather than a sign flip when a repository is pathological
//! enough to overflow a Q32.32.
//!
//! Division is the one exception. It panics on a zero divisor, exactly as integer division
//! does, because a zero divisor is a bug in the caller rather than an extreme input.
//! [`Fx::checked_div`] is there for callers who expect one.

use core::fmt;
use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// Number of fractional bits in [`Fx`].
pub const FRAC_BITS: u32 = 32;

/// Fractional bits of the Q62 mantissa [`Fx::log2_u64`] squares.
///
/// Sixty-two rather than sixty-four so that a mantissa in `[1, 2)` and its square in
/// `[1, 4)` both fit a `u64`'s worth of magnitude with the squaring done in a `u128`. It is
/// unrelated to [`FRAC_BITS`] and deliberately larger: the extra bits are headroom against
/// the truncation compounding over thirty-two iterations.
const MANTISSA_BITS: u32 = 62;

/// Clamp a 128-bit intermediate back into the representable range.
const fn sat(v: i128) -> i64 {
    if v > i64::MAX as i128 {
        i64::MAX
    } else if v < i64::MIN as i128 {
        i64::MIN
    } else {
        v as i64
    }
}

/// A Q32.32 fixed-point scalar.
///
/// All arithmetic is integer arithmetic, so results are identical on every platform and in
/// every build profile. See the [module docs](self) for the saturation policy.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Fx(i64);

impl Fx {
    /// Zero.
    pub const ZERO: Self = Self(0);
    /// One.
    pub const ONE: Self = Self(1 << FRAC_BITS);
    /// One half.
    pub const HALF: Self = Self(1 << (FRAC_BITS - 1));
    /// Negative one.
    pub const NEG_ONE: Self = Self(-(1 << FRAC_BITS));
    /// The smallest representable positive value, 2⁻³².
    pub const EPSILON: Self = Self(1);
    /// The largest representable value.
    pub const MAX: Self = Self(i64::MAX);
    /// The smallest representable value.
    pub const MIN: Self = Self(i64::MIN);

    /// π.
    pub const PI: Self = Self(13_493_037_705);
    /// 2π — a full turn in radians.
    pub const TAU: Self = Self(26_986_075_409);

    /// Reinterprets a raw Q32.32 bit pattern.
    #[must_use]
    pub const fn from_bits(bits: i64) -> Self {
        Self(bits)
    }

    /// Returns the raw Q32.32 bit pattern.
    ///
    /// This is the canonical serialization of an `Fx`: hashing or persisting these bits is
    /// exactly what makes `AC-DET-1` checkable.
    #[must_use]
    pub const fn to_bits(self) -> i64 {
        self.0
    }

    /// Converts from a whole number.
    #[must_use]
    pub const fn from_int(v: i32) -> Self {
        Self((v as i64) << FRAC_BITS)
    }

    /// Converts from the exact rational `num / den`, truncating toward zero.
    ///
    /// This is the precise way to write a constant: `Fx::from_ratio(1, 3)` rather than a
    /// decimal approximation routed through a float.
    ///
    /// # Panics
    ///
    /// If `den` is zero.
    #[must_use]
    pub const fn from_ratio(num: i64, den: i64) -> Self {
        assert!(den != 0, "Fx::from_ratio: zero denominator");
        Self(sat(((num as i128) << FRAC_BITS) / den as i128))
    }

    /// Converts from an `f64`.
    ///
    /// The conversion itself is deterministic — IEEE-754 defines it exactly — so this is
    /// the correct way to bring a parameter-file value across the boundary. What is *not*
    /// permitted is float arithmetic on the far side: convert once, then compute in `Fx`.
    #[must_use]
    // One of exactly two places in this crate where a float is permitted to appear. The
    // multiply is IEEE-exact for every value with fewer than 53 significant bits, and the
    // saturating `as` cast is defined behaviour that clamps at the extremes rather than
    // producing an unspecified value.
    #[allow(clippy::float_arithmetic)]
    pub fn from_f64(v: f64) -> Self {
        Self((v * 4_294_967_296.0) as i64)
    }

    /// Converts to an `f64`, exactly. Every `Fx` is representable — `f64` has 53 bits of
    /// mantissa and `Fx` carries 64 bits of which at most 53 are ever significant in
    /// practice, so this is lossless for any value the pipeline produces and lossy only at
    /// extremes it never reaches.
    ///
    /// Intended for rendering and diagnostics — the far side of the determinism boundary
    /// (architecture D6). Nothing that flows back into generation may come from here.
    #[must_use]
    // The second and last permitted float. Dividing by a power of two is exact in IEEE-754
    // — it adjusts the exponent and touches no mantissa bit — so this cannot round
    // differently anywhere.
    #[allow(clippy::float_arithmetic)]
    pub fn to_f64(self) -> f64 {
        self.0 as f64 / 4_294_967_296.0
    }

    /// The greatest whole number less than or equal to this value.
    #[must_use]
    pub const fn floor(self) -> i64 {
        self.0 >> FRAC_BITS
    }

    /// The least whole number greater than or equal to this value.
    #[must_use]
    pub const fn ceil(self) -> i64 {
        self.0.saturating_add((1 << FRAC_BITS) - 1) >> FRAC_BITS
    }

    /// The nearest whole number, halves rounding toward positive infinity.
    #[must_use]
    pub const fn round(self) -> i64 {
        self.0.saturating_add(1 << (FRAC_BITS - 1)) >> FRAC_BITS
    }

    /// The fractional part, always in `[0, 1)` — consistent with [`Fx::floor`].
    #[must_use]
    pub const fn fract(self) -> Self {
        Self(self.0 & 0xFFFF_FFFF)
    }

    /// Absolute value, saturating at [`Fx::MAX`].
    #[must_use]
    pub const fn abs(self) -> Self {
        Self(self.0.saturating_abs())
    }

    /// `-1`, `0`, or `1` as an `Fx`.
    #[must_use]
    pub const fn signum(self) -> Self {
        if self.0 > 0 {
            Self::ONE
        } else if self.0 < 0 {
            Self::NEG_ONE
        } else {
            Self::ZERO
        }
    }

    /// The lesser of two values.
    #[must_use]
    pub const fn min(self, other: Self) -> Self {
        if self.0 < other.0 { self } else { other }
    }

    /// The greater of two values.
    #[must_use]
    pub const fn max(self, other: Self) -> Self {
        if self.0 > other.0 { self } else { other }
    }

    /// Constrains a value to `[lo, hi]`.
    ///
    /// # Panics
    ///
    /// If `lo > hi`.
    #[must_use]
    pub const fn clamp(self, lo: Self, hi: Self) -> Self {
        assert!(lo.0 <= hi.0, "Fx::clamp: inverted bounds");
        self.max(lo).min(hi)
    }

    /// Saturating addition.
    #[must_use]
    pub const fn add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }

    /// Saturating subtraction.
    #[must_use]
    pub const fn sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }

    /// Saturating multiplication, rounding halves toward positive infinity.
    #[must_use]
    pub const fn mul(self, rhs: Self) -> Self {
        let product = (self.0 as i128) * (rhs.0 as i128);
        Self(sat((product + (1 << (FRAC_BITS - 1))) >> FRAC_BITS))
    }

    /// Saturating division, truncating toward zero.
    ///
    /// # Panics
    ///
    /// If `rhs` is zero. Use [`Fx::checked_div`] where a zero divisor is expected.
    #[must_use]
    pub const fn div(self, rhs: Self) -> Self {
        assert!(rhs.0 != 0, "Fx::div: division by zero");
        Self(sat(((self.0 as i128) << FRAC_BITS) / rhs.0 as i128))
    }

    /// Division, or `None` if `rhs` is zero.
    #[must_use]
    pub const fn checked_div(self, rhs: Self) -> Option<Self> {
        if rhs.0 == 0 {
            None
        } else {
            Some(Self(sat(((self.0 as i128) << FRAC_BITS) / rhs.0 as i128)))
        }
    }

    /// Multiplies by the rational `num / den` without an intermediate rounding step.
    ///
    /// # Panics
    ///
    /// If `den` is zero.
    #[must_use]
    pub const fn scale(self, num: i64, den: i64) -> Self {
        assert!(den != 0, "Fx::scale: zero denominator");
        Self(sat((self.0 as i128 * num as i128) / den as i128))
    }

    /// Square root, truncating toward zero.
    ///
    /// # Panics
    ///
    /// If the value is negative. Use [`Fx::checked_sqrt`] where a negative is expected.
    #[must_use]
    pub const fn sqrt(self) -> Self {
        assert!(self.0 >= 0, "Fx::sqrt: negative value");
        // sqrt(v) where v = bits / 2^32 is isqrt(bits << 32) / 2^32.
        Self(((self.0 as u128) << FRAC_BITS).isqrt() as i64)
    }

    /// Square root, or `None` if the value is negative.
    #[must_use]
    pub const fn checked_sqrt(self) -> Option<Self> {
        if self.0 < 0 {
            None
        } else {
            Some(Self(((self.0 as u128) << FRAC_BITS).isqrt() as i64))
        }
    }

    /// Base-2 logarithm of a count, or `None` for zero.
    ///
    /// `F-MAT-3` opens with "size normalization is logarithmic", and `#![deny(clippy::
    /// float_arithmetic)]` puts `f64::log2` out of reach in every crate that would call it.
    /// This is that logarithm, computed the way [`sqrt`](Self::sqrt) is: integer arithmetic
    /// only, therefore identical on every platform (`AC-DET-2`).
    ///
    /// # Why the argument is a `u64` rather than an `Fx`
    ///
    /// What actually gets logged is a byte count, and byte counts outgrow [`Fx`]. Q32.32
    /// tops out near 2.1 × 10⁹ — a repository with more than two gigabytes of content would
    /// saturate on the way *in*, and every such repository would then normalize to the same
    /// budget. A `u64` covers sixteen exabytes and returns a result no larger than 64, which
    /// is comfortably inside the range. The narrower signature is the one with no failure
    /// mode.
    ///
    /// Zero yields `None` rather than saturating at [`MIN`](Self::MIN). An empty directory
    /// is an ordinary path, not an extreme one, and a caller that silently took the most
    /// negative representable number for it would produce a budget nobody could explain.
    /// `F-MAT-3`'s representation floor is the correct answer and the caller is where it is
    /// applied.
    ///
    /// # How it works
    ///
    /// `log2(v) = e + log2(m)` where `v = 2ᵉ · m` and `m ∈ [1, 2)`. The integer part is
    /// [`u64::ilog2`]; the fraction is read one bit at a time by repeatedly squaring the
    /// mantissa — `m² ∈ [1, 4)`, and `m² ≥ 2` is exactly the statement that the next bit of
    /// the fraction is set, after which halving returns the mantissa to `[1, 2)` for the
    /// following bit. Thirty-two iterations produce thirty-two fractional bits, which is
    /// every bit `Fx` has.
    ///
    /// Truncating rather than rounding, as [`sqrt`](Self::sqrt) does: the result is never
    /// above the true value, and never more than one ulp below it. The thirty-two
    /// truncations do not compound the way they might be expected to, because each one is
    /// applied to a mantissa carrying thirty extra bits and the bit it feeds is worth
    /// 2⁻ⁱ — `log2_stays_within_its_stated_accuracy` measures the bound rather than
    /// arguing it.
    #[must_use]
    pub const fn log2_u64(value: u64) -> Option<Self> {
        if value == 0 {
            return None;
        }

        let exponent = value.ilog2();
        // The mantissa in Q62 — `value / 2^exponent`, so exactly `[2^62, 2^63)`. Shifting
        // left before right keeps every significant bit of a value up to `u64::MAX`, and
        // `value << 62 < 2^126` stays inside a `u128`.
        let mut mantissa = ((value as u128) << MANTISSA_BITS) >> exponent;

        let mut fraction: u64 = 0;
        let mut bit: u64 = 1 << (FRAC_BITS - 1);
        while bit != 0 {
            // `mantissa < 2^63`, so the square stays below `2^126` and cannot overflow.
            mantissa = (mantissa * mantissa) >> MANTISSA_BITS;
            if mantissa >= 2 << MANTISSA_BITS {
                mantissa >>= 1;
                fraction |= bit;
            }
            bit >>= 1;
        }

        Some(Self(((exponent as i64) << FRAC_BITS) | fraction as i64))
    }

    /// Linear interpolation: `self` at `t == 0`, `other` at `t == 1`.
    ///
    /// `t` is not clamped — extrapolation is sometimes what a transition wants.
    ///
    /// Defined here rather than at each call site so that every interpolated value in the
    /// system rounds the same way. Two subtly different `lerp`s would produce two subtly
    /// different trees.
    #[must_use]
    pub const fn lerp(self, other: Self, t: Self) -> Self {
        self.add(other.sub(self).mul(t))
    }

    /// Whether the value is exactly zero.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }
}

impl Add for Fx {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::add(self, rhs)
    }
}

impl Sub for Fx {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::sub(self, rhs)
    }
}

impl Mul for Fx {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self::mul(self, rhs)
    }
}

impl Div for Fx {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        Self::div(self, rhs)
    }
}

impl Neg for Fx {
    type Output = Self;
    fn neg(self) -> Self {
        Self(self.0.saturating_neg())
    }
}

impl AddAssign for Fx {
    fn add_assign(&mut self, rhs: Self) {
        *self = Self::add(*self, rhs);
    }
}

impl SubAssign for Fx {
    fn sub_assign(&mut self, rhs: Self) {
        *self = Self::sub(*self, rhs);
    }
}

impl MulAssign for Fx {
    fn mul_assign(&mut self, rhs: Self) {
        *self = Self::mul(*self, rhs);
    }
}

impl DivAssign for Fx {
    fn div_assign(&mut self, rhs: Self) {
        *self = Self::div(*self, rhs);
    }
}

impl From<i32> for Fx {
    fn from(v: i32) -> Self {
        Self::from_int(v)
    }
}

impl fmt::Display for Fx {
    /// Six fractional digits, computed in integer arithmetic so that even the debug
    /// representation cannot vary between machines.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let negative = self.0 < 0;
        let magnitude = self.0.unsigned_abs();
        let whole = magnitude >> FRAC_BITS;
        let frac = ((magnitude & 0xFFFF_FFFF) * 1_000_000) >> FRAC_BITS;
        if negative {
            f.write_str("-")?;
        }
        write!(f, "{whole}.{frac:06}")
    }
}

impl fmt::Debug for Fx {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fx({self})")
    }
}

/// A binary angle: a full turn is exactly 2³² units.
///
/// This representation is what makes turtle rotation exact. Adding angles is `u32` wrapping
/// addition, so a heading can be rotated ten million times without normalizing, without
/// drifting, and without ever leaving `[0, 1)` turns. A degrees-or-radians representation
/// in any float or fixed-point type accumulates error on every one of those rotations and
/// needs a range-reduction step that is itself a source of divergence.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Angle(u32);

impl Angle {
    /// Zero.
    pub const ZERO: Self = Self(0);
    /// A quarter turn (90°).
    pub const QUARTER: Self = Self(0x4000_0000);
    /// A half turn (180°).
    pub const HALF: Self = Self(0x8000_0000);
    /// Three quarters of a turn (270°).
    pub const THREE_QUARTER: Self = Self(0xC000_0000);

    /// Reinterprets a raw binary-angle value.
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Returns the raw binary-angle value — the canonical serialization.
    #[must_use]
    pub const fn to_bits(self) -> u32 {
        self.0
    }

    /// Converts from whole degrees. Values outside `0..360` wrap exactly.
    #[must_use]
    pub const fn from_degrees(deg: i32) -> Self {
        Self(((deg as i64 * (1i64 << 32)) / 360) as u32)
    }

    /// Converts from thousandths of a degree. Values outside a turn wrap exactly.
    #[must_use]
    pub const fn from_millidegrees(mdeg: i32) -> Self {
        Self(((mdeg as i64 * (1i64 << 32)) / 360_000) as u32)
    }

    /// Converts from the exact fraction `num / den` of a turn.
    ///
    /// # Panics
    ///
    /// If `den` is zero.
    #[must_use]
    pub const fn from_turns_ratio(num: i64, den: i64) -> Self {
        assert!(den != 0, "Angle::from_turns_ratio: zero denominator");
        Self((((num as i128) << 32) / den as i128) as u32)
    }

    /// Converts from radians, rounding to the nearest representable angle.
    #[must_use]
    pub const fn from_radians(rad: Fx) -> Self {
        let tau = Fx::TAU.to_bits() as i128;
        let numerator = (rad.to_bits() as i128) << 32;
        // Round half away from zero, so the conversion is symmetric about zero.
        let rounded = if numerator >= 0 {
            (numerator + tau / 2) / tau
        } else {
            (numerator - tau / 2) / tau
        };
        Self(rounded as u32)
    }

    /// Converts to radians, in `[0, 2π)`, rounding to the nearest representable value.
    ///
    /// Rounds rather than truncates so that the cardinal angles land on their constants
    /// exactly: `Angle::HALF.to_radians() == Fx::PI`.
    #[must_use]
    pub const fn to_radians(self) -> Fx {
        let product = self.0 as i128 * Fx::TAU.to_bits() as i128;
        Fx::from_bits(((product + (1 << 31)) >> 32) as i64)
    }

    /// Converts to thousandths of a degree, in `[0, 360_000)`.
    #[must_use]
    pub const fn to_millidegrees(self) -> u32 {
        ((self.0 as u64 * 360_000) >> 32) as u32
    }

    /// Which quadrant the angle lies in: 0 for `[0°, 90°)` through 3 for `[270°, 360°)`.
    #[must_use]
    pub const fn quadrant(self) -> u32 {
        self.0 >> 30
    }
}

impl Add for Angle {
    type Output = Self;
    /// Wraps at a full turn, exactly.
    fn add(self, rhs: Self) -> Self {
        Self(self.0.wrapping_add(rhs.0))
    }
}

impl Sub for Angle {
    type Output = Self;
    /// Wraps at a full turn, exactly.
    fn sub(self, rhs: Self) -> Self {
        Self(self.0.wrapping_sub(rhs.0))
    }
}

impl Neg for Angle {
    type Output = Self;
    /// Wraps at a full turn, exactly.
    fn neg(self) -> Self {
        Self(self.0.wrapping_neg())
    }
}

impl AddAssign for Angle {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl SubAssign for Angle {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl fmt::Display for Angle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mdeg = self.to_millidegrees();
        write!(f, "{}.{:03}°", mdeg / 1000, mdeg % 1000)
    }
}

impl fmt::Debug for Angle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Angle({self})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_numbers_round_trip() {
        for v in [-1000, -1, 0, 1, 7, 1000, i32::MAX, i32::MIN] {
            assert_eq!(Fx::from_int(v).floor(), i64::from(v));
        }
    }

    #[test]
    fn ratios_are_exact() {
        assert_eq!(Fx::from_ratio(1, 2), Fx::HALF);
        assert_eq!(Fx::from_ratio(-1, 2), -Fx::HALF);
        assert_eq!(Fx::from_ratio(4, 2), Fx::from_int(2));
        // 1/3 is not representable; it must truncate toward zero, not round-trip.
        assert_eq!(Fx::from_ratio(1, 3).to_bits(), (1i64 << 32) / 3);
    }

    #[test]
    fn arithmetic_matches_rational_expectation() {
        let third = Fx::from_ratio(1, 3);
        assert_eq!((third * Fx::from_int(3)).round(), 1);
        assert_eq!(Fx::from_int(7) / Fx::from_int(2), Fx::from_ratio(7, 2));
        assert_eq!(Fx::from_int(-7) + Fx::from_int(7), Fx::ZERO);
        assert_eq!(Fx::from_ratio(3, 4) * Fx::from_ratio(4, 3), Fx::ONE);
    }

    #[test]
    fn saturates_instead_of_wrapping() {
        // The property that matters: identical in debug and release, and never a sign flip.
        assert_eq!(Fx::MAX + Fx::ONE, Fx::MAX);
        assert_eq!(Fx::MIN - Fx::ONE, Fx::MIN);
        assert_eq!(Fx::MAX * Fx::MAX, Fx::MAX);
        assert_eq!(Fx::MIN * Fx::MAX, Fx::MIN);
        assert_eq!(-Fx::MIN, Fx::MAX);
        assert_eq!(Fx::MIN.abs(), Fx::MAX);
    }

    #[test]
    fn rounding_helpers() {
        let v = Fx::from_ratio(7, 2); // 3.5
        assert_eq!(v.floor(), 3);
        assert_eq!(v.ceil(), 4);
        assert_eq!(v.round(), 4);
        assert_eq!(v.fract(), Fx::HALF);

        let n = Fx::from_ratio(-7, 2); // -3.5
        assert_eq!(n.floor(), -4);
        assert_eq!(n.ceil(), -3);
        assert_eq!(n.round(), -3);
        assert_eq!(n.fract(), Fx::HALF);

        assert_eq!(Fx::from_int(5).ceil(), 5);
        assert_eq!(Fx::from_int(-5).ceil(), -5);
    }

    #[test]
    fn sqrt_is_exact_on_squares() {
        for n in [0i64, 1, 4, 9, 16, 144, 1_000_000] {
            assert_eq!(Fx::from_int(n as i32).sqrt().round(), n.isqrt());
        }
        // 2^0.5 to within one part in 2^31.
        let two = Fx::from_int(2).sqrt();
        assert!((two.to_bits() - 6_074_001_000).abs() <= 1, "{two}");
        assert_eq!(Fx::NEG_ONE.checked_sqrt(), None);
    }

    #[test]
    fn log2_is_exact_on_powers_of_two() {
        for exponent in 0u32..64 {
            assert_eq!(
                Fx::log2_u64(1 << exponent),
                Some(Fx::from_int(exponent as i32)),
                "2^{exponent}"
            );
        }
    }

    /// The fraction is what makes the logarithm useful — without it every byte count between
    /// two powers of two would normalize to the same budget (`F-MAT-3`).
    ///
    /// Measured against `f64::log2` rather than against hand-computed constants, because the
    /// interesting property is the *bound* and a handful of constants cannot establish one.
    /// The reference is only ever the thing being compared to; nothing computed here reaches
    /// a generated value, which is the line architecture D2 draws.
    #[test]
    #[allow(clippy::float_arithmetic, clippy::float_cmp)]
    fn log2_stays_within_its_stated_accuracy() {
        let mut worst = 0i64;
        let mut worst_at = 0u64;

        // Powers of two, their neighbours, and a spread across the whole magnitude range —
        // byte counts from one byte to sixteen exabytes.
        let mut values: alloc::vec::Vec<u64> = (1..4_000).collect();
        for exponent in 0..64u32 {
            let power = 1u64 << exponent;
            values.extend([power, power.saturating_add(1), power.saturating_sub(1)]);
            values.push(power.saturating_add(power / 3));
        }

        for value in values.into_iter().filter(|&v| v > 0) {
            let got = Fx::log2_u64(value).unwrap().to_bits();
            let want = ((value as f64).log2() * 4_294_967_296.0) as i64;
            let error = got - want;
            // Truncating, so it may sit below the true value but never above it.
            assert!(error <= 1, "log2({value}) overshot by {error}");
            if (-error) > worst {
                worst = -error;
                worst_at = value;
            }
        }

        // The bound the doc comment states: one ulp of Q32.32, which is as close as the
        // `f64` reference can even be asked about. A regression here means the mantissa lost
        // headroom, not that a platform disagreed — that half is `cargo xtask determinism`.
        assert!(
            worst <= 1,
            "worst deviation {worst} ulps, at log2({worst_at})"
        );
    }

    /// Truncating, never overshooting: a budget derived from this must not exceed the one a
    /// real logarithm would give, or the soft clamp above it is reasoning about a value that
    /// cannot occur.
    #[test]
    fn log2_never_overshoots_and_never_decreases() {
        let mut previous = Fx::MIN;
        for value in 1u64..2_000 {
            let got = Fx::log2_u64(value).unwrap();
            assert!(got >= previous, "log2({value}) went backwards");
            previous = got;
            // Truncated, so it sits in [floor(log2 v), floor(log2 v) + 1).
            assert!(
                got.floor() == i64::from(value.ilog2()),
                "log2({value}) = {got}"
            );
        }
    }

    /// An empty path is an ordinary path. `F-MAT-3`'s floor is the answer, and it is applied
    /// by the caller rather than guessed at here.
    #[test]
    fn log2_of_nothing_is_absent_not_saturated() {
        assert_eq!(Fx::log2_u64(0), None);
    }

    #[test]
    fn lerp_hits_its_endpoints() {
        let a = Fx::from_int(10);
        let b = Fx::from_int(20);
        assert_eq!(a.lerp(b, Fx::ZERO), a);
        assert_eq!(a.lerp(b, Fx::ONE), b);
        assert_eq!(a.lerp(b, Fx::HALF), Fx::from_int(15));
    }

    #[test]
    #[should_panic(expected = "division by zero")]
    fn division_by_zero_panics() {
        let _ = Fx::ONE / Fx::ZERO;
    }

    #[test]
    fn display_is_readable() {
        assert_eq!(Fx::from_ratio(7, 2).to_string(), "3.500000");
        assert_eq!(Fx::from_ratio(-7, 2).to_string(), "-3.500000");
        assert_eq!(Fx::ZERO.to_string(), "0.000000");
    }

    #[test]
    fn angles_wrap_at_a_full_turn() {
        assert_eq!(Angle::from_degrees(360), Angle::ZERO);
        assert_eq!(Angle::from_degrees(90), Angle::QUARTER);
        assert_eq!(Angle::from_degrees(-90), Angle::THREE_QUARTER);
        assert_eq!(Angle::from_degrees(450), Angle::QUARTER);
        assert_eq!(Angle::QUARTER + Angle::THREE_QUARTER, Angle::ZERO);
        assert_eq!(-Angle::QUARTER, Angle::THREE_QUARTER);
    }

    #[test]
    fn angle_rotation_never_drifts() {
        // The property a float heading cannot offer: ten million rotations of an exact
        // fraction of a turn land exactly back where they started, with no normalization
        // step anywhere and no accumulated error to normalize away.
        let step = Angle::from_turns_ratio(1, 512);
        let start = Angle::from_degrees(37);

        let mut a = start;
        for _ in 0..512 * 19_531 {
            a += step;
        }
        assert_eq!(a, start, "whole number of turns");

        // A partial turn lands exactly on its arithmetic result, not near it.
        for _ in 0..128 {
            a += step;
        }
        assert_eq!(a, start + Angle::from_degrees(90));
    }

    #[test]
    fn angle_conversions() {
        assert_eq!(Angle::from_millidegrees(90_000), Angle::QUARTER);
        assert_eq!(Angle::QUARTER.to_millidegrees(), 90_000);
        assert_eq!(Angle::THREE_QUARTER.to_millidegrees(), 270_000);
        assert_eq!(Angle::from_turns_ratio(1, 4), Angle::QUARTER);
        assert_eq!(Angle::from_turns_ratio(-1, 4), Angle::THREE_QUARTER);

        // Radians round-trip to within the resolution of the representation.
        for deg in [0, 1, 45, 90, 179, 270, 359] {
            let a = Angle::from_degrees(deg);
            let back = Angle::from_radians(a.to_radians());
            assert!(
                a.to_bits().abs_diff(back.to_bits()) <= 2,
                "{deg}°: {a} -> {back}"
            );
        }
        assert_eq!(Angle::HALF.to_radians().to_bits(), Fx::PI.to_bits());
    }

    #[test]
    fn quadrants() {
        assert_eq!(Angle::from_degrees(0).quadrant(), 0);
        assert_eq!(Angle::from_degrees(89).quadrant(), 0);
        assert_eq!(Angle::from_degrees(91).quadrant(), 1);
        assert_eq!(Angle::from_degrees(181).quadrant(), 2);
        assert_eq!(Angle::from_degrees(271).quadrant(), 3);
    }
}
