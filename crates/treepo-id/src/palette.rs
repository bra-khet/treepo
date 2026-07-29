//! The colour a contributor is shown in — `F-ID-4`, `AC-MAT-4`.
//!
//! > Author colors are seeded from the identity key and drawn from a palette with enforced
//! > minimum perceptual separation so that adjacent mosaic segments remain distinguishable.
//!
//! Three words in that sentence decide the design of this module.
//!
//! # "Perceptual" — so the palette is authored in OKLCh, not in sRGB
//!
//! Separation that means anything has to be measured in a perceptually uniform space; the
//! Euclidean distance between two sRGB triples is not a statement about whether a person can
//! tell them apart. The usual conversion — linearize sRGB through a 2.4 power, an LMS
//! matrix, a cube root, another matrix — needs two transcendental functions this workspace
//! has no integer implementation of, and `N3` will not accept a float in a value that
//! reaches generated output.
//!
//! Authoring the palette in **OKLCh** removes the problem rather than solving it. An entry
//! is a lightness, a chroma and a hue, which is how a separated palette gets designed
//! anyway; the only conversion needed is LCh → Lab, and that is
//! `a = C·cos(h)`, `b = C·sin(h)` — [`treepo_det::trig`], exact and identical on every
//! platform. sRGB is then the render layer's problem (Phase 5), where the conversion runs
//! once per colour, in float, downstream of everything `AC-DET-2` covers.
//!
//! One consequence is worth stating: **nothing here checks that an entry is inside the sRGB
//! gamut.** A lightness of 0.9 with a chroma of 0.35 is a perfectly good OKLCh coordinate
//! and not a colour a monitor can show. Gamut mapping belongs where the conversion is, and
//! the built-in palette stays conservative enough not to need it.
//!
//! # "Adjacent" — which means every pair, not neighbours in the file
//!
//! An entry is chosen by hashing an [`AuthorKey`], so any two entries can end up beside each
//! other in a mosaic. Reading `AC-MAT-4`'s "adjacent" as "adjacent in the list" would test
//! a property no rendered pixel depends on. [`Palette::validate`] checks **every pair**.
//!
//! # "Seeded" — and why the seed does more than pick an entry
//!
//! A palette of eighteen entries gives eighteen colours, and a repository can have thousands
//! of contributors. Rather than grow the file, each entry is a *family*: the key picks the
//! family and then a point within a bounded neighbourhood of it, so two contributors sharing
//! a family still differ. The bound is what keeps the guarantee — [`Jitter`] states the
//! largest displacement it can produce, and validation requires every pair of entries to be
//! separated by the threshold **plus both neighbourhood radii**, so two colours from
//! different families are separated no matter which points inside them were drawn.
//!
//! Without that arithmetic the jitter would quietly eat the property the file exists to
//! guarantee, and it would do it invisibly — the palette would still parse, and two colours
//! would occasionally be closer than the threshold that was supposed to be enforced.

use alloc::string::String;
use core::fmt;
use serde::Deserialize;
use treepo_det::{Angle, Fx, Seed, sin_cos};
use treepo_model::identity::AuthorKey;

/// The compiled-in palette. See `treepo-gen::params` for why `include_str!`: this crate is
/// `no_std` and has no business resolving asset paths.
const BUILT_IN_RON: &str = include_str!("../../../assets/palettes/author-palette.ron");

/// The palette format this crate understands.
///
/// Independent of [`treepo_model::SCHEMA_VERSION`] and of `treepo-gen`'s table version: a
/// colour is derived rather than stored, so a palette edit invalidates no manifest.
pub const PALETTE_VERSION: u32 = 1;

/// Domain separator for the colour seed. Changing it re-colours every contributor in every
/// repository — a deliberate act, not an edit.
const DOMAIN: &[u8] = b"treepo/author-color/v1";

/// The largest chroma an entry may declare.
///
/// Not a gamut bound — see the module docs. It is a sanity bound: OKLab chroma above about
/// 0.4 is outside anything a display reproduces at any lightness, so a larger value is a
/// units mistake (someone writing 1200 meaning "1.2") rather than a colour.
const CHROMA_CEILING: i32 = 400;

