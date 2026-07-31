//! ★ What a material *looks* like — `F-MAT-1`…`F-MAT-6` turned into texels.
//!
//! > Primary material is driven by data type / language / binary vs text / asset class. These
//! > determine the base color family, **texture, and physical "feel"** of the pixels
//! > (wood-like, crystalline, metallic, leafy, dusty, etc.).
//!
//! `design/feature-system.md` §8.5. Until this module existed, [`bake`](crate::bake) rendered
//! that sentence as six hex values. Until the grain landed it rendered it as six *fields* — six
//! anisotropic noise sums, which is a texture but is not a surface: a limb had a pattern on it
//! and no structure in it, and the ownership mosaic sat on top as hard bands with vertical cuts
//! that no amount of noise could disguise.
//!
//! What is here now is a **surface with relief**, and ownership that is *in* it rather than
//! painted over it.
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
//! # The four layers, and why they are in this order
//!
//! 1. **The flow** ([`noise2`]). One two-channel value-noise sample bends the limb's coordinate
//!    frame before anything reads it. Everything downstream is therefore *warped*, which is
//!    what separates wood from corduroy: straight parallel lines are a woven fabric, lines that
//!    wander, converge and fork are grain.
//! 2. **The whorls** ([`Knot`]). Nought to two per limb, placed from the node's own path hash.
//!    A knot drags the flow lengthwise toward itself, spreads it sideways and curls it, so the
//!    grain streams past a hard obstacle. This is the feature that makes the result read as
//!    *wood* rather than as noise with a good aspect ratio, and it costs no hash at all.
//! 3. **The plates** ([`Surface::relief`]). Longitudinal ridges separated by grooves, carved
//!    out of a triangle wave of the *warped* across-coordinate. Because the wave is analytic,
//!    its slope is known exactly — so the grooves can be lit as geometry, one wall bright and
//!    the other in shadow, for the price of a sign and a multiply. That is where the depth in
//!    the picture comes from; a noise field cannot supply it, because nobody knows which way a
//!    noise field is facing.
//! 4. **The grain** ([`fbm`]). The family's own character, sampled in the warped frame, which
//!    is what makes six families six materials rather than one material six colours.
//!
//! # One noise field, read eight ways
//!
//! The cost discipline of this module. [`fbm`] is evaluated **once** per texel; grain,
//! faceting, groove modulation, fissures, veining, weathering, ownership feathering and the
//! ring reading are all derived from that one value, and [`noise2`] adds one more sample that
//! is read three ways. Sampling a field per effect is the natural way to write it and would
//! multiply the per-texel cost of the whole bake by the number of effects.
//!
//! Anisotropy is what makes one field enough. Sampling it at `(along / p.x, across / p.y)` with
//! `p.x` far larger than `p.y` gives streaks running along the limb — wood grain; sampling with
//! the two equal gives isotropic blotching — stone. Grain and mottle are the same operation at
//! different aspect ratios, so the six families differ in their *parameters* rather than in
//! their code path, and no family costs more to draw than any other.
//!
//! # Age is saturation first and brightness second
//!
//! `F-MAT-4` says older material is basal and recent material distal, and the first rendering
//! of that was a brightness ramp. Brightness is the wrong axis: it collides with the relief
//! lighting, with the rings, with the fissures and with the cylinder profile, all of which are
//! also brightness, so an old limb and a shaded one were the same picture. Saturation collides
//! with nothing here — no other reading touches it — and it is what actually happens to
//! weathered wood. So old material goes **grey**, and only slightly dark.
//!
//! The reading it buys is the one the design wants and could not previously draw: the material
//! family reads loudest where the material is young, ownership stays legible on old grey bark
//! because the accent is applied *after* the desaturation, and `AC-MAT-2`'s 2% contributor is
//! visible on a three-year-old limb rather than lost in a dark end.
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
//! The flow, the whorls and the plates are deliberately **outside** that count: they are single
//! samples and closed forms, identical at every band. The relief of a limb is therefore the
//! same relief however finely it is sampled, and only its grain gets finer — which is why
//! `the_same_limb_point_shades_the_same_at_every_octave_count` can hold a tight tolerance while
//! the surface carries far more structure than it used to.
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
//! The temptation is now *two* features rather than one. [`StressKind::Sparse`] —
//! "coarse, thin, few-grained material" — reads as an instruction to punch holes; it is drawn
//! as coarser, harder, fewer grains, which is both the safe rendering and the more literal one,
//! since the signal behind it is mass concentrated in a handful of large files rather than mass
//! that is missing. And a **groove** is a hole waiting to happen: the honest rendering of a
//! fissure in bark is a deep dark line, not a gap, because there is wood at the bottom of it.
//!
//! [`StressKind::Sparse`]: treepo_model::StressKind::Sparse

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

/// How many knots one limb may carry.
///
/// Two. One reads as an accident and three as a pattern; two is where a limb starts to look
/// like it grew. The cost is a fixed-size array in [`Shading`] and an unrolled loop with no
/// allocation anywhere, which is what lets the whole feature be free of hashes at draw time.
pub const MAX_KNOTS: usize = 2;

/// The surface treatment of one material family — `F-MAT-1`, `design/feature-system.md` §8.5.
///
/// Nine sets of numbers rather than nine functions, so that adding a family is a row and so
/// that no family is more expensive to draw than another. The fields are read in the order
/// [`shade`] applies them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Surface {
    /// The family's colour before any of the rest of this is applied.
    pub base: LinearRgba,
    /// How strongly the noise field modulates brightness, in `0..=1`.
    pub grain: f32,
    /// Noise cells per half-width, along and across the limb — the reciprocal of the period
    /// [`Surface::of`] is written with.
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
    /// explicit ring geometry**", which is why this modulates the age reading rather than
    /// drawing rings of its own — see [`shade`].
    pub rings: f32,
    /// How glossy the surface is, in `0..=1`.
    ///
    /// The single cheapest cue that a limb is a solid object rather than a coloured region: it
    /// is a function of the across-coordinate alone, so it costs three operations and it is
    /// what separates matte parchment from wet resin.
    pub sheen: f32,
    /// How deep the plate-and-groove structure is carved, in `0..=1`.
    ///
    /// The bark. Zero leaves a smooth surface with only its noise on it — poured resin; one
    /// gives deep fissures with lit and shadowed walls — a mature trunk. This is the field
    /// that carries *depth*, and depth is what the eye reads as material before it reads
    /// colour.
    pub relief: f32,
    /// How many grooves run the length of the limb, per half-width across it.
    ///
    /// Across rather than along, because a fissure is a line *parallel* to the limb: the wave
    /// that makes it is a function of the across-coordinate, so its frequency is counted the
    /// same way. Low values give a few broad plates — a young smooth trunk; high values give
    /// fine striation — paper fibre, or the tool marks of a machined surface.
    pub ridges: f32,
    /// How often a ridge is cut through, per half-width **along** the limb.
    ///
    /// The difference between bark and hair, and the field is here because the first version of
    /// this surface did not have it: longitudinal grooves alone give endless parallel strands
    /// running the whole length of a limb, which is a brushed texture however well it flows.
    /// Bark is *plates* — finite tiles with fissures on all four sides — and one more wave,
    /// along the limb and staggered per ridge, is what turns strands into tiles.
    ///
    /// Zero switches the breaks off entirely, which is not a degenerate case but a family:
    /// [`Machined`](MaterialFamily::Machined)'s tool marks and [`Resin`](MaterialFamily::Resin)'s
    /// poured surface are exactly the two materials whose lines should run uninterrupted.
    pub breaks: f32,
    /// How far the flow field bends this surface, in half-widths.
    ///
    /// The single number separating grown material from made material. At zero the grooves are
    /// dead straight and evenly spaced, which is exactly right for
    /// [`Machined`](MaterialFamily::Machined) and exactly wrong for everything else; at one a
    /// groove wanders more than a whole period, so ridges visibly fork and rejoin the way bark
    /// does.
    pub flow: f32,
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

/// How much of its **saturation** the oldest material loses — `F-MAT-4`.
///
/// The primary age signal, and the module header argues at length for why it is saturation
/// rather than the brightness it used to be. Nearly total: at full age the material is within a
/// sixth of neutral grey, which is what makes "this part of the repository is old" readable at
/// a glance across a whole crown rather than only against a neighbouring limb.
const AGE_DESATURATION: f32 = 0.84;

/// How much of its brightness the oldest material loses — `F-MAT-4`, the secondary reading.
///
/// Cut from 0.45 to a quarter when saturation took over as the primary signal. It is kept, and
/// kept this large, for one reason: desaturation alone cannot separate old material from a
/// family that is *already* neutral, and two of the six — [`Machined`](MaterialFamily::Machined)
/// and [`Stone`](MaterialFamily::Stone) — are exactly that.
const AGE_DARKENING: f32 = 0.26;

/// How much the noise field mottles the age reading, in age units.
///
/// Weathering is patchy. Without this the desaturation is a perfectly smooth ramp along the
/// limb, which reads as a gradient someone applied rather than as material that has been out in
/// the weather for six years.
const AGE_MOTTLE: f32 = 0.16;

/// How much [`Restless`](treepo_model::StressKind::Restless) freshens the age reading.
///
/// Churn is the one local signal that bears on age: a path being rewritten weekly is not old
/// material however long ago its first commit landed, and `F-MAT-4`'s gradient — which is a
/// span between two timestamps — cannot see that. A third rather than all of it, because churn
/// is evidence about the present and the gradient is evidence about the past, and the picture
/// should not let either delete the other.
const CHURN_FRESHENS: f32 = 0.35;

