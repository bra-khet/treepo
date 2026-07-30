//! Level-of-detail bands — `F-NAV-1`, `F-NAV-3`, `F-NAV-4`.
//!
//! > The tree is legible at far, medium and near zoom.
//!
//! A band is a **quantized texel density**: how many texels of baked layer one world unit is
//! worth. Everything about LOD in this slice reduces to that one number, because the bake is
//! the only thing that consumes it — what changes between far and near is the resolution the
//! same geometry is rasterized at, not which geometry is drawn.
//!
//! # Why quantize at all
//!
//! The ideal density is one texel per screen pixel, which is a continuous function of the
//! camera's scale — so a continuous LOD would rebake every chunk on every frame of a zoom, and
//! a zoom is exactly when there is no frame budget to spare. Bands are powers of two, so a
//! continuous zoom crosses a boundary about once per doubling and the bake between boundaries
//! is at worst 2× sharper per axis than it needs to be. Never blurrier: the band is chosen by
//! *flooring* the exponent, and erring towards sharp costs texels while erring towards coarse
//! costs `AC-NAV-1`, which is a user's ability to read the picture.
//!
//! # Bands are relative to the framed view, not absolute
//!
//! Skeleton units come out of the parameter table, and a T0 repository and a T3 monorepo do not
//! agree within two orders of magnitude about how big a world unit is. So band 0 is defined as
//! *the whole tree fits the window* — [`TreeCamera::fit_scale`](crate::camera::TreeCamera) —
//! and the index counts doublings from there. That is the same reasoning that makes the
//! camera's zoom limits relative, and it means [`Band::SHARPEST`] and
//! [`TreeCamera::MAX_IN`](crate::camera::TreeCamera::MAX_IN) describe the same place.

/// A quantized texel density, counted in doublings from the framed view.
///
/// Negative is sharper than the framed view and positive is coarser, which reads backwards
/// until you notice it is an exponent of the camera's *scale* — world units per pixel — and
/// zooming in makes that number smaller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Band(i8);

impl Band {
    /// The whole tree in the window: one texel per screen pixel at the framed scale.
    pub const FRAMED: Self = Self(0);

    /// The sharpest band, matching the camera's zoom-in limit.
    ///
    /// [`TreeCamera::MAX_IN`](crate::camera::TreeCamera::MAX_IN) is 1/2000 of the framed view,
    /// and `log2(2000)` is just under eleven — so eleven doublings is where the camera stops
    /// and there is nothing sharper to bake for.
    pub const SHARPEST: Self = Self(-11);

    /// The coarsest band, matching the camera's zoom-out limit.
    ///
    /// [`TreeCamera::MAX_OUT`](crate::camera::TreeCamera::MAX_OUT) is four times the framed
    /// view, which is two doublings.
    pub const COARSEST: Self = Self(2);

    /// The band whose density suits a camera scale, given the scale the tree was framed at.
    ///
    /// Both scales are world units per pixel. A non-finite or non-positive input yields
    /// [`FRAMED`](Self::FRAMED) rather than a panic: an unframed camera on the first frame is
    /// an ordinary state, not an error, and its own scale is the right reference for it.
    #[must_use]
    pub fn for_scale(scale: f32, fit_scale: f32) -> Self {
        // Finiteness first, so the comparisons that follow are total: `NaN <= 0.0` is false,
        // which would let a NaN scale through into `log2` and out as an arbitrary band.
        if !scale.is_finite() || !fit_scale.is_finite() || scale <= 0.0 || fit_scale <= 0.0 {
            return Self::FRAMED;
        }
        let exponent = (scale / fit_scale).log2().floor();
        if !exponent.is_finite() {
            return Self::FRAMED;
        }
        Self(exponent.clamp(f32::from(Self::SHARPEST.0), f32::from(Self::COARSEST.0)) as i8)
    }