/// Per mille — the unit ratios are written in, as in `assets/params/lsystem.ron`.
const PER_MILLE: i64 = 1000;

fn per_mille(value: i32) -> Fx {
    Fx::from_ratio(i64::from(value), PER_MILLE)
}

/// A colour in OKLab — the space the separation metric is defined in.
///
/// Cartesian rather than polar, because distance is what this type exists for and distance
/// in a polar parameterization is not a subtraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Oklab {
    /// Perceptual lightness, `0..=1`.
    pub l: Fx,
    /// Green–red axis.
    pub a: Fx,
    /// Blue–yellow axis.
    pub b: Fx,
}

impl Oklab {
    /// The perceptual distance between two colours — ΔE<sub>OK</sub>.
    ///
    /// Plain Euclidean distance, which is the whole point of a uniform space: OKLab is
    /// constructed so that equal distances read as equally different, so no weighting is
    /// needed and adding one would be inventing a metric.
    #[must_use]
    pub fn separation(self, other: Self) -> Fx {
        let dl = self.l - other.l;
        let da = self.a - other.a;
        let db = self.b - other.b;
        (dl * dl + da * da + db * db).sqrt()
    }
}

/// One entry in the palette: a colour family, in OKLCh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaletteEntry {
    /// Perceptual lightness, per mille. `780` is `0.78`.
    pub lightness: i32,
    /// Chroma — distance from grey — per mille. `120` is `0.12`.
    pub chroma: i32,
    /// Hue, in millidegrees. `140000` is 140°.
    pub hue: i32,
}

impl PaletteEntry {
    /// This entry as an OKLab coordinate.
    #[must_use]
    pub fn to_oklab(self) -> Oklab {
        oklab(
            per_mille(self.lightness),
            per_mille(self.chroma),
            Angle::from_millidegrees(self.hue),
        )
    }
}

/// How far from its family's centre a contributor's colour may be drawn.
///
/// Zero in every field is legal and turns the palette back into a plain lookup table — the
/// behaviour `F-ID-4` describes literally, and a useful thing to be able to fall back to
/// while tuning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Jitter {
    /// Maximum lightness deviation either way, per mille.
    pub lightness: i32,
    /// Maximum chroma deviation either way, per mille.
    pub chroma: i32,
    /// Maximum hue deviation either way, in millidegrees.
    pub hue: i32,
}

impl Jitter {
    /// The radius, in OKLab, of the neighbourhood this jitter can reach around an entry of
    /// the given chroma.
    ///
    /// An **upper** bound, and deliberately a loose one: it is what validation subtracts
    /// from every pairwise distance, so erring high costs a slightly stricter palette and
    /// erring low would cost the guarantee.
    ///
    /// The chroma-and-hue term is `ΔC + (C + ΔC)·Δh`, the arc bound — a chord is never
    /// longer than the arc it subtends, so this covers the worst case of the two
    /// perturbations combining.
    fn radius(self, chroma: Fx) -> Fx {
        let dl = per_mille(self.lightness);
        let dc = per_mille(self.chroma);
        let dh = Angle::from_millidegrees(self.hue).to_radians();
        let planar = dc + (chroma + dc) * dh;
        (dl * dl + planar * planar).sqrt()
    }
}

/// The contributor colour palette — `F-ID-4`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Palette {
    /// The format version this file was written for.
    pub version: u32,
    /// The minimum ΔE<sub>OK</sub> any two contributors' colours must differ by, per mille.
    ///
    /// This is `AC-MAT-4`'s threshold. It is in the file rather than a constant because it
    /// is a legibility judgement about mosaic cells at a given size, which is exactly the
    /// kind of thing a later phase measures and revises.
    pub min_separation: i32,
    /// How far a contributor's colour may be drawn from its family's centre.
    pub jitter: Jitter,
    /// The families themselves.
    pub entries: alloc::vec::Vec<PaletteEntry>,
}

