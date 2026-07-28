//! The productions — parametric and stochastic, and deliberately low-arity.
//!
//! `design/l-system-parameterization.md` §2.2 gives the classic form this implements:
//!
//! ```text
//! A(s, w) : s >= min  →  !(w) F(s) [+(θ1) A(s·r1, w·q^e)] [-(θ2) A(s·r2, w·(1-q)^e)]
//! ```
//!
//! # Why arity two, when a directory has forty children
//!
//! Because the two questions are separate, and conflating them is what makes procedural
//! trees look like org charts. A node's *children* are a fact about the repository; a node's
//! *branching* is a fact about how a limb divides. `compose` decides how many attachment
//! sites a limb needs and this module produces a limb that has them, by binary division —
//! which is what a tree does. Fifteen children on one fan of fifteen branches reads as a
//! diagram. Fifteen children distributed across a limb that forked four times reads as a
//! branch.
//!
//! So the derivation depth, not the arity, absorbs the child count: `n` generations yield
//! `2^n` sites. That is also the mechanism behind the composition threshold — the sites a
//! limb can offer are bounded by `A3`'s recursion cap, and a limb asked for more than it can
//! carry is exactly the condition [`compose`](super::compose) turns into aggregation.
//!
//! # Stochastic, never random
//!
//! §2.3 wants stochastic productions, and §8 requires the draws to come from a seeded RNG
//! derived from the path hash. The [`ChaCha8Rng`] here is constructed from the limb's
//! [`Seed`] and from nothing else — there is no way to build one from entropy or a clock, so
//! "stochastic" cannot become "different between two runs" (`N3`, `AC-DET-1`).
//!
//! The draws are made in a fixed order — left angle, right angle, left length, right length,
//! per generation, depth-first — because the RNG is a stream and the order it is consumed in
//! *is* part of the output. Reordering these two lines would reshape every tree treepo draws.

use crate::params::LimbParams;
use alloc::vec::Vec;
use treepo_det::{Angle, ChaCha8Rng, Fx, Seed};

/// One symbol of the module string a limb derives to.
///
/// The standard turtle alphabet from §2.1, narrowed to the 2-D subset the initial skeleton
/// needs, plus [`Tip`](Module::Tip) — which is not a drawing instruction but the mark
/// composition hangs children on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Module {
    /// `F(s)` — move forward `s` and draw a segment.
    Forward(Fx),
    /// `+(θ)` / `-(θ)` — yaw by a signed delta.
    ///
    /// One variant rather than two, because [`Angle`] wraps exactly at a full turn: turning
    /// left by θ is adding the wrapped negative of θ, and the turtle needs no sign handling
    /// for it. That is the same property that makes rotation drift-free.
    Yaw(Angle),
    /// `!(w)` — set the current line width.
    Width(Fx),
    /// `[` — push turtle state.
    Push,
    /// `]` — pop turtle state.
    Pop,
    /// An attachment site: where composition may hang a child limb or a container.
    Tip,
}

/// How many generations are needed to offer at least `sites` attachment points.
///
/// `ceil(log2(sites))`, since each generation doubles. Zero sites and one site both need no
/// branching at all — a limb with a single child is a limb, not a fork.
#[must_use]
pub fn generations_for(sites: u16, cap: u8) -> u8 {
    let mut generations = 0u8;
    let mut available = 1u32;
    while available < u32::from(sites) && generations < cap {
        available *= 2;
        generations += 1;
    }
    generations
}

/// How many attachment sites `generations` of binary division yields.
#[must_use]
pub const fn sites_for(generations: u8) -> u16 {
    // A3 caps generations at 5, so this cannot approach the shift width; the guard is here
    // because a caller passing an unclamped value should get a saturated answer rather than
    // undefined behaviour from an over-wide shift.
    if generations >= 15 {
        return 1 << 15;
    }
    1 << generations
}

/// Derives a limb's module string.
///
/// `sites` is how many attachment points the caller needs; the derivation runs
/// `ceil(log2(sites))` generations, capped by
/// [`LimbParams::recursion_depth`](crate::params::LimbParams::recursion_depth) — which is
/// `A3`, and is what makes a limb able to refuse more children than it can legibly carry.
///
/// A branch also stops early once its segment would fall below `min_length`, which is §3's
/// termination threshold. The caller therefore treats the tip count as an *outcome* rather
/// than a request, and [`super::turtle`] reports what was actually produced.
#[must_use]
pub fn derive(params: &LimbParams, min_length: Fx, seed: &Seed, sites: u16) -> Vec<Module> {
    let generations = generations_for(sites, params.recursion_depth);
    let mut rng = ChaCha8Rng::from_seed(*seed.as_bytes());
    let mut modules = Vec::new();
    expand(
        &mut modules,
        params,
        min_length,
        &mut rng,
        params.base_length,
        params.base_width,
        generations,
    );
    modules
}

