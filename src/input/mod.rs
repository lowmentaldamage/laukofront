//! Player input (P5): click to spawn or attack, keys for attack size, pause, speed and retreat.
//! Input never changes the game directly; it queues `Intent`s for the next tick.

use bevy::picking::hover::HoverMap;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::client::{ClientSettings, GameState, Notices, PendingIntents};
use crate::engine::boats::can_send_boat;
use crate::engine::structures::{can_upgrade, placement};
use crate::engine::units::{UnitId, UnitKind};
use crate::engine::weapons::{can_launch, warship_spawn};
use crate::engine::{Game, Intent, HUMAN};
use crate::engine_lib::game_map::TileRef;
use crate::render::camera::MapCamera;
use crate::render::world_to_tile;
use crate::ui::pointer_over_ui;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Hovered>()
            .init_resource::<PlayerActions>()
            .add_systems(
                Update,
                (track_hover, click_map, keyboard, track_size_changes),
            );
    }
}

/// What the player has done this match (drives the tutorial).
#[derive(Resource, Default, Debug, Clone, PartialEq)]
pub struct PlayerActions {
    /// Sent an attack into free land.
    pub attacked_land: bool,
    /// Changed the attack size.
    pub changed_size: bool,
    /// Sent an attack at another player.
    pub attacked_player: bool,
}

/// Tile under the mouse cursor (`None` over the HUD or outside the map).
#[derive(Resource, Default)]
pub struct Hovered(pub Option<TileRef>);