impl Palette {
    /// The compiled-in palette.
    ///
    /// # Panics
    ///
    /// If the compiled-in palette is malformed, which a unit test in this module rules out.
    #[must_use]
    pub fn built_in() -> Self {
        Self::from_ron(BUILT_IN_RON).expect("built-in palette must parse and validate")
    }

    /// Parses and validates a palette from RON.
    ///
    /// # Errors
    ///
    /// [`PaletteError::Parse`] if the text is not a well-formed palette, or one of the
    /// validation variants if it is well-formed but does not guarantee what `AC-MAT-4`
    /// requires of it.
    pub fn from_ron(text: &str) -> Result<Self, PaletteError> {
        let palette: Self = ron::from_str(text).map_err(|error| PaletteError::Parse {
            detail: alloc::format!("{error}"),
        })?;
        palette.validate()?;
        Ok(palette)
    }

    /// Checks that this palette keeps the promise `F-ID-4` makes about it.
    ///
    /// # Errors
    ///
    /// The first rule violated, naming the entry or pair that violated it.
    pub fn validate(&self) -> Result<(), PaletteError> {
        if self.version != PALETTE_VERSION {
            return Err(PaletteError::Version {
                found: self.version,
                expected: PALETTE_VERSION,
            });
        }
        // One entry is not a palette — every contributor would share a colour and the
        // separation threshold would be vacuously satisfied, which is the worst way for a
        // guarantee to hold.
        if self.entries.len() < 2 {
            return Err(PaletteError::TooFewEntries {
                found: self.entries.len(),
            });
        }
        // A threshold of zero is `AC-MAT-4` switched off by an edit. Refused rather than
        // honoured, for the reason `treepo-gen::params::Table::validate` gives: a user
        // tuning a parameter that has stopped responding is a worse failure than a refused
        // file.
        if self.min_separation <= 0 {
            return Err(PaletteError::NoThreshold);
        }
        if self.jitter.lightness < 0 || self.jitter.chroma < 0 || self.jitter.hue < 0 {
            return Err(PaletteError::NegativeJitter);
        }

        for (index, entry) in self.entries.iter().enumerate() {
            if !(0..=1000).contains(&entry.lightness) {
                return Err(PaletteError::Entry {
                    index,
                    field: "lightness",
                    detail: "per mille, and a lightness outside 0..=1000 is not a colour",
                });
            }
            if !(0..=CHROMA_CEILING).contains(&entry.chroma) {
                return Err(PaletteError::Entry {
                    index,
                    field: "chroma",
                    detail: "per mille, and no display reproduces OKLab chroma above 0.400",
                });
            }
            if !(0..360_000).contains(&entry.hue) {
                return Err(PaletteError::Entry {
                    index,
                    field: "hue",
                    detail: "millidegrees, written once around the circle: 0..360000",
                });
            }
        }

        let threshold = per_mille(self.min_separation);
        for i in 0..self.entries.len() {
            for j in (i + 1)..self.entries.len() {
                let (a, b) = (self.entries[i], self.entries[j]);
                let distance = a.to_oklab().separation(b.to_oklab());
                let required = threshold
                    + self.jitter.radius(per_mille(a.chroma))
                    + self.jitter.radius(per_mille(b.chroma));
                if distance < required {
                    return Err(PaletteError::TooClose {
                        first: i,
                        second: j,
                        // Reported in per mille so the message is in the file's own units.
                        distance: to_per_mille(distance),
                        required: to_per_mille(required),
                    });
                }
            }
        }
        Ok(())
    }