/// How far the ring banding can push the age reading either way.
///
/// Small against [`AGE_DARKENING`], because a ring that swung further than the gradient it
/// bands would make a young limb with rings darker than an old one without — which would
/// invert `F-MAT-4`'s direction wherever the two met.
const RING_DEPTH: f32 = 0.14;

/// Where a plate stops rising and goes flat, as a fraction of the groove-to-crest distance.
///
/// A fifth, so a ridge is a broad flat top with a *narrow* valley between it and the next —
/// which is what bark is. At a half the profile is a sine and the surface reads as corrugated
/// iron; 0.42 looked like planed timber and 0.32 like folded cloth. The number is small because
/// what makes a fissure read as a fissure is the *steepness* of its walls, not its depth: a
/// wide shallow trough and a wide deep one are both dents.
const PLATE_EDGE: f32 = 0.22;

/// The same, for the transverse breaks that cut a ridge into plates.
///
/// Wider than [`PLATE_EDGE`], so a break is a soft crease rather than a second groove as hard
/// as the ones running the length of the limb. Bark splits *along* the grain first and across
/// it only because it has to.
const BREAK_EDGE: f32 = 0.42;

/// How deep a transverse break cuts, against a longitudinal groove's full depth.
///
/// **Half, and it was nearly full until a picture said otherwise.** A transverse break is short
/// — it spans one plate — and it is straight, because it runs along a coordinate the flow barely
/// bends. A short straight groove at full depth does not read as a crack in bark; it reads as a
/// black tick mark ruled across the limb, which is exactly what the near-band tree view showed.
/// Bark splits along the grain and only reluctantly across it, so the shallower cut is the more
/// literal one as well as the one that looks right.
const BREAK_DEPTH: f32 = 0.52;

/// How shallow the shallowest *ridge* is against the deepest, as a fraction.
///
/// Bark is not one fissure repeated. It has a hierarchy: a handful of deep primary splits with
/// shallower secondary ones between them, and a surface where every groove is the same depth
/// reads as machined however irregularly it is spaced. This gives each plate a depth of its
/// own, from its lane index alone — no hash, no sample, one fractional part.
const TIER_FLOOR: f32 = 0.55;

/// How the per-plate depth tier advances from one ridge to the next.
///
/// The plastic number's fractional part, and *not* [`PLATE_STAGGER`] — the two are read off the
/// same lane index, and using one constant for both would make every deep plate break at the
/// same offset along the limb, which is a correlation the eye finds immediately.
const TIER_STEP: f32 = 0.754_878;

/// How far consecutive ridges stagger their breaks, in break-periods.
///
/// The golden ratio's fractional part, which is the standard low-discrepancy stagger and is
/// here for a specific failure: with no offset every ridge breaks at the same place along the
/// limb and the breaks line up into a second set of grooves running *across* it. That is
/// masonry. Any irrational offset removes it; this one removes it fastest, so even two
/// adjacent ridges are visibly out of step.
const PLATE_STAGGER: f32 = 0.618_034;

/// How much darker the bottom of a groove is than the top of a plate.
///
/// Ambient occlusion, in one constant: a crevice sees less of the sky whichever way the light
/// comes from. It is the larger of the two relief terms because it is the one that survives
/// being zoomed away from — at the far band a groove is a dark line and its two walls are the
/// same texel.
const RELIEF_SHADE: f32 = 0.86;

/// How much brighter a groove wall facing the light is than one facing away.
///
/// The directional half of the relief, and the reason the plates read as *carved* rather than
/// as stripes. Light comes from the same side the [`Surface::sheen`] highlight does, so the two
/// cues agree about where the sun is.
const RELIEF_FACE: f32 = 0.30;

/// How shallow the shallowest groove is, as a fraction of [`Surface::relief`].
///
/// Grooves vary in depth or they read as machined. Modulated by the **flow** field rather than
/// by [`fbm`], deliberately: the flow is one sample and is identical at every LOD band, so a
/// limb's relief is the same relief however finely it is sampled and only its grain gets finer.
const DEPTH_FLOOR: f32 = 0.50;

/// Where on the noise field a [`Cracked`](treepo_model::StressKind::Cracked) fissure opens.
///
/// A cut rather than a modulation, and high enough that a crack claims a small minority of the
/// surface: `F-MAT-6` says stress coexists with the material rather than replacing it, and a
/// fissure everywhere is a family. The field's p90 is 0.37, so a cut at 0.34 opens roughly the
/// top eighth of the surface — and only the part of it that is already in a groove.
const CRACK_CUT: f32 = 0.34;

/// How dark a fissure gets — [`Cracked`](treepo_model::StressKind::Cracked).
///
/// A crack is bark failing where it was already weak, so it deepens an existing groove rather
/// than drawing a new line. That is both cheaper — no second field, no second wave — and the
/// only version that survives the surface having relief: a dark line painted across a lit plate
/// reads as dirt.
const CRACK_DEPTH: f32 = 0.60;

/// How far [`Restless`](treepo_model::StressKind::Restless) jitters a texel.
///
/// §8.8's "slight visual unease". Per-texel rather than per-feature: restlessness is the one
/// stress with no shape, so it is drawn as the surface failing to settle. In a still picture
/// that is a fine speckle; Phase 8 is where it can move.
const RESTLESS_JITTER: f32 = 0.22;

/// How much of a family's [`Surface::sheen`] reaches the picture.
///
/// A gloss highlight competes with the relief for the one channel the relief lives in, and it
/// wins, because it is broad and the relief is fine. Damped so that the two glossiest families
/// keep the reading that separates them from the matte ones without their plates disappearing
/// into a band of white.
const SHEEN_WEIGHT: f32 = 0.62;

/// How much [`Sparse`](treepo_model::StressKind::Sparse) coarsens the grain.
///
/// At full intensity the period trebles and the amplitude nearly doubles: fewer, larger, harder
/// grains, and fewer, broader plates. Deliberately not transparency — see the module header for
/// why that is the one thing this module must not do.
const SPARSE_COARSENING: f32 = 2.0;
/// How much [`Sparse`](treepo_model::StressKind::Sparse) hardens the grain.
const SPARSE_CONTRAST: f32 = 0.8;

/// How strongly a contributor's colour tints the material under it — `F-MAT-2`.
///
/// §8.5 makes ownership an *accent over* the primary material: "a limb whose primary material
/// is 'TypeScript wood' can still carry author-coloured veins". A tint rather than a
/// replacement is that sentence — the family's grain, plates and rings stay visible through the
/// accent, so a vein reads as pigment in the wood rather than as a limb made of a contributor.
///
/// How strongly a contributor's colour tints the material *away* from its threads — `F-MAT-2`.
///
/// Faint. It is what marks a stretch of limb as somebody's without claiming any particular texel
/// of it: a run reads as tinted from across the room, and up close the tint resolves into
/// [`ACCENT_THREAD`]'s veins with bark between them.
const ACCENT_WASH: f32 = 0.16;

/// How strongly a contributor's colour tints the centre line of one of its threads.
///
/// **Two numbers rather than one, and the arithmetic says why.** A single mid-strength tint
/// everywhere cannot work, and the reason is not taste: a linear mix of two colours at the same
/// luminance averages their chromatic vectors, and two hues far apart on the wheel have vectors
/// that partly *cancel*. Heartwood is a red-dominant tan; the palette's green entry is nearly
/// its complement; at a half-and-half mix the measured result was khaki — less colourful than
/// either input. Boosting the accent does not fix it, because the boost is what is cancelling.
///
/// What does fix it is not mixing halfway anywhere. On a thread the accent takes nearly all of
/// the hue, so there is nothing to cancel against and the contributor's colour arrives intact;
/// off a thread it takes almost none, so the material's does. The picture stops being an average
/// of two readings and becomes both of them, in different places — which is what "author-coloured
/// veins" says on the page, and what the reference photographs of bark actually look like.
const ACCENT_THREAD: f32 = 0.55;

/// The most of a texel a contributor may ever claim.
///
/// Under one, so a trace of the material is present in every texel of the tree and "an accent
/// *over* the primary material" is literally rather than approximately true.
const ACCENT_CEILING: f32 = 0.88;

/// Where a thread begins, on the comb's `-1..=1` wave.
///
/// A third of the way up, so threads are a clear minority of the surface and bark is the
/// majority — at this cut and [`THREAD_SHARPEN`] a holder's run is about a fifth veins and four
/// fifths material.
const THREAD_CUT: f32 = 0.34;

/// How abruptly a thread's edge arrives.
///
/// Above one, so the ribbon saturates before the comb's crest and a thread has a *flat middle*
/// rather than a single bright line. A thread with no width is a scratch.
const THREAD_SHARPEN: f32 = 2.20;

/// How far the fine flow lets a thread wander off the comb, in comb periods.
///
/// What makes threads split, thin, thicken and drift rather than run as a regular corduroy.
/// Read off the same fine warp that forks the bark, so a vein wanders *with* the grain it is in.
const THREAD_WANDER: f32 = 0.40;

/// How much louder ownership is allowed to be on fully weathered material.
///
/// The two readings hand off to each other, which is the reason both are legible at once. Where
/// the material is young it is saturated and reads first, so ownership is dressing on it; where
/// the material is old it has gone grey and has nothing left to say about its family, so
/// ownership takes over the colour that is no longer being used.
///
/// That is not a compromise between the two — it is the arrangement in which neither is ever
/// quiet. A fixed tint had to be weak enough not to shout over young material and was therefore
/// too weak to show on old, which is where `AC-MAT-2`'s hardest case actually lives: the 2%
/// contributor to a directory nobody has touched in three years.
///
/// A third rather than the two thirds it was first set to. Grey material offers a contributor's
/// colour no competition at all, so the mix arrives at full chroma — and the palette is authored
/// at chroma the eye reads as *vivid* when it is not sitting on anything. Past this the veins
/// on an old limb stop being pigment and start being neon.
const ACCENT_ON_GREY: f32 = 0.35;

