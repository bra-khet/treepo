//! Turtle interpretation — `F-SKEL-6`, and where Phase 0's determinism work is spent.
//!
//! §2.1: "The string produced by the L-system is interpreted by a turtle that carries
//! position, orientation, and current thickness." Keeping that a separate pass from
//! [`grammar`](super::grammar) is not ceremony — it is what lets the productions be tested as
//! a string and the geometry be tested as coordinates, and it is the seam a 3-D turtle would
//! arrive at without touching the productions at all.
//!
//! # Every sine in treepo comes from here, and none of them from `libm`
//!
//! `F-SKEL-6` exists because `sin` and `cos` are not guaranteed bit-identical across platform
//! maths libraries, and a turtle interpreter is built on them — so `RISK-2` was that
//! `AC-DET-2` would fail late and inexplicably. [`treepo_det::trig`] is the answer: a
//! 1025-entry table with deterministic interpolation, verified byte-identical on Windows,
//! macOS and Linux in Phase 0.
//!
//! This module is the entire reason that work was done first. It is also the only place in
//! the product that calls a trigonometric function at all.
//!
//! # The heading convention
//!
//! Zero is straight up, increasing clockwise: `QUARTER` is right, `HALF` is down. A tree's
//! natural direction is therefore the zero of the coordinate system, and a limb's angle from
//! vertical is read directly rather than derived — which matters because that angle is
//! exactly the quantity `E3`'s droop is proportional to.
//!
//! So `x` advances with `sin(heading)` and `y` with `cos(heading)`, which is the transpose of
//! the usual screen-space convention and is worth stating once rather than rediscovering.
//!
//! # Tropism: one line, and it is the physics
//!
//! `E3` asks for size-driven droop on heavy limbs. After each segment the heading is bent by
//! `droop × sin(heading)`, and that single expression is correct in all four quadrants
//! without a branch:
//!
//! * pointing up, `sin` is zero — a vertical limb does not sag;
//! * pointing right, `sin` is positive — the heading rotates further clockwise, downward;
//! * pointing left, `sin` is negative — it rotates anticlockwise, which is also downward;
//! * pointing down, `sin` is zero again — a hanging limb has nowhere further to fall.
//!
//! The magnitude is largest at the horizontal, which is where a real limb's bending moment is
//! largest. That it falls out of the sign convention rather than needing a lookup is the
//! reason the convention was chosen.

use super::grammar::Module;
use alloc::vec::Vec;
use treepo_det::{Angle, Fx, sin_cos};
use treepo_model::{NodeId, Point, Segment};

/// An attachment site left by the derivation — where composition may hang a child.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tip {
    /// Where the branch ended.
    pub position: Point,
    /// The direction it was travelling in.
    pub heading: Angle,
    /// The width it had tapered to.
    pub width: Fx,
    /// How many generations deep in this limb's derivation the site sits.
    pub generation: u8,
}

/// What one limb's interpretation produced.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Interpretation {
    /// The drawn geometry.
    pub segments: Vec<Segment>,
    /// The attachment sites, in derivation order.
    ///
    /// Order is deterministic and meaningful: it is depth-first, left before right, so the
    /// same child always lands on the same site of the same limb. Composition relies on that
    /// — a stable site order is what keeps a Grow from reshuffling a limb's children when
    /// one of them changes size.
    pub tips: Vec<Tip>,
}

/// Where a limb's interpretation starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Start {
    /// The point the first segment leaves.
    pub position: Point,
    /// The direction it leaves in — zero is up, increasing clockwise.
    pub heading: Angle,
    /// The node these segments will be attributed to.
    pub node: NodeId,
}

/// One turtle state, as pushed and popped by `[` and `]`.
#[derive(Debug, Clone, Copy)]
struct State {
    position: Point,
    heading: Angle,
    width: Fx,
    generation: u8,
    /// The segment this branch is growing from, if any.
    ///
    /// Carried in the turtle state rather than searched for afterwards: a fork's children
    /// both continue from the segment that was current when `[` was pushed, so the stack
    /// already knows the answer. Reconstructing it later means matching coordinates, which
    /// is quadratic and — worse — cannot tell a genuine joint from two segments that happen
    /// to meet.
    parent: Option<usize>,
}