    /// The colour this contributor is shown in.
    ///
    /// A pure function of the key and this file. Nothing about the repository, the other
    /// contributors, or the order anything was walked in enters it — which is what makes the
    /// colour stable across Grow cycles, as `F-ID-4` requires, without anything having to
    /// remember it.
    #[must_use]
    pub fn color_of(&self, key: &AuthorKey) -> AuthorColor {
        let mut rng = Seed::root(DOMAIN).derive(key.as_bytes()).rng();

        // Cast is safe: `validate` bounds the length below, and a palette long enough to
        // overflow a u32 would have failed to parse long before this.
        let family = rng.below_u32(self.entries.len() as u32);
        let entry = self.entries[family as usize];

        let lightness = (per_mille(entry.lightness)
            + rng.signed_unit_fx() * per_mille(self.jitter.lightness))
        .clamp(Fx::ZERO, Fx::ONE);
        let chroma = (per_mille(entry.chroma)
            + rng.signed_unit_fx() * per_mille(self.jitter.chroma))
        .clamp(Fx::ZERO, per_mille(CHROMA_CEILING));
        // Drawn in millidegrees rather than as a scaled `Fx`, so the hue jitter reads in the
        // same unit the file declares it in. `hi > lo` holds even for a zero jitter.
        let hue = Angle::from_millidegrees(entry.hue)
            + Angle::from_millidegrees(rng.range_i32(-self.jitter.hue, self.jitter.hue + 1));

        AuthorColor {
            family: u16::try_from(family).unwrap_or(u16::MAX),
            lightness,
            chroma,
            hue,
        }
    }

    /// The closest pair in this palette, and how far apart they are — for tests and for the
    /// determinism report.
    ///
    /// Returns `None` for a palette with fewer than two entries, which [`validate`] refuses
    /// anyway.
    ///
    /// [`validate`]: Self::validate
    #[must_use]
    pub fn tightest_pair(&self) -> Option<(usize, usize, Fx)> {
        let mut tightest: Option<(usize, usize, Fx)> = None;
        for i in 0..self.entries.len() {
            for j in (i + 1)..self.entries.len() {
                let distance = self.entries[i]
                    .to_oklab()
                    .separation(self.entries[j].to_oklab());
                if tightest.is_none_or(|(_, _, best)| distance < best) {
                    tightest = Some((i, j, distance));
                }
            }
        }
        tightest
    }
}

/// One contributor's colour: which family, and where inside it.
///
/// Carried in OKLCh rather than OKLab because that is the form the palette is written in and
/// the form a person reasons about — "the mid band, warmer than its centre". [`to_oklab`]
/// converts for anything that needs to measure.
///
/// [`to_oklab`]: Self::to_oklab
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthorColor {
    family: u16,
    lightness: Fx,
    chroma: Fx,
    hue: Angle,
}

impl AuthorColor {
    /// Which palette entry this colour belongs to.
    ///
    /// Two contributors sharing a family are near-neighbours in colour and are meant to
    /// read as related, which is a thing a renderer may legitimately want to know.
    #[must_use]
    pub const fn family(&self) -> u16 {
        self.family
    }

    /// Perceptual lightness, `0..=1`.
    #[must_use]
    pub const fn lightness(&self) -> Fx {
        self.lightness
    }

    /// Chroma — distance from grey.
    #[must_use]
    pub const fn chroma(&self) -> Fx {
        self.chroma
    }

    /// Hue.
    #[must_use]
    pub const fn hue(&self) -> Angle {
        self.hue
    }

    /// This colour in OKLab, for measuring against another.
    #[must_use]
    pub fn to_oklab(&self) -> Oklab {
        oklab(self.lightness, self.chroma, self.hue)
    }
}

/// LCh → Lab. The one conversion this module performs, and the reason the palette is
/// authored in polar form at all.
fn oklab(lightness: Fx, chroma: Fx, hue: Angle) -> Oklab {
    let (sin, cos) = sin_cos(hue);
    Oklab {
        l: lightness,
        a: chroma * cos,
        b: chroma * sin,
    }
}

/// For error messages only — the file's units, so a report can be read against the file.
fn to_per_mille(value: Fx) -> i32 {
    i32::try_from((value * Fx::from_int(PER_MILLE as i32)).round()).unwrap_or(i32::MAX)
}

