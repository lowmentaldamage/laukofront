//! Map rendering (P5): one texture with one texel per tile, shown as a sprite. Each tick the
//! tiles that changed owner, and their four neighbours (whose border look may change), are
//! repainted; the texture is only touched when something changed.

pub mod camera;
pub mod palette;
pub mod perf;

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::client::{GameState, TileChanges};
use crate::engine_lib::game_map::{GameMap, TileRef};

/// Map size in world units (1 unit = 1 tile), for the camera.
#[derive(Resource, Debug, Clone, Copy)]
pub struct MapBounds {
    pub size: Vec2,
}

#[derive(Resource)]
pub struct MapTexture(pub Handle<Image>);

pub struct MapRenderPlugin;

impl Plugin for MapRenderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb_u8(10, 12, 16)))
            .init_resource::<camera::CameraZoom>()
            .add_systems(Startup, (camera::spawn_camera, setup_map))
            .add_systems(Update, camera::camera_controls)
            .add_systems(PostUpdate, repaint_changed_tiles);
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

fn setup_map(mut commands: Commands, game: Res<GameState>, mut images: ResMut<Assets<Image>>) {
    let map = game.0.map();
    let (w, h) = (map.width(), map.height());
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for t in 0..map.num_tiles() {
        data.extend_from_slice(&palette::tile_color(map, t));
    }
    let mut image = Image::new(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    // Crisp tiles when zoomed in.
    image.sampler = ImageSampler::nearest();
    let handle = images.add(image);
    commands.spawn(Sprite {
        image: handle.clone(),
        custom_size: Some(Vec2::new(w as f32, h as f32)),
        ..default()
    });
    commands.insert_resource(MapTexture(handle));
    commands.insert_resource(MapBounds {
        size: Vec2::new(w as f32, h as f32),
    });
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
    let mut buf = [0; 4];
    let mut paint = |t: TileRef| {
        let i = t as usize * 4;
        data[i..i + 4].copy_from_slice(&palette::tile_color(map, t));
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
}
