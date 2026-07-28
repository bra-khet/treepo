//! An integer rasterizer for tapered, round-capped strokes.
//!
//! # Why this is integer-only when nothing forces it to be
//!
//! `N3` binds the crates under `crates/`, and this tool is not one of them: it may use floats
//! and the workspace manifest says the render layer eventually will. It does not, because a
//! float-free rasterizer buys something the milestone actually wants.
//!
//! `AC-DET-2` asks for identical output on Windows, macOS and Linux. Hashing the skeleton
//! proves that for the *numbers*. Hashing the PNG proves it for the numbers **and** every
//! step between them, and it is a check a human can run without a debugger: same file, same
//! bytes, three machines. That check is only worth anything if the drawing itself cannot
//! drift, so the drawing is `i64` and `i128` from end to end.
//!
//! # How a stroke is drawn
//!
//! One distance computation per pixel, not a supersample grid. For a point `p` and a segment
//! `a → b` with radii `ra → rb`, find the nearest point on the segment, take the distance to
//! it, and compare against the radius interpolated to the same position. Coverage is a linear
//! ramp one pixel wide across that boundary, which is what produces the anti-aliased edge.
//!
//! It is not an exact area integral — a true tapered capsule's silhouette bulges slightly
//! where the radius changes — and at these thicknesses the difference is under a pixel. What
//! it buys is a rasterizer that is a dozen lines of arithmetic with no sampling pattern to
//! get wrong.
//!
//! # Units
//!
//! Everything a caller passes is in **sub-units**: [`SUB`] of them to the pixel. Positions
//! are the pixel grid scaled up, so `(SUB/2, SUB/2)` is the centre of the top-left pixel.
//! Fractional geometry is the whole point — a skeleton whose limbs snapped to pixel corners
//! would read as far more orderly than it is.

/// Sub-units per pixel. A power of two so the pixel a sub-unit falls in is a shift.
pub(crate) const SUB: i64 = 256;

/// Half a pixel, in sub-units. The anti-aliasing ramp runs from `-HALF` to `+HALF` of the
/// stroke boundary, so an edge is exactly one pixel wide.
const HALF: i64 = SUB / 2;

/// Coverage levels per ink family, minus one. 64 levels × 4 families is exactly 256 palette
/// entries, which is what an 8-bit indexed PNG has.
pub(crate) const LEVELS: i64 = 63;

/// How many ink families the palette is divided into.
pub(crate) const FAMILIES: usize = 4;

/// A drawing surface holding, per pixel, how covered it is and by which family.
///
/// Two parallel planes rather than one blended colour, because blending is the palette's job
/// and doing it here would mean deciding what happens where a limb crosses a container. This
/// way that decision is one comparison in [`Canvas::stroke`] and can be read in one place.
#[derive(Debug)]
pub(crate) struct Canvas {
    width: usize,
    height: usize,
    cover: Vec<u8>,
    family: Vec<u8>,
}

impl Canvas {
    /// A blank canvas: no coverage anywhere.
    #[must_use]
    pub(crate) fn new(width: u32, height: u32) -> Self {
        let (width, height) = (width as usize, height as usize);
        Self {
            width,
            height,
            cover: vec![0; width * height],
            family: vec![0; width * height],
        }
    }

    /// The canvas width in pixels.
    #[must_use]
    pub(crate) const fn width(&self) -> u32 {
        self.width as u32
    }

    /// The canvas height in pixels.
    #[must_use]
    pub(crate) const fn height(&self) -> u32 {
        self.height as u32
    }

    /// Draws one tapered stroke from `a` to `b`, in sub-units.
    ///
    /// Where strokes overlap the most-covered one shows, and ties go to whichever was drawn
    /// last. Both halves of that rule matter: max-coverage is what makes the hybrid trunk
    /// appear at all — it is nothing but overlapping primary limbs, and any rule other than
    /// union would carve seams through it — while the tie-break is what lets a caller give a
    /// family visual priority by drawing it later.
    pub(crate) fn stroke(&mut self, a: (i64, i64), b: (i64, i64), ra: i64, rb: i64, family: u8) {
        debug_assert!(
            usize::from(family) < FAMILIES,
            "family {family} is outside the palette"
        );

        // The furthest a covered pixel's centre can be from the stroke's own extent.
        let reach = ra.max(rb) + HALF;
        let Some((x0, x1)) = self.span(a.0.min(b.0) - reach, a.0.max(b.0) + reach, self.width)
        else {
            return;
        };
        let Some((y0, y1)) = self.span(a.1.min(b.1) - reach, a.1.max(b.1) + reach, self.height)
        else {
            return;
        };

        let dx = b.0 - a.0;
        let dy = b.1 - a.1;
        let length_sq = dx * dx + dy * dy;

        for py in y0..=y1 {
            let row = py * self.width;
            for px in x0..=x1 {
                let point = (px as i64 * SUB + HALF, py as i64 * SUB + HALF);
                let cover = coverage(point, a, (dx, dy), length_sq, (ra, rb));
                if cover == 0 {
                    continue;
                }
                let at = row + px;
                if cover >= self.cover[at] {
                    self.cover[at] = cover;
                    self.family[at] = family;
                }
            }
        }
    }

