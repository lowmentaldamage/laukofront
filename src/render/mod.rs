//! Map rendering: one texture with one texel per tile, shown as a sprite. Each tick the tiles
//! that changed owner, and their four neighbours (whose border look may change), are repainted;
//! the texture is only touched when something changed. A new match repaints everything.

pub mod camera;
pub mod effects;
pub mod icons;
pub mod palette;
pub mod perf;
pub mod units;

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::client::{GameState, MapRebuild, TileChanges};
use crate::engine_lib::game_map::{GameMap, TileRef};

/// Map size in world units (1 unit = 1 tile), for the camera.
#[derive(Resource, Debug, Clone, Copy)]
pub struct MapBounds {
    pub size: Vec2,
}

#[derive(Resource)]
pub struct MapTexture(pub Handle<Image>);

#[derive(Component)]
pub struct MapSprite;

pub struct MapRenderPlugin;

impl Plugin for MapRenderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb_u8(10, 12, 16)))
            .init_resource::<camera::CameraZoom>()
            .add_plugins((effects::EffectsPlugin, units::UnitsRenderPlugin))
            .add_systems(Startup, (camera::spawn_camera, setup_map))
            .add_systems(Update, camera::camera_controls)
            .add_systems(PostUpdate, (rebuild_map, repaint_changed_tiles).chain())
            .add_systems(Last, clear_rebuild_flag);
    }
}

/// World position of a tile's centre.
pub fn tile_center(map: &GameMap, t: TileRef) -> Vec2 {
    Vec2::new(
        map.x(t) as f32 + 0.5 - map.width() as f32 / 2.0,
        map.height() as f32 / 2.0 - map.y(t) as f32 - 0.5,
    )
}

/// Tile under a world position, or `None` outside the map.
pub fn world_to_tile(map: &GameMap, world: Vec2) -> Option<TileRef> {
    let x = (world.x + map.width() as f32 / 2.0).floor() as i64;
    let y = (map.height() as f32 / 2.0 - world.y).floor() as i64;
    map.tile(x, y)
}

/// RGBA bytes for every tile of the map.
pub fn paint_all(map: &GameMap) -> Vec<u8> {
    let mut data = Vec::with_capacity(map.num_tiles() as usize * 4);
    for t in 0..map.num_tiles() {
        data.extend_from_slice(&palette::tile_color(map, t));
    }
    data
}

/// A texture the size of the map (one texel per tile), nearest-sampled.
pub fn map_sized_image(map: &GameMap, data: Vec<u8>) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: map.width(),
            height: map.height(),
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    image
}

fn setup_map(mut commands: Commands, game: Res<GameState>, mut images: ResMut<Assets<Image>>) {
    palette::set_teams(&game.0);
    let map = game.0.map();
    let handle = images.add(map_sized_image(map, paint_all(map)));
    commands.spawn((
        Sprite {
            image: handle.clone(),
            custom_size: Some(Vec2::new(map.width() as f32, map.height() as f32)),
            ..default()
        },
        MapSprite,
    ));
    commands.insert_resource(MapTexture(handle));
    commands.insert_resource(MapBounds {
        size: Vec2::new(map.width() as f32, map.height() as f32),
    });
}

/// After a restart: new texture contents (and size, if the map size changed).
fn rebuild_map(
    rebuild: Res<MapRebuild>,
    game: Res<GameState>,
    texture: Option<Res<MapTexture>>,
    mut images: ResMut<Assets<Image>>,
    mut sprites: Query<&mut Sprite, With<MapSprite>>,
    mut bounds: ResMut<MapBounds>,
    mut zoom: ResMut<camera::CameraZoom>,
) {
    if !rebuild.0 {
        return;
    }
    let Some(texture) = texture else { return };
    palette::set_teams(&game.0);
    let map = game.0.map();
    let _ = images.insert(&texture.0, map_sized_image(map, paint_all(map)));
    let size = Vec2::new(map.width() as f32, map.height() as f32);
    for mut s in sprites.iter_mut() {
        s.custom_size = Some(size);
    }
    bounds.size = size;
    zoom.0 = 1.0;
}

fn clear_rebuild_flag(mut rebuild: ResMut<MapRebuild>) {
    if rebuild.0 {
        rebuild.0 = false;
    }
}

fn repaint_changed_tiles(
    game: Res<GameState>,
    mut changes: ResMut<TileChanges>,
    texture: Option<Res<MapTexture>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(texture) = texture else { return };
    if changes.0.is_empty() {
        return;
    }
    let Some(mut image) = images.get_mut(&texture.0) else {
        return;
    };
    let Some(data) = image.data.as_mut() else {
        return;
    };
    let map = game.0.map();
    // A mass annexation: one full repaint is cheaper than five paints per changed tile.
    if changes.0.len() > 50_000 {
        let all = paint_all(map);
        if all.len() == data.len() {
            data.copy_from_slice(&all);
        }
        changes.0.clear();
        return;
    }
    let mut buf = [0; 4];
    let mut paint = |t: TileRef| {
        let i = t as usize * 4;
        if i + 4 <= data.len() {
            data[i..i + 4].copy_from_slice(&palette::tile_color(map, t));
        }
    };
    for &t in &changes.0 {
        paint(t);
        let n = map.neighbors4(t, &mut buf);
        for &nb in &buf[..n] {
            paint(nb);
        }
    }
    changes.0.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine_lib::game_map::terrain_byte;

    #[test]
    fn tile_and_world_round_trip() {
        let map = GameMap::new(40, 30, vec![terrain_byte(true, false, false, 1); 1200]).unwrap();
        for t in [0, 39, 40, 615, 1199] {
            assert_eq!(world_to_tile(&map, tile_center(&map, t)), Some(t));
        }
        assert_eq!(world_to_tile(&map, Vec2::new(-21.0, 0.0)), None);
        assert_eq!(world_to_tile(&map, Vec2::new(0.0, 15.5)), None);
    }

    #[test]
    fn paint_all_has_one_texel_per_tile() {
        let map = GameMap::new(7, 5, vec![terrain_byte(true, false, false, 1); 35]).unwrap();
        assert_eq!(paint_all(&map).len(), 35 * 4);
    }
}
