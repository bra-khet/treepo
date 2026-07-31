//! ★ What a material *looks* like — `F-MAT-1`…`F-MAT-6` turned into texels.
//!
//! > Primary material is driven by data type / language / binary vs text / asset class. These
//! > determine the base color family, **texture, and physical "feel"** of the pixels
//! > (wood-like, crystalline, metallic, leafy, dusty, etc.).
//!
//! `design/feature-system.md` §8.5. Until this module existed, [`bake`](crate::bake) rendered
//! that sentence as six hex values: the wiring from a measured category to a pixel was visible
//! and everything after the word "color" was not. A [`Material`](treepo_model::Material) carries
//! five things, and four of them — composition, mosaic, gradient, stress — reached the picture
//! either not at all or as a single multiply.
//!
//! # Limb space, not world space and not UV space
//!
//! Every coordinate here is measured in units of the limb's **own half-width** at the texel
//! being shaded. That one choice answers three questions at once.
//!
//! *It does not swim.* A chunk is baked once per LOD band at that band's texel density, so a
//! pattern parameterized by the texture's UVs is a different pattern in each band and the bark
//! visibly re-textures every time the zoom crosses one — during the exact gesture `AC-NAV-2`
//! measures. Limb coordinates are a property of the *tree*, so crossing a band resamples the
//! same surface more finely.
//!
//! *It has no absolute scale to get wrong.* World units are whatever the L-system happened to
//! emit, and a grain period in them would be a constant tuned against one repository's
//! silhouette. A period in half-widths is dimensionless.
//!
//! *It is physically the right variable anyway.* A thick trunk has coarse grain and a twig has
//! fine grain, because grain scales with the thing it is in. Dividing by the width gets that
//! for free rather than by a second rule.
//!
//! # One noise field, read six ways
//!
//! The cost discipline of this module: [`fbm`] is evaluated **once** per texel and grain,
//! faceting, fissures and veining are all derived from that one value. Sampling a second field
//! for each would be the natural way to write it and would multiply the per-texel cost of the
//! whole bake by the number of effects.
//!
//! Anisotropy is what makes one field enough. Sampling it at `(along / p.x, across / p.y)` with
//! `p.x` far larger than `p.y` gives streaks running along the limb — wood grain; sampling with
//! the two equal gives isotropic blotching — stone. Grain and mottle are the same operation at
//! different aspect ratios, so the six families differ in their *parameters* rather than in
//! their code path, and no family costs more to draw than any other.
//!
//! # Octaves are dropped below the texel, which is why the tree gains detail rather than noise
//!
//! An octave whose features are finer than a texel cannot be seen; sampled anyway it aliases
//! into static, and the static is drawn from a different sampling grid in every band, which is
//! the swim this module's coordinate choice was meant to remove — re-entering by another door.
//! [`octaves_for`] therefore stops at the texel size. Crossing a band adds or removes one
//! octave, so zooming in reveals a finer octave over an unchanged coarse structure. That reads
//! as detail appearing, which is what looking closer at a real surface does.
//!
//! # What this module must never do, and why the temptation is specific
//!
//! **It returns a colour. It never returns coverage.** `N7` holds because
//! [`fill`](crate::bake) writes a colour and an [`ElementId`](crate::ElementId) at the same
//! index in the same iteration, so a texel with colour and no id has nowhere to come from. A
//! surface that could make a texel transparent would break that from the inside: the id plane
//! would name an element at a texel the picture does not show, which `xtask id-coverage` counts
//! as `invisible` and `Coverage::is_clean` refuses.
//!
//! The temptation is [`StressKind::Sparse`](treepo_model::StressKind::Sparse) — "coarse, thin,
//! few-grained material" reads as an instruction to punch holes. It is drawn here as *coarser
//! and higher-contrast grain*: fewer, larger grains. That is both the safe rendering and the
//! more literal one, since the signal behind it is mass concentrated in a handful of large
//! files rather than mass that is missing.

use bevy::prelude::*;
use treepo_id::AuthorColor;
use treepo_model::MaterialFamily;

/// How many octaves of [`fbm`] the finest band may use.
///
/// Three. The first carries the family's character, the second breaks up its regularity, and
/// the third is at the edge of what a texel can hold — a fourth is under it at every band this
/// renderer produces, so it would cost a hash per texel to draw nothing.
const MAX_OCTAVES: u32 = 3;

/// The finest feature [`octaves_for`] will ask for, in texels.
///
/// Two, which is the sampling limit rather than a taste: a feature one texel across is a
/// texel-sized value with no shape, and one below that is aliasing. Sitting exactly at the
/// limit rather than comfortably above it is deliberate — the cost of the last octave is real
/// and the band above it is where the detail is.
const FINEST_FEATURE_TEXELS: f32 = 2.0;

/// The surface treatment of one material family — `F-MAT-1`, `design/feature-system.md` §8.5.
///
/// Six sets of numbers rather than six functions, so that adding a family is a row and so that
/// no family is more expensive to draw than another. The fields are read in the order
/// [`shade`] applies them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Surface {
    /// The family's colour before any of the rest of this is applied.
    pub base: LinearRgba,
    /// How strongly the noise field modulates brightness, in `0..=1`.
    pub grain: f32,
    /// Noise cells per half-width, along and across the limb — the reciprocal of the period
    /// [`Surface::new`] is written with.
    ///
    /// Stored inverted because [`shade`] needs it that way once per texel and a division there
    /// is a dozen cycles on the critical path of everything after it. Private, so the stored
    /// form and the authored one cannot disagree; [`period`](Self::period) reads it back.
    frequency: Vec2,
    /// How hard-edged the noise is, in `0..=1` — `0` leaves it smooth, `1` quantizes it into
    /// flat plates.
    ///
    /// What makes a crystalline or machined surface read as *made of facets* rather than as
    /// wood with the contrast turned down.
    pub facet: f32,
    /// How strongly growth rings band the limb, in `0..=1`.
    ///
    /// §8.3 asks for "a natural growth rings + tip vitality reading **without requiring
    /// explicit ring geometry**", which is why this modulates the age gradient rather than
    /// drawing rings of its own — see [`shade`].
    pub rings: f32,
    /// How glossy the surface is, in `0..=1`.
    ///
    /// The single cheapest cue that a limb is a solid object rather than a coloured region: it
    /// is a function of the across-coordinate alone, so it costs three operations and it is
    /// what separates matte parchment from wet resin.
    pub sheen: f32,
}

