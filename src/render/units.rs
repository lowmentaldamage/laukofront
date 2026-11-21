//! Buildings and ships on the map. Each one is an icon sprite (`icons.rs`, our own art): buildings
//! are a round badge in the owner's colour with a white picture, ships are the picture itself in
//! the owner's colour, facing where they sail. Icons keep the same size on screen at any zoom.
//! Ships and missiles slide smoothly between game ticks (`UnitMotion`).
//!
//! Gizmos add the rest: factory rails, your boats' routes, SAM ranges, warship health, the
//! construction ring, level dots, and missiles with their trails.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use super::camera::MapCamera;
use super::icons::{render, Glyph, ICON_PX};
use super::{palette, tile_center};
use crate::client::{ClientSettings, GameState, MapRebuild, UnitMotion};
use crate::engine::structures::factory_stations;
use crate::engine::units::{Unit, UnitId, UnitKind};
use crate::engine::HUMAN;
use crate::engine_lib::game_map::GameMap;

/// Icon size on screen in pixels.
const BUILDING_PX: f32 = 24.0;
const SHIP_PX: f32 = 20.0;
/// Smallest icon size in world units (tiles) when zoomed far in.
const MIN_WORLD: f32 = 3.0;
/// Icons sit above the map and effects, below the name labels.
const ICON_Z: f32 = 4.0;

pub struct UnitsRenderPlugin;

impl Plugin for UnitsRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<IconEntities>()
            .add_systems(Startup, load_icons)
            .add_systems(Update, (sync_icons, draw_unit_gizmos));
    }
}

#[derive(Resource)]
struct IconImages(HashMap<Glyph, Handle<Image>>);

/// Icon entity of each unit.
#[derive(Resource, Default)]
struct IconEntities(HashMap<UnitId, Entity>);

#[derive(Component)]
struct UnitIcon;