/// One application of the production, depth-first.
fn expand(
    out: &mut Vec<Module>,
    params: &LimbParams,
    min_length: Fx,
    rng: &mut ChaCha8Rng,
    length: Fx,
    width: Fx,
    remaining: u8,
) {
    out.push(Module::Width(width));
    out.push(Module::Forward(length));

    // §2.2's `s >= min` guard, and `A3`'s generation count. Either one ending the branch
    // leaves an attachment site, so nothing is ever lost by stopping — the site is where
    // composition puts whatever this branch would have carried.
    if remaining == 0 || length.mul(params.length_ratio) < min_length {
        out.push(Module::Tip);
        return;
    }

    // Draw order is part of the output — see the module header. Both children's draws are
    // taken before either recurses, so a subtree's shape does not depend on how deep its
    // sibling went.
    let left_angle = jittered_angle(rng, params, Turn::Left);
    let right_angle = jittered_angle(rng, params, Turn::Right);
    let left_length = jittered_length(rng, params, length);
    let right_length = jittered_length(rng, params, length);

    for (angle, child_length) in [(left_angle, left_length), (right_angle, right_length)] {
        out.push(Module::Push);
        out.push(Module::Yaw(angle));
        expand(
            out,
            params,
            min_length,
            rng,
            child_length,
            width.mul(params.width_ratio),
            remaining - 1,
        );
        out.push(Module::Pop);
    }
}

/// Which side of the parent a child leaves on.
#[derive(Debug, Clone, Copy)]
enum Turn {
    Left,
    Right,
}

/// The branching angle for one child, jittered.
///
/// Left is the wrapped negative of right, so a zero-jitter limb is exactly symmetric — the
/// "orderly, near-symmetric silhouette" `AC-SKEL-1` asks a clean repository for is the
/// *absence* of noise here rather than a separate code path.
fn jittered_angle(rng: &mut ChaCha8Rng, params: &LimbParams, turn: Turn) -> Angle {
    let jitter = signed_angle(rng, params.angle_jitter);
    let magnitude = params.branch_angle.to_bits().wrapping_add(jitter.to_bits());
    match turn {
        Turn::Right => Angle::from_bits(magnitude),
        Turn::Left => -Angle::from_bits(magnitude),
    }
}

/// A child's length, jittered around the parent's length times the falloff ratio.
fn jittered_length(rng: &mut ChaCha8Rng, params: &LimbParams, parent: Fx) -> Fx {
    let base = parent.mul(params.length_ratio);
    let jitter = rng.signed_unit_fx().mul(params.length_jitter);
    // `1 + jitter`, so zero jitter is exactly the falloff ratio and nothing is scaled by a
    // near-zero factor when the noise happens to draw low.
    base.mul(Fx::ONE.add(jitter))
}