/// How far a contributor's colour is pushed from grey before it is mixed in.
///
/// **One, and the history is the point.** It was raised to 1.45 and then to 2.2 while the mosaic
/// was still a mid-strength wash, because the veins would not show — and it never worked, because
/// what was eating them was chroma cancellation in the mix rather than a shortage of chroma at
/// the source. Once [`ACCENT_THREAD`] stopped mixing halfway, the boost was not merely
/// unnecessary but actively wrong: a thread at 0.6 of a 2.2× accent is neon, and `AC-MAT-4`'s
/// palette is already exactly as colourful as it was authored to be.
///
/// Kept as a named one rather than deleted. The knob is where the reasoning is written down, and
/// the next person to find the veins too quiet should read [`ACCENT_THREAD`] before reaching for
/// this.
const ACCENT_CHROMA: f32 = 1.00;

/// How much plate-to-plate variation the relief carries, as a fraction of full brightness.
///
/// Adjacent plates of bark do not sit at the same height and do not catch the light equally.
/// Without this the surface is a network of fissures over a uniform field, which reads as a
/// pattern printed on a cylinder; with it, the plates are objects. It is read off the same
/// per-plate tier the groove depth uses, so it costs nothing beyond an add.
const PLATE_VARY: f32 = 0.20;

/// How much of the ownership tint survives at the bottom of a groove, in `0..=1`.
///
/// Pigment sits on what is raised. Below one, so a vein is visibly *in* the surface — it
/// thins where the bark splits and pools on the plates — and above zero, because a groove that
/// erased ownership would put black gaps through every contributor's vein and lose
/// `AC-MAT-2`'s smallest holder first.
const ACCENT_FLOOR: f32 = 0.38;

/// How far an ownership thread may wander from the fraction it nominally belongs to.
///
/// The whole of the interleaving, in one number. At zero the mosaic is what it was — hard
/// vertical cuts at the run boundaries. At the value here a thread strays a fifth of the limb,
/// which against typical run widths means two neighbouring contributors interpenetrate over
/// most of their shared boundary rather than meeting at a line.
const WEAVE_REACH: f32 = 0.20;

/// How much of [`WEAVE_REACH`] applies at the very base of a limb.
///
/// Small, and that is the chronology. The runs are laid base to tip, so the holder at the base
/// is the one the sequence starts with — and a schedule that grows tip-ward means the start is
/// crisp and every later boundary is more interpenetrated than the one before it. The reading
/// is "this began here, and everything after it grew through everything before it", which is
/// what a repository's history does.
const WEAVE_BASE: f32 = 0.12;

/// How much further a later holder reaches back than an earlier one reaches forward.
///
/// The excursion is skewed rather than symmetric — `w * (1 + skew * w)` — so a positive stray,
/// which fetches a *later* run's colour, is amplified and a negative one is damped. Later
/// material therefore weaves down through earlier material in broad fingers, while earlier
/// material survives up the limb as the thin filaments a damped negative tail produces. Both
/// halves of the requirement fall out of one multiply.
const WEAVE_SKEW: f32 = 0.45;

/// How much of an ownership thread's wander comes from the broad flow.
const WEAVE_FLOW: f32 = 0.55;
/// How much comes from the family's own fine grain.
const WEAVE_STRAND: f32 = 0.35;
/// How much comes from the comb — the term that makes threads parallel rather than blotchy.
const WEAVE_COMB: f32 = 0.45;

/// How many ownership threads run across a half-width.
///
/// Family-independent, unlike [`Surface::ridges`], and that is an `AC-MAT-4` argument rather
/// than an aesthetic one: whether a contributor is legible must not depend on which language
/// they happened to write in. The comb is a triangle wave of the *warped* across-coordinate, so
/// the threads still bend with the grain of whatever they are drawn on — they are just always
/// threads.
const WEAVE_RIDGES: f32 = 3.0;

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

// --- the flow field -------------------------------------------------------------------

/// The coarse warp's lattice, in cells per half-width along and across the limb.
///
/// Long along and moderate across: this is the bend that takes the whole limb's grain one way
/// and then the other, over a period of four or five half-widths.
const FLOW_COARSE: Vec2 = Vec2::new(0.22, 0.60);

/// The fine warp's lattice, in cells per half-width.
///
/// **Two samples rather than one, and this is the one that makes it wood.** A single coarse warp
/// varies slowly across a limb, so it displaces every groove by nearly the same amount and they
/// stay parallel — the surface wanders as a unit, which is what brushed hair does. Bark forks:
/// grooves converge, merge and split, and that requires the warp to have a *gradient across the
/// limb* comparable to the groove spacing itself. Nearly three cells per half-width against
/// [`Surface::ridges`]'s five is exactly that, and where the displacement folds — where the warp
/// is steep enough to run backwards — two grooves meet and become one.
///
/// It could have come from [`fbm`], which is already sampled and would have been free. It does
/// not, and the reason is the property in the module header: `fbm`'s octave count varies with
/// the LOD band, so a relief built on it would *slide* as the camera crossed a band. A second
/// [`noise2`] costs four hashes and keeps the bark nailed to the limb.
const FLOW_FINE: Vec2 = Vec2::new(1.10, 2.40);

/// How much of the total warp the fine octave contributes.
///
/// Small, and the bound on it is arithmetic rather than taste. The warp's *gradient across the
/// limb* is its amplitude times its across-frequency, and where that exceeds one the coordinate
/// folds and two grooves merge. A little folding is what a fork is; a lot of it turns the
/// surface into an isotropic maze — cork, or lichen, but not wood. Coarse and fine together
/// come to about six tenths here, which forks occasionally and never dissolves the longitudinal
/// reading `F-MAT-2`'s mosaic is laid along.
const FINE_SHARE: f32 = 0.22;

/// How much further the flow drags the along-axis than the across-axis.
///
/// Grain stretches lengthwise, so a warp that moved both axes equally would make the surface
/// look stirred. This is the one number that keeps the bend anisotropic when the family's own
/// noise is not.
const FLOW_STRETCH: f32 = 1.4;

// --- knots ----------------------------------------------------------------------------

