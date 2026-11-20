//! Player names and troop counts written on the map (sized by territory), and the pulsing
//! "YOU" marker over your land for the first 30 seconds.

use bevy::prelude::*;
use bevy::sprite::{Anchor, Text2dShadow};

use super::{fmt_num, font, ACCENT, DANGER, GOOD};
use crate::client::{GameState, MapRebuild, MatchStats};
use crate::engine::diplomacy::REACTIONS;
use crate::engine::{Game, PlayerId, HUMAN};
use crate::render::camera::MapCamera;
use crate::render::effects::HIGHLIGHT_SECS;
use crate::render::tile_center;

/// Players with fewer tiles get no label.
const MIN_TILES: u32 = 60;
/// Glyphs are rasterised at this size and scaled in world space.
const RASTER_PX: f32 = 48.0;

/// Where each label goes.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelPlacement {
    pub id: PlayerId,
    pub pos: Vec2,
    /// Text height in world units (tiles).
    pub size: f32,
    pub text: String,
    pub color: Color,
}

#[derive(Resource, Default)]
pub struct Labels {
    pub placements: Vec<LabelPlacement>,
    last_tick: Option<u64>,
}

impl Labels {
    pub fn human_center(&self) -> Option<Vec2> {
        self.placements
            .iter()
            .find(|p| p.id == HUMAN)
            .map(|p| p.pos)
    }
}

#[derive(Component)]
struct MapLabel(PlayerId);

#[derive(Component)]
struct YouMarker;

pub struct LabelsPlugin;

impl Plugin for LabelsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Labels>()
            .add_systems(Startup, spawn_you_marker)
            .add_systems(Update, (compute, sync_entities, you_marker).chain());
    }
}

/// Label placement for every player big enough: the average position of its tiles, moved onto
/// its own land if the average falls outside (split or curved territory). Pure for tests.
pub fn place_labels(game: &Game) -> Vec<LabelPlacement> {
    let map = game.map();
    let mut out = Vec::new();
    for p in game.alive_players() {
        let n = p.num_tiles();
        if n < MIN_TILES && p.id() != HUMAN {
            continue;
        }
        let (mut sx, mut sy) = (0f64, 0f64);
        for &t in p.tiles() {
            sx += map.x(t) as f64;
            sy += map.y(t) as f64;
        }
        let (cx, cy) = ((sx / n as f64) as u32, (sy / n as f64) as u32);
        let mut tile = map.tile(cx as i64, cy as i64).unwrap_or(p.tiles()[0]);
        if map.owner_id(tile) != p.id() {
            let mut best = (u64::MAX, tile);
            for &t in p.tiles() {
                let dx = map.x(t).abs_diff(cx) as u64;
                let dy = map.y(t).abs_diff(cy) as u64;
                let d = dx * dx + dy * dy;
                if d < best.0 {
                    best = (d, t);
                }
            }
            tile = best.1;
        }
        let size = ((n as f32).sqrt() * 0.13).clamp(2.0, 36.0);
        let name = if p.id() == HUMAN { "You" } else { p.name() };
        let mut text = format!("{name}\n{}", fmt_num(p.troops() as f64));
        let id = p.id();
        let allied = game.is_allied(HUMAN, id);
        if allied {
            text.push_str("\nally");
        }
        if game.is_traitor(id) {
            text.push_str("\nTRAITOR");
        }
        if game.targets(HUMAN).contains(&id)
            || game
                .allies(HUMAN)
                .iter()
                .any(|&a| game.targets(a).contains(&id))
        {
            text.push_str("\n(target)");
        }
        // The newest reaction this player showed to you or to everyone.
        if let Some(r) = game
            .reactions()
            .iter()
            .rev()
            .find(|r| r.from == id && (r.to.is_none() || r.to == Some(HUMAN) || id == HUMAN))
        {
            text.push_str(&format!("\n\"{}\"", REACTIONS[r.what as usize]));
        }
        let color = if id == HUMAN {
            ACCENT
        } else if game.is_traitor(id) {
            DANGER
        } else if allied {
            GOOD
        } else {
            Color::srgba(1.0, 1.0, 1.0, 0.92)
        };
        out.push(LabelPlacement {
            id,
            pos: tile_center(map, tile),
            size,
            text,
            color,
        });
    }
    out
}

