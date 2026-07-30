//! The navigation camera — `F-NAV-2`.
//!
//! > Zoom and pan are continuous; the tree is legible at far, medium and near zoom.
//!
//! An orthographic 2-D camera, framed on the tree when a snapshot lands and moved by the
//! pointer thereafter. It carries no level-of-detail logic: `F-NAV-1`/`F-NAV-3`/`F-NAV-4` are
//! about *what is drawn* at a zoom level, which is `lod.rs` and the chunked bake, and putting
//! a band threshold here would make the camera the thing that decides what a tree looks like.
//!
//! # The three gestures, and why the left button does two of them
//!
//! Drag to pan, wheel to zoom, click to select. A consumer product (`R1`) is judged on the
//! gesture a user tries first, and on a picture the first gesture is drag-to-pan — so the left
//! button has to serve both panning and selection. [`PointerDrag`] is how they are told apart:
//! it accumulates cursor travel while the button is down, and a gesture that moved less than
//! [`PointerDrag::CLICK_SLOP`] logical pixels was a click. It is a resource rather than
//! private state because the system that acts on a click lives in `treepo-app` (`F-INSP-1`),
//! and both halves reading one measurement is what keeps a drag from also selecting.

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

use crate::chunk::Extent;

/// The camera that looks at the tree.
///
/// One per app. It carries the scale the tree was framed at, so that the zoom limits are
/// relative to *this* tree rather than absolute: skeleton units come out of the parameter
/// table, and a T0 repository and a T3 monorepo do not agree within two orders of magnitude
/// about how big a world unit is.
#[derive(Component, Debug, Clone, Copy)]
pub struct TreeCamera {
    /// The projection scale at which the whole tree fits the window.
    ///
    /// `None` until a snapshot has been framed, which is also the state in which zooming is
    /// unclamped — there is nothing yet to be zoomed relative to.
    pub fit_scale: Option<f32>,
}

impl TreeCamera {
    /// How far out the camera may travel from the framed view: four times the whole tree.
    ///
    /// Far enough to lose the tree in the window, which is what makes "zoom out to find your
    /// bearings" work, and near enough that a user cannot scroll the tree into a speck and
    /// conclude the app is broken.
    pub const MAX_OUT: f32 = 4.0;
    /// How far in: 1/2000 of the framed view.
    ///
    /// `F-NAV-4`'s near band is individual files on a T3 tree, which is roughly four orders of
    /// magnitude below the whole. This is deliberately looser than the useful range rather
    /// than tuned to it — a limit the user can feel before they reach the detail they were
    /// looking for is worse than one they never reach.
    pub const MAX_IN: f32 = 1.0 / 2000.0;

    /// The scale range this camera may be zoomed within.
    #[must_use]
    pub fn scale_bounds(&self) -> Option<(f32, f32)> {
        self.fit_scale
            .map(|fit| (fit * Self::MAX_IN, fit * Self::MAX_OUT))
    }
}

/// Ask the camera to frame an extent on the next frame.
///
/// A resource the camera *takes from* rather than a message, because framing is idempotent and
/// the only sensible answer to two requests in one frame is the newer one. `snapshot_sync` sets
/// it when a snapshot is committed.
#[derive(Resource, Debug, Default)]
pub struct FrameTarget(pub Option<Extent>);

/// What the left mouse button is doing, so a drag pans and a click selects.
#[derive(Resource, Debug, Default)]
pub struct PointerDrag {
    /// Where the cursor was last frame, while the button is down.
    last: Option<Vec2>,
    /// Total travel in logical pixels since the button went down.
    travel: f32,
}

impl PointerDrag {
    /// How far the cursor may move during a press and still count as a click.
    ///
    /// Three logical pixels. A mouse click carries a pixel or two of tremor and a trackpad tap
    /// carries more; a threshold of zero would make selection fail for exactly the users least
    /// able to work around it.
    pub const CLICK_SLOP: f32 = 3.0;

    /// Whether the gesture in progress has stayed within [`CLICK_SLOP`](Self::CLICK_SLOP).
    ///
    /// Read on the frame the button is released.
    #[must_use]
    pub fn was_click(&self) -> bool {
        self.travel <= Self::CLICK_SLOP
    }
}