/// How much longer than wide a knot's influence is.
///
/// A knot on a limb is seen from the side, and grain has to part further ahead of an obstacle
/// than beside it, so the disturbance is an ellipse lying along the limb rather than a disc.
const KNOT_STRETCH: f32 = 2.1;
/// How hard a knot drags the grain lengthwise toward itself.
const KNOT_DRAG: f32 = 0.55;
/// How hard a knot pushes the grain sideways out of its way.
const KNOT_SPREAD: f32 = 0.80;
/// How much a knot curls the grain around itself.
///
/// The difference between grain that *parts* around a knot and grain that *whorls* into one.
/// Small, because a large value spins the surface into a pinwheel — which is a rendering
/// artefact wearing a botanical name.
const KNOT_SWIRL: f32 = 0.38;
/// How dark a knot's core draws.
const KNOT_DARKEN: f32 = 0.38;

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
    /// and a tile atlas, not a hex value", is answered by the eight fields beside it rather than
    /// by retuning the ninth.
    ///
    /// These are still not through `AC-MAT-4`'s perceptual-separation check, which applies to
    /// the *author* palette and is a different question from whether six materials read as six
    /// materials. They stay in code rather than moving to `assets/palettes/` for the reason they
    /// always did: a file there would look like a decision, and the decision this slice makes is
    /// about texture.
    #[must_use]
    pub fn of(family: MaterialFamily) -> Self {
        match family {
            // Living wood. Long grain running the length of the limb, deep bark plates that
            // fork and rejoin, strong rings, and enough roundness to read as a bough. The
            // default reading, and the one the other five are departures from.
            MaterialFamily::Heartwood => Self {
                base: LinearRgba::rgb(0.42, 0.26, 0.13),
                grain: 0.42,
                frequency: period(7.0, 0.30),
                facet: 0.0,
                rings: 0.85,
                sheen: 0.22,
                relief: 1.0,
                ridges: 3.4,
                breaks: 0.85,
                flow: 0.55,
            },
            // Dense, resource-like matter. Faceted plates at near-isotropic scale, broad
            // irregular fracture and a hard highlight: `F-MAT-1`'s "resource-like material
            // rather than living wood" is carried by the *absence* of grain and rings as much
            // as by the presence of facets.
            MaterialFamily::Ore => Self {
                base: LinearRgba::rgb(0.36, 0.38, 0.44),
                grain: 0.46,
                frequency: period(1.3, 0.9),
                facet: 0.85,
                rings: 0.0,
                sheen: 0.55,
                relief: 0.58,
                ridges: 1.8,
                breaks: 1.20,
                flow: 0.85,
            },
            // Uniform, tooled, machine-cut. §8.5 asks for "a slightly different, more uniform"
            // treatment, and the visual claim is *regularity*. Almost no noise, and — the part
            // that now does the work — almost no **flow**: this is the one family whose grooves
            // run dead straight and evenly spaced, which beside five families that wander is
            // unmistakable and costs nothing to say.
            MaterialFamily::Machined => Self {
                base: LinearRgba::rgb(0.52, 0.54, 0.50),
                grain: 0.08,
                frequency: period(2.0, 2.0),
                facet: 0.6,
                rings: 0.30,
                sheen: 0.40,
                relief: 0.30,
                ridges: 3.6,
                breaks: 0.0,
                flow: 0.06,
            },
            // Fibrous, pale, layered. Fine grain at high frequency — fibres rather than boughs —
            // shallow close striation, and a matte finish. Paper does not shine.
            MaterialFamily::Parchment => Self {
                base: LinearRgba::rgb(0.72, 0.66, 0.48),
                grain: 0.20,
                frequency: period(3.0, 0.14),
                facet: 0.0,
                rings: 0.45,
                sheen: 0.10,
                relief: 0.48,
                ridges: 6.0,
                breaks: 0.15,
                flow: 0.22,
            },
            // Hardened sap. Smooth, flowing, and glossy: the highest sheen of the six and almost
            // no relief to interrupt it, so config reads as something poured rather than grown
            // or cut.
            MaterialFamily::Resin => Self {
                base: LinearRgba::rgb(0.62, 0.42, 0.14),
                grain: 0.16,
                frequency: period(5.0, 1.2),
                facet: 0.0,
                rings: 0.10,
                sheen: 0.85,
                relief: 0.10,
                ridges: 1.1,
                breaks: 0.0,
                flow: 0.45,
            },
            // Inert, uncarved. Isotropic mottling at two scales, the most disordered flow of the
            // six so its fractures run in no direction at all, and no sheen — the material that
            // reads as *un-grown* rather than as dead, which is the distinction `N4` asks for
            // about files treepo could not name.
            MaterialFamily::Stone => Self {
                base: LinearRgba::rgb(0.38, 0.38, 0.40),
                grain: 0.30,
                frequency: period(1.1, 1.1),
                facet: 0.15,
                rings: 0.0,
                sheen: 0.06,
                relief: 0.70,
                ridges: 2.2,
                breaks: 1.60,
                flow: 1.20,
            },
        }
    }

    /// This surface coarsened and hardened by a
    /// [`Sparse`](treepo_model::StressKind::Sparse) intensity in `0..=1`.
    ///
    /// Applied once per node rather than per texel, which is what makes this stress free to
    /// draw: it changes the surface's parameters, and the surface was going to be sampled
    /// anyway. Both scales coarsen together — the noise cells *and* the plates — because a
    /// material whose grain grew while its bark stayed fine would read as two materials.
    #[must_use]
    pub fn coarsened(mut self, intensity: f32) -> Self {
        let amount = intensity.clamp(0.0, 1.0);
        let spread = 1.0 + SPARSE_COARSENING * amount;
        self.frequency /= spread;
        self.ridges /= spread;
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

/// A whorl in the grain, where something hard interrupted it.
///
/// Placed by [`bake`](crate::bake) from the node's **own path hash**, so a limb keeps its knots
/// across re-scans, across LOD bands and across structural change: adding a file elsewhere in
/// the repository shifts node ids and reorders chunks, and neither of those is allowed to
/// re-roll what a limb looks like. See [`Shading::lineage`] for the other half of that
/// argument.
///
/// A knot is not a claim about the repository. Nothing in a git history says where a branch left
/// a bough — `N4` would have something to say about a picture that implied one — so this is
/// ornament, seeded from the same hash the rest of the limb's character comes from, and it is
/// here because grain that flows past obstacles is what makes a surface read as grown.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Knot {
    /// Where it sits, in the same limb coordinates as [`LimbPoint`].
    pub at: Vec2,
    /// How far its influence reaches across the limb, in half-widths.
    ///
    /// Zero means there is no knot in this slot, which is how a limb carries fewer than
    /// [`MAX_KNOTS`] of them without a length beside the array.
    pub reach: f32,
    /// How dark its core draws, in `0..=1`.
    pub depth: f32,
}