/// A noise period in half-widths, as the frequency [`Surface`] stores.
///
/// Named so the family table below still reads in the units it is authored in — a table of
/// reciprocals would be a table nobody could check against a limb.
#[must_use]
fn period(along: f32, across: f32) -> Vec2 {
    Vec2::new(1.0 / along, 1.0 / across)
}

/// The ring period along the limb, in half-widths.
///
/// Rings about as often as the limb is wide. Not a measurement — nothing in a repository says
/// how far apart growth rings are — but a proportion rather than a constant, so a trunk and a
/// twig both read as ringed at the same zoom.
const RING_PERIOD: f32 = 1.6;

/// How much of its brightness the oldest material loses — `F-MAT-4`.
///
/// Moved here from [`bake`](crate::bake) unchanged. It is no longer the whole of the age
/// reading, which is what its old doc comment called a placeholder: [`Surface::rings`] now
/// bands it, so age reads as rings that crowd toward the old end rather than as a limb someone
/// turned the brightness down on.
pub const AGE_DARKENING: f32 = 0.45;

/// How far the ring banding can push the age reading either way.
///
/// Small against [`AGE_DARKENING`], because a ring that swung further than the gradient it
/// bands would make a young limb with rings darker than an old one without — which would
/// invert `F-MAT-4`'s direction wherever the two met.
const RING_DEPTH: f32 = 0.14;

/// How dark a fissure gets — [`StressKind::Cracked`](treepo_model::StressKind::Cracked).
const CRACK_DEPTH: f32 = 0.55;

/// How wide a fissure is, in noise units.
///
/// Fissures are the *ridges* of the noise field — where its value passes through zero — so this
/// is a distance from that crossing rather than a feature size. Narrow, because `F-MAT-6` says
/// stress is subtle and because a wide one stops reading as a crack and starts reading as the
/// limb being two colours.
const CRACK_WIDTH: f32 = 0.09;

/// How far [`StressKind::Restless`](treepo_model::StressKind::Restless) jitters a texel.
///
/// §8.8's "slight visual unease". Per-texel rather than per-feature: restlessness is the one
/// stress with no shape, so it is drawn as the surface failing to settle. In a still picture
/// that is a fine speckle; Phase 8 is where it can move.
const RESTLESS_JITTER: f32 = 0.30;

/// How much [`StressKind::Sparse`](treepo_model::StressKind::Sparse) coarsens the grain.
///
/// At full intensity the period trebles and the amplitude nearly doubles: fewer, larger, harder
/// grains. Deliberately not transparency — see the module header for why that is the one thing
/// this module must not do.
const SPARSE_COARSENING: f32 = 2.0;
/// How much [`StressKind::Sparse`](treepo_model::StressKind::Sparse) hardens the grain.
const SPARSE_CONTRAST: f32 = 0.8;

/// How strongly a contributor's colour tints the material under it — `F-MAT-2`.
///
/// §8.5 makes ownership an *accent over* the primary material: "a limb whose primary material
/// is 'TypeScript wood' can still carry author-coloured veins". A tint rather than a
/// replacement is that sentence — the family's grain and rings stay visible through the accent,
/// so a mosaic cell reads as painted-on rather than as a limb made of a contributor.
///
/// It is also what keeps `AC-MAT-2` honest at this end. A 2%-share contributor holds a short
/// run, and a short run of a *tint* is still a visible band; a short run of a colour swap would
/// be too, but the limb would have stopped being made of anything.
///
/// **Set by looking at the T3 pin, not by argument.** The first value was 0.55, which passes
/// every test in this module — the accent shows, the material still varies under it — and is
/// plainly wrong on screen: the tree reads as candy-striped contributor colours with a material
/// somewhere underneath, which is §8.5's sentence backwards. At 0.28 the family reads first and
/// ownership reads as dressing on it, which is the order the design asks for. Nothing but a
/// screenshot could have caught that, which is worth remembering the next time a constant here
/// looks defensible.
const MOSAIC_ACCENT: f32 = 0.28;

/// How strongly a second material veins the first — `F-MAT-1`.
const VEIN_STRENGTH: f32 = 0.8;

/// Converts a secondary family's *share* into a threshold on the noise field.
///
/// A limb made 20% of a second material should be about 20% veined, and a threshold picked by
/// eye would not do that: [`fbm`] is a sum of octaves and its values cluster near zero, so a cut
/// at "0.6 looks about right" fires on under one texel in a hundred whatever the share says.
/// The number is the slope that puts the cut at the field's own `1 - share` quantile, measured
/// over the field rather than assumed — the field's p90 is 0.37 and its p50 is 0.00, and
/// `a_vein_claims_about_its_own_share_of_the_limb` is what holds it to that.
///
/// Not a distribution to be clever about. It is a property of `fbm`'s octave count and
/// interpolation, so changing either invalidates this constant and the test says so.
const VEIN_SPREAD: f32 = 0.82;