/// Interprets a module string into segments and attachment sites.
///
/// `droop` is `E3`'s tropism, applied after each segment as described in the module header.
#[must_use]
pub fn interpret(modules: &[Module], start: Start, droop: Angle) -> Interpretation {
    let mut result = Interpretation::default();
    let mut state = State {
        position: start.position,
        heading: start.heading,
        width: Fx::ZERO,
        generation: 0,
        parent: None,
    };
    let mut stack: Vec<State> = Vec::new();
    // The widest thing leaving each segment's far end, or `None` where nothing does.
    // Parallel to `segments` rather than a field on it, so `Segment` carries no sentinel a
    // consumer could misread as a real width.
    let mut widest_child: Vec<Option<Fx>> = Vec::new();

    for module in modules {
        match *module {
            Module::Width(width) => state.width = width,
            Module::Yaw(delta) => state.heading += delta,
            Module::Forward(length) => {
                let end = advance(state.position, state.heading, length);
                let index = result.segments.len();
                result.segments.push(Segment {
                    start: state.position,
                    end,
                    base_width: state.width,
                    tip_width: state.width,
                    node: start.node,
                    generation: state.generation,
                });
                widest_child.push(None);
                // A segment tapers to whatever leaves it, so the joint stays covered and a
                // limb reads as a continuous cone rather than a stack of cylinders that step
                // visibly at every fork. Where two children leave one joint, the wider wins.
                if let Some(parent) = state.parent {
                    let widest = widest_child[parent].map_or(state.width, |w| w.max(state.width));
                    widest_child[parent] = Some(widest);
                }
                state.position = end;
                state.heading = bend(state.heading, droop);
                state.parent = Some(index);
            }
            Module::Push => {
                stack.push(state);
                state.generation = state.generation.saturating_add(1);
            }
            Module::Pop => {
                // A balanced string cannot underflow, and `grammar` has a test that says so.
                // Ignoring an unbalanced pop rather than panicking keeps a hand-written
                // string from being able to abort a Grow.
                if let Some(previous) = stack.pop() {
                    state = previous;
                }
            }
            Module::Tip => result.tips.push(Tip {
                position: state.position,
                heading: state.heading,
                width: state.width,
                generation: state.generation,
            }),
        }
    }

    for (segment, widest) in result.segments.iter_mut().zip(widest_child) {
        if let Some(width) = widest {
            segment.tip_width = width;
        }
    }
    result
}

/// Moves `length` along `heading` from `from`.
fn advance(from: Point, heading: Angle, length: Fx) -> Point {
    let (sin, cos) = sin_cos(heading);
    Point::new(from.x.add(sin.mul(length)), from.y.add(cos.mul(length)))
}