/// A uniform draw in `-spread..=spread`.
fn signed_angle(rng: &mut ChaCha8Rng, spread: Angle) -> Angle {
    if spread == Angle::ZERO {
        return Angle::ZERO;
    }
    let unit = rng.signed_unit_fx();
    // i128 so the product cannot overflow whatever a future table permits; the shift undoes
    // `Fx`'s Q32.32 scaling, and the cast back wraps, which is exactly right for a binary
    // angle whose negative *is* its wrapped complement.
    let scaled = (i128::from(spread.to_bits()) * i128::from(unit.to_bits())) >> 32;
    Angle::from_bits(scaled as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Table;

    fn params() -> LimbParams {
        LimbParams {
            recursion_depth: 3,
            branch_angle: Angle::from_millidegrees(30_000),
            angle_jitter: Angle::ZERO,
            length_ratio: Fx::from_ratio(7, 10),
            width_ratio: Fx::from_ratio(9, 10),
            length_jitter: Fx::ZERO,
            droop: Angle::ZERO,
            base_length: Fx::from_int(1),
            base_width: Fx::from_ratio(2, 10),
            branch_capacity: 4,
        }
    }

    fn seed() -> Seed {
        Seed::root(b"grammar-test")
    }

    fn tips(modules: &[Module]) -> usize {
        modules.iter().filter(|m| **m == Module::Tip).count()
    }

    #[test]
    fn generations_double_the_available_sites() {
        assert_eq!(generations_for(0, 5), 0);
        assert_eq!(generations_for(1, 5), 0);
        assert_eq!(generations_for(2, 5), 1);
        assert_eq!(generations_for(3, 5), 2);
        assert_eq!(generations_for(4, 5), 2);
        assert_eq!(generations_for(9, 5), 4);
        assert_eq!(sites_for(4), 16);
        // A3's cap is the ceiling, and asking for more does not raise it.
        assert_eq!(generations_for(1_000, 5), 5);
    }

    #[test]
    fn a_derivation_offers_the_sites_it_was_asked_for() {
        for requested in [1u16, 2, 3, 5, 8] {
            let modules = derive(&params(), Fx::from_ratio(1, 1000), &seed(), requested);
            assert!(
                tips(&modules) >= usize::from(requested),
                "asked for {requested} sites, got {}",
                tips(&modules)
            );
        }
    }

    /// `F-SKEL-1`: the same inputs produce the same string, every time.
    #[test]
    fn derivation_is_a_pure_function_of_its_inputs() {
        let table = Table::built_in();
        let mut noisy = params();
        noisy.angle_jitter = Angle::from_millidegrees(9_000);
        noisy.length_jitter = Fx::from_ratio(3, 10);

        let once = derive(&noisy, table.min_length(), &seed(), 8);
        let again = derive(&noisy, table.min_length(), &seed(), 8);
        assert_eq!(once, again, "a seeded derivation must be reproducible");

        let elsewhere = derive(&noisy, table.min_length(), &Seed::root(b"other"), 8);
        assert_ne!(
            once, elsewhere,
            "a different seed must produce a different limb"
        );
    }

    /// With no noise the two sides of every fork must mirror exactly. This is what makes a
    /// clean repository read as orderly rather than as merely quieter.
    #[test]
    fn a_noiseless_limb_is_exactly_symmetric() {
        let modules = derive(&params(), Fx::from_ratio(1, 1000), &seed(), 2);
        let yaws: Vec<Angle> = modules
            .iter()
            .filter_map(|m| match m {
                Module::Yaw(angle) => Some(*angle),
                _ => None,
            })
            .collect();

        assert_eq!(yaws.len(), 2);
        assert_eq!(
            yaws[0].to_bits().wrapping_add(yaws[1].to_bits()),
            0,
            "left and right must be exact negatives: {:?}",
            yaws
        );
    }

    /// The termination threshold has to bite before the generation cap does, or §3's
    /// "prevents microscopic branching" is not doing anything.
    #[test]
    fn a_branch_below_the_minimum_length_stops_early() {
        let mut short = params();
        short.recursion_depth = 5;
        short.base_length = Fx::from_ratio(1, 100);

        let generous = derive(&short, Fx::from_ratio(1, 100_000), &seed(), 32);
        let strict = derive(&short, Fx::from_ratio(1, 100), &seed(), 32);

        assert!(
            tips(&strict) < tips(&generous),
            "a strict threshold must terminate branches early: {} vs {}",
            tips(&strict),
            tips(&generous)
        );
        assert_eq!(
            tips(&strict),
            1,
            "nothing should branch below the threshold"
        );
    }

    /// Every push is matched, or the turtle's stack underflows or leaks.
    #[test]
    fn brackets_balance() {
        let modules = derive(&params(), Fx::from_ratio(1, 1000), &seed(), 16);
        let mut depth = 0i32;
        for module in &modules {
            match module {
                Module::Push => depth += 1,
                Module::Pop => depth -= 1,
                _ => {}
            }
            assert!(depth >= 0, "a pop preceded its push");
        }
        assert_eq!(depth, 0, "every push must be popped");
    }

    /// Widths taper monotonically down any single path through the limb.
    #[test]
    fn width_never_grows_toward_the_tips() {
        let modules = derive(&params(), Fx::from_ratio(1, 1000), &seed(), 8);
        let mut stack = alloc::vec![Fx::MAX];
        for module in &modules {
            match module {
                Module::Width(w) => {
                    let current = stack.last_mut().unwrap();
                    assert!(*w <= *current, "width grew from {current:?} to {w:?}");
                    *current = *w;
                }
                Module::Push => stack.push(*stack.last().unwrap()),
                Module::Pop => {
                    stack.pop();
                }
                _ => {}
            }
        }
    }
}