/// The camera's systems, so that anything reading [`PointerDrag`] can run after them.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CameraSystems;

/// Spawns the camera. Registered on `Startup` by
/// [`TreepoRenderPlugin`](crate::TreepoRenderPlugin).
pub fn spawn(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        TreeCamera { fit_scale: None },
        Projection::Orthographic(OrthographicProjection::default_2d()),
    ));
}

/// Frames a requested extent: centres on it and scales so it fits with a margin.
pub fn frame(
    mut target: ResMut<FrameTarget>,
    window: Option<Single<&Window>>,
    camera: Option<Single<(&mut Transform, &mut Projection, &mut TreeCamera)>>,
) {
    let Some(extent) = target.0.take() else {
        return;
    };
    let (Some(window), Some(camera)) = (window, camera) else {
        return;
    };
    let (mut transform, mut projection, mut tree_camera) = camera.into_inner();
    let Projection::Orthographic(ortho) = &mut *projection else {
        return;
    };

    let viewport = Vec2::new(window.width(), window.height());
    let fit = fit_scale(extent.size(), viewport);
    let center = extent.center();

    transform.translation.x = center.x;
    transform.translation.y = center.y;
    ortho.scale = fit;
    tree_camera.fit_scale = Some(fit);
}

/// Wheel to zoom, about the point under the cursor.
///
/// Zooming about the cursor rather than the window centre is what makes "zoom towards the
/// thing you are looking at" a single gesture instead of alternating zoom and pan — which
/// matters directly for `AC-NAV-1`, where a user has thirty seconds to find a directory by eye.
pub fn zoom(
    mut wheel: MessageReader<MouseWheel>,
    window: Option<Single<&Window>>,
    camera: Option<Single<(&mut Transform, &mut Projection, &TreeCamera)>>,
) {
    let notches: f32 = wheel
        .read()
        .map(|event| match event.unit {
            // A line is one wheel notch. A pixel is a trackpad, where a single flick delivers
            // hundreds — dividing brings the two devices into the same range rather than
            // making a trackpad zoom straight through the clamp.
            MouseScrollUnit::Line => event.y,
            MouseScrollUnit::Pixel => event.y / 40.0,
        })
        .sum();
    if notches == 0.0 {
        return;
    }
    let (Some(window), Some(camera)) = (window, camera) else {
        return;
    };
    let (mut transform, mut projection, tree_camera) = camera.into_inner();
    let Projection::Orthographic(ortho) = &mut *projection else {
        return;
    };

    let before = ortho.scale;
    // Exponential, so a notch is a constant *ratio* at every zoom level. Linear steps are
    // imperceptible when zoomed out and violent when zoomed in.
    let mut after = before * ZOOM_PER_NOTCH.powf(-notches);
    if let Some((min, max)) = tree_camera.scale_bounds() {
        after = after.clamp(min, max);
    }
    if after == before {
        return;
    }
    ortho.scale = after;

    // Keep the world point under the cursor where it is. With `ScalingMode::WindowSize` the
    // world offset of a pixel from the viewport centre is that offset times the scale, so the
    // correction is the offset times the difference — no round trip through the camera's
    // global transform, which has not been recomputed yet this frame anyway.
    if let Some(cursor) = window.cursor_position() {
        let from_center = cursor - Vec2::new(window.width(), window.height()) * 0.5;
        let world_per_pixel = Vec2::new(from_center.x, -from_center.y);
        let correction = world_per_pixel * (before - after);
        transform.translation.x += correction.x;
        transform.translation.y += correction.y;
    }
}

/// Drag to pan, and measure the drag so a click can be told from it.
pub fn pan(
    buttons: Res<ButtonInput<MouseButton>>,
    mut drag: ResMut<PointerDrag>,
    window: Option<Single<&Window>>,
    camera: Option<Single<(&mut Transform, &Projection), With<TreeCamera>>>,
) {
    if buttons.just_pressed(MouseButton::Left) {
        drag.travel = 0.0;
        drag.last = window.as_ref().and_then(|w| w.cursor_position());
    }
    if buttons.just_released(MouseButton::Left) {
        drag.last = None;
        return;
    }
    if !buttons.pressed(MouseButton::Left) {
        return;
    }

    let (Some(window), Some(camera)) = (window, camera) else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Some(previous) = drag.last.replace(cursor) else {
        return;
    };

    let moved = cursor - previous;
    drag.travel += moved.length();

    let (mut transform, projection) = camera.into_inner();
    let Projection::Orthographic(ortho) = projection else {
        return;
    };
    // The cursor holds onto the world point it grabbed, so the world moves with the hand:
    // rightward cursor travel moves the camera left, and screen-down is world-up.
    transform.translation.x -= moved.x * ortho.scale;
    transform.translation.y += moved.y * ortho.scale;
}

