//! Minimap (bottom right): the whole map at 1/`SCALE` resolution, refreshed once per second of
//! game time, with the camera's view drawn as a rectangle. Clicking it moves the camera there.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::{PANEL, PANEL_BORDER};
use crate::client::{GameState, MapRebuild};
use crate::engine_lib::game_map::GameMap;
use crate::render::camera::MapCamera;
use crate::render::{map_sized_image, palette, MapBounds};

/// Tiles per minimap pixel along each axis.
pub const SCALE: u32 = 4;
/// Minimap width on screen in logical pixels (height follows the map's aspect ratio).
const WIDTH: f32 = 260.0;
const MARGIN: f32 = 8.0;

#[derive(Resource)]
struct MinimapImage {
    handle: Handle<Image>,
    last_tick: u64,
}

#[derive(Component)]
struct MinimapNode;

#[derive(Component)]
struct ViewRect;

pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, (refresh, view_rect, click));
    }
}

/// Minimap pixel size for a map.
pub fn minimap_dims(map: &GameMap) -> (u32, u32) {
    (map.width().div_ceil(SCALE), map.height().div_ceil(SCALE))
}

/// RGBA bytes of the minimap: the colour of the tile at the centre of each block.
pub fn minimap_pixels(map: &GameMap) -> Vec<u8> {
    let (w, h) = minimap_dims(map);
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let tx = (x * SCALE + SCALE / 2).min(map.width() - 1);
            let ty = (y * SCALE + SCALE / 2).min(map.height() - 1);
            let t = ty * map.width() + tx;
            let mut c = palette::terrain_color(map, t);
            let o = map.owner_id(t);
            if o != 0 {
                let pc = palette::player_color(o);
                c = if o == crate::engine::HUMAN {
                    palette::OWN_BORDER
                } else {
                    [pc[0], pc[1], pc[2], 255]
                };
            }
            out.extend_from_slice(&c);
        }
    }
    out
}

fn small_image(map: &GameMap) -> Image {
    let (w, h) = minimap_dims(map);
    // Build a tiny map-sized image of the right dimensions by reusing the map image helper.
    let tiny = GameMap::new(w, h, vec![0; (w * h) as usize]);
    match tiny {
        Some(t) => map_sized_image(&t, minimap_pixels(map)),
        None => Image::default(),
    }
}

fn node_height(map: &GameMap) -> f32 {
    WIDTH * map.height() as f32 / map.width() as f32
}

fn setup(mut commands: Commands, game: Res<GameState>, mut images: ResMut<Assets<Image>>) {
    let map = game.0.map();
    let handle = images.add(small_image(map));
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(MARGIN),
                bottom: px(MARGIN),
                width: px(WIDTH),
                height: px(node_height(map)),
                border: UiRect::all(px(2)),
                ..default()
            },
            BackgroundColor(PANEL),
            BorderColor::all(PANEL_BORDER),
            ImageNode::new(handle.clone()),
            MinimapNode,
        ))
        .with_child((
            Node {
                position_type: PositionType::Absolute,
                border: UiRect::all(px(1)),
                ..default()
            },
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.9)),
            ViewRect,
        ));
    commands.insert_resource(MinimapImage {
        handle,
        last_tick: 0,
    });
}

fn refresh(
    game: Res<GameState>,
    rebuild: Res<MapRebuild>,
    mut mini: ResMut<MinimapImage>,
    mut images: ResMut<Assets<Image>>,
    mut nodes: Query<&mut Node, With<MinimapNode>>,
) {
    let g = &game.0;
    if !rebuild.0 && g.ticks() < mini.last_tick + 10 && mini.last_tick != 0 {
        return;
    }
    mini.last_tick = g.ticks().max(1);
    let _ = images.insert(&mini.handle, small_image(g.map()));
    if rebuild.0 {
        for mut n in nodes.iter_mut() {
            n.height = px(node_height(g.map()));
        }
    }
}

/// The camera's view in map fractions (0..1), clamped: (left, top, width, height).
pub fn view_fraction(center: Vec2, view_size: Vec2, map_size: Vec2) -> (f32, f32, f32, f32) {
    let left = ((center.x - view_size.x / 2.0) / map_size.x + 0.5).clamp(0.0, 1.0);
    let right = ((center.x + view_size.x / 2.0) / map_size.x + 0.5).clamp(0.0, 1.0);
    let top = (0.5 - (center.y + view_size.y / 2.0) / map_size.y).clamp(0.0, 1.0);
    let bottom = (0.5 - (center.y - view_size.y / 2.0) / map_size.y).clamp(0.0, 1.0);
    (left, top, right - left, bottom - top)
}

fn view_rect(
    camera: Single<(&Transform, &Projection), With<MapCamera>>,
    window: Single<&Window, With<PrimaryWindow>>,
    bounds: Option<Res<MapBounds>>,
    game: Res<GameState>,
    mut rects: Query<&mut Node, With<ViewRect>>,
) {
    let Some(bounds) = bounds else { return };
    let (t, proj) = *camera;
    let Projection::Orthographic(o) = proj else {
        return;
    };
    let view = Vec2::new(window.width(), window.height()) * o.scale;
    let (l, top, w, h) = view_fraction(t.translation.truncate(), view, bounds.size);
    let mh = node_height(game.0.map()) - 4.0;
    let mw = WIDTH - 4.0;
    for mut n in rects.iter_mut() {
        n.left = px(l * mw);
        n.top = px(top * mh);
        n.width = px((w * mw).max(2.0));
        n.height = px((h * mh).max(2.0));
    }
}

fn click(
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    game: Res<GameState>,
    bounds: Option<Res<MapBounds>>,
    mut camera: Single<&mut Transform, With<MapCamera>>,
) {
    if !buttons.pressed(MouseButton::Left) {
        return;
    }
    let (Some(cursor), Some(bounds)) = (window.cursor_position(), bounds) else {
        return;
    };
    let mh = node_height(game.0.map());
    let x0 = window.width() - MARGIN - WIDTH;
    let y0 = window.height() - MARGIN - mh;
    let fx = (cursor.x - x0) / WIDTH;
    let fy = (cursor.y - y0) / mh;
    if !(0.0..=1.0).contains(&fx) || !(0.0..=1.0).contains(&fy) {
        return;
    }
    camera.translation.x = (fx - 0.5) * bounds.size.x;
    camera.translation.y = (0.5 - fy) * bounds.size.y;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine_lib::game_map::terrain_byte;

    #[test]
    fn minimap_size_and_pixels() {
        let map = GameMap::new(41, 22, vec![terrain_byte(true, false, false, 1); 41 * 22]).unwrap();
        assert_eq!(minimap_dims(&map), (11, 6));
        assert_eq!(minimap_pixels(&map).len(), 11 * 6 * 4);
    }

    #[test]
    fn view_fraction_of_whole_and_quarter_view() {
        let map = Vec2::new(100.0, 50.0);
        assert_eq!(view_fraction(Vec2::ZERO, map, map), (0.0, 0.0, 1.0, 1.0));
        let (l, t, w, h) = view_fraction(Vec2::new(25.0, 12.5), map / 2.0, map);
        assert_eq!((l, t, w, h), (0.5, 0.0, 0.5, 0.5));
    }
}
