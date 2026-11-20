//! Player input (P5): click to spawn or attack, keys for attack size, pause, speed and retreat.
//! Input never changes the game directly; it queues `Intent`s for the next tick.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::client::{ClientSettings, GameState, Notices, PendingIntents};
use crate::engine::{Game, Intent, HUMAN};
use crate::engine_lib::game_map::TileRef;
use crate::render::camera::MapCamera;
use crate::render::world_to_tile;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Hovered>()
            .add_systems(Update, (track_hover, click_map, keyboard));
    }
}

/// Tile under the mouse cursor.
#[derive(Resource, Default)]
pub struct Hovered(pub Option<TileRef>);

/// What a left click on `tile` should do, or why it does nothing.
#[derive(Debug, Clone, PartialEq)]
pub enum ClickAction {
    Spawn(TileRef),
    Attack { target: Option<u16>, troops: f64 },
    Refuse(&'static str),
}

/// Decides what a click does. Pure (no Bevy) so it can be tested.
pub fn click_action(game: &Game, tile: TileRef, attack_ratio: f64) -> ClickAction {
    let map = game.map();
    if game.winner().is_some() {
        return ClickAction::Refuse("The match is over");
    }
    if game.in_spawn_phase() {
        return if map.is_passable_land(tile) && !map.has_owner(tile) {
            ClickAction::Spawn(tile)
        } else {
            ClickAction::Refuse("Pick unclaimed land to spawn")
        };
    }
    let Some(me) = game.player(HUMAN) else {
        return ClickAction::Refuse("No player");
    };
    if !me.is_alive() {
        return ClickAction::Refuse(if me.has_spawned() {
            "You have been eliminated"
        } else {
            "You did not spawn"
        });
    }
    if !map.is_passable_land(tile) {
        return ClickAction::Refuse("Boats are not available yet");
    }
    let target = game.owner(tile);
    if target == Some(HUMAN) {
        return ClickAction::Refuse("That is your land");
    }
    if !game.nearby(HUMAN).contains(&target) {
        return ClickAction::Refuse("You can only attack your neighbours");
    }
    if let Some(t) = target {
        if !game.can_attack_player(HUMAN, t) {
            return ClickAction::Refuse("Spawn immunity is still active");
        }
    }
    let troops = (me.troops() as f64 * attack_ratio).floor();
    if troops < 1.0 {
        return ClickAction::Refuse("Not enough troops");
    }
    ClickAction::Attack { target, troops }
}

fn cursor_tile(
    camera: &Camera,
    cam_transform: &GlobalTransform,
    window: &Window,
    game: &Game,
) -> Option<TileRef> {
    let p = window.cursor_position()?;
    let world = camera.viewport_to_world_2d(cam_transform, p).ok()?;
    world_to_tile(game.map(), world)
}

fn track_hover(
    camera: Single<(&Camera, &GlobalTransform), With<MapCamera>>,
    window: Single<&Window, With<PrimaryWindow>>,
    game: Res<GameState>,
    mut hovered: ResMut<Hovered>,
) {
    let (cam, t) = *camera;
    let tile = cursor_tile(cam, t, &window, &game.0);
    if hovered.0 != tile {
        hovered.0 = tile;
    }
}

fn click_map(
    buttons: Res<ButtonInput<MouseButton>>,
    hovered: Res<Hovered>,
    game: Res<GameState>,
    settings: Res<ClientSettings>,
    mut pending: ResMut<PendingIntents>,
    mut notices: ResMut<Notices>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(tile) = hovered.0 else { return };
    match click_action(&game.0, tile, settings.attack_ratio) {
        ClickAction::Spawn(tile) => pending.0.push(Intent::Spawn {
            player: HUMAN,
            tile,
        }),
        ClickAction::Attack { target, troops } => pending.0.push(Intent::Attack {
            player: HUMAN,
            target,
            troops: Some(troops),
        }),
        ClickAction::Refuse(why) => notices.push(why),
    }
}

fn keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    game: Res<GameState>,
    mut settings: ResMut<ClientSettings>,
    mut pending: ResMut<PendingIntents>,
    mut notices: ResMut<Notices>,
) {
    const DIGITS: [KeyCode; 9] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    for (i, k) in DIGITS.iter().enumerate() {
        if keys.just_pressed(*k) {
            settings.attack_ratio = (i + 1) as f64 / 10.0;
        }
    }
    if keys.just_pressed(KeyCode::Space) {
        settings.paused = !settings.paused;
    }
    for (k, speed) in [(KeyCode::F1, 1), (KeyCode::F2, 2), (KeyCode::F3, 3)] {
        if keys.just_pressed(k) {
            settings.speed = speed;
        }
    }
    if keys.just_pressed(KeyCode::KeyR) {
        // Retreat the newest attack.
        match game
            .0
            .player(HUMAN)
            .and_then(|p| p.outgoing_attacks().last().copied())
        {
            Some(attack) => {
                pending.0.push(Intent::CancelAttack {
                    player: HUMAN,
                    attack,
                });
                notices.push("Retreating (25% losses against players)");
            }
            None => notices.push("No attack to retreat"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GameConfig, MapSize, GAME_CONFIG_PATH};
    use crate::engine::util::spawn_tiles;
    use crate::engine::{new_match, run_turn, MatchSetup};

    fn game() -> Game {
        let cfg = GameConfig::load(GAME_CONFIG_PATH).unwrap();
        new_match(
            cfg,
            MatchSetup {
                size: MapSize::Small,
                seed: 3,
                bots: 4,
            },
        )
        .unwrap()
    }

    #[test]
    fn spawn_phase_clicks_spawn_on_free_land_only() {
        let g = game();
        let map = g.map();
        let land = (0..map.num_tiles())
            .find(|&t| map.is_passable_land(t))
            .unwrap();
        let water = (0..map.num_tiles()).find(|&t| map.is_water(t)).unwrap();
        assert_eq!(click_action(&g, land, 0.2), ClickAction::Spawn(land));
        assert!(matches!(
            click_action(&g, water, 0.2),
            ClickAction::Refuse(_)
        ));
    }

    #[test]
    fn after_spawning_clicks_attack_neighbours_only() {
        let mut g = game();
        run_turn(&mut g, &[]);
        let map = g.map();
        let spawn = (0..map.num_tiles())
            .step_by(97)
            .find(|&t| spawn_tiles(map, t, 4, true).is_some())
            .unwrap();
        run_turn(
            &mut g,
            &[Intent::Spawn {
                player: HUMAN,
                tile: spawn,
            }],
        );
        run_turn(&mut g, &[]);
        assert!(!g.in_spawn_phase());
        let me = g.player(HUMAN).unwrap();
        let troops = me.troops();
        let mut buf = [0; 4];
        // A free land tile touching our border.
        let edge = me
            .border_tiles()
            .iter()
            .find_map(|&b| {
                let n = g.map().neighbors4(b, &mut buf);
                buf[..n]
                    .iter()
                    .copied()
                    .find(|&x| g.map().is_passable_land(x) && !g.map().has_owner(x))
            })
            .unwrap();
        assert_eq!(
            click_action(&g, edge, 0.3),
            ClickAction::Attack {
                target: None,
                troops: (troops as f64 * 0.3).floor()
            }
        );
        assert_eq!(
            click_action(&g, spawn, 0.3),
            ClickAction::Refuse("That is your land")
        );
    }
}