fn load_icons(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut map = HashMap::new();
    for g in Glyph::ALL {
        let mut img = Image::new(
            Extent3d {
                width: ICON_PX,
                height: ICON_PX,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            render(g),
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        );
        img.sampler = ImageSampler::linear();
        map.insert(g, images.add(img));
    }
    commands.insert_resource(IconImages(map));
}

fn owner_color(owner: u16) -> Color {
    if owner == HUMAN {
        let c = palette::OWN_BORDER;
        return Color::srgb_u8(c[0], c[1], c[2]);
    }
    let c = palette::player_color(owner);
    Color::srgb_u8(c[0], c[1], c[2])
}

fn darker(c: Color) -> Color {
    let s = c.to_srgba();
    Color::srgb(s.red * 0.62, s.green * 0.62, s.blue * 0.62)
}

/// How far the current tick has progressed (0 = just ticked, 1 = next tick due).
fn tick_alpha(time: &Time<Fixed>, settings: &ClientSettings) -> f32 {
    if settings.paused {
        1.0
    } else {
        time.overstep_fraction().clamp(0.0, 1.0)
    }
}

/// Where to draw a unit: between its tile before the last tick and its tile now.
fn smooth_pos(map: &GameMap, u: &Unit, motion: &UnitMotion, alpha: f32) -> Vec2 {
    let now = tile_center(map, u.tile());
    match motion.prev.get(&u.id()) {
        // Missiles jump far each tick; ships one or two tiles. Anything further is a teleport
        // (a new match or a respawned id) and is not smoothed.
        Some(&p) if map.manhattan_dist(p, u.tile()) <= 40 => tile_center(map, p).lerp(now, alpha),
        _ => now,
    }
}

fn camera_scale(p: &Projection) -> f32 {
    match p {
        Projection::Orthographic(o) => o.scale,
        _ => 1.0,
    }
}

#[allow(clippy::too_many_arguments)]
fn sync_icons(
    mut commands: Commands,
    game: Res<GameState>,
    motion: Res<UnitMotion>,
    settings: Res<ClientSettings>,
    time: Res<Time<Fixed>>,
    rebuild: Res<MapRebuild>,
    images: Option<Res<IconImages>>,
    camera: Single<&Projection, With<MapCamera>>,
    mut known: ResMut<IconEntities>,
    mut icons: Query<(&mut Transform, &mut Sprite), With<UnitIcon>>,
) {
    let Some(images) = images else { return };
    if rebuild.0 {
        for (_, e) in known.0.drain() {
            commands.entity(e).despawn();
        }
    }
    let g = &game.0;
    let map = g.map();
    let now = g.ticks();
    let scale = camera_scale(&camera);
    let alpha = tick_alpha(&time, &settings);
    let building = (scale * BUILDING_PX).max(MIN_WORLD);
    let ship = (scale * SHIP_PX).max(MIN_WORLD);
    let mut alive: Vec<UnitId> = Vec::new();
    for u in g.units().iter().filter(|u| u.is_active()) {
        let Some(glyph) = Glyph::of(u.kind()) else {
            continue;
        };
        alive.push(u.id());
        let is_ship = !u.kind().is_structure();
        let pos = if is_ship {
            smooth_pos(map, u, &motion, alpha)
        } else {
            tile_center(map, u.tile())
        };
        let size = if is_ship { ship } else { building };
        // Badges are a darker shade of the owner's colour so they stand out on the owner's land.
        let col = if is_ship {
            owner_color(u.owner())
        } else {
            darker(owner_color(u.owner()))
        };
        let color = if u.kind().is_structure() && u.under_construction(now) {
            col.with_alpha(0.5)
        } else {
            col
        };
        // Ships face the way they sail.
        let flip = is_ship
            && u.remaining_path()
                .first()
                .is_some_and(|&n| map.x(n) < map.x(u.tile()));
        let tf = Transform::from_translation(pos.extend(ICON_Z)).with_scale(Vec3::splat(size));
        if let Some(&e) = known.0.get(&u.id()) {
            if let Ok((mut t, mut s)) = icons.get_mut(e) {
                *t = tf;
                s.color = color;
                s.flip_x = flip;
                continue;
            }
        }
        let main = if is_ship { glyph } else { Glyph::Badge };
        let mut e = commands.spawn((
            Sprite {
                image: images.0[&main].clone(),
                color,
                flip_x: flip,
                custom_size: Some(Vec2::ONE),
                ..default()
            },
            tf,
            UnitIcon,
        ));
        if !is_ship {
            e.with_child((
                Sprite {
                    image: images.0[&glyph].clone(),
                    custom_size: Some(Vec2::splat(0.68)),
                    ..default()
                },
                Transform::from_xyz(0.0, 0.0, 0.01),
            ));
        }
        known.0.insert(u.id(), e.id());
    }
    if known.0.len() > alive.len() {
        alive.sort_unstable();
        known.0.retain(|id, e| {
            let keep = alive.binary_search(id).is_ok();
            if !keep {
                commands.entity(*e).despawn();
            }
            keep
        });
    }
}

fn draw_unit_gizmos(
    game: Res<GameState>,
    motion: Res<UnitMotion>,
    settings: Res<ClientSettings>,
    time: Res<Time<Fixed>>,
    camera: Single<&Projection, With<MapCamera>>,
    mut gizmos: Gizmos,
) {
    let g = &game.0;
    let map = g.map();
    let now = g.ticks();
    let scale = camera_scale(&camera);
    let alpha = tick_alpha(&time, &settings);
    let r = (scale * BUILDING_PX / 2.0).max(MIN_WORLD / 2.0);

    // Rails first.
    for u in g
        .units()
        .iter()
        .filter(|u| u.is_active() && u.kind() == UnitKind::Factory)
    {
        if u.under_construction(now) {
            continue;
        }
        let from = tile_center(map, u.tile());
        let col = owner_color(u.owner()).with_alpha(0.45);
        for s in factory_stations(g, u.id()) {
            if let Some(st) = g.unit(s) {
                gizmos.line_2d(from, tile_center(map, st.tile()), col);
            }
        }
    }

    for u in g.units().iter().filter(|u| u.is_active()) {
        let col = owner_color(u.owner());
        match u.kind() {
            UnitKind::TransportShip if u.owner() == HUMAN => {
                let mut pts = vec![smooth_pos(map, u, &motion, alpha)];
                pts.extend(
                    u.remaining_path()
                        .iter()
                        .step_by(3)
                        .map(|&t| tile_center(map, t)),
                );
                if pts.len() > 1 {
                    gizmos.linestrip_2d(pts, col.with_alpha(0.45));
                }
            }
            UnitKind::SamLauncher if u.owner() == HUMAN && !u.under_construction(now) => {
                let range = crate::engine::weapons::sam_range(g, u.level()) as f32;
                gizmos.circle_2d(tile_center(map, u.tile()), range, col.with_alpha(0.18));
            }
            UnitKind::Warship => {
                let c = smooth_pos(map, u, &motion, alpha);
                let hp =
                    (u.health() as f32 / g.config().weapons.warship_health as f32).clamp(0.0, 1.0);
                if hp < 1.0 {
                    let a = c + Vec2::new(-r, r * 1.15);
                    gizmos.line_2d(
                        a,
                        a + Vec2::new(r * 2.0, 0.0),
                        Color::srgba(0.0, 0.0, 0.0, 0.7),
                    );
                    gizmos.line_2d(
                        a,
                        a + Vec2::new(r * 2.0 * hp, 0.0),
                        Color::srgb(0.4, 0.9, 0.4),
                    );
                }
            }
            UnitKind::AtomBomb | UnitKind::HydrogenBomb | UnitKind::Mirv => {
                let c = smooth_pos(map, u, &motion, alpha);
                let red = Color::srgb(1.0, 0.25, 0.2);
                let size = if u.kind() == UnitKind::AtomBomb {
                    r * 0.6
                } else {
                    r * 0.85
                };
                gizmos.circle_2d(c, size, red);
                gizmos.circle_2d(c, size * 0.45, Color::srgb(1.0, 0.8, 0.4));
                if let Some(src) = u.path.first() {
                    gizmos.line_2d(tile_center(map, *src), c, red.with_alpha(0.35));
                }
                if let Some(t) = u.target_tile() {
                    gizmos.circle_2d(tile_center(map, t), r * 1.4, red.with_alpha(0.6));
                }
            }
            UnitKind::MirvWarhead => {
                let c = smooth_pos(map, u, &motion, alpha);
                gizmos.circle_2d(c, r * 0.25, Color::srgb(1.0, 0.4, 0.2));
            }
            _ => {}
        }
        if u.kind().is_structure() {
            let c = tile_center(map, u.tile());
            if u.under_construction(now) {
                let p = u.construction_progress(now).clamp(0.0, 1.0);
                gizmos.arc_2d(
                    Isometry2d::from_translation(c),
                    std::f32::consts::TAU * p,
                    r * 1.2,
                    Color::srgba(1.0, 1.0, 1.0, 0.9),
                );
            }
            // One dot per extra level under the badge.
            let extra = u.level().saturating_sub(1).min(5);
            for i in 0..extra {
                let x = (i as f32 - (extra as f32 - 1.0) / 2.0) * r * 0.45;
                let dot = c + Vec2::new(x, -r * 1.25);
                gizmos.circle_2d(dot, r * 0.14, col);
            }
        }
    }
}