/// Why a palette was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteError {
    /// The text is not a well-formed palette.
    Parse {
        /// The RON parser's message, including its position.
        detail: String,
    },
    /// The palette was written for a different format version.
    Version {
        /// The version the file declares.
        found: u32,
        /// The version this build understands.
        expected: u32,
    },
    /// Fewer than two entries — nothing for the threshold to separate.
    TooFewEntries {
        /// How many the file declares.
        found: usize,
    },
    /// `min_separation` is zero or negative, which is `AC-MAT-4` disabled.
    NoThreshold,
    /// A jitter amount is negative, which is not a distance.
    NegativeJitter,
    /// An entry is not a colour.
    Entry {
        /// Its position in `entries`.
        index: usize,
        /// The field that failed.
        field: &'static str,
        /// What the field means and what it must be.
        detail: &'static str,
    },
    /// Two entries are closer than the threshold plus their jitter neighbourhoods —
    /// `AC-MAT-4` violated.
    TooClose {
        /// Position of the first entry.
        first: usize,
        /// Position of the second.
        second: usize,
        /// How far apart they are, per mille.
        distance: i32,
        /// How far apart they had to be, per mille.
        required: i32,
    },
}

impl fmt::Display for PaletteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse { detail } => write!(f, "author palette is not well-formed: {detail}"),
            Self::Version { found, expected } => write!(
                f,
                "author palette declares version {found}, this build reads version {expected}"
            ),
            Self::TooFewEntries { found } => write!(
                f,
                "author palette has {found} entries; F-ID-4 needs at least 2 to separate"
            ),
            Self::NoThreshold => write!(
                f,
                "author palette sets min_separation to zero, which switches AC-MAT-4 off"
            ),
            Self::NegativeJitter => write!(f, "author palette declares a negative jitter amount"),
            Self::Entry {
                index,
                field,
                detail,
            } => write!(f, "author palette entry {index}: {field} — {detail}"),
            Self::TooClose {
                first,
                second,
                distance,
                required,
            } => write!(
                f,
                "author palette entries {first} and {second} are {distance} apart in OKLab \
                 and must be at least {required} (AC-MAT-4: min_separation plus both jitter \
                 radii)"
            ),
        }
    }
}