    /// Clips a sub-unit interval to the pixel range it touches, or `None` if it misses.
    fn span(&self, low: i64, high: i64, limit: usize) -> Option<(usize, usize)> {
        let limit = limit as i64;
        let low = low.div_euclid(SUB);
        let high = high.div_euclid(SUB);
        if high < 0 || low >= limit {
            return None;
        }
        Some((low.max(0) as usize, high.min(limit - 1) as usize))
    }

    /// Flattens to palette indices: `family * 64 + coverage`.
    ///
    /// Coverage zero lands on each family's first entry, and [`crate::draw::palette`] makes
    /// all four of those the background colour, so an untouched pixel needs no special case.
    #[must_use]
    pub(crate) fn into_indices(self) -> Vec<u8> {
        self.family
            .iter()
            .zip(&self.cover)
            .map(|(family, cover)| (*family << 6) | *cover)
            .collect()
    }
}

/// Coverage of one pixel centre by one tapered stroke, from 0 to [`LEVELS`].
fn coverage(
    point: (i64, i64),
    a: (i64, i64),
    (dx, dy): (i64, i64),
    length_sq: i64,
    (ra, rb): (i64, i64),
) -> u8 {
    let wx = point.0 - a.0;
    let wy = point.1 - a.1;

    // How far along the segment the nearest point lies, as a Q16 fraction clamped to the
    // segment's own extent — which is what turns an infinite line into a capped stroke.
    // i128 because the numerator is a squared distance shifted left by 16.
    let t = if length_sq == 0 {
        0
    } else {
        let along = i128::from(wx) * i128::from(dx) + i128::from(wy) * i128::from(dy);
        ((along << 16) / i128::from(length_sq)).clamp(0, 1 << 16) as i64
    };

    let ex = wx - ((dx * t) >> 16);
    let ey = wy - ((dy * t) >> 16);
    let distance = ((ex * ex + ey * ey) as u64).isqrt() as i64;
    let radius = ra + (((rb - ra) * t) >> 16);

    // Full ink half a pixel inside the boundary, none half a pixel outside it.
    (((radius + HALF - distance) * LEVELS) / SUB).clamp(0, LEVELS) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads back one pixel as `(family, coverage)`.
    fn at(canvas: &Canvas, x: usize, y: usize) -> (u8, u8) {
        let index = y * canvas.width + x;
        (canvas.family[index], canvas.cover[index])
    }

    /// Centre of pixel `(x, y)` in sub-units.
    const fn centre(x: i64, y: i64) -> (i64, i64) {
        (x * SUB + HALF, y * SUB + HALF)
    }

    /// The property the whole silhouette rests on: a stroke is opaque along its spine, fades
    /// across its edge, and stops.
    #[test]
    fn a_stroke_is_solid_inside_and_absent_outside() {
        let mut canvas = Canvas::new(32, 32);
        // A vertical bar four pixels wide, down the middle.
        canvas.stroke(centre(16, 4), centre(16, 28), 2 * SUB, 2 * SUB, 0);

        assert_eq!(
            at(&canvas, 16, 16).1,
            LEVELS as u8,
            "the spine is not solid"
        );
        assert_eq!(
            at(&canvas, 15, 16).1,
            LEVELS as u8,
            "one pixel in is not solid"
        );

        let edge = at(&canvas, 18, 16).1;
        assert!(
            edge > 0 && edge < LEVELS as u8,
            "the edge should be partial coverage, got {edge}"
        );

        assert_eq!(at(&canvas, 22, 16).1, 0, "ink four pixels clear of the bar");
        assert_eq!(at(&canvas, 16, 0).1, 0, "ink well past the cap");
    }

    /// Taper is the whole of `C1`'s width falloff made visible. A stroke that ignored its
    /// two radii would still look like a tree and would say nothing about thickness.
    #[test]
    fn a_tapered_stroke_is_wider_at_its_base() {
        let mut canvas = Canvas::new(64, 64);
        canvas.stroke(centre(32, 8), centre(32, 56), 6 * SUB, SUB, 0);

        let width_at = |y: usize| (0..64).filter(|&x| at(&canvas, x, y).1 > 0).count();
        let base = width_at(12);
        let tip = width_at(52);
        assert!(
            base > tip + 4,
            "base {base} should be clearly wider than tip {tip}"
        );
    }

    /// The hybrid trunk is overlapping limbs and nothing else, so overlap must union rather
    /// than seam. Sabotaged by replacing the comparison in `stroke` with an average: the
    /// crossing then reads as half-covered and this fails.
    #[test]
    fn overlapping_strokes_union_rather_than_seam() {
        let mut canvas = Canvas::new(32, 32);
        canvas.stroke(centre(4, 16), centre(28, 16), 3 * SUB, 3 * SUB, 0);
        canvas.stroke(centre(16, 4), centre(16, 28), 3 * SUB, 3 * SUB, 0);

        assert_eq!(
            at(&canvas, 16, 16).1,
            LEVELS as u8,
            "the crossing lost coverage to the second stroke"
        );
    }

    /// Draw order is the caller's lever for visual priority — it is how an aggregate
    /// container stays visible in a canopy full of limbs.
    #[test]
    fn a_later_family_takes_the_pixel_at_equal_coverage() {
        let mut canvas = Canvas::new(16, 16);
        canvas.stroke(centre(2, 8), centre(14, 8), 2 * SUB, 2 * SUB, 0);
        assert_eq!(at(&canvas, 8, 8), (0, LEVELS as u8));

        canvas.stroke(centre(8, 2), centre(8, 14), 2 * SUB, 2 * SUB, 3);
        assert_eq!(at(&canvas, 8, 8), (3, LEVELS as u8));

        // ...but only at equal or greater coverage. A grazing stroke does not steal a pixel
        // the first one owns outright.
        canvas.stroke(centre(2, 5), centre(14, 5), SUB / 2, SUB / 2, 1);
        assert_eq!(at(&canvas, 8, 8), (3, LEVELS as u8));
    }

    /// Geometry off the canvas must be clipped, not wrapped onto the opposite edge — the
    /// failure a naive clamp produces, and one that would read as a mysterious stray limb.
    #[test]
    fn geometry_outside_the_canvas_draws_nothing() {
        let mut canvas = Canvas::new(16, 16);
        canvas.stroke(centre(-50, 8), centre(-40, 8), 2 * SUB, 2 * SUB, 0);
        canvas.stroke(centre(60, 8), centre(70, 8), 2 * SUB, 2 * SUB, 0);
        canvas.stroke(centre(8, -30), centre(8, -20), 2 * SUB, 2 * SUB, 0);

        assert!(
            canvas.cover.iter().all(|&c| c == 0),
            "off-canvas geometry left ink behind"
        );
    }

    /// A degenerate segment is a dot, not a panic and not a division by zero. Compose can
    /// produce one wherever a length rounds to nothing.
    #[test]
    fn a_zero_length_stroke_is_a_dot() {
        let mut canvas = Canvas::new(16, 16);
        canvas.stroke(centre(8, 8), centre(8, 8), 2 * SUB, 2 * SUB, 0);

        assert_eq!(at(&canvas, 8, 8).1, LEVELS as u8);
        assert_eq!(at(&canvas, 12, 8).1, 0);
    }

    /// The packing the palette depends on: six bits of coverage, two of family.
    #[test]
    fn indices_pack_family_above_coverage() {
        let mut canvas = Canvas::new(4, 4);
        canvas.stroke(centre(1, 1), centre(1, 1), SUB, SUB, 2);
        let indices = canvas.into_indices();

        // Pixel (1, 1) is the dot itself; (3, 3) is well clear of it, and must land on index
        // zero rather than on its family's zero-coverage entry — the palette makes all four
        // of those background, but only index zero costs nothing to reason about.
        assert_eq!(indices[5], 2 * 64 + LEVELS as u8);
        assert_eq!(indices[15], 0, "an untouched pixel is not index zero");
    }
}