/// What a left click on `tile` should do, or why it does nothing.
#[derive(Debug, Clone, PartialEq)]
pub enum ClickAction {
    Spawn(TileRef),
    Attack {
        target: Option<u16>,
        troops: f64,
    },
    /// Send a transport ship at `tile` (across water).
    Boat {
        tile: TileRef,
        troops: f64,
    },
    /// Place a building (at the snapped tile).
    Build {
        kind: UnitKind,
        tile: TileRef,
    },
    /// Upgrade an own building.
    Upgrade(UnitId),
    /// Launch a missile at `tile`.
    Nuke {
        kind: UnitKind,
        tile: TileRef,
    },
    /// Build a warship that patrols around `tile`.
    Warship {
        tile: TileRef,
    },
    /// Select an own warship.
    SelectWarship(UnitId),
    /// Move the selected warship.
    MoveWarship {
        unit: UnitId,
        tile: TileRef,
    },
    Refuse(&'static str),
}

/// Decides what a click does without a building selected.
pub fn click_action(game: &Game, tile: TileRef, attack_ratio: f64) -> ClickAction {
    click_action_with(game, tile, attack_ratio, None, None)
}

/// Decides what a click does. Pure (no Bevy) so it can be tested.
/// - building selected: place it there (ports snap to the coast);
/// - your own land: upgrade the building you clicked (within 2 tiles), if any;
/// - a neighbour's land or free land next to you: attack;
/// - land across the water: send a transport ship.
pub fn click_action_with(
    game: &Game,
    tile: TileRef,
    attack_ratio: f64,
    build: Option<UnitKind>,
    selected_ship: Option<UnitId>,
) -> ClickAction {
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
    if let Some(kind) = build {
        if kind.is_nuke() {
            return match can_launch(game, HUMAN, kind, tile) {
                Ok(_) => ClickAction::Nuke { kind, tile },
                Err(why) => ClickAction::Refuse(why),
            };
        }
        if kind == UnitKind::Warship {
            return match warship_spawn(game, HUMAN, tile) {
                Ok(_) => ClickAction::Warship { tile },
                Err(why) => ClickAction::Refuse(why),
            };
        }
        return match placement(game, HUMAN, kind, tile) {
            Ok(at) => ClickAction::Build { kind, tile: at },
            Err(why) => ClickAction::Refuse(why),
        };
    }
    // Your warships: click one to select it, then click water to send it there.
    let own_ship = game
        .player_units(HUMAN, Some(UnitKind::Warship))
        .filter(|u| map.manhattan_dist(u.tile(), tile) <= 3)
        .min_by_key(|u| map.manhattan_dist(u.tile(), tile))
        .map(|u| u.id());
    if let Some(ship) = own_ship {
        return ClickAction::SelectWarship(ship);
    }
    if let Some(ship) = selected_ship.filter(|&s| game.unit(s).is_some_and(|u| u.is_active())) {
        if map.is_water(tile) {
            return ClickAction::MoveWarship { unit: ship, tile };
        }
    }
    if !map.is_passable_land(tile) {
        return ClickAction::Refuse("Click land across the water to send a boat");
    }
    let target = game.owner(tile);
    if target == Some(HUMAN) {
        let own = game
            .player_units(HUMAN, None)
            .filter(|u| u.kind().is_structure() && map.manhattan_dist(u.tile(), tile) <= 2)
            .min_by_key(|u| map.manhattan_dist(u.tile(), tile))
            .map(|u| u.id());
        return match own {
            Some(unit) => match can_upgrade(game, HUMAN, unit) {
                Ok(_) => ClickAction::Upgrade(unit),
                Err(why) => ClickAction::Refuse(why),
            },
            None => ClickAction::Refuse("That is your land"),
        };
    }
    if let Some(t) = target {
        if game.is_friendly(HUMAN, t) {
            return ClickAction::Refuse("That is your ally (right click for diplomacy)");
        }
        if !game.can_attack_player(HUMAN, t) {
            return ClickAction::Refuse("Spawn immunity is still active");
        }
    }
    let troops = (me.troops() as f64 * attack_ratio).floor();
    if troops < 1.0 {
        return ClickAction::Refuse("Not enough troops");
    }
    if game.can_attack_tile(HUMAN, tile) {
        return ClickAction::Attack { target, troops };
    }
    if can_send_boat(game, HUMAN, tile).is_some() {
        return ClickAction::Boat { tile, troops };
    }
    if game.unit_count(HUMAN, UnitKind::TransportShip)
        >= game.config().boats.max_transport_ships as usize
    {
        return ClickAction::Refuse("All 3 boats are at sea");
    }
    // Say why no boat can go.
    let touches_sea = me.border_tiles().iter().any(|&b| map.is_shore(b));
    if !touches_sea {
        return ClickAction::Refuse("No boats yet: expand your land to the coast first");
    }
    ClickAction::Refuse("Not reachable: that land is not on a sea your coast touches")
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
    hover_map: Res<HoverMap>,
    nodes: Query<(), With<Node>>,
    mut hovered: ResMut<Hovered>,
) {
    let (cam, t) = *camera;
    let tile = if pointer_over_ui(&hover_map, &nodes) {
        None
    } else {
        cursor_tile(cam, t, &window, &game.0)
    };
    if hovered.0 != tile {
        hovered.0 = tile;
    }
}

#[allow(clippy::too_many_arguments)]
fn click_map(
    buttons: Res<ButtonInput<MouseButton>>,
    hovered: Res<Hovered>,
    game: Res<GameState>,
    mut pending: ResMut<PendingIntents>,
    mut notices: ResMut<Notices>,
    mut actions: ResMut<PlayerActions>,
    mut settings: ResMut<ClientSettings>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut right_press: Local<Option<Vec2>>,
    screen: Res<crate::client::Screen>,
) {
    if *screen != crate::client::Screen::Playing {
        return;
    }
    if buttons.just_pressed(MouseButton::Right) {
        if settings.build.is_some() {
            settings.build = None;
            *right_press = None;
            return;
        }
        *right_press = window.cursor_position();
    }
    if buttons.just_released(MouseButton::Right) {
        // A right click (not a camera drag) opens the action menu for that tile (upstream's
        // radial menu): attack, boat, build, warship and diplomacy, whatever fits there.
        let pos = window.cursor_position();
        let clicked = matches!((*right_press, pos), (Some(a), Some(b)) if a.distance(b) < 6.0);
        *right_press = None;
        if clicked {
            settings.menu_tile = hovered.0;
            settings.menu_pos = pos.unwrap_or_default();
        }
        return;
    }
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(tile) = hovered.0 else { return };
    if settings.menu_tile.take().is_some() {
        // A click on the map closes the menu and does nothing else.
        return;
    }
    match click_action_with(
        &game.0,
        tile,
        settings.attack_ratio,
        settings.build,
        settings.selected_ship,
    ) {
        ClickAction::Spawn(tile) => pending.0.push(Intent::Spawn {
            player: HUMAN,
            tile,
        }),
        ClickAction::Attack { target, troops } => {
            if target.is_some() {
                actions.attacked_player = true;
            } else {
                actions.attacked_land = true;
            }
            pending.0.push(Intent::Attack {
                player: HUMAN,
                target,
                troops: Some(troops),
            })
        }
        ClickAction::Boat { tile, troops } => {
            actions.attacked_land = true;
            pending.0.push(Intent::Boat {
                player: HUMAN,
                tile,
                troops: Some(troops),
            });
            let eta = can_send_boat(&game.0, HUMAN, tile).map(|(s, d)| {
                game.0.map().manhattan_dist(s, d) as f64 * 0.8 / game.0.config().game.tick_hz as f64
            });
            notices.push(match eta {
                Some(s) => format!("Boat sailing with {troops:.0} troops, lands in about {s:.0} s"),
                None => format!("Boat sailing with {troops:.0} troops"),
            });
        }
        ClickAction::Build { kind, tile } => {
            pending.0.push(Intent::Build {
                player: HUMAN,
                kind,
                tile,
            });
            settings.build = None;
            notices.push(format!("Building a {}", kind.name().to_lowercase()));
        }
        ClickAction::Nuke { kind, tile } => {
            pending.0.push(Intent::Nuke {
                player: HUMAN,
                kind,
                tile,
            });
            settings.build = None;
            notices.push_kind(
                format!("{} launched", kind.name()),
                crate::client::NoticeKind::Warning,
            );
        }
        ClickAction::Warship { tile } => {
            pending.0.push(Intent::Warship {
                player: HUMAN,
                tile,
            });
            settings.build = None;
            notices.push("Warship ordered");
        }
        ClickAction::SelectWarship(unit) => {
            settings.selected_ship = Some(unit);
            notices.push("Warship selected: click water to send it there");
        }
        ClickAction::MoveWarship { unit, tile } => {
            pending.0.push(Intent::MoveWarship {
                player: HUMAN,
                unit,
                tile,
            });
            settings.selected_ship = None;
        }
        ClickAction::Upgrade(unit) => {
            pending.0.push(Intent::Upgrade {
                player: HUMAN,
                unit,
            });
            notices.push("Upgrading");
        }
        ClickAction::Refuse(why) => notices.push(why),
    }
}

#[allow(clippy::too_many_arguments)]
fn keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    game: Res<GameState>,
    mut settings: ResMut<ClientSettings>,
    mut pending: ResMut<PendingIntents>,
    mut notices: ResMut<Notices>,
    mut screen: ResMut<crate::client::Screen>,
    mut save: ResMut<crate::client::save::SaveRequested>,
    mut load: ResMut<crate::client::save::LoadRequested>,
) {
    use crate::client::Screen;
    if *screen != Screen::Playing {
        // Esc leaves the pause menu; other menus use their buttons.
        if *screen == Screen::Paused && keys.just_pressed(KeyCode::Escape) {
            *screen = Screen::Playing;
        }
        return;
    }
    if keys.just_pressed(KeyCode::F5) {
        save.0 = true;
    }
    if keys.just_pressed(KeyCode::F9) {
        load.0 = true;
    }
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
    if keys.just_pressed(KeyCode::KeyH) {
        settings.show_help = !settings.show_help;
    }
    for (k, kind) in [
        (KeyCode::KeyQ, UnitKind::City),
        (KeyCode::KeyE, UnitKind::Port),
        (KeyCode::KeyF, UnitKind::Factory),
        (KeyCode::KeyG, UnitKind::DefensePost),
        (KeyCode::KeyV, UnitKind::MissileSilo),
        (KeyCode::KeyB, UnitKind::SamLauncher),
        (KeyCode::KeyN, UnitKind::Warship),
        (KeyCode::KeyZ, UnitKind::AtomBomb),
        (KeyCode::KeyX, UnitKind::HydrogenBomb),
        (KeyCode::KeyC, UnitKind::Mirv),
    ] {
        if keys.just_pressed(k) {
            settings.build = if settings.build == Some(kind) {
                None
            } else {
                Some(kind)
            };
        }
    }
    if keys.just_pressed(KeyCode::Escape) {
        // Esc cancels what is selected; with nothing selected it opens the pause menu.
        if settings.build.is_some()
            || settings.selected_ship.is_some()
            || settings.menu_tile.is_some()
        {
            settings.build = None;
            settings.selected_ship = None;
            settings.menu_tile = None;
        } else {
            *screen = Screen::Paused;
        }
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

/// Marks the tutorial step "change the attack size" when the size changes by any means.
fn track_size_changes(
    settings: Res<ClientSettings>,
    mut actions: ResMut<PlayerActions>,
    mut last: Local<Option<f64>>,
) {
    let now = settings.attack_ratio;
    if let Some(prev) = *last {
        if prev != now {
            actions.changed_size = true;
        }
    }
    *last = Some(now);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GameConfig, MapSize, GAME_CONFIG_PATH};
    use crate::engine::util::spawn_tiles;
    use crate::engine::{new_match, run_turn, MatchSetup};

    fn game() -> Game {
        let cfg = GameConfig::load(GAME_CONFIG_PATH).unwrap();
        new_match(cfg, MatchSetup::bots_only(MapSize::Small, 3, 4)).unwrap()
    }

    /// Spawned human with lots of gold, after spawn immunity.
    fn settled() -> (Game, TileRef) {
        let mut g = game();
        run_turn(&mut g, &[]);
        let map = g.map();
        let spawn = (0..map.num_tiles())
            .step_by(97)
            .find(|&t| {
                spawn_tiles(map, t, 4, true).is_some()
                    && map.is_ocean_shore(t + 3).then_some(()).is_none()
            })
            .unwrap();
        run_turn(
            &mut g,
            &[Intent::Spawn {
                player: HUMAN,
                tile: spawn,
            }],
        );
        while g.is_spawn_immunity_active() {
            run_turn(&mut g, &[]);
        }
        g.player_mut(HUMAN).unwrap().add_gold(5_000_000);
        (g, spawn)
    }

    #[test]
    fn build_mode_places_buildings_on_own_land_only() {
        let (g, spawn) = settled();
        assert_eq!(
            click_action_with(&g, spawn, 0.2, Some(UnitKind::City), None),
            ClickAction::Build {
                kind: UnitKind::City,
                tile: spawn
            }
        );
        let water = (0..g.map().num_tiles())
            .find(|&t| g.map().is_water(t))
            .unwrap();
        assert!(matches!(
            click_action_with(&g, water, 0.2, Some(UnitKind::City), None),
            ClickAction::Refuse(_)
        ));
    }

    #[test]
    fn clicking_own_building_upgrades_it() {
        let (mut g, spawn) = settled();
        run_turn(
            &mut g,
            &[Intent::Build {
                player: HUMAN,
                kind: UnitKind::City,
                tile: spawn,
            }],
        );
        for _ in 0..25 {
            run_turn(&mut g, &[]);
        }
        let city = g.structure_at(spawn).unwrap().id();
        assert_eq!(click_action(&g, spawn, 0.2), ClickAction::Upgrade(city));
    }

    #[test]
    fn clicking_far_land_across_water_sends_a_boat() {
        let (g, _) = settled();
        let map = g.map();
        // Free coast that the human's land does not reach over land, but a boat can.
        let far = (0..map.num_tiles()).step_by(7).find(|&t| {
            map.is_shore(t)
                && !map.has_owner(t)
                && !g.can_attack_tile(HUMAN, t)
                && crate::engine::boats::can_send_boat(&g, HUMAN, t).is_some()
        });
        let t = far.expect("seed 3 has free coast across the water");
        assert!(matches!(click_action(&g, t, 0.3), ClickAction::Boat { .. }));
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
