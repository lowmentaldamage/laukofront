//! Visual effects over the map:
//! - **capture flash:** newly conquered tiles flash in a light tint of the new owner's colour and
//!   fade out over `FLASH_SECS`, drawn in an overlay texture above the map;
//! - **your attack fronts** shimmer;
//! - **spawn highlight:** for the first `HIGHLIGHT_SECS` after you spawn, pulsing rings mark your
//!   start position.

use bevy::prelude::*;

use crate::client::{CaptureEvents, GameState, MapRebuild, MatchStats};
use crate::engine::HUMAN;
use crate::engine_lib::game_map::TileRef;

use super::{map_sized_image, palette, tile_center};

/// How long a capture flash lasts.
pub const FLASH_SECS: f32 = 0.6;
/// Strongest alpha of a flash (0-255).
const FLASH_ALPHA: f32 = 190.0;
/// Most flashes kept at once (oldest are dropped first).
const MAX_FLASHES: usize = 60_000;
/// How long your start position is highlighted.
pub const HIGHLIGHT_SECS: f32 = 30.0;

#[derive(Resource)]
pub struct OverlayTexture(pub Handle<Image>);

#[derive(Component)]
struct OverlaySprite;

/// Active flashes: tile, new owner, age in seconds.
#[derive(Resource, Default)]
struct Flashes {
    active: Vec<(TileRef, u16, f32)>,
    front: Vec<TileRef>,
}

pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Flashes>()
            .add_systems(Startup, setup_overlay.after(super::setup_map))
            .add_systems(PostUpdate, update_overlay.after(super::rebuild_map))
            .add_systems(Update, spawn_highlight);
    }
}

/// Alpha of a flash of `age` seconds (0 when finished).
pub fn flash_alpha(age: f32) -> u8 {
    if age >= FLASH_SECS || age < 0.0 {
        0
    } else {
        (FLASH_ALPHA * (1.0 - age / FLASH_SECS)) as u8
    }
}

fn setup_overlay(mut commands: Commands, game: Res<GameState>, mut images: ResMut<Assets<Image>>) {
    let map = game.0.map();
    let data = vec![0u8; map.num_tiles() as usize * 4];
    let handle = images.add(map_sized_image(map, data));
    commands.spawn((
        Sprite {
            image: handle.clone(),
            custom_size: Some(Vec2::new(map.width() as f32, map.height() as f32)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 1.0),
        OverlaySprite,
    ));
    commands.insert_resource(OverlayTexture(handle));
}

#[allow(clippy::too_many_arguments)]
fn update_overlay(
    time: Res<Time>,
    game: Res<GameState>,
    rebuild: Res<MapRebuild>,
    mut captures: ResMut<CaptureEvents>,
    mut flashes: ResMut<Flashes>,
    overlay: Option<Res<OverlayTexture>>,
    mut images: ResMut<Assets<Image>>,
    mut sprites: Query<&mut Sprite, With<OverlaySprite>>,
) {
    let Some(overlay) = overlay else { return };
    let map = game.0.map();
    if rebuild.0 {
        let data = vec![0u8; map.num_tiles() as usize * 4];
        let _ = images.insert(&overlay.0, map_sized_image(map, data));
        for mut s in sprites.iter_mut() {
            s.custom_size = Some(Vec2::new(map.width() as f32, map.height() as f32));
        }
        flashes.active.clear();
        flashes.front.clear();
        captures.0.clear();
        return;
    }

    // Your attack fronts this frame.
    let mut front: Vec<TileRef> = Vec::new();
    if let Some(me) = game.0.player(HUMAN) {
        for &a in me.outgoing_attacks() {
            if let Some(at) = game.0.attack(a) {
                front.extend(at.border_tiles().iter().copied().take(4_000));
            }
        }
    }
    if captures.0.is_empty()
        && flashes.active.is_empty()
        && front.is_empty()
        && flashes.front.is_empty()
    {
        return;
    }
    let Some(mut image) = images.get_mut(&overlay.0) else {
        return;
    };
    let Some(data) = image.data.as_mut() else {
        return;
    };
    let n = data.len();
    let mut put = |t: TileRef, rgba: [u8; 4]| {
        let i = t as usize * 4;
        if i + 4 <= n {
            data[i..i + 4].copy_from_slice(&rgba);
        }
    };

    // Clear last frame's front, age and draw flashes, then draw the new front on top.
    for &t in &flashes.front {
        put(t, [0, 0, 0, 0]);
    }
    let dt = time.delta_secs();
    // Flash at most ~20 000 new tiles per frame (a sample of mass annexations).
    let step = (captures.0.len() / 20_000).max(1);
    for (t, owner) in captures.0.drain(..).step_by(step) {
        flashes.active.push((t, owner, 0.0));
    }
    if flashes.active.len() > MAX_FLASHES {
        let drop = flashes.active.len() - MAX_FLASHES;
        for &(t, _, _) in &flashes.active[..drop] {
            put(t, [0, 0, 0, 0]);
        }
        flashes.active.drain(..drop);
    }
    flashes.active.retain_mut(|(t, owner, age)| {
        *age += dt;
        let a = flash_alpha(*age);
        if a == 0 {
            put(*t, [0, 0, 0, 0]);
            return false;
        }
        let c = if *owner == 0 {
            [255, 255, 255]
        } else {
            palette::player_color(*owner)
        };
        let light = |x: u8| x / 3 + 170;
        put(*t, [light(c[0]), light(c[1]), light(c[2]), a]);
        true
    });
    let pulse = (time.elapsed_secs() * 6.0).sin() * 0.5 + 0.5;
    let a = (60.0 + 110.0 * pulse) as u8;
    for &t in &front {
        put(t, [255, 236, 150, a]);
    }
    flashes.front = front;
}

fn spawn_highlight(
    game: Res<GameState>,
    stats: Res<MatchStats>,
    time: Res<Time>,
    mut gizmos: Gizmos,
) {
    let Some(age) = stats.since_spawn else { return };
    if age > HIGHLIGHT_SECS {
        return;
    }
    let Some(spawn) = game.0.player(HUMAN).and_then(|p| p.spawn_tile()) else {
        return;
    };
    let c = tile_center(game.0.map(), spawn);
    let fade = 1.0 - age / HIGHLIGHT_SECS;
    let t = time.elapsed_secs();
    for k in 0..3 {
        let phase = (t * 0.8 + k as f32 / 3.0).fract();
        let r = 6.0 + phase * 40.0;
        let alpha = fade * (1.0 - phase);
        gizmos.circle_2d(c, r, Color::srgba(1.0, 0.85, 0.35, alpha));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flash_fades_to_zero() {
        assert_eq!(flash_alpha(0.0), FLASH_ALPHA as u8);
        assert!(flash_alpha(FLASH_SECS / 2.0) < flash_alpha(0.0));
        assert_eq!(flash_alpha(FLASH_SECS), 0);
        assert_eq!(flash_alpha(-1.0), 0);
    }
}