/// Applies tropism — see the module header for why this is one expression and not four.
fn bend(heading: Angle, droop: Angle) -> Angle {
    if droop == Angle::ZERO {
        return heading;
    }
    let (sin, _) = sin_cos(heading);
    // i128 so no table value can overflow the product; the shift undoes `Fx`'s Q32.32
    // scaling and the cast wraps, which is what a binary angle's negative already is.
    let scaled = (i128::from(droop.to_bits()) * i128::from(sin.to_bits())) >> 32;
    heading + Angle::from_bits(scaled as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsystem::grammar;
    use crate::params::{LimbParams, Table};
    use treepo_det::Seed;

    fn start() -> Start {
        Start {
            position: Point::ORIGIN,
            heading: Angle::ZERO,
            node: NodeId::new(0),
        }
    }

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

    /// The convention, asserted rather than described: zero is up, a quarter turn is right.
    #[test]
    fn heading_zero_is_straight_up_and_a_quarter_turn_is_right() {
        let modules = [Module::Width(Fx::ONE), Module::Forward(Fx::from_int(1))];

        let up = interpret(&modules, start(), Angle::ZERO);
        let end = up.segments[0].end;
        assert_eq!(end.x, Fx::ZERO);
        assert_eq!(end.y, Fx::ONE);

        let right = interpret(
            &modules,
            Start {
                heading: Angle::QUARTER,
                ..start()
            },
            Angle::ZERO,
        );
        let end = right.segments[0].end;
        assert_eq!(end.x, Fx::ONE);
        assert_eq!(end.y, Fx::ZERO);
    }

    /// `E3` in all four quadrants, which is the whole claim the one-line tropism makes.
    #[test]
    fn droop_pulls_every_heading_downward_and_leaves_the_vertical_alone() {
        let droop = Angle::from_millidegrees(10_000);

        // Straight up and straight down are fixed points.
        assert_eq!(bend(Angle::ZERO, droop), Angle::ZERO);
        assert_eq!(bend(Angle::HALF, droop), Angle::HALF);

        // Pointing right, the heading turns further clockwise — toward the ground.
        let right = Angle::QUARTER;
        assert!(bend(right, droop).to_bits() > right.to_bits());

        // Pointing left, it turns anticlockwise — also toward the ground.
        let left = Angle::THREE_QUARTER;
        assert!(bend(left, droop).to_bits() < left.to_bits());

        // And the horizontal sags hardest, which is where the bending moment is largest.
        let diagonal = Angle::from_millidegrees(45_000);
        let horizontal_sag = bend(right, droop).to_bits() - right.to_bits();
        let diagonal_sag = bend(diagonal, droop).to_bits() - diagonal.to_bits();
        assert!(horizontal_sag > diagonal_sag);
    }

    #[test]
    fn no_droop_leaves_a_limb_perfectly_straight() {
        let modules = [
            Module::Width(Fx::ONE),
            Module::Forward(Fx::ONE),
            Module::Forward(Fx::ONE),
            Module::Forward(Fx::ONE),
        ];
        let straight = interpret(&modules, start(), Angle::ZERO);
        for segment in &straight.segments {
            assert_eq!(segment.start.x, Fx::ZERO);
            assert_eq!(segment.end.x, Fx::ZERO);
        }
    }

    /// A pop must restore position, heading and width together, or a limb's second branch
    /// starts wherever its first one finished.
    #[test]
    fn a_pop_restores_the_whole_turtle_state() {
        let modules = [
            Module::Width(Fx::ONE),
            Module::Forward(Fx::ONE),
            Module::Push,
            Module::Yaw(Angle::QUARTER),
            Module::Width(Fx::HALF),
            Module::Forward(Fx::ONE),
            Module::Tip,
            Module::Pop,
            Module::Push,
            Module::Yaw(-Angle::QUARTER),
            Module::Width(Fx::HALF),
            Module::Forward(Fx::ONE),
            Module::Tip,
            Module::Pop,
        ];

        let result = interpret(&modules, start(), Angle::ZERO);
        assert_eq!(result.tips.len(), 2);
        // Both branches left the same fork, going opposite ways.
        assert_eq!(result.tips[0].position.y, result.tips[1].position.y);
        assert_eq!(result.tips[0].position.x, -result.tips[1].position.x);
    }

    #[test]
    fn an_unbalanced_pop_is_ignored_rather_than_fatal() {
        let modules = [
            Module::Pop,
            Module::Width(Fx::ONE),
            Module::Forward(Fx::ONE),
        ];
        let result = interpret(&modules, start(), Angle::ZERO);
        assert_eq!(result.segments.len(), 1);
    }

    /// Segments meet: a fork's parent tapers to whatever leaves it, so no joint shows a step.
    #[test]
    fn a_limb_tapers_continuously_through_its_joints() {
        let modules = grammar::derive(
            &params(),
            Table::built_in().min_length(),
            &Seed::root(b"taper"),
            4,
        );
        let result = interpret(&modules, start(), Angle::ZERO);

        for segment in &result.segments {
            let children: Vec<&Segment> = result
                .segments
                .iter()
                .filter(|other| other.start == segment.end)
                .collect();
            if let Some(widest) = children.iter().map(|c| c.base_width).max() {
                assert_eq!(
                    segment.tip_width, widest,
                    "a joint stepped from {:?} to {widest:?}",
                    segment.tip_width
                );
            }
        }
    }

    /// `F-SKEL-6`, and the point of the whole crate: interpretation is a pure function.
    #[test]
    fn interpretation_is_reproducible() {
        let modules = grammar::derive(
            &params(),
            Table::built_in().min_length(),
            &Seed::root(b"repro"),
            8,
        );
        let droop = Angle::from_millidegrees(6_000);
        assert_eq!(
            interpret(&modules, start(), droop),
            interpret(&modules, start(), droop)
        );
    }

    /// Every module string the grammar produces yields at least one tip, whatever it looks
    /// like — composition has nowhere to hang a child otherwise.
    #[test]
    fn every_derivation_yields_somewhere_to_attach() {
        for sites in [1u16, 2, 4, 7, 16] {
            let modules = grammar::derive(
                &params(),
                Table::built_in().min_length(),
                &Seed::root(b"attach"),
                sites,
            );
            let result = interpret(&modules, start(), Angle::ZERO);
            assert!(!result.tips.is_empty());
            assert_eq!(
                result.tips.len(),
                modules.iter().filter(|m| **m == Module::Tip).count()
            );
        }
    }
}