/// How much one wheel notch changes the scale.
const ZOOM_PER_NOTCH: f32 = 1.2;

/// How much of the window the framed tree fills.
///
/// Short of the whole so the silhouette has air around it — a tree touching all four edges
/// reads as cropped, and `AC-NAV-1`'s far band is a question about the shape as a whole.
const FRAME_FILL: f32 = 0.88;

/// The projection scale at which `content` fits inside `viewport`, with a margin.
///
/// Separated from [`frame`] because it is the only arithmetic in this module worth being
/// wrong, and a system cannot be called from a test without a `World`.
#[must_use]
pub fn fit_scale(content: Vec2, viewport: Vec2) -> f32 {
    // A degenerate axis is a real skeleton: a repository with one limb is a vertical line. Its
    // width does not decide the zoom, and dividing by it would decide it as infinity.
    let by_width = ratio(content.x, viewport.x);
    let by_height = ratio(content.y, viewport.y);
    let needed = by_width.max(by_height);
    if needed <= 0.0 {
        1.0
    } else {
        needed / FRAME_FILL
    }
}

/// `content / viewport`, or zero where the viewport has no extent to divide by.
fn ratio(content: f32, viewport: f32) -> f32 {
    if viewport > 0.0 {
        content / viewport
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tree_wider_than_it_is_tall_is_framed_on_its_width() {
        // 800 world units across a 400-pixel window is two units per pixel, plus the margin.
        let scale = fit_scale(Vec2::new(800.0, 100.0), Vec2::new(400.0, 400.0));
        assert!((scale - 2.0 / FRAME_FILL).abs() < 1e-5);
    }

    #[test]
    fn a_tree_taller_than_it_is_wide_is_framed_on_its_height() {
        let scale = fit_scale(Vec2::new(100.0, 800.0), Vec2::new(400.0, 400.0));
        assert!((scale - 2.0 / FRAME_FILL).abs() < 1e-5);
    }

    /// A single vertical limb has no width. Framing on it must not divide by zero, and the
    /// height must still decide the zoom.
    #[test]
    fn a_tree_with_no_width_is_framed_on_its_height() {
        let scale = fit_scale(Vec2::new(0.0, 500.0), Vec2::new(500.0, 500.0));
        assert!(scale.is_finite());
        assert!((scale - 1.0 / FRAME_FILL).abs() < 1e-5);
    }

    /// `AC-SKEL-2`'s empty repository, before it has any geometry at all.
    #[test]
    fn a_tree_with_no_extent_does_not_produce_an_impossible_scale() {
        let scale = fit_scale(Vec2::ZERO, Vec2::new(500.0, 500.0));
        assert!(scale > 0.0 && scale.is_finite());
    }

    #[test]
    fn zoom_bounds_are_relative_to_the_framed_scale() {
        let camera = TreeCamera {
            fit_scale: Some(10.0),
        };
        let (min, max) = camera.scale_bounds().unwrap();
        assert!(min < 10.0 && max > 10.0);
        assert_eq!(max, 40.0);
    }

    /// Before a tree is framed there is nothing to be zoomed relative to, and clamping to an
    /// invented range would leave the user unable to zoom at all on a slow first load.
    #[test]
    fn an_unframed_camera_has_no_zoom_bounds() {
        assert!(TreeCamera { fit_scale: None }.scale_bounds().is_none());
    }

    #[test]
    fn a_gesture_that_barely_moved_is_a_click() {
        let barely = PointerDrag {
            travel: 1.0,
            ..PointerDrag::default()
        };
        assert!(barely.was_click());

        let dragged = PointerDrag {
            travel: 40.0,
            ..PointerDrag::default()
        };
        assert!(!dragged.was_click());
    }
}