impl Surface {
    /// The noise period along and across the limb, in half-widths.
    ///
    /// A large `x` against a small `y` gives streaks running along the limb; equal values give
    /// isotropic blotches. This one pair is the whole difference between wood and stone.
    #[must_use]
    pub fn period(self) -> Vec2 {
        self.frequency.recip()
    }

    /// The surface treatment of one family.
    ///
    /// The colours are the six that were in [`bake`](crate::bake), unchanged, because they were
    /// chosen to be distinguishable and they still are. What has changed is that a colour is no
    /// longer all a family is — and the old doc comment's objection, that "a surface is a shader
    /// and a tile atlas, not a hex value", is answered by the five fields beside it rather than
    /// by retuning the sixth.
    ///
    /// These are still not through `AC-MAT-4`'s perceptual-separation check, which applies to
    /// the *author* palette and is a different question from whether six materials read as six
    /// materials. They stay in code rather than moving to `assets/palettes/` for the reason they
    /// always did: a file there would look like a decision, and the decision this slice makes is
    /// about texture.
    #[must_use]
    pub fn of(family: MaterialFamily) -> Self {
        match family {
            // Living wood. Long grain running the length of the limb, strong rings, and enough
            // roundness to read as a bough. The default reading, and the one the other five are
            // departures from.
            MaterialFamily::Heartwood => Self {
                base: LinearRgba::rgb(0.42, 0.26, 0.13),
                grain: 0.34,
                frequency: period(7.0, 0.30),
                facet: 0.0,
                rings: 0.85,
                sheen: 0.22,
            },
            // Dense, resource-like matter. Faceted plates at near-isotropic scale and a hard
            // highlight: `F-MAT-1`'s "resource-like material rather than living wood" is carried
            // by the *absence* of grain and rings as much as by the presence of facets.
            MaterialFamily::Ore => Self {
                base: LinearRgba::rgb(0.36, 0.38, 0.44),
                grain: 0.46,
                frequency: period(1.3, 0.9),
                facet: 0.85,
                rings: 0.0,
                sheen: 0.55,
            },
            // Uniform, tooled, machine-cut. §8.5 asks for "a slightly different, more uniform"
            // treatment, and the visual claim is *regularity*: almost no noise, a faint regular
            // banding that reads as tool marks, and a flat sheen. It is legible only against
            // families that have grain, which is the argument for it being subtle rather than
            // for it being loud.
            MaterialFamily::Machined => Self {
                base: LinearRgba::rgb(0.52, 0.54, 0.50),
                grain: 0.08,
                frequency: period(2.0, 2.0),
                facet: 0.6,
                rings: 0.30,
                sheen: 0.40,
            },
            // Fibrous, pale, layered. Fine grain at high frequency — fibres rather than boughs —
            // layered banding, and a matte finish. Paper does not shine.
            MaterialFamily::Parchment => Self {
                base: LinearRgba::rgb(0.72, 0.66, 0.48),
                grain: 0.20,
                frequency: period(3.0, 0.14),
                facet: 0.0,
                rings: 0.45,
                sheen: 0.10,
            },
            // Hardened sap. Smooth, flowing, and glossy: the highest sheen of the six and almost
            // no texture to interrupt it, so config reads as something poured rather than grown
            // or cut.
            MaterialFamily::Resin => Self {
                base: LinearRgba::rgb(0.62, 0.42, 0.14),
                grain: 0.16,
                frequency: period(5.0, 1.2),
                facet: 0.0,
                rings: 0.10,
                sheen: 0.85,
            },
            // Inert, uncarved. Isotropic mottling at two scales and no sheen at all — the
            // material that reads as *un-grown* rather than as dead, which is the distinction
            // `N4` asks for about files treepo could not name.
            MaterialFamily::Stone => Self {
                base: LinearRgba::rgb(0.38, 0.38, 0.40),
                grain: 0.30,
                frequency: period(1.1, 1.1),
                facet: 0.15,
                rings: 0.0,
                sheen: 0.06,
            },
        }
    }

    /// This surface coarsened and hardened by a
    /// [`Sparse`](treepo_model::StressKind::Sparse) intensity in `0..=1`.
    ///
    /// Applied once per node rather than per texel, which is what makes this stress free to
    /// draw: it changes the noise's parameters, and the noise was going to be sampled anyway.
    #[must_use]
    pub fn coarsened(mut self, intensity: f32) -> Self {
        let amount = intensity.clamp(0.0, 1.0);
        self.frequency /= 1.0 + SPARSE_COARSENING * amount;
        self.grain = (self.grain * (1.0 + SPARSE_CONTRAST * amount)).min(1.0);
        self.facet = (self.facet + 0.35 * amount).min(1.0);
        self
    }
}

/// The colour of one material family, before any surface treatment.
///
/// Kept as a function of its own because it is what a legend, a swatch, or a container's
/// inventory band needs: those want the family's identity, not its texture.
#[must_use]
pub fn family_color(family: MaterialFamily) -> LinearRgba {
    Surface::of(family).base
}

/// Where one texel sits on the limb it is part of.
///
/// Every field is in units of the limb's own half-width at that texel, except
/// [`fraction`](Self::fraction), which is the position along the whole **node** and is what the
/// age gradient and the ownership mosaic are indexed by. The two axes are different questions:
/// texture is about the local surface, and the gradient and the mosaic are about the limb as a
/// whole.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LimbPoint {
    /// Distance from the node's base, in half-widths.
    pub along: f32,
    /// Signed distance from the centre line, in half-widths — `-1` and `+1` are the edges.
    pub across: f32,
    /// How far along the whole node this is, in `0..=1`.
    pub fraction: f32,
}