    /// How many texels of baked layer one world unit is worth in this band.
    ///
    /// The number the bake sizes a texture with, and the number [`piece_grid`] splits a limb
    /// on. Zero-or-worse `fit_scale` yields one texel per unit — a value that draws something
    /// rather than a division that draws nothing.
    ///
    /// [`piece_grid`]: crate::chunk::piece_grid
    #[must_use]
    pub fn texels_per_unit(self, fit_scale: f32) -> f32 {
        if !fit_scale.is_finite() || fit_scale <= 0.0 {
            return 1.0;
        }
        let density = 2f32.powi(-i32::from(self.0)) / fit_scale;
        if density.is_finite() && density > 0.0 {
            density
        } else {
            1.0
        }
    }

    /// How many doublings from the framed view this band is.
    #[must_use]
    pub fn index(self) -> i8 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_framed_scale_is_band_zero() {
        assert_eq!(Band::for_scale(3.0, 3.0), Band::FRAMED);
    }

    #[test]
    fn zooming_in_by_a_doubling_moves_one_band_sharper() {
        assert_eq!(Band::for_scale(1.5, 3.0).index(), -1);
        assert_eq!(Band::for_scale(0.75, 3.0).index(), -2);
    }

    #[test]
    fn zooming_out_by_a_doubling_moves_one_band_coarser() {
        assert_eq!(Band::for_scale(6.0, 3.0).index(), 1);
    }

    /// The rounding direction, as a property rather than a constant: between two bands the
    /// chosen one is at least as sharp as the camera needs, never blurrier.
    #[test]
    fn a_band_is_never_blurrier_than_the_camera_needs() {
        let fit = 3.0;
        for step in 0..64 {
            let scale = fit * 0.5f32.powf(step as f32 / 8.0);
            let band = Band::for_scale(scale, fit);
            let needed = 1.0 / scale;
            let baked = band.texels_per_unit(fit);
            assert!(
                baked >= needed - 1e-6,
                "band {} bakes {baked} texels/unit where {needed} is needed",
                band.index()
            );
        }
    }

    /// …and not extravagantly sharper, which is the other half of quantizing: at most one
    /// doubling per axis, so at most four times the texels of the ideal density.
    #[test]
    fn a_band_is_never_more_than_a_doubling_sharper_than_needed() {
        let fit = 3.0;
        for step in 0..64 {
            let scale = fit * 0.5f32.powf(step as f32 / 8.0);
            let baked = Band::for_scale(scale, fit).texels_per_unit(fit);
            assert!(baked <= 2.0 / scale + 1e-6);
        }
    }

    #[test]
    fn bands_stop_where_the_camera_stops() {
        use crate::camera::TreeCamera;
        let fit = 3.0;
        assert_eq!(
            Band::for_scale(fit * TreeCamera::MAX_IN, fit),
            Band::SHARPEST
        );
        assert_eq!(
            Band::for_scale(fit * TreeCamera::MAX_OUT, fit),
            Band::COARSEST
        );
    }

    /// An unframed camera has no reference to be relative to. Band zero at its own scale is
    /// the answer that bakes at one texel per pixel, which is what it would want anyway.
    #[test]
    fn an_unframed_camera_falls_back_to_its_own_scale() {
        assert_eq!(Band::for_scale(2.0, 0.0), Band::FRAMED);
        assert_eq!(Band::for_scale(f32::NAN, 3.0), Band::FRAMED);
        assert_eq!(Band::for_scale(0.0, 3.0), Band::FRAMED);
    }

    #[test]
    fn density_never_becomes_zero_or_infinite() {
        for band in [Band::SHARPEST, Band::FRAMED, Band::COARSEST] {
            for fit in [1e-30, 1.0, 1e30, 0.0, f32::NAN] {
                let density = band.texels_per_unit(fit);
                assert!(density.is_finite() && density > 0.0, "{band:?} at {fit}");
            }
        }
    }
}
