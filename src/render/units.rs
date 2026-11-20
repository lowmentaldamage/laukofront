//! Ships and buildings on the map, drawn with gizmos in the owner's colour:
//! city = square, port = circle with a dot, factory = triangle, defense post = diamond,
//! transport ship = arrow (your own show their route), trade ship = small circle.
//! Unfinished buildings show a progress arc; upgraded ones a dot per extra level; factories draw
//! rail lines to the cities and ports they serve.

use bevy::prelude::*;

use super::camera::MapCamera;
use super::{palette, tile_center};
use crate::client::GameState;
use crate::engine::structures::factory_stations;
use crate::engine::units::UnitKind;
use crate::engine::HUMAN;

pub struct UnitsRenderPlugin;

impl Plugin for UnitsRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, draw_units);
    }
}

fn owner_color(owner: u16) -> Color {
    if owner == HUMAN {
        let c = palette::OWN_BORDER;
        return Color::srgb_u8(c[0], c[1], c[2]);
    }
    let c = palette::player_color(owner);
    Color::srgb_u8(c[0], c[1], c[2])
}

fn draw_units(
    game: Res<GameState>,
    camera: Single<&Projection, With<MapCamera>>,
    mut gizmos: Gizmos,
) {
    let g = &game.0;
    let map = g.map();
    let now = g.ticks();
    // Keep icons at least ~9 px wide however far you zoom out.
    let scale = match *camera {
        Projection::Orthographic(o) => o.scale,
        _ => 1.0,
    };
    let r = (scale * 9.0).max(2.2);
    let dark = Color::srgba(0.0, 0.0, 0.0, 0.85);

    // Rails first (below icons).
    for u in g
        .units()
        .iter()
        .filter(|u| u.is_active() && u.kind() == UnitKind::Factory)
    {
        if u.under_construction(now) {
            continue;
        }
        let from = tile_center(map, u.tile());
        let col = owner_color(u.owner()).with_alpha(0.55);
        for s in factory_stations(g, u.id()) {
            if let Some(st) = g.unit(s) {
                gizmos.line_2d(from, tile_center(map, st.tile()), col);
            }
        }
    }

    for u in g.units().iter().filter(|u| u.is_active()) {
        let c = tile_center(map, u.tile());
        let col = owner_color(u.owner());
        match u.kind() {
            UnitKind::City => {
                gizmos.rect_2d(Isometry2d::from_translation(c), Vec2::splat(r * 2.2), dark);
                gizmos.rect_2d(Isometry2d::from_translation(c), Vec2::splat(r * 1.8), col);
            }
            UnitKind::Port => {
                gizmos.circle_2d(c, r * 1.15, dark);
                gizmos.circle_2d(c, r, col);
                gizmos.circle_2d(c, r * 0.3, col);
            }
            UnitKind::Factory => {
                let tri = |k: f32| {
                    [
                        c + Vec2::new(0.0, k),
                        c + Vec2::new(-k * 0.9, -k * 0.6),
                        c + Vec2::new(k * 0.9, -k * 0.6),
                        c + Vec2::new(0.0, k),
                    ]
                };
                gizmos.linestrip_2d(tri(r * 1.35), dark);
                gizmos.linestrip_2d(tri(r * 1.15), col);
            }
            UnitKind::DefensePost => {
                let dia = |k: f32| {
                    [
                        c + Vec2::new(0.0, k),
                        c + Vec2::new(k, 0.0),
                        c + Vec2::new(0.0, -k),
                        c + Vec2::new(-k, 0.0),
                        c + Vec2::new(0.0, k),
                    ]
                };
                gizmos.linestrip_2d(dia(r * 1.35), dark);
                gizmos.linestrip_2d(dia(r * 1.15), col);
            }
            UnitKind::TransportShip => {
                let next = u
                    .remaining_path()
                    .first()
                    .copied()
                    .map(|t| tile_center(map, t));
                let dir = next
                    .map(|n| (n - c).normalize_or_zero())
                    .filter(|d| *d != Vec2::ZERO)
                    .unwrap_or(Vec2::X);
                let side = Vec2::new(-dir.y, dir.x);
                let tip = c + dir * r * 1.3;
                let a = c - dir * r * 0.8 + side * r * 0.8;
                let b = c - dir * r * 0.8 - side * r * 0.8;
                gizmos.linestrip_2d([tip, a, b, tip], col);
                if u.owner() == HUMAN {
                    let pts: Vec<Vec2> = u
                        .remaining_path()
                        .iter()
                        .step_by(3)
                        .map(|&t| tile_center(map, t))
                        .collect();
                    if pts.len() > 1 {
                        gizmos.linestrip_2d(pts, col.with_alpha(0.45));
                    }
                }
            }
            UnitKind::TradeShip => {
                gizmos.circle_2d(c, r * 0.5, col.with_alpha(0.8));
            }
            UnitKind::MissileSilo => {
                gizmos.circle_2d(c, r * 1.25, dark);
                gizmos.circle_2d(c, r * 1.05, col);
                gizmos.line_2d(c + Vec2::new(-r, -r) * 0.7, c + Vec2::new(r, r) * 0.7, col);
                gizmos.line_2d(c + Vec2::new(-r, r) * 0.7, c + Vec2::new(r, -r) * 0.7, col);
            }
            UnitKind::SamLauncher => {
                gizmos.rect_2d(Isometry2d::from_translation(c), Vec2::splat(r * 2.2), dark);
                gizmos.circle_2d(c, r * 0.9, col);
                gizmos.line_2d(c + Vec2::new(0.0, -r), c + Vec2::new(0.0, r), col);
                if u.owner() == HUMAN && !u.under_construction(now) {
                    let range = crate::engine::weapons::sam_range(g, u.level()) as f32;
                    gizmos.circle_2d(c, range, col.with_alpha(0.18));
                }
            }
            UnitKind::Warship => {
                let next = u
                    .remaining_path()
                    .first()
                    .copied()
                    .map(|t| tile_center(map, t));
                let dir = next
                    .map(|n| (n - c).normalize_or_zero())
                    .filter(|d| *d != Vec2::ZERO)
                    .unwrap_or(Vec2::X);
                let side = Vec2::new(-dir.y, dir.x);
                let hull = [
                    c + dir * r * 1.8,
                    c + side * r * 0.8,
                    c - dir * r * 1.4 + side * r * 0.6,
                    c - dir * r * 1.4 - side * r * 0.6,
                    c - side * r * 0.8,
                    c + dir * r * 1.8,
                ];
                gizmos.linestrip_2d(hull, col);
                let hp =
                    (u.health() as f32 / g.config().weapons.warship_health as f32).clamp(0.0, 1.0);
                let a = c + Vec2::new(-r * 1.2, r * 1.6);
                gizmos.line_2d(
                    a,
                    a + Vec2::new(r * 2.4 * hp, 0.0),
                    Color::srgb(0.4, 0.9, 0.4),
                );
            }
            UnitKind::AtomBomb | UnitKind::HydrogenBomb | UnitKind::Mirv => {
                let red = Color::srgb(1.0, 0.25, 0.2);
                let size = if u.kind() == UnitKind::AtomBomb {
                    r * 0.9
                } else {
                    r * 1.3
                };
                gizmos.circle_2d(c, size, red);
                if let Some(src) = u.remaining_path().first().or(u.path.first()) {
                    gizmos.line_2d(tile_center(map, *src), c, red.with_alpha(0.35));
                }
                if let Some(t) = u.target_tile() {
                    gizmos.circle_2d(tile_center(map, t), r * 2.0, red.with_alpha(0.6));
                }
            }
            UnitKind::MirvWarhead => {
                gizmos.circle_2d(c, r * 0.35, Color::srgb(1.0, 0.4, 0.2));
            }
        }
        if u.kind().is_structure() {
            if u.under_construction(now) {
                let p = u.construction_progress(now).clamp(0.0, 1.0);
                gizmos.arc_2d(
                    Isometry2d::from_translation(c),
                    std::f32::consts::TAU * p,
                    r * 1.8,
                    Color::srgba(1.0, 1.0, 1.0, 0.9),
                );
            }
            for i in 1..u.level().min(6) {
                let dot = c + Vec2::new(-r + (i as f32) * r * 0.5, -r * 1.7);
                gizmos.circle_2d(dot, r * 0.18, col);
            }
        }
    }
}