/// Everything about a node that does not vary from texel to texel.
///
/// Assembled once per node by [`bake`](crate::bake) and read for every texel of it. The split is
/// the whole performance argument of this slice: what is in here is paid once for a limb, and
/// what is in [`shade`] is paid once for each of the limb's texels — so a field that could be
/// in either belongs here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shading {
    /// The primary family's surface, already carrying any
    /// [`Sparse`](treepo_model::StressKind::Sparse) coarsening.
    pub surface: Surface,
    /// The colour a [`Blended`](treepo_model::Composition::Blended) node's veins are drawn in,
    /// and how much of the node they claim.
    pub vein: Option<(LinearRgba, f32)>,
    /// Normalized age at the node's base and at its tip — `F-MAT-4`.
    ///
    /// Both zero where the node has no history, which is the reading
    /// [`Material::gradient`](treepo_model::Material::gradient) being `None` asks for: unknown
    /// age draws at full brightness, the same as new, because nothing here can tell them apart.
    pub age: (f32, f32),
    /// [`Cracked`](treepo_model::StressKind::Cracked) intensity, in `0..=1`.
    pub cracked: f32,
    /// [`Restless`](treepo_model::StressKind::Restless) intensity, in `0..=1`.
    pub restless: f32,
    /// How many octaves of noise this node's texels are worth sampling at this band.
    pub octaves: u32,
    /// Decorrelates this node's noise from its neighbours'.
    pub seed: u32,
}

/// The colour of one texel of one limb — every field of a
/// [`Material`](treepo_model::Material) except the budget, which is geometry rather than
/// appearance.
///
/// `base` is the colour before any treatment. It is a parameter rather than
/// [`Surface::base`] because a [`Subordinate`](treepo_model::Composition::Subordinate)
/// container is banded along its length by what it *holds*, so the colour varies within one
/// node while the texture does not — a container is one surface showing an inventory, not
/// several materials pretending to be one limb.
///
/// `accent` is the contributor colour the ownership mosaic puts at this fraction along the
/// limb, if any. Both are resolved by [`bake`](crate::bake) from per-node run tables, because
/// those lookups are per-node data and this function is per-texel.
///
/// Returns a colour and only a colour. See the module header for why a coverage value would
/// break `N7` from the inside.
#[must_use]
#[inline(always)]
pub fn shade(
    shading: &Shading,
    base: LinearRgba,
    at: LimbPoint,
    accent: Option<LinearRgba>,
) -> LinearRgba {
    let surface = &shading.surface;

    // One evaluation, four readings. Anisotropic, so the same call gives wood grain or stone
    // mottle depending only on the period this family carries.
    let field = fbm(
        Vec2::new(
            at.along * surface.frequency.x,
            at.across * surface.frequency.y,
        ),
        shading.seed,
        shading.octaves,
    );
    // Quantized against smooth, mixed by `facet`. Symmetric about zero so faceting brightens
    // and darkens equally — a one-sided quantization would shift the family's mean colour as
    // `facet` rose, which would look like a different family.
    let plated = quantize(field, FACET_STEPS) * FACET_STEP;
    let texture = field + (plated - field) * surface.facet;

    // Age, banded. §8.3 wants "growth rings + tip vitality without requiring explicit ring
    // geometry", so the rings are a modulation *of the age gradient* rather than geometry of
    // their own: where the gradient is old the rings are dark on dark, where it is young they
    // are barely there, and the crowding toward the old end falls out of that rather than being
    // a second rule.
    let age = shading.age.0 + (shading.age.1 - shading.age.0) * at.fraction;
    let ring = triangle(at.along / RING_PERIOD) * surface.rings;
    let aged = 1.0 - AGE_DARKENING * age.clamp(0.0, 1.0) + RING_DEPTH * ring * age.clamp(0.0, 1.0);

    // Roundness and highlight, from the across-coordinate alone. `round` is the z of a
    // cylinder's normal, which is what makes a flat quad read as a bough; the highlight is
    // offset from the centre so the limb is lit from somewhere rather than glowing.
    let profile = at.across.clamp(-1.0, 1.0);
    let round = (1.0 - profile * profile).max(0.0).sqrt();
    let highlight = (1.0 - (profile + 0.45).abs() * 2.2).max(0.0);
    let solidity = 0.62 + 0.38 * round + surface.sheen * highlight;

    // Fissures ride the ridges of the field that is already computed. A second noise sample
    // would be the obvious implementation and would cost as much as everything above.
    let fissure = if shading.cracked > 0.0 {
        (1.0 - (texture.abs() / CRACK_WIDTH)).max(0.0) * shading.cracked * CRACK_DEPTH
    } else {
        0.0
    };

    // Restlessness has no shape, so it is drawn as the surface failing to settle: a per-texel
    // hash rather than a sampled field. One hash, and only for nodes that carry the stress.
    let unease = if shading.restless > 0.0 {
        (hash(
            at.along.to_bits() as i32,
            at.across.to_bits() as i32,
            shading.seed,
        ) - 0.5)
            * RESTLESS_JITTER
            * shading.restless
    } else {
        0.0
    };

    let light = (aged * solidity * (1.0 + texture * surface.grain) - fissure + unease).max(0.0);

    // Veining is the second *material*, not the second contributor — `F-MAT-1`. It is a
    // threshold on the same field, so a node made of two families shows one veined with the
    // other at no cost beyond a comparison.
    let mut color = base;
    if let Some((secondary, weight)) = shading.vein {
        // The threshold falls as the secondary's share rises, so `weight` reads as "how much of
        // the limb is veined" rather than as an opaque knob. At a half share the cut is zero and
        // the vein claims half the surface, which is where `Blended` stops being able to grow —
        // the runner-up is by definition no larger than the winner.
        let cut = VEIN_SPREAD * (0.5 - weight.clamp(0.0, 0.5));
        if texture > cut {
            let strength = VEIN_STRENGTH * ((texture - cut) / (1.0 - cut).max(1e-3)).min(1.0);
            color = mix(color, secondary, strength);
        }
    }
    // The mosaic goes on last and over everything, because `F-MAT-2` makes it an accent over the
    // primary material — including over the vein, which is also primary-material information.
    if let Some(accent) = accent {
        color = mix(color, accent, MOSAIC_ACCENT);
    }

    LinearRgba::new(
        color.red * light,
        color.green * light,
        color.blue * light,
        1.0,
    )
}

