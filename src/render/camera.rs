//! 2D map camera: pan with WASD / arrows or right-drag, zoom with the wheel toward the cursor,
//! always clamped to the map. The maths is in plain functions so it can be tested.

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::MapBounds;

/// Zoom limits relative to "whole map fits the window" (1.0).
pub const MIN_ZOOM: f32 = 0.5;
pub const MAX_ZOOM: f32 = 24.0;
/// Keyboard pan speed in screen pixels per second.
pub const PAN_SPEED_PX: f32 = 700.0;
/// Zoom factor per wheel line.
pub const ZOOM_STEP: f32 = 1.15;
/// Empty margin around the map at zoom 1.0.
const FIT_MARGIN: f32 = 1.06;

#[derive(Component)]
pub struct MapCamera;

/// Current zoom level, 1.0 = whole map fits the window.
#[derive(Resource, Debug, Clone, Copy)]
pub struct CameraZoom(pub f32);

impl Default for CameraZoom {
    fn default() -> Self {
        Self(1.0)
    }
}

/// World units per pixel at zoom 1.0 for a window of `win` pixels and a map of `map` units.
pub fn fit_scale(map: Vec2, win: Vec2) -> f32 {
    if win.x <= 0.0 || win.y <= 0.0 {
        return 1.0;
    }
    (map.x / win.x).max(map.y / win.y) * FIT_MARGIN
}

/// Keeps the view inside the map: if the view is wider/taller than the map, centre that axis;
/// otherwise keep the view's edges inside the map's edges.
pub fn clamp_center(center: Vec2, view_half: Vec2, map_half: Vec2) -> Vec2 {
    let axis = |c: f32, v: f32, m: f32| if v >= m { 0.0 } else { c.clamp(-m + v, m - v) };
    Vec2::new(
        axis(center.x, view_half.x, map_half.x),
        axis(center.y, view_half.y, map_half.y),
    )
}

/// New camera centre so the world point under the cursor stays put when the scale changes.
/// `cursor_offset` is the cursor position relative to the window centre, in pixels, y down.
pub fn zoom_about(center: Vec2, cursor_offset: Vec2, old_scale: f32, new_scale: f32) -> Vec2 {
    let flip = Vec2::new(cursor_offset.x, -cursor_offset.y);
    let world_under_cursor = center + flip * old_scale;
    world_under_cursor - flip * new_scale
}

pub fn spawn_camera(mut commands: Commands) {
    commands.spawn((Camera2d, MapCamera));
}

#[allow(clippy::too_many_arguments)]
pub fn camera_controls(
    camera: Single<(&mut Transform, &mut Projection), With<MapCamera>>,
    window: Single<&Window, With<PrimaryWindow>>,
    bounds: Res<MapBounds>,
    mut zoom: ResMut<CameraZoom>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    time: Res<Time<Real>>,
) {
    let (mut transform, mut projection) = camera.into_inner();
    let Projection::Orthographic(ortho) = &mut *projection else {
        return;
    };
    let win = Vec2::new(window.width(), window.height());
    let fit = fit_scale(bounds.size, win);
    let old_scale = ortho.scale;
    let mut center = transform.translation.truncate();

    // Zoom toward the cursor.
    let lines = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / 100.0,
    };
    if lines != 0.0 {
        zoom.0 = (zoom.0 * ZOOM_STEP.powf(lines)).clamp(MIN_ZOOM, MAX_ZOOM);
    }
    let new_scale = fit / zoom.0;
    if lines != 0.0 {
        if let Some(cursor) = window.cursor_position() {
            center = zoom_about(center, cursor - win / 2.0, old_scale, new_scale);
        }
    }

    // Pan with keys.
    let mut dir = Vec2::ZERO;
    if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        dir.y += 1.0;
    }
    if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        dir.y -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        dir.x -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        dir.x += 1.0;
    }
    center += dir.normalize_or_zero() * PAN_SPEED_PX * new_scale * time.delta_secs();

    // Pan with right-drag: the map follows the mouse.
    if buttons.pressed(MouseButton::Right) {
        center += Vec2::new(-motion.delta.x, motion.delta.y) * new_scale;
    }

    let center = clamp_center(center, win * new_scale / 2.0, bounds.size / 2.0);
    ortho.scale = new_scale;
    transform.translation.x = center.x;
    transform.translation.y = center.y;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_scale_fits_the_longer_axis() {
        let s = fit_scale(Vec2::new(192.0, 120.0), Vec2::new(1920.0, 1080.0));
        // 120/1080 = 0.111 > 192/1920 = 0.1, so height decides.
        assert!((s - 120.0 / 1080.0 * FIT_MARGIN).abs() < 1e-6);
    }

    #[test]
    fn clamp_keeps_view_inside_map() {
        let map_half = Vec2::new(96.0, 60.0);
        let view_half = Vec2::new(20.0, 10.0);
        let c = clamp_center(Vec2::new(500.0, -500.0), view_half, map_half);
        assert_eq!(c, Vec2::new(76.0, -50.0));
        let inside = clamp_center(Vec2::new(5.0, 5.0), view_half, map_half);
        assert_eq!(inside, Vec2::new(5.0, 5.0));
    }

    #[test]
    fn clamp_centres_axis_when_view_is_bigger_than_map() {
        let c = clamp_center(
            Vec2::new(30.0, 4.0),
            Vec2::new(200.0, 10.0),
            Vec2::new(96.0, 60.0),
        );
        assert_eq!(c, Vec2::new(0.0, 4.0));
    }

    #[test]
    fn zoom_keeps_point_under_cursor() {
        let center = Vec2::new(10.0, -5.0);
        let cursor = Vec2::new(200.0, -100.0);
        let (old, new) = (0.2, 0.1);
        let before = center + Vec2::new(cursor.x, -cursor.y) * old;
        let c2 = zoom_about(center, cursor, old, new);
        let after = c2 + Vec2::new(cursor.x, -cursor.y) * new;
        assert!((before - after).length() < 1e-5);
    }
}