impl core::error::Error for PaletteError {}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    /// Distinct keys from distinct numbers — see `pseudonym::tests::author` for why this is
    /// spelled as an address rather than as raw bytes.
    fn author(n: u8) -> AuthorKey {
        AuthorKey::from_email(alloc::format!("contributor-{n}@example.invalid").as_bytes())
    }

    #[test]
    fn the_built_in_palette_parses_and_validates() {
        let palette = Palette::built_in();
        assert_eq!(palette.version, PALETTE_VERSION);
        assert!(palette.entries.len() >= 2);
    }

    /// `AC-MAT-4`, stated as the criterion states it: **every** pair of colours two
    /// contributors can be given is at least the threshold apart.
    ///
    /// This is the palette-level half, over the family centres plus their neighbourhoods.
    /// `colors_from_different_families_stay_separated` is the drawn-colour half.
    #[test]
    fn every_pair_of_entries_meets_the_separation_threshold() {
        let palette = Palette::built_in();
        let threshold = per_mille(palette.min_separation);
        let (first, second, tightest) = palette.tightest_pair().expect("at least two entries");
        assert!(
            tightest >= threshold,
            "entries {first} and {second} are {tightest:?}, threshold {threshold:?}"
        );

        // The headroom figure the file's own comment quotes, so an edit that eats it has to
        // change the comment too rather than leaving a stale number behind. Both are in per
        // mille, the file's units.
        let required = threshold
            + palette
                .jitter
                .radius(per_mille(palette.entries[first].chroma))
            + palette
                .jitter
                .radius(per_mille(palette.entries[second].chroma));
        assert_eq!(
            (to_per_mille(tightest), to_per_mille(required)),
            (110, 95),
            "tightest pair is entries {first} and {second}"
        );
    }

    /// The guarantee that survives the jitter: two contributors in *different* families are
    /// separated whatever points inside those families they were drawn at.
    ///
    /// Checked against real draws rather than against the bound, so a mistake in the bound
    /// itself would still show up here.
    #[test]
    fn colors_from_different_families_stay_separated() {
        let palette = Palette::built_in();
        let threshold = per_mille(palette.min_separation);

        // One drawn colour per family, from the first key that lands in it.
        let mut sampled: Vec<Option<AuthorColor>> = alloc::vec![None; palette.entries.len()];
        for byte in 0u8..=255 {
            let color = palette.color_of(&author(byte));
            sampled[color.family() as usize].get_or_insert(color);
        }
        let drawn: Vec<AuthorColor> = sampled.into_iter().flatten().collect();
        assert!(
            drawn.len() >= 2,
            "256 keys should reach at least two families, reached {}",
            drawn.len()
        );

        for (i, a) in drawn.iter().enumerate() {
            for b in &drawn[(i + 1)..] {
                let distance = a.to_oklab().separation(b.to_oklab());
                assert!(
                    distance >= threshold,
                    "families {} and {} drew colours {distance:?} apart, threshold {threshold:?}",
                    a.family(),
                    b.family()
                );
            }
        }
    }

    /// `F-ID-4`: "Stable across Grow cycles." The colour is a function of the key alone, so
    /// stability is the absence of any other input rather than a cache.
    #[test]
    fn a_color_is_a_function_of_the_key_alone() {
        let palette = Palette::built_in();
        let key = author(7);
        assert_eq!(palette.color_of(&key), palette.color_of(&key));
        assert_ne!(palette.color_of(&key), palette.color_of(&author(8)));
    }

    /// The jitter has to actually do something, or the palette is eighteen colours and the
    /// bookkeeping around it is dead weight.
    #[test]
    fn contributors_sharing_a_family_still_differ() {
        let palette = Palette::built_in();
        let mut seen: Option<(u16, AuthorColor)> = None;
        for byte in 0u8..=255 {
            let color = palette.color_of(&author(byte));
            match seen {
                Some((family, first)) if family == color.family() => {
                    assert_ne!(
                        first, color,
                        "two contributors in family {family} drew the identical colour"
                    );
                    return;
                }
                Some(_) => {}
                None => seen = Some((color.family(), color)),
            }
        }
        panic!("no two of 256 keys shared a family — the sample is too small to test this");
    }

    /// Everything a mosaic will do with a colour needs it to be a colour.
    #[test]
    fn drawn_colors_stay_inside_the_declared_bounds() {
        let palette = Palette::built_in();
        for byte in 0u8..=255 {
            let color = palette.color_of(&author(byte));
            assert!(color.lightness() >= Fx::ZERO && color.lightness() <= Fx::ONE);
            assert!(color.chroma() >= Fx::ZERO && color.chroma() <= per_mille(CHROMA_CEILING));
            assert!((color.family() as usize) < palette.entries.len());
        }
    }

    /// LCh → Lab, checked at the four cardinal hues where the answer is exact by hand.
    #[test]
    fn the_polar_conversion_is_the_one_everyone_expects() {
        let chroma = per_mille(200);
        let at = |mdeg| oklab(Fx::HALF, chroma, Angle::from_millidegrees(mdeg));

        // The trig table interpolates, so agreement is to within its stated bound rather
        // than exact. `3e-7` is `treepo-det::trig`'s worst case; this allows ten times it.
        let tolerance = Fx::from_ratio(3, 1_000_000);
        let close = |a: Fx, b: Fx| (a - b).abs() <= tolerance;

        assert!(close(at(0).a, chroma) && close(at(0).b, Fx::ZERO));
        assert!(close(at(90_000).a, Fx::ZERO) && close(at(90_000).b, chroma));
        assert!(close(at(180_000).a, -chroma) && close(at(180_000).b, Fx::ZERO));
        assert!(close(at(270_000).a, Fx::ZERO) && close(at(270_000).b, -chroma));
    }

    /// Each refusal, exercised by making the edit it exists to refuse.
    #[test]
    fn each_rule_refuses_the_edit_that_breaks_it() {
        let base = Palette::built_in();

        let mut wrong_version = base.clone();
        wrong_version.version = PALETTE_VERSION + 1;
        assert!(matches!(
            wrong_version.validate(),
            Err(PaletteError::Version { .. })
        ));

        let mut one_entry = base.clone();
        one_entry.entries.truncate(1);
        assert!(matches!(
            one_entry.validate(),
            Err(PaletteError::TooFewEntries { found: 1 })
        ));

        let mut no_threshold = base.clone();
        no_threshold.min_separation = 0;
        assert!(matches!(
            no_threshold.validate(),
            Err(PaletteError::NoThreshold)
        ));

        let mut negative = base.clone();
        negative.jitter.hue = -1;
        assert!(matches!(
            negative.validate(),
            Err(PaletteError::NegativeJitter)
        ));

        let mut bad_entry = base.clone();
        bad_entry.entries[0].lightness = 1400;
        assert!(matches!(
            bad_entry.validate(),
            Err(PaletteError::Entry {
                index: 0,
                field: "lightness",
                ..
            })
        ));

        // The rule the file exists for: a duplicated entry is zero apart from its original.
        let mut collided = base.clone();
        collided.entries[1] = collided.entries[0];
        assert!(matches!(
            collided.validate(),
            Err(PaletteError::TooClose {
                first: 0,
                second: 1,
                distance: 0,
                ..
            })
        ));

        // And the rule the *jitter* exists to be checked against: a jitter wide enough to
        // reach the next family is refused even though every entry is still fine.
        let mut greedy = base.clone();
        greedy.jitter.hue = 90_000;
        assert!(matches!(
            greedy.validate(),
            Err(PaletteError::TooClose { .. })
        ));
    }

    /// The jitter arithmetic, tested where it is the only thing that decides the answer.
    ///
    /// The built-in palette cannot test this: it clears the threshold by 110 against a
    /// requirement of 95, so dropping the radii from [`Palette::validate`] would leave it
    /// passing and every existing test green. This palette is built to sit in the gap —
    /// 70 apart, which is above `min_separation` and below `min_separation` plus both
    /// radii. It must be accepted with the jitter off and refused with it on.
    #[test]
    fn a_palette_legal_only_without_jitter_is_refused_with_it() {
        let two_entries = |jitter_hue: i32| Palette {
            version: PALETTE_VERSION,
            min_separation: 60,
            jitter: Jitter {
                lightness: 10,
                chroma: 8,
                hue: jitter_hue,
            },
            // Same lightness and chroma, 34° of hue apart: 2·0.120·sin(17°) ≈ 0.070.
            entries: alloc::vec![
                PaletteEntry {
                    lightness: 500,
                    chroma: 120,
                    hue: 0
                },
                PaletteEntry {
                    lightness: 500,
                    chroma: 120,
                    hue: 34_000
                },
            ],
        };

        let separation = two_entries(0)
            .tightest_pair()
            .map(|(_, _, d)| to_per_mille(d));
        assert_eq!(separation, Some(70), "the fixture is no longer in the gap");

        // Without a neighbourhood to reach out of, 70 clears the threshold of 60.
        let mut still_fits = two_entries(0);
        still_fits.jitter.lightness = 0;
        still_fits.jitter.chroma = 0;
        assert_eq!(still_fits.validate(), Ok(()));

        // With one, it does not — and this is the only test that says so.
        // 96 rather than the built-in palette's 95: these entries carry chroma 120 against
        // its tightest pair's 110, and the radius grows with the chroma the hue jitter
        // swings through.
        assert!(matches!(
            two_entries(3_000).validate(),
            Err(PaletteError::TooClose {
                distance: 70,
                required: 96,
                ..
            })
        ));
    }

    /// `deny_unknown_fields`, for the reason `treepo-gen`'s weights carry it: a misspelled
    /// key that parses is a knob the user turns and never sees respond.
    #[test]
    fn an_unknown_field_is_refused_rather_than_ignored() {
        let text = "(version: 1, min_separation: 60, jitter: (lightness: 0, chroma: 0, hue: 0, \
                    saturation: 5), entries: [(lightness: 800, chroma: 110, hue: 0)])";
        assert!(matches!(
            Palette::from_ron(text),
            Err(PaletteError::Parse { .. })
        ));
    }
}