/// How many octaves are worth sampling for a period at a texel density.
///
/// `period` is the coarser of the family's two axes in half-widths, and `texels_per_half_width`
/// is how many texels this band puts across half a limb. An octave halves the feature size, so
/// the count is how many halvings fit before a feature is [`FINEST_FEATURE_TEXELS`] across.
///
/// At least one: a limb narrower than a texel still gets its family's colour and its base
/// modulation, and dropping to zero octaves would make the thinnest limbs — which is most of a
/// tree at `F-NAV-3`'s far band — the only ones drawn flat.
#[must_use]
pub fn octaves_for(period: f32, texels_per_half_width: f32) -> u32 {
    let coarsest = period * texels_per_half_width;
    if !coarsest.is_finite() || coarsest <= FINEST_FEATURE_TEXELS {
        return 1;
    }
    let halvings = (coarsest / FINEST_FEATURE_TEXELS).log2().floor();
    (halvings as u32 + 1).clamp(1, MAX_OCTAVES)
}

/// Fractional Brownian motion — `octaves` of [`noise`], each half the amplitude and twice the
/// frequency of the last, normalized to `-1..=1`.
///
/// **Dispatched to a fixed count rather than looping on a variable one**, and that is worth the
/// three arms. With the count a runtime value the compiler cannot unroll the loop, so the four
/// hashes of each octave are computed one octave at a time — and a hash is a chain of dependent
/// multiplies, so a serial loop spends most of its cycles waiting. Unrolled, twelve independent
/// hashes are in flight at once and the same arithmetic costs a fraction of the time. The bake
/// is the one place in this workspace where that distinction is worth writing down.
#[must_use]
#[inline]
fn fbm(at: Vec2, seed: u32, octaves: u32) -> f32 {
    match octaves {
        0 | 1 => octaves_of::<1>(at, seed),
        2 => octaves_of::<2>(at, seed),
        _ => octaves_of::<3>(at, seed),
    }
}

/// [`fbm`] at a count the compiler knows.
#[must_use]
#[inline(always)]
fn octaves_of<const N: usize>(at: Vec2, seed: u32) -> f32 {
    let mut sum = 0.0;
    let mut amplitude = 1.0;
    let mut point = at;
    for octave in 0..N {
        // The seed is perturbed per octave so the octaves are independent fields rather than
        // one field read at several scales — without it they correlate at the lattice points
        // they share and the sum shows a grid.
        sum += noise(point, seed ^ (octave as u32).wrapping_mul(0x9e37_79b9)) * amplitude;
        amplitude *= 0.5;
        point *= 2.0;
    }
    // The amplitudes are a geometric series, so their sum is `2 - 2^(1-N)` and `amplitude` has
    // been halved to exactly `2^-N` by the loop. Written this way it folds to a constant.
    sum / (2.0 - 2.0 * amplitude)
}

/// Value noise on the integer lattice, smoothstep-interpolated, in `-1..=1`.
///
/// Value rather than gradient noise: it is four hashes and three interpolations against
/// Perlin's four hashes, four dot products and three interpolations, and the visible difference
/// — value noise's slight axis alignment — is hidden by the anisotropic sampling this module
/// does anyway.
#[must_use]
#[inline(always)]
fn noise(at: Vec2, seed: u32) -> f32 {
    let (x, y) = (floor_i(at.x), floor_i(at.y));
    let offset = Vec2::new(at.x - x as f32, at.y - y as f32);
    // Smoothstep, so the field's derivative is continuous at the lattice lines. Linear
    // interpolation leaves a visible crease along every one of them, which on a limb reads as
    // a grid drawn over the bark.
    let weight = offset * offset * (Vec2::splat(3.0) - 2.0 * offset);

    let top = lerp(hash(x, y, seed), hash(x + 1, y, seed), weight.x);
    let bottom = lerp(hash(x, y + 1, seed), hash(x + 1, y + 1, seed), weight.x);
    lerp(top, bottom, weight.y) * 2.0 - 1.0
}

/// A hash of two lattice coordinates and a seed, in `0..1`.
///
/// Integer mixing rather than anything from `treepo-det`: this is a rendering value and
/// architecture D6/E1 puts the determinism boundary at the data this crate *receives*, not at
/// its pixels. What it does have to be is stable within a run and identical across LOD bands,
/// which a pure function of the lattice coordinate is by construction.
#[must_use]
#[inline(always)]
fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut value =
        (x as u32).wrapping_mul(0x27d4_eb2d) ^ (y as u32).wrapping_mul(0x1656_67b1) ^ seed;
    value ^= value >> 15;
    value = value.wrapping_mul(0x2c1b_3c6d);
    value ^= value >> 12;
    value = value.wrapping_mul(0x2971_1cb5);
    value ^= value >> 15;
    // The top 24 bits, so the result is an exactly representable multiple of 2⁻²⁴ and the
    // interpolation below has no rounding of its own to add.
    (value >> 8) as f32 * (1.0 / 16_777_216.0)
}

/// A triangle wave on `0..1`, in `-1..=1`. Rings, without a `sin`.
#[must_use]
#[inline]
fn triangle(at: f32) -> f32 {
    let phase = at - floor_i(at) as f32;
    1.0 - 4.0 * (phase - 0.5).abs()
}