impl Knot {
    /// How this knot bends the grain at a point, and how much it darkens it.
    ///
    /// Returns the offset to add to the limb coordinate and a core intensity in `0..=1`. Three
    /// deflections at once, and the reason they are worth writing out: the grain is **dragged**
    /// lengthwise toward the knot, because material flowed around a fixed obstacle and had
    /// further to go; **spread** sideways out of its way; and **curled**, because the two
    /// streams rejoining behind it do not rejoin cleanly. Any one of the three alone reads as a
    /// dent.
    #[must_use]
    #[inline]
    fn bend(self, at: Vec2) -> (Vec2, f32) {
        if self.reach <= 0.0 {
            return (Vec2::ZERO, 0.0);
        }
        // Elliptical, and in units of the reach, so the falloff below is a comparison against
        // one rather than against a length.
        let delta = Vec2::new(
            (at.x - self.at.x) / (self.reach * KNOT_STRETCH),
            (at.y - self.at.y) / self.reach,
        );
        let square = delta.length_squared();
        if square >= 1.0 {
            return (Vec2::ZERO, 0.0);
        }
        // Quadratic in the *squared* radius, so the influence and its first derivative both
        // vanish at the rim — a linear falloff leaves a visible circle where the knot's
        // influence stops, which is the one thing worse than no knot.
        let fall = 1.0 - square;
        let falloff = fall * fall;
        let bend = Vec2::new(
            (-delta.x * KNOT_DRAG - delta.y * KNOT_SWIRL) * falloff,
            (delta.y * KNOT_SPREAD + delta.x * KNOT_SWIRL) * falloff,
        ) * self.reach;
        (bend, falloff * fall * self.depth)
    }
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
/// the whole performance argument of this module: what is in here is paid once for a limb, and
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
    /// age draws at full saturation, the same as new, because nothing here can tell them apart.
    pub age: (f32, f32),
    /// [`Cracked`](treepo_model::StressKind::Cracked) intensity, in `0..=1`.
    pub cracked: f32,
    /// [`Restless`](treepo_model::StressKind::Restless) intensity, in `0..=1`.
    ///
    /// Read twice: as the speckle §8.8 asks for, and as the churn that freshens the age
    /// reading — see [`CHURN_FRESHENS`].
    pub restless: f32,
    /// How many octaves of noise this node's texels are worth sampling at this band.
    pub octaves: u32,
    /// Decorrelates this node's **fine** detail from its neighbours' — the node's own path.
    pub seed: u32,
    /// Decorrelates this node's **coarse** structure — the node's parent path.
    ///
    /// Two hashes rather than one, and the second is what makes the result a tree rather than a
    /// collection of limbs. The flow field, the plate phase and the ownership comb all read
    /// this one, so every child of a bough inherits the direction its parent's grain was running
    /// and the surface carries across a joint instead of restarting at it. Fine detail — the
    /// grain octaves, the knots, the speckle — reads [`seed`](Self::seed) instead, so siblings
    /// are alike without being identical.
    ///
    /// Both come from the path rather than from the node id, which is a stability argument and
    /// not a determinism one: `D6`/`E1` puts the determinism boundary at the data this crate
    /// receives, and node ids are perfectly deterministic. What they are not is *stable* — add
    /// one file and every id after it shifts, so an id-seeded tree re-rolls its entire
    /// appearance on a re-scan. A path-seeded one does not: the limbs that did not change look
    /// exactly as they did.
    pub lineage: u32,
    /// The whorls in this limb's grain, `reach == 0.0` for an empty slot.
    pub knots: [Knot; MAX_KNOTS],
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
/// `accents` is the ownership mosaic's run table, `F-MAT-2`. It arrives as the table rather
/// than as a resolved colour because *where* the table is read is a per-texel decision that
/// only this function can make: the lookup fraction is displaced by the same flow field that
/// bends the grain, which is what turns the mosaic's contiguous runs into interleaved threads
/// without moving a single cell boundary. See [`WEAVE_REACH`].
///
/// Returns a colour and only a colour. See the module header for why a coverage value would
/// break `N7` from the inside.
#[must_use]
#[inline(always)]
pub fn shade(
    shading: &Shading,
    base: LinearRgba,
    at: LimbPoint,
    accents: &[(f32, LinearRgba)],
) -> LinearRgba {
    let surface = &shading.surface;
    let here = Vec2::new(at.along, at.across);

    // --- the flow. Two samples, four channels, and every one of them read more than once: the
    // pair warps the frame everything below is measured in, the coarse `x` sets how deep the
    // grooves are cut, and the two `y`s are the broad and fine halves of the ownership weave.
    //
    // Seeded differently on purpose. The coarse bend comes from the *lineage*, so a limb bends
    // the way its parent does and the grain carries across a joint; the fine one comes from the
    // node's own hash, so siblings are alike at arm's length and different up close.
    let drift = noise2(here * FLOW_COARSE, shading.lineage);
    let fine = noise2(here * FLOW_FINE, shading.seed);
    let mut warped = here
        + (Vec2::new(drift.x * FLOW_STRETCH, drift.y)
            + Vec2::new(fine.x * FLOW_STRETCH, fine.y) * FINE_SHARE)
            * surface.flow;

    // --- the whorls. Closed form, no hash, and unrolled over a fixed-size array so an empty
    // slot costs one compare.
    let mut core = 0.0f32;
    for knot in &shading.knots {
        let (bend, centre) = knot.bend(here);
        warped += bend;
        core = core.max(centre);
    }

    // --- the grain. Sampled in the warped frame, so the family's character flows with the limb
    // instead of lying across it.
    let field = fbm(
        Vec2::new(
            warped.x * surface.frequency.x,
            warped.y * surface.frequency.y,
        ),
        shading.seed,
        shading.octaves,
    );
    // Quantized against smooth, mixed by `facet`. Symmetric about zero so faceting brightens
    // and darkens equally — a one-sided quantization would shift the family's mean colour as
    // `facet` rose, which would look like a different family.
    let plated = quantize(field, FACET_STEPS) * FACET_STEP;
    let texture = field + (plated - field) * surface.facet;

    // --- the plates. A triangle wave of the warped across-coordinate, carved into a broad flat
    // top and a narrow groove. `slope` is the wave's exact derivative sign, which is what lets
    // the groove walls be *lit* rather than merely shaded; see RELIEF_FACE.
    let comb = warped.y * surface.ridges + phase_of(shading.lineage);
    let (crest, slope) = wave(comb);
    let (ridge, ramp) = shoulder(crest, PLATE_EDGE);

    // Which plate this is. `wave`'s trough sits on the integers, so the lane index changes
    // exactly at a groove and identifies the plate between two of them — which is what both the
    // stagger and the depth tier below want to be indexed by.
    let lane = floor_i(comb) as f32;

    // Cut across, into tiles. Without this the grooves run the entire length of a limb and the
    // surface is brushed hair however well it flows; with it, a ridge is a row of plates. The
    // stagger is per ridge, so the breaks do not line up into a second grid — see PLATE_STAGGER.
    let plate = if surface.breaks > 0.0 {
        let (bar, _) = wave(warped.x * surface.breaks + lane * PLATE_STAGGER);
        ridge.min(1.0 - BREAK_DEPTH * (1.0 - shoulder(bar, BREAK_EDGE).0))
    } else {
        ridge
    };

    // Depth from the flow rather than from `field`: the flow is one sample and does not change
    // with the octave count, so a limb's relief is identical at every band and only its grain
    // gets finer. That is the property the module header calls out as load-bearing. Tiered per
    // plate on top of it, so the fissure network has primary splits and secondary ones instead
    // of one depth repeated.
    let tier = TIER_FLOOR + (1.0 - TIER_FLOOR) * fract(lane * TIER_STEP);
    let depth = surface.relief * tier * (DEPTH_FLOOR + (1.0 - DEPTH_FLOOR) * (drift.x * 0.5 + 0.5));

    // A fissure is a groove that failed, so it deepens one rather than drawing over it — and
    // only where the field says the material was already weak, which is what keeps `F-MAT-6`'s
    // stress a minority of the surface rather than a seventh family.
    let fissure = if shading.cracked > 0.0 {
        ((field - CRACK_CUT) / (1.0 - CRACK_CUT)).max(0.0)
            * (1.0 - plate)
            * shading.cracked
            * CRACK_DEPTH
    } else {
        0.0
    };
    // Occlusion from the whole fissure network, direction from the longitudinal walls alone: a
    // transverse break faces along the limb, which is across the light, so it has a depth to it
    // and no side that catches the sun.
    let relief = (1.0 - depth * (RELIEF_SHADE * (1.0 - plate) + RELIEF_FACE * slope * ramp)
        + PLATE_VARY * (tier - 0.7) * plate)
        .max(0.05);

    // --- age, as saturation first. Mottled by the field so weathering is patchy, and freshened
    // by churn because a path rewritten weekly is not old material whatever its first commit
    // says.
    let span = shading.age.0 + (shading.age.1 - shading.age.0) * at.fraction;
    let weathered =
        (span * (1.0 - CHURN_FRESHENS * shading.restless) + AGE_MOTTLE * field).clamp(0.0, 1.0);

    // Roundness and highlight, from the across-coordinate alone. `round` is the z of a
    // cylinder's normal, which is what makes a flat quad read as a bough; the highlight is
    // offset from the centre so the limb is lit from somewhere rather than glowing — and from
    // the same side RELIEF_FACE lights the groove walls from.
    let profile = at.across.clamp(-1.0, 1.0);
    let round = (1.0 - profile * profile).max(0.0).sqrt();
    // Narrow, because it is a glint rather than a lit side. At the width it used to be, a family
    // with real sheen — ore at 0.55, resin at 0.85 — brightened a third of its own width past
    // white and the plates in that band stopped being visible at all. A specular that erases the
    // relief is worse than no specular.
    let highlight = (1.0 - (profile + 0.45).abs() * 3.4).max(0.0);
    let solidity = 0.40 + 0.55 * round + SHEEN_WEIGHT * surface.sheen * highlight;

    // §8.3 wants "growth rings + tip vitality without requiring explicit ring geometry", so the
    // rings are a modulation *of the age reading* rather than geometry of their own: where the
    // material is old the rings are dark on dark, where it is young they are barely there, and
    // the crowding toward the old end falls out of that rather than being a second rule.
    let ring = triangle(at.along / RING_PERIOD) * surface.rings;
    let aged = 1.0 - AGE_DARKENING * weathered + RING_DEPTH * ring * weathered;

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

    let light =
        (aged * solidity * relief * (1.0 + texture * surface.grain) - fissure - core * KNOT_DARKEN
            + unease)
            .max(0.0);

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

    // Age desaturates the **material**, and only the material. Applied here — after the vein,
    // which is a material reading, and before the accent, which is not — so an old limb is grey
    // bark carrying colour rather than a grey limb: `AC-MAT-2`'s smallest contributor stays
    // legible on the oldest wood in the repository, which is exactly where a brightness-only age
    // signal used to lose them.
    //
    // The luminance is taken once and used twice, which is why it is hoisted out of
    // `desaturate`: mixing a colour toward its own grey leaves its luminance untouched, so the
    // same number is still the right one for the accent below.
    // Squared, so the greying is back-loaded. `age_full_scale_days` is ten years on a log scale,
    // which puts most of a live repository somewhere in the middle third — and a *linear* ramp
    // there takes half the colour out of the entire tree at once, so every limb reads as old and
    // none of them reads as older. Squared, the middle third keeps most of its family colour and
    // the grey is spent where it says something: on the parts nobody has touched in years.
    let key = luminance(color);
    color = toward(color, key, AGE_DESATURATION * weathered * weathered);

    // The mosaic goes on last and over everything, because `F-MAT-2` makes it an accent over the
    // primary material — including over the vein, which is also primary-material information.
    //
    // The comb is computed here rather than inside `weave` because it is read twice: it decides
    // *which* holder a texel belongs to, by displacing the lookup, and *how much* of one, by
    // being the thread itself. Those are the same line of wood, which is why they are the same
    // number.
    let (comb, _) = wave(warped.y * WEAVE_RIDGES + fine.y * THREAD_WANDER);
    if let Some(accent) = weave(accents, at.fraction, drift.y, fine.y, comb) {
        // A wash over the whole of a holder's run, and a thread on top of it. See ACCENT_THREAD
        // for why one number could not do both jobs.
        let ribbon = ((comb - THREAD_CUT) * THREAD_SHARPEN).clamp(0.0, 1.0);
        let bite = ((ACCENT_WASH + ACCENT_THREAD * ribbon)
            * (1.0 + ACCENT_ON_GREY * weathered)
            * (ACCENT_FLOOR + (1.0 - ACCENT_FLOOR) * plate))
            .min(ACCENT_CEILING);
        color = mix(color, keyed(accent, key), bite);
    }

    LinearRgba::new(
        color.red * light,
        color.green * light,
        color.blue * light,
        1.0,
    )
}

/// Which contributor's thread crosses this texel — `F-MAT-2`, as a field rather than a cut.
///
/// [`Mosaic`](treepo_model::Mosaic) lays holders out as contiguous runs along the limb, and
/// [`bake`](crate::bake) turns that into a table of upper bounds. Read at `fraction` the table
/// is a step function of one scalar, so every texel in a column gets the same answer and the
/// mosaic draws as hard vertical cuts — which is precisely what it looked like.
///
/// Nothing about the runs changes here. What changes is *where the table is read*: the lookup
/// fraction is displaced by a field that varies across the limb as well as along it, so a run
/// boundary stops being a line and becomes a contour. Three terms, each doing a different job.
///
/// * **The coarse flow** puts broad fingers of one holder into the next, over several
///   half-widths.
/// * **The fine flow** feathers their edges at the scale of the bark's own fibre.
/// * **The comb** — a triangle wave of the same warped coordinate the plates are cut from —
///   makes the result *threads* rather than blotches, and does it at a family-independent
///   frequency so that a contributor's legibility does not depend on which language they wrote
///   in.
///
/// All three are octave-independent, which is deliberate: an ownership thread that moved when
/// the camera crossed an LOD band would be `AC-NAV-2`'s zoom gesture rewriting who wrote what.
///
/// The displacement is skewed and grows tip-ward; [`WEAVE_SKEW`] and [`WEAVE_BASE`] are where
/// the chronology in that lives.
#[must_use]
#[inline(always)]
fn weave(
    accents: &[(f32, LinearRgba)],
    fraction: f32,
    flow: f32,
    fibre: f32,
    comb: f32,
) -> Option<LinearRgba> {
    if accents.is_empty() {
        return None;
    }
    let strand = flow * WEAVE_FLOW + fibre * WEAVE_STRAND + comb * WEAVE_COMB;
    let woven = strand * (1.0 + WEAVE_SKEW * strand);
    // Clamped into the limb's own fraction space, and that is a correctness bound rather than a
    // tidiness one. A run table's last bound is `claimed / cells`, so "past the end" already
    // means the unclaimed remainder showing bark — but a table where one contributor holds
    // everything ends at exactly 1.0, and an unclamped stray past it would punch unowned
    // patches into a limb that is wholly owned. That is not a rendering artefact, it is the
    // picture saying something false about the repository.
    let displaced = fraction + WEAVE_REACH * (WEAVE_BASE + fraction) * woven;
    run_at(accents, displaced.clamp(0.0, 1.0))
}

/// The colour a run table gives at a fraction along the limb.
///
/// A linear scan, because a run table has one entry per contributor drawn on the node or per
/// family a container holds — single digits in both cases, and a binary search over five
/// entries is slower than looking at them.
///
/// A fraction past the last bound gives `None`, which is the unclaimed remainder showing the
/// primary material (`F-MAT-2`). [`weave`] reaches that case deliberately — a thread that
/// strays past the last holder should fade into bark — which is why the table is scanned for
/// the first bound at or above the fraction rather than clamped to the last entry.
#[must_use]
#[inline]
pub(crate) fn run_at(table: &[(f32, LinearRgba)], fraction: f32) -> Option<LinearRgba> {
    table
        .iter()
        .find(|(bound, _)| fraction <= *bound)
        .map(|(_, color)| *color)
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
    let weight = smooth(at, x, y);

    let top = lerp(hash(x, y, seed), hash(x + 1, y, seed), weight.x);
    let bottom = lerp(hash(x, y + 1, seed), hash(x + 1, y + 1, seed), weight.x);
    lerp(top, bottom, weight.y) * 2.0 - 1.0
}

/// **Two** independent noise fields for the price of one, both in `-1..=1`.
///
/// The trick that makes the flow field affordable, and the reason the whole domain-warp layer
/// costs a quarter of what the obvious implementation would. A lattice sample is four hashes
/// and each hash is a full 32-bit avalanche — but a single noise value only consumes the top
/// twenty-four bits of one, and the bits below that are just as well mixed as the bits above.
/// So the same four hashes are split into halves and interpolated twice, and the *dependent*
/// work — the multiply chains, which is where a hash's latency actually is — is paid once.
///
/// A domain warp needs a vector, so it needs exactly two fields at exactly one point. That is
/// the shape this function has, and it is why the warp is not simply two calls to [`noise`].
///
/// Sixteen bits per channel rather than twenty-four. That is 65 536 distinct lattice values,
/// smoothstepped between — four orders of magnitude finer than the eight bits a texel can hold,
/// so the quantization is not reachable from a picture.
#[must_use]
#[inline(always)]
fn noise2(at: Vec2, seed: u32) -> Vec2 {
    let (x, y) = (floor_i(at.x), floor_i(at.y));
    let weight = smooth(at, x, y);

    let corners = [
        bits(x, y, seed),
        bits(x + 1, y, seed),
        bits(x, y + 1, seed),
        bits(x + 1, y + 1, seed),
    ];
    Vec2::new(
        blend(corners.map(|corner| corner & 0xffff), weight),
        blend(corners.map(|corner| corner >> 16), weight),
    )
}

/// Smoothstep weights for a lattice cell, so the field's derivative is continuous at the
/// lattice lines.
///
/// Linear interpolation leaves a visible crease along every one of them, which on a limb reads
/// as a grid drawn over the bark.
#[must_use]
#[inline(always)]
fn smooth(at: Vec2, x: i32, y: i32) -> Vec2 {
    let offset = Vec2::new(at.x - x as f32, at.y - y as f32);
    offset * offset * (Vec2::splat(3.0) - 2.0 * offset)
}

/// Bilinear interpolation of four 16-bit lattice values, into `-1..=1`.
#[must_use]
#[inline(always)]
fn blend(corners: [u32; 4], weight: Vec2) -> f32 {
    const SCALE: f32 = 2.0 / 65_535.0;
    let top = lerp(corners[0] as f32, corners[1] as f32, weight.x);
    let bottom = lerp(corners[2] as f32, corners[3] as f32, weight.x);
    lerp(top, bottom, weight.y) * SCALE - 1.0
}

/// A hash of two lattice coordinates and a seed.
///
/// Integer mixing rather than anything from `treepo-det`: this is a rendering value and
/// architecture D6/E1 puts the determinism boundary at the data this crate *receives*, not at
/// its pixels. What it does have to be is stable within a run and identical across LOD bands,
/// which a pure function of the lattice coordinate is by construction.
#[must_use]
#[inline(always)]
fn bits(x: i32, y: i32, seed: u32) -> u32 {
    let mut value =
        (x as u32).wrapping_mul(0x27d4_eb2d) ^ (y as u32).wrapping_mul(0x1656_67b1) ^ seed;
    value ^= value >> 15;
    value = value.wrapping_mul(0x2c1b_3c6d);
    value ^= value >> 12;
    value = value.wrapping_mul(0x2971_1cb5);
    value ^ (value >> 15)
}

/// [`bits`] as a value in `0..1`.
///
/// The top 24 bits, so the result is an exactly representable multiple of 2⁻²⁴ and the
/// interpolation above has no rounding of its own to add.
#[must_use]
#[inline(always)]
fn hash(x: i32, y: i32, seed: u32) -> f32 {
    (bits(x, y, seed) >> 8) as f32 * (1.0 / 16_777_216.0)
}

/// A hash as a phase in `0..1` — where a node's plates start.
///
/// Read from the **lineage**, so a limb's grooves line up with its parent's rather than
/// restarting at the joint. That continuity is the whole of what the second hash buys, and it
/// is the difference between a tree and a pile of separately textured sticks.
#[must_use]
#[inline]
fn phase_of(lineage: u32) -> f32 {
    (lineage >> 8) as f32 * (1.0 / 16_777_216.0)
}

/// The fractional part of a value, in `0..1` for the inputs this module has.
#[must_use]
#[inline]
fn fract(at: f32) -> f32 {
    at - floor_i(at) as f32
}

/// A triangle wave on `0..1`, in `-1..=1`. Rings, without a `sin`.
#[must_use]
#[inline]
fn triangle(at: f32) -> f32 {
    1.0 - 4.0 * (fract(at) - 0.5).abs()
}

/// A triangle wave **and the sign of its slope** — the plates, and how they are lit.
///
/// The second return is what a noise field cannot give: the exact direction the surface is
/// facing. A triangle wave's derivative is `±4` everywhere, so knowing which of the two costs a
/// compare — and a groove wall whose facing is known can be lit as geometry rather than merely
/// darkened. That one bit is where the depth in the picture comes from.
#[must_use]
#[inline]
fn wave(at: f32) -> (f32, f32) {
    let phase = fract(at);
    if phase < 0.5 {
        (4.0 * phase - 1.0, 1.0)
    } else {
        (3.0 - 4.0 * phase, -1.0)
    }
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

/// How bright a colour is — Rec. 709 in linear light, which is where those coefficients are
/// defined and where this crate works.
#[must_use]
#[inline]
fn luminance(color: LinearRgba) -> f32 {
    0.2126 * color.red + 0.7152 * color.green + 0.0722 * color.blue
}

/// A colour pulled toward a grey — the age axis, `F-MAT-4`.
///
/// Toward its *own* luminance rather than a fixed one, so desaturating never changes how bright
/// a limb is. That axis is spoken for by the relief, the rings and the cylinder profile, and an
/// age signal that also moved brightness would be competing with all three.
#[must_use]
#[inline]
fn toward(color: LinearRgba, grey: f32, amount: f32) -> LinearRgba {
    if amount <= 0.0 {
        return color;
    }
    let at = amount.min(1.0);
    LinearRgba::new(
        lerp(color.red, grey, at),
        lerp(color.green, grey, at),
        lerp(color.blue, grey, at),
        color.alpha,
    )
}

/// A contributor's colour rescaled to the luminance of the material it is being drawn on.
///
/// **The one operation that lets ownership sit *in* the bark rather than on it.** The author
/// palette is laid out in OKLab for perceptual separation (`AC-MAT-4`), so every entry carries a
/// lightness of its own — and the built-in palette's brightest family sits at OKLab 0.8, which
/// is far above anything a shaded groove is. Mixed in raw, that lightness paints over the
/// relief the surface just spent a warped triangle wave carving: a vein reads as pale paint
/// applied to a photograph of bark.
///
/// Rescaled, the accent contributes **hue and chroma only**. The plates keep their light and the
/// grooves keep their shadow, the ownership reading rides across both, and the two cues stop
/// competing for the one channel they were both using. It costs a divide and three multiplies
/// against the entire rest of the shader, and it is the difference between the mosaic being a
/// property of the surface and being a decal on it.
///
/// Nothing about separation is lost: a rescale is a change in lightness alone, and the palette's
/// guarantee is over hue and chroma at a *shared* lightness — which is exactly the condition
/// this creates.
/// The chroma also rises, and that is the second half of the same argument. `AC-MAT-4` authors
/// the palette for *separation* at three lightnesses, which is a different requirement from
/// surviving a mix; pushing away from grey costs three multiply-adds and cannot weaken the
/// guarantee, since scaling every entry's chroma by one factor scales the distance between any
/// two of them by it too.
///
/// The match is made **after** the boost and to the last decimal, which is what makes this
/// function an invariant rather than an approximation: mixing toward a colour of identical
/// luminance cannot change a texel's luminance at any weight. Everything the bark is — the
/// plates, the grooves, the whorls, the cylinder, the rings, the age — lives in luminance, so
/// this is the guarantee that lets the mosaic be mixed in at a weight that can actually be seen
/// instead of at a weight chosen to avoid damaging the surface.
#[must_use]
#[inline]
fn keyed(color: LinearRgba, to: f32) -> LinearRgba {
    let grey = luminance(color);
    // Clamped at zero because the palette's gamut is OKLab's and a boosted entry near the sRGB
    // edge can name a negative coordinate; clamping desaturates rather than hue-shifts, which
    // is the same treatment `author_color` gives the same problem one step earlier. It is also
    // why the rescale below is measured off the boosted colour rather than folded into it — a
    // clamp moves the luminance, and this function promises it does not.
    let lift = |channel: f32| (grey + (channel - grey) * ACCENT_CHROMA).max(0.0);
    let (red, green, blue) = (lift(color.red), lift(color.green), lift(color.blue));
    let scale = to / (0.2126 * red + 0.7152 * green + 0.0722 * blue).max(1e-4);
    LinearRgba::new(red * scale, green * scale, blue * scale, color.alpha)
}

/// A wave's crest carved into a broad flat plate and a narrow groove, and how steeply it is
/// sloping there.
///
/// `edge` is where the rise stops, as a fraction of the groove-to-crest distance: below it the
/// profile is a smoothstep ramp — the groove wall — and above it the plate is flat. The second
/// return is the ramp's own magnitude, normalized so that the flat parts report zero, which is
/// what the directional relief term needs and what a plain smoothstep does not give.
#[must_use]
#[inline]
fn shoulder(crest: f32, edge: f32) -> (f32, f32) {
    let rise = ((crest * 0.5 + 0.5) / edge).min(1.0);
    (rise * rise * (3.0 - 2.0 * rise), 6.0 * rise * (1.0 - rise))
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
            lineage: 0xb00c,
            knots: [Knot::default(); MAX_KNOTS],
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

    /// A node with nobody drawn on it.
    const UNOWNED: &[(f32, LinearRgba)] = &[];

    /// Sixteen samples down the middle of a limb, as a family's signature.
    fn signature(family: MaterialFamily) -> Vec<f32> {
        let shading = shading(family);
        (0..16)
            .map(|step| {
                let point = at(step as f32 * 0.37, 0.1);
                shade(&shading, base_of(&shading), point, UNOWNED).red
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
    ///
    /// The tolerance is the one the flat surface held, and holding it while the surface carries
    /// relief is the point of taking the plate depth from the flow field rather than from
    /// [`fbm`]: everything structural is octave-independent, and only the grain gets finer.
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
            shade(&coarse, base_of(&coarse), point, UNOWNED).red,
            shade(&fine, base_of(&fine), point, UNOWNED).red,
        );
        // Not equal — a finer band is meant to show more — but the same surface underneath,
        // which is what "gains detail" rather than "re-textures" means.
        assert!(
            (a - b).abs() < 0.09,
            "the coarse and fine bands disagree by {}",
            (a - b).abs()
        );
    }

    /// The structural half of the same claim, stated where it is strongest: the plates do not
    /// move at all between bands. A limb's relief is its relief; only its grain resolves.
    #[test]
    fn the_relief_is_the_same_relief_at_every_band() {
        let mut worst = 0.0f32;
        for step in 0..64 {
            let point = at(step as f32 * 0.11, step as f32 / 32.0 - 1.0);
            let one = Shading {
                octaves: 1,
                surface: Surface {
                    grain: 0.0,
                    ..Surface::of(MaterialFamily::Heartwood)
                },
                ..shading(MaterialFamily::Heartwood)
            };
            let three = Shading { octaves: 3, ..one };
            let drift = (shade(&one, base_of(&one), point, UNOWNED).red
                - shade(&three, base_of(&three), point, UNOWNED).red)
                .abs();
            worst = worst.max(drift);
        }
        // Not exactly zero: the age mottle and the fissure cut still read `fbm`. With the grain
        // silenced, what is left is small enough that no band boundary can show it.
        assert!(worst < 0.02, "the relief moved by {worst} between bands");
    }

    /// `F-MAT-4`'s direction, which inverts against everything else in the material: a large
    /// age is *older*, and older is greyer.
    #[test]
    fn older_material_draws_greyer_than_newer() {
        let young = Shading {
            age: (0.0, 0.0),
            ..shading(MaterialFamily::Heartwood)
        };
        let old = Shading {
            age: (1.0, 1.0),
            ..shading(MaterialFamily::Heartwood)
        };
        let point = at(2.0, 0.0);
        let (fresh, weathered) = (
            shade(&young, base_of(&young), point, UNOWNED),
            shade(&old, base_of(&old), point, UNOWNED),
        );
        assert!(
            chroma(weathered) < chroma(fresh) * 0.5,
            "old material kept its colour: {} against {}",
            chroma(weathered),
            chroma(fresh)
        );
    }

    /// Saturation is the *primary* signal and brightness the secondary one, so both move — but
    /// the one that must not be swamped is the one that reads across a whole crown.
    #[test]
    fn older_material_also_draws_darker() {
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
            luma(shade(&old, base_of(&old), point, UNOWNED))
                < luma(shade(&young, base_of(&young), point, UNOWNED))
        );
    }

    /// How far from grey a colour is — the axis `F-MAT-4` now runs along.
    fn chroma(color: LinearRgba) -> f32 {
        let high = color.red.max(color.green).max(color.blue);
        let low = color.red.min(color.green).min(color.blue);
        high - low
    }

    fn luma(color: LinearRgba) -> f32 {
        0.2126 * color.red + 0.7152 * color.green + 0.0722 * color.blue
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
            chroma(shade(&shading, base_of(&shading), base, UNOWNED))
                < chroma(shade(&shading, base_of(&shading), tip, UNOWNED)),
            "the base is not older than the tip"
        );
    }

    /// Churn is the local modulation on age: a path being rewritten weekly is not old material
    /// however long ago its first commit landed.
    #[test]
    fn churn_freshens_old_material() {
        let still = Shading {
            age: (1.0, 1.0),
            ..shading(MaterialFamily::Heartwood)
        };
        let churning = Shading {
            restless: 1.0,
            ..still
        };
        let point = at(2.0, 0.0);
        assert!(
            chroma(shade(&churning, base_of(&churning), point, UNOWNED))
                > chroma(shade(&still, base_of(&still), point, UNOWNED)),
            "churn did not freshen the age reading"
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
        let (mut differences, mut wildly_different, mut total) = (0, 0, 0);
        for step in 0..64 {
            for lane in 0..4 {
                let point = at(step as f32 * 0.21, lane as f32 * 0.45 - 0.7);
                let (a, b) = (
                    shade(&clear, base_of(&clear), point, UNOWNED).red,
                    shade(&cracked, base_of(&cracked), point, UNOWNED).red,
                );
                total += 1;
                if (a - b).abs() > 1e-4 {
                    differences += 1;
                }
                if (a - b).abs() > 0.5 {
                    wildly_different += 1;
                }
            }
        }
        assert!(differences > 0, "cracking changed nothing");
        assert!(
            differences * 2 < total,
            "cracking marked {differences} of {total} samples — that is a material, not a stress"
        );
        assert_eq!(wildly_different, 0, "a fissure went past CRACK_DEPTH");
    }

    /// `Sparse` is the stress that must not become transparency — see the module header. It is
    /// checked here as the property that replaces it: fewer, larger grains and fewer, broader
    /// plates.
    #[test]
    fn sparse_coarsens_the_grain_rather_than_removing_material() {
        let plain = Surface::of(MaterialFamily::Heartwood);
        let sparse = plain.coarsened(1.0);
        assert!(sparse.period().x > plain.period().x && sparse.period().y > plain.period().y);
        assert!(sparse.grain > plain.grain);
        assert!(sparse.ridges < plain.ridges, "the plates did not coarsen");
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
            knots: [
                Knot {
                    at: Vec2::new(4.0, 0.0),
                    reach: 0.9,
                    depth: 1.0,
                },
                Knot {
                    at: Vec2::new(9.0, -0.4),
                    reach: 0.7,
                    depth: 1.0,
                },
            ],
            ..shading(MaterialFamily::Stone)
        };
        for step in 0..128 {
            let point = LimbPoint {
                along: step as f32 * 0.13,
                across: (step as f32 * 0.07).sin(),
                fraction: step as f32 / 128.0,
            };
            let drawn = shade(&stressed, base_of(&stressed), point, UNOWNED);
            assert_eq!(drawn.alpha, 1.0, "a texel was drawn see-through");
            assert!(drawn.red >= 0.0 && drawn.green >= 0.0 && drawn.blue >= 0.0);
        }
    }

    /// A two-holder run table, laid the way `bake` lays one.
    fn owners() -> Vec<(f32, LinearRgba)> {
        Vec::from([
            (0.5, LinearRgba::rgb(0.9, 0.15, 0.1)),
            (1.0, LinearRgba::rgb(0.1, 0.35, 0.95)),
        ])
    }

    /// `F-MAT-2`: the accent is *over* the material, so the material is still under it. A
    /// replacement would make every cell of a mosaic flat.
    ///
    /// Stated as a comparison against the same texel drawn unowned, rather than as "the blue
    /// channel wins". The channel version is what was here, it held by a hair at the tint's
    /// previous strength, and it was quietly asserting the *wrong* thing: a tint that made the
    /// contributor's hue dominate the family's would be §8.5 backwards. What has to be true is
    /// that ownership moves the colour and the material survives the move.
    ///
    /// The strength varies over a run and that is the design, not slack in the test: a texel is
    /// either in one of [`ACCENT_THREAD`]'s veins or in [`ACCENT_WASH`]'s tint between them. So
    /// the direction is asserted everywhere and the magnitude only where a thread is.
    #[test]
    fn the_mosaic_accent_tints_rather_than_replaces() {
        let shading = shading(MaterialFamily::Heartwood);
        let accent = Vec::from([(1.0, LinearRgba::rgb(0.1, 0.7, 0.9))]);
        let (mut varied, mut threaded) = (0, 0);
        let mut previous: Option<f32> = None;
        for step in 0..64 {
            let point = at(step as f32 * 0.4, step as f32 / 32.0 - 1.0);
            let bare = shade(&shading, base_of(&shading), point, UNOWNED);
            let drawn = shade(&shading, base_of(&shading), point, &accent);
            // Every texel of a holder's run moves toward the contributor's colour...
            assert!(
                drawn.blue > bare.blue && drawn.red < bare.red,
                "the accent pushed the wrong way at {point:?}: {bare:?} became {drawn:?}"
            );
            // ...and on the veins it moves a long way.
            if drawn.blue - bare.blue > 0.04 {
                threaded += 1;
            }
            // ...and the family's texture is still varying under it.
            if let Some(before) = previous
                && (drawn.red - before).abs() > 1e-4
            {
                varied += 1;
            }
            previous = Some(drawn.red);
        }
        assert!(threaded > 8, "only {threaded} of 64 samples carried a vein");
        assert!(varied > 16, "the material stopped varying under the accent");
    }

    /// "An accent *over* the primary material" as arithmetic rather than as taste — the half of
    /// §8.5 a drawing cannot check.
    ///
    /// Whatever the tint is tuned to, and wherever on the relief it lands, a texel has to stay a
    /// minority mix of the contributor's colour over a majority of what the material was. At a
    /// half the two are equal partners; above it the contributor *is* the material, which is the
    /// sentence backwards and is what 0.55 looked like on screen.
    #[test]
    fn ownership_is_a_minority_of_every_texel_it_touches() {
        // Something of the material is in every texel of the tree, threads included, which is
        // what "an accent *over* the primary material" means taken literally. A `const` block,
        // so a ceiling raised to one fails the build rather than a test run.
        const { assert!(ACCENT_CEILING < 1.0) };
        // And between the threads, the material leads by a long way.
        let between = ACCENT_WASH * (1.0 + ACCENT_ON_GREY);
        assert!(
            between < 0.5,
            "the wash out-votes the material it washes: {between}"
        );
        assert!(
            (0.0..=1.0).contains(&ACCENT_FLOOR) && (0.0..=1.0).contains(&THREAD_CUT),
            "a mosaic parameter escaped its range"
        );
    }

    /// The threads are a *minority* of a holder's run — which is what makes them threads rather
    /// than a coat of paint, and what keeps the material visible between them.
    #[test]
    fn a_holder_veins_its_run_rather_than_covering_it() {
        let mut on = 0;
        let mut total = 0;
        for step in 0..4096 {
            // Straight across the comb, which is the axis the threads are laid on.
            let (comb, _) = wave(step as f32 * 0.011);
            total += 1;
            if ((comb - THREAD_CUT) * THREAD_SHARPEN).clamp(0.0, 1.0) > 0.5 {
                on += 1;
            }
        }
        let covered = on as f32 / total as f32;
        assert!(
            (0.1..0.45).contains(&covered),
            "threads cover {covered} of a run — that is not veining"
        );
    }

    /// **The invariant the whole treatment rests on**, and the reason [`MOSAIC_ACCENT`] is
    /// allowed to be as large as it is: ownership changes a texel's *hue* and never its
    /// brightness.
    ///
    /// Everything the surface says other than "which family" is carried in luminance — the
    /// plates, the grooves, the whorls, the cylinder, the rings, the age, the stress. If the
    /// accent moved luminance, then a limb's bark would be flatter where somebody owned it, an
    /// unowned remainder would read as a change in relief, and `AC-MAT-4`'s brightest palette
    /// entry would erase more of the material than its darkest. It does not, by construction —
    /// [`keyed`] matches the target exactly and a mix of two equal luminances is that
    /// luminance — and this is the test that keeps it that way.
    #[test]
    fn the_accent_changes_hue_and_not_brightness() {
        let table = owners();
        let mut worst = 0.0f32;
        for family in MaterialFamily::ALL {
            let shading = Shading {
                age: (0.8, 0.1),
                ..shading(family)
            };
            for step in 0..96 {
                let point = LimbPoint {
                    along: step as f32 * 0.29,
                    across: (step % 17) as f32 / 8.0 - 1.0,
                    fraction: step as f32 / 96.0,
                };
                let bare = luma(shade(&shading, base_of(&shading), point, UNOWNED));
                let owned = luma(shade(&shading, base_of(&shading), point, &table));
                worst = worst.max((owned - bare).abs() / bare.max(1e-3));
            }
        }
        assert!(
            worst < 0.02,
            "ownership moved a texel's brightness by {:.1}%",
            worst * 100.0
        );
    }

    /// Which holder a column of texels reads as, across the limb at one fraction along it.
    fn column(shading: &Shading, table: &[(f32, LinearRgba)], fraction: f32) -> Vec<bool> {
        (0..48)
            .map(|lane| {
                let point = LimbPoint {
                    along: fraction * 24.0,
                    across: lane as f32 / 24.0 - 1.0,
                    fraction,
                };
                let drawn = shade(shading, base_of(shading), point, table);
                // The first holder is red and the second blue, so which one won is a comparison
                // rather than a match against a colour the lighting has already moved.
                drawn.red > drawn.blue
            })
            .collect()
    }

    /// **The slice's central claim.** A mosaic boundary is no longer a cut across the limb: at
    /// a fraction near where two holders meet, a column of texels contains both of them.
    ///
    /// Before this, `run_at(accents, fraction)` was a step function of one scalar, so every
    /// texel in a column had the same answer by construction and the mosaic drew as vertical
    /// bands. This is that defect stated as a test rather than as a screenshot.
    #[test]
    fn two_holders_interleave_across_the_limb_rather_than_meeting_at_a_line() {
        let shading = shading(MaterialFamily::Heartwood);
        let table = owners();
        let mut mixed = 0;
        for step in 0..12 {
            let fraction = 0.38 + step as f32 * 0.02;
            let lanes = column(&shading, &table, fraction);
            if lanes.iter().any(|first| *first) && lanes.iter().any(|first| !*first) {
                mixed += 1;
            }
        }
        assert!(
            mixed >= 6,
            "only {mixed} of 12 columns near the boundary held both holders"
        );
    }

    /// The chronology in [`WEAVE_BASE`], as the property it exists for: the sequence starts
    /// crisply and grows more interpenetrated the further along the limb it gets.
    #[test]
    fn the_weave_starts_tight_and_opens_tip_ward() {
        let shading = shading(MaterialFamily::Heartwood);
        let table = owners();
        let purity = |fraction: f32| {
            let lanes = column(&shading, &table, fraction);
            let first = lanes.iter().filter(|holder| **holder).count();
            first.max(lanes.len() - first) as f32 / lanes.len() as f32
        };
        assert!(
            purity(0.02) > purity(0.98),
            "the base is no crisper than the tip: {} against {}",
            purity(0.02),
            purity(0.98)
        );
    }

    /// A knot is a whorl, not a dent: it moves the grain around it, over a region, smoothly.
    #[test]
    fn a_knot_bends_the_grain_around_itself() {
        let knot = Knot {
            at: Vec2::new(3.0, 0.1),
            reach: 0.8,
            depth: 0.7,
        };
        let (inside, core) = knot.bend(Vec2::new(3.4, 0.3));
        assert!(inside.length() > 0.01, "the knot bent nothing");
        assert!(core > 0.0, "the knot has no core");

        // Nothing outside the reach, and nothing discontinuous at the rim.
        let (outside, none) = knot.bend(Vec2::new(3.0, 1.6));
        assert_eq!(outside, Vec2::ZERO);
        assert_eq!(none, 0.0);
        let rim = knot.bend(Vec2::new(3.0, 0.1 + 0.795)).0;
        assert!(rim.length() < 0.02, "the influence steps at the rim: {rim}");

        // An empty slot costs a comparison and draws nothing.
        assert_eq!(Knot::default().bend(Vec2::new(0.0, 0.0)), (Vec2::ZERO, 0.0));
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
                if (shade(&veined, base_of(&veined), point, UNOWNED).red
                    - shade(&plain, base_of(&plain), point, UNOWNED).red)
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
                relief: 0.0,
                ..Surface::of(MaterialFamily::Heartwood)
            },
            ..shading(MaterialFamily::Heartwood)
        };
        let middle = shade(&shading, base_of(&shading), at(1.0, 0.0), UNOWNED).red;
        let edge = shade(&shading, base_of(&shading), at(1.0, 0.98), UNOWNED).red;
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
            let pair = noise2(point, 0xabcd);
            assert!(
                (-1.0..=1.0).contains(&pair.x) && (-1.0..=1.0).contains(&pair.y),
                "noise2 left its range at {point:?}: {pair}"
            );
        }
    }

    /// The two channels of one lattice sample have to be *independent*, or the domain warp is a
    /// diagonal slide rather than a bend. Checked as a correlation over the field, because two
    /// channels could differ at a point and still track each other everywhere.
    #[test]
    fn the_two_noise_channels_are_independent() {
        let (mut sum, mut squares) = (0.0f32, 0.0f32);
        for step in 0..1024 {
            let point = Vec2::new(step as f32 * 0.31, (step % 37) as f32 * 0.43);
            let pair = noise2(point, 0x1234);
            sum += pair.x * pair.y;
            squares += pair.x * pair.x + pair.y * pair.y;
        }
        let correlation = 2.0 * sum / squares.max(1e-6);
        assert!(
            correlation.abs() < 0.1,
            "the flow field's two channels correlate at {correlation}"
        );
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

    /// The plates are lit by their own slope, so the wave has to report one — and report it
    /// correctly, because a sign error here lights every groove from inside.
    #[test]
    fn the_plate_wave_reports_the_slope_it_actually_has() {
        for step in 0..64 {
            let x = step as f32 * 0.037;
            let (value, slope) = wave(x);
            let (ahead, _) = wave(x + 1e-3);
            assert!(
                (ahead - value).signum() == slope || (ahead - value).abs() < 1e-6,
                "at {x} the wave moves {} and reports {slope}",
                ahead - value
            );
            assert!((-1.0..=1.0).contains(&value));
        }
    }
}