fn compute(game: Res<GameState>, rebuild: Res<MapRebuild>, mut labels: ResMut<Labels>) {
    let t = game.0.ticks();
    if !rebuild.0 && labels.last_tick.is_some_and(|l| t < l + 10) {
        return;
    }
    labels.last_tick = Some(t);
    labels.placements = place_labels(&game.0);
}

fn sync_entities(
    mut commands: Commands,
    labels: Res<Labels>,
    mut existing: Query<(
        Entity,
        &MapLabel,
        &mut Text2d,
        &mut Transform,
        &mut TextColor,
    )>,
) {
    if !labels.is_changed() {
        return;
    }
    let mut seen: Vec<PlayerId> = Vec::new();
    for (e, l, mut text, mut tf, mut color) in existing.iter_mut() {
        match labels.placements.iter().find(|p| p.id == l.0) {
            Some(p) => {
                if text.0 != p.text {
                    text.0 = p.text.clone();
                }
                if color.0 != p.color {
                    color.0 = p.color;
                }
                tf.translation = p.pos.extend(5.0);
                tf.scale = Vec3::splat(p.size / RASTER_PX);
                seen.push(l.0);
            }
            None => commands.entity(e).despawn(),
        }
    }
    for p in labels.placements.iter().filter(|p| !seen.contains(&p.id)) {
        let color = p.color;
        commands.spawn((
            Text2d::new(p.text.clone()),
            font(RASTER_PX),
            TextColor(color),
            TextLayout::justify(Justify::Center),
            Text2dShadow {
                offset: Vec2::new(3.0, -3.0),
                color: Color::srgba(0.0, 0.0, 0.0, 0.75),
            },
            Anchor::CENTER,
            Transform::from_translation(p.pos.extend(5.0))
                .with_scale(Vec3::splat(p.size / RASTER_PX)),
            MapLabel(p.id),
        ));
    }
}

fn spawn_you_marker(mut commands: Commands) {
    commands.spawn((
        Text::new("YOU"),
        font(22.0),
        TextColor(ACCENT),
        TextShadow::default(),
        Node {
            position_type: PositionType::Absolute,
            display: Display::None,
            ..default()
        },
        YouMarker,
    ));
}

fn you_marker(
    labels: Res<Labels>,
    stats: Res<MatchStats>,
    time: Res<Time>,
    camera: Single<(&Camera, &GlobalTransform), With<MapCamera>>,
    mut q: Query<(&mut Node, &mut TextColor), With<YouMarker>>,
) {
    let (cam, cam_t) = *camera;
    let active = stats.since_spawn.filter(|&s| s < HIGHLIGHT_SECS);
    let screen = labels
        .human_center()
        .and_then(|c| cam.world_to_viewport(cam_t, c.extend(0.0)).ok());
    for (mut node, mut color) in q.iter_mut() {
        match (active, screen) {
            (Some(age), Some(p)) => {
                node.display = Display::Flex;
                let bob = (time.elapsed_secs() * 4.0).sin() * 4.0;
                node.left = px(p.x - 22.0);
                node.top = px(p.y - 70.0 + bob);
                let fade = 1.0 - age / HIGHLIGHT_SECS;
                color.0 = ACCENT.with_alpha(0.4 + 0.6 * fade);
            }
            _ => {
                if node.display != Display::None {
                    node.display = Display::None;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GameConfig, MapSize, GAME_CONFIG_PATH};
    use crate::engine::{new_match, run_turn, MatchSetup};

    #[test]
    fn labels_sit_on_their_owners_land() {
        let cfg = GameConfig::load(GAME_CONFIG_PATH).unwrap();
        let mut g = new_match(cfg, MatchSetup::bots_only(MapSize::Small, 6, 8)).unwrap();
        for _ in 0..3_000 {
            run_turn(&mut g, &[]);
        }
        let labels = place_labels(&g);
        assert!(!labels.is_empty());
        for l in &labels {
            let t = crate::render::world_to_tile(g.map(), l.pos).unwrap();
            assert_eq!(g.owner(t), Some(l.id), "label of {} is off its land", l.id);
            assert!(l.text.contains('\n'));
        }
        // Bigger territory, bigger label.
        let big = labels
            .iter()
            .max_by_key(|l| g.player(l.id).unwrap().num_tiles())
            .unwrap();
        let small = labels
            .iter()
            .min_by_key(|l| g.player(l.id).unwrap().num_tiles())
            .unwrap();
        assert!(big.size >= small.size);
    }
}