/// How many plates [`Surface::facet`] quantizes the noise into, and its reciprocal.
const FACET_STEPS: f32 = 2.5;
/// The width of one plate — a multiply, because `2.5` is not a divisor LLVM can fold.
const FACET_STEP: f32 = 1.0 / FACET_STEPS;

/// The nearest multiple of `1 / steps`, as a multiple of `steps` — round-half-up, symmetric.
#[must_use]
#[inline]
fn quantize(value: f32, steps: f32) -> f32 {
    floor_i(value * steps + 0.5) as f32
}

/// The largest integer not greater than `value`.
///
/// **Not [`f32::floor`], and that is a measured decision rather than a preference.** `floor`
/// lowers to a single instruction only where the target has SSE4.1, and this workspace pins no
/// `target-cpu` on purpose — `.cargo/config.toml` records why, and the reason (a build flag
/// that varies by machine is an `AC-DET-2` failure waiting to happen) is a good one. On the
/// baseline x86-64 the compiler emits a call to `floorf` instead.
///
/// The first version of this module made eight such calls per texel — six in [`noise`], one in
/// [`triangle`], one rounding the facets — and `xtask id-coverage`, which bakes 90.7 M texels,
/// went from **4.2 s to 23.1 s**. Removing them is most of the way back. Cast, compare,
/// subtract: three instructions, no call, and the same answer.
///
/// Rust's float-to-int cast saturates rather than wrapping, so a coordinate outside `i32` gives
/// a clamped lattice cell instead of an unspecified one. None occur — the coordinates are a
/// limb's own length in half-widths — but a noise field is exactly the kind of thing that gets
/// handed a NaN one day, and a saturating cast turns that into a flat patch rather than a
/// panic.
#[must_use]
#[inline]
fn floor_i(value: f32) -> i32 {
    let truncated = value as i32;
    truncated - i32::from(value < truncated as f32)
}

/// Linear interpolation, in the one direction this module needs it.
#[must_use]
#[inline]
fn lerp(from: f32, to: f32, at: f32) -> f32 {
    from + (to - from) * at
}

/// Two colours mixed in linear space.
#[must_use]
#[inline]
fn mix(from: LinearRgba, to: LinearRgba, at: f32) -> LinearRgba {
    LinearRgba::new(
        lerp(from.red, to.red, at),
        lerp(from.green, to.green, at),
        lerp(from.blue, to.blue, at),
        from.alpha,
    )
}

/// A contributor's palette colour as something a texel can be tinted with.
///
/// `treepo-id` works in OKLab throughout, because that is the space `AC-MAT-4`'s
/// perceptual-separation threshold is defined in and a palette that guarantees separation in
/// one space guarantees nothing in another. This is the one conversion out of it, and it is
/// here rather than there for the reason every float in this crate is here: the palette is
/// integer arithmetic over `Fx` so that three platforms agree on it, and a matrix of transfer
/// coefficients is not.
///
/// Björn Ottosson's inverse OKLab matrix. Clamped at zero because the OKLab gamut is larger
/// than sRGB's and a palette entry near its edge can name a colour with no non-negative sRGB
/// coordinate; clamping is the standard treatment and desaturates rather than hue-shifts.
#[must_use]
pub fn author_color(color: &AuthorColor) -> LinearRgba {
    let lab = color.to_oklab();
    let (l, a, b) = (
        lab.l.to_f64() as f32,
        lab.a.to_f64() as f32,
        lab.b.to_f64() as f32,
    );

    let long = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let medium = (l - 0.105_561_35 * a - 0.063_854_17 * b).powi(3);
    let short = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);

    LinearRgba::new(
        (4.076_741_7 * long - 3.307_711_6 * medium + 0.230_969_94 * short).clamp(0.0, 1.0),
        (-1.268_438 * long + 2.609_757_4 * medium - 0.341_319_4 * short).clamp(0.0, 1.0),
        (-0.004_196_086_3 * long - 0.703_418_6 * medium + 1.707_614_7 * short).clamp(0.0, 1.0),
        1.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shading(family: MaterialFamily) -> Shading {
        Shading {
            surface: Surface::of(family),
            vein: None,
            age: (0.0, 0.0),
            cracked: 0.0,
            restless: 0.0,
            octaves: MAX_OCTAVES,
            seed: 0x5eed,
        }
    }

    fn at(along: f32, across: f32) -> LimbPoint {
        LimbPoint {
            along,
            across,
            fraction: 0.5,
        }
    }

    /// What `bake` passes for a node that is not a container: the family's own colour.
    fn base_of(shading: &Shading) -> LinearRgba {
        shading.surface.base
    }

    /// Sixteen samples down the middle of a limb, as a family's signature.
    fn signature(family: MaterialFamily) -> Vec<f32> {
        let shading = shading(family);
        (0..16)
            .map(|step| {
                let point = at(step as f32 * 0.37, 0.1);
                shade(&shading, base_of(&shading), point, None).red
            })
            .collect()
    }

    /// The whole claim of the module: the six families are six *surfaces*, not six colours.
    /// Two families with the same base colour would still have to differ here.
    #[test]
    fn every_family_has_a_distinct_surface() {
        let mut seen: Vec<(MaterialFamily, Surface)> = Vec::new();
        for family in MaterialFamily::ALL {
            let surface = Surface::of(family);
            for (other, previous) in &seen {
                assert!(
                    *previous != surface,
                    "{family:?} and {other:?} have the same surface"
                );
            }
            seen.push((family, surface));
        }
    }

    /// Distinguishable *when drawn*, which is a stronger claim than distinguishable as
    /// parameters: two families could differ in a field that happens not to reach a texel.
    #[test]
    fn every_family_draws_differently() {
        let mut seen: Vec<(MaterialFamily, Vec<f32>)> = Vec::new();
        for family in MaterialFamily::ALL {
            let drawn = signature(family);
            for (other, previous) in &seen {
                assert!(
                    previous != &drawn,
                    "{family:?} draws exactly like {other:?}"
                );
            }
            seen.push((family, drawn));
        }
    }

    /// A textured surface is one whose texels are not all the same. Stated per family, because
    /// a family whose amplitudes were all zero would pass every other test here.
    #[test]
    fn every_family_varies_across_its_own_surface() {
        for family in MaterialFamily::ALL {
            let drawn = signature(family);
            let low = drawn.iter().copied().fold(f32::INFINITY, f32::min);
            let high = drawn.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            assert!(high - low > 0.01, "{family:?} is flat: {low} to {high}");
        }
    }

    /// The band-invariance property this module's coordinate choice exists for. The same point
    /// on the same limb is the same colour however finely the band samples it — so crossing a
    /// band adds detail rather than re-texturing the tree.
    #[test]
    fn the_same_limb_point_shades_the_same_at_every_octave_count() {
        let point = at(3.5, 0.25);
        let coarse = Shading {
            octaves: 1,
            ..shading(MaterialFamily::Heartwood)
        };
        let fine = Shading {
            octaves: MAX_OCTAVES,
            ..shading(MaterialFamily::Heartwood)
        };
        let (a, b) = (
            shade(&coarse, base_of(&coarse), point, None).red,
            shade(&fine, base_of(&fine), point, None).red,
        );
        // Not equal — a finer band is meant to show more — but the same surface underneath,
        // which is what "gains detail" rather than "re-textures" means.
        assert!(
            (a - b).abs() < 0.09,
            "the coarse and fine bands disagree by {}",
            (a - b).abs()
        );
    }

    /// Octaves stop at the texel. A band that cannot resolve a feature must not pay to sample
    /// it, and — the reason that matters — must not alias it into static.
    #[test]
    fn a_band_that_cannot_resolve_a_feature_does_not_sample_it() {
        assert_eq!(octaves_for(7.0, 0.05), 1, "a far band asked for detail");
        assert_eq!(
            octaves_for(7.0, 64.0),
            MAX_OCTAVES,
            "a near band was starved"
        );
        assert!(octaves_for(7.0, 8.0) >= 1);
        assert_eq!(
            octaves_for(f32::NAN, 8.0),
            1,
            "a bad period was not refused"
        );
    }

    /// `F-MAT-4`'s direction, which inverts against everything else in the material: a large
    /// age is *older*, and older is darker.
    #[test]
    fn older_material_draws_darker_than_newer() {
        let young = Shading {
            age: (0.0, 0.0),
            ..shading(MaterialFamily::Heartwood)
        };
        let old = Shading {
            age: (1.0, 1.0),
            ..shading(MaterialFamily::Heartwood)
        };
        let point = at(2.0, 0.0);
        assert!(
            shade(&old, base_of(&old), point, None).red
                < shade(&young, base_of(&young), point, None).red
        );
    }

    /// The gradient runs base to tip, so the two ends of a limb differ even though every other
    /// input is identical.
    #[test]
    fn the_age_gradient_runs_along_the_limb() {
        let shading = Shading {
            age: (1.0, 0.0),
            ..shading(MaterialFamily::Heartwood)
        };
        let base = LimbPoint {
            fraction: 0.0,
            ..at(2.0, 0.0)
        };
        let tip = LimbPoint {
            fraction: 1.0,
            ..at(2.0, 0.0)
        };
        assert!(
            shade(&shading, base_of(&shading), base, None).red
                < shade(&shading, base_of(&shading), tip, None).red,
            "the base is not older than the tip"
        );
    }

    /// `F-MAT-6`'s "coexists with the primary material", as arithmetic: a stressed limb is the
    /// same limb with an extra reading, so it must still be recognizably its own family.
    #[test]
    fn stress_marks_the_surface_without_replacing_it() {
        let clear = shading(MaterialFamily::Heartwood);
        let cracked = Shading {
            cracked: 1.0,
            ..clear
        };
        let mut differences = 0;
        let mut wildly_different = 0;
        for step in 0..64 {
            let point = at(step as f32 * 0.21, 0.15);
            let (a, b) = (
                shade(&clear, base_of(&clear), point, None).red,
                shade(&cracked, base_of(&cracked), point, None).red,
            );
            if (a - b).abs() > 1e-4 {
                differences += 1;
            }
            if (a - b).abs() > 0.5 {
                wildly_different += 1;
            }
        }
        assert!(differences > 0, "cracking changed nothing");
        assert!(
            differences < 32,
            "cracking marked {differences} of 64 samples — that is a material, not a stress"
        );
        assert_eq!(wildly_different, 0, "a fissure went past CRACK_DEPTH");
    }

    /// `Sparse` is the stress that must not become transparency — see the module header. It is
    /// checked here as the property that replaces it: fewer, larger grains.
    #[test]
    fn sparse_coarsens_the_grain_rather_than_removing_material() {
        let plain = Surface::of(MaterialFamily::Heartwood);
        let sparse = plain.coarsened(1.0);
        assert!(sparse.period().x > plain.period().x && sparse.period().y > plain.period().y);
        assert!(sparse.grain > plain.grain);
        assert!(
            sparse.grain <= 1.0 && sparse.facet <= 1.0,
            "amplitudes escaped"
        );
    }

    /// The `N7` invariant, at the one place it could be lost: this function returns opaque
    /// colours for every input, including the ones that darken hardest.
    #[test]
    fn no_input_makes_a_texel_transparent() {
        let stressed = Shading {
            cracked: 1.0,
            restless: 1.0,
            age: (1.0, 1.0),
            ..shading(MaterialFamily::Stone)
        };
        for step in 0..128 {
            let point = LimbPoint {
                along: step as f32 * 0.13,
                across: (step as f32 * 0.07).sin(),
                fraction: step as f32 / 128.0,
            };
            let drawn = shade(&stressed, base_of(&stressed), point, None);
            assert_eq!(drawn.alpha, 1.0, "a texel was drawn see-through");
            assert!(drawn.red >= 0.0 && drawn.green >= 0.0 && drawn.blue >= 0.0);
        }
    }

    /// `F-MAT-2`: the accent is *over* the material, so the material is still under it. A
    /// replacement would make every cell of a mosaic flat.
    #[test]
    fn the_mosaic_accent_tints_rather_than_replaces() {
        let shading = shading(MaterialFamily::Heartwood);
        let accent = LinearRgba::rgb(0.1, 0.7, 0.9);
        let mut varied = 0;
        let mut previous: Option<f32> = None;
        for step in 0..24 {
            let drawn = shade(
                &shading,
                base_of(&shading),
                at(step as f32 * 0.4, 0.1),
                Some(accent),
            );
            // The accent moved it toward the contributor's colour...
            assert!(drawn.blue > drawn.red, "the accent did not show");
            // ...and the family's texture is still varying under it.
            if let Some(before) = previous
                && (drawn.red - before).abs() > 1e-4
            {
                varied += 1;
            }
            previous = Some(drawn.red);
        }
        assert!(varied > 4, "the material stopped varying under the accent");
    }

    /// The share of a surface a vein covers, over a patch big enough to sample the field.
    fn veined_fraction(weight: f32) -> f32 {
        let veined = Shading {
            vein: Some((LinearRgba::rgb(0.9, 0.1, 0.1), weight)),
            ..shading(MaterialFamily::Heartwood)
        };
        let plain = shading(MaterialFamily::Heartwood);
        let (mut marked, mut total) = (0, 0);
        for row in 0..48 {
            for column in 0..48 {
                let point = LimbPoint {
                    along: column as f32 * 0.9,
                    across: row as f32 / 24.0 - 1.0,
                    fraction: 0.5,
                };
                total += 1;
                if (shade(&veined, base_of(&veined), point, None).red
                    - shade(&plain, base_of(&plain), point, None).red)
                    .abs()
                    > 1e-4
                {
                    marked += 1;
                }
            }
        }
        marked as f32 / total as f32
    }

    /// `F-MAT-1`'s "a limb of X veined with Y": some of the limb is the second material and
    /// most of it is not.
    #[test]
    fn a_blended_node_is_veined_rather_than_mixed() {
        let claimed = veined_fraction(0.2);
        assert!(claimed > 0.02, "a blended node showed no vein: {claimed}");
        assert!(
            claimed < 0.5,
            "the vein claimed {claimed} of the limb — that is a mixture, not a vein"
        );
    }

    /// The property [`VEIN_SPREAD`] exists for, and the reason it is a measured number rather
    /// than one picked by eye: a limb made a fifth of something else looks about a fifth veined.
    #[test]
    fn a_vein_claims_about_its_own_share_of_the_limb() {
        for share in [0.08, 0.2, 0.35, 0.5] {
            let claimed = veined_fraction(share);
            assert!(
                (claimed - share).abs() < 0.12,
                "a {share} share veined {claimed} of the limb"
            );
        }
    }

    /// Monotone, which the threshold being a *linear* fit to a non-linear quantile function does
    /// not guarantee by itself: more of a second material is never less vein.
    #[test]
    fn more_of_a_second_material_is_more_vein() {
        let mut previous = 0.0;
        for share in [0.08, 0.15, 0.25, 0.35, 0.5] {
            let claimed = veined_fraction(share);
            assert!(
                claimed >= previous,
                "{share} veined less than the share below it"
            );
            previous = claimed;
        }
    }

    /// The roundness cue, as the property it exists for: a limb is brighter down its middle
    /// than at its edges, which is what stops it reading as a flat coloured strip.
    #[test]
    fn a_limb_reads_as_round_rather_than_flat() {
        let shading = Shading {
            surface: Surface {
                grain: 0.0,
                rings: 0.0,
                ..Surface::of(MaterialFamily::Heartwood)
            },
            ..shading(MaterialFamily::Heartwood)
        };
        let middle = shade(&shading, base_of(&shading), at(1.0, 0.0), None).red;
        let edge = shade(&shading, base_of(&shading), at(1.0, 0.98), None).red;
        assert!(middle > edge, "the middle is not brighter than the edge");
    }

    #[test]
    fn the_noise_field_stays_inside_its_range() {
        for step in 0..512 {
            let point = Vec2::new(step as f32 * 0.37, step as f32 * -0.19);
            let value = fbm(point, 0xabcd, MAX_OCTAVES);
            assert!(
                (-1.0..=1.0).contains(&value),
                "fbm left its range at {point:?}: {value}"
            );
        }
    }

    /// The lattice is what the noise is anchored to, so a hash that ignored one of its
    /// coordinates would give a field that is constant along an axis — invisible in most
    /// samples and unmistakable on a limb.
    #[test]
    fn the_hash_depends_on_both_coordinates_and_the_seed() {
        assert!((hash(1, 2, 0) - hash(2, 2, 0)).abs() > 1e-6);
        assert!((hash(1, 2, 0) - hash(1, 3, 0)).abs() > 1e-6);
        assert!((hash(1, 2, 0) - hash(1, 2, 1)).abs() > 1e-6);
        assert!(
            (hash(-1, -2, 7)).is_finite(),
            "negative lattice cells break it"
        );
    }
}
