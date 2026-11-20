//! Milestone P9: missiles, SAMs, warships, fallout.

use laukofront::config::{GameConfig, GAME_CONFIG_PATH};
use laukofront::engine::attack_exec::WinCheckExecution;
use laukofront::engine::boats::TradeExecution;
use laukofront::engine::executions::PlayerExecution;
use laukofront::engine::units::UnitKind;
use laukofront::engine::weapons::{can_launch, nuke_cost, SamExecution};
use laukofront::engine::{run_turn, Game, Intent, PlayerId, PlayerInfo, PlayerType, HUMAN};
use laukofront::engine_lib::game_map::{terrain_byte, GameMap, TileRef};

const W: u32 = 300;
const H: u32 = 120;
const BOT: PlayerId = 2;

fn config() -> GameConfig {
    GameConfig::load(GAME_CONFIG_PATH).expect("game.toml")
}

/// Two big islands (x 2..=120 and x 180..=297) with ocean between and around.
fn islands() -> GameMap {
    let mut t = vec![terrain_byte(false, false, true, 0); (W * H) as usize];
    let land = |x: u32, y: u32| {
        (2..H - 2).contains(&y) && (2..=120).contains(&x)
            || (2..H - 2).contains(&y) && (180..=297).contains(&x)
    };
    for y in 0..H {
        for x in 0..W {
            if land(x, y) {
                t[(y * W + x) as usize] = terrain_byte(true, false, false, 2);
            }
        }
    }
    let mut out = t.clone();
    for y in 0..H {
        for x in 0..W {
            let me = land(x, y);
            let mixed = [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .any(|(dx, dy)| {
                    let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                    nx >= 0
                        && ny >= 0
                        && nx < W as i64
                        && ny < H as i64
                        && land(nx as u32, ny as u32) != me
                });
            if mixed {
                out[(y * W + x) as usize] |= 1 << 6;
            }
        }
    }
    GameMap::new(W, H, out).unwrap()
}

fn t(x: u32, y: u32) -> TileRef {
    y * W + x
}

fn arena() -> Game {
    let mut g = Game::new(config(), islands(), "test");
    g.add_player(PlayerInfo::new("human", "You", PlayerType::Human));
    g.add_player(PlayerInfo::new("bot_a", "A", PlayerType::Bot));
    g.end_spawn_phase();
    for p in 1..=2 {
        g.add_execution(Box::new(PlayerExecution::new(p)));
    }
    g.add_execution(Box::new(WinCheckExecution::new()));
    g.add_execution(Box::new(TradeExecution::new(1)));
    g.add_execution(Box::new(SamExecution));
    for y in 2..H - 2 {
        for x in 2..=120 {
            g.conquer(HUMAN, t(x, y));
        }
        for x in 180..=297 {
            g.conquer(BOT, t(x, y));
        }
    }
    while g.is_spawn_immunity_active() {
        run_turn(&mut g, &[]);
    }
    g
}

fn build(g: &mut Game, p: PlayerId, kind: UnitKind, tile: TileRef) {
    run_turn(
        g,
        &[Intent::Build {
            player: p,
            kind,
            tile,
        }],
    );
    let done = kind.construction_ticks() + 2;
    for _ in 0..done {
        run_turn(g, &[]);
    }
}

fn rich(g: &mut Game, p: PlayerId) {
    g.player_mut(p).unwrap().add_gold(200_000_000);
}

#[test]
fn atom_bomb_needs_a_silo_then_destroys_and_irradiates() {
    let mut g = arena();
    rich(&mut g, HUMAN);
    assert_eq!(
        can_launch(&g, HUMAN, UnitKind::AtomBomb, t(240, 60)),
        Err("Build a missile silo first")
    );
    build(&mut g, HUMAN, UnitKind::MissileSilo, t(60, 60));
    let gold = g.player(HUMAN).unwrap().gold();
    let bot_tiles = g.player(BOT).unwrap().num_tiles();
    let bot_troops = g.player(BOT).unwrap().troops();
    run_turn(
        &mut g,
        &[Intent::Nuke {
            player: HUMAN,
            kind: UnitKind::AtomBomb,
            tile: t(240, 60),
        }],
    );
    assert!(g.player(HUMAN).unwrap().gold() <= gold - 750_000 + 1_000);
    assert_eq!(g.unit_count(HUMAN, UnitKind::AtomBomb), 1, "in flight");
    for _ in 0..40 {
        run_turn(&mut g, &[]);
    }
    assert_eq!(g.unit_count(HUMAN, UnitKind::AtomBomb), 0, "detonated");
    assert_eq!(g.owner(t(240, 60)), None, "centre lost");
    assert!(g.map().has_fallout(t(240, 60)), "fallout at the centre");
    assert!(
        g.map().has_fallout(t(240, 70)),
        "fallout inside the inner radius (12)"
    );
    assert!(
        g.owner(t(240, 100)).is_some(),
        "outside the blast untouched"
    );
    let lost = bot_tiles - g.player(BOT).unwrap().num_tiles();
    assert!(lost > 450 && lost < 2_900, "lost {lost} tiles");
    assert!(g.player(BOT).unwrap().troops() < bot_troops);
}

#[test]
fn hydrogen_bomb_is_much_bigger() {
    let mut g = arena();
    rich(&mut g, HUMAN);
    build(&mut g, HUMAN, UnitKind::MissileSilo, t(60, 60));
    let before = g.player(BOT).unwrap().num_tiles();
    run_turn(
        &mut g,
        &[Intent::Nuke {
            player: HUMAN,
            kind: UnitKind::HydrogenBomb,
            tile: t(240, 60),
        }],
    );
    for _ in 0..40 {
        run_turn(&mut g, &[]);
    }
    let lost = before - g.player(BOT).unwrap().num_tiles();
    assert!(lost > 10_000, "hydrogen bomb took only {lost} tiles");
}

#[test]
fn silo_reloads_and_levels_add_slots() {
    let mut g = arena();
    rich(&mut g, HUMAN);
    build(&mut g, HUMAN, UnitKind::MissileSilo, t(60, 60));
    run_turn(
        &mut g,
        &[Intent::Nuke {
            player: HUMAN,
            kind: UnitKind::AtomBomb,
            tile: t(240, 30),
        }],
    );
    assert_eq!(
        can_launch(&g, HUMAN, UnitKind::AtomBomb, t(240, 90)),
        Err("All silos are reloading")
    );
    for _ in 0..92 {
        run_turn(&mut g, &[]);
    }
    assert!(
        can_launch(&g, HUMAN, UnitKind::AtomBomb, t(240, 90)).is_ok(),
        "reloaded after 90 ticks"
    );
    let silo = g.structure_at(t(60, 60)).unwrap().id();
    run_turn(
        &mut g,
        &[Intent::Upgrade {
            player: HUMAN,
            unit: silo,
        }],
    );
    run_turn(&mut g, &[]);
    run_turn(
        &mut g,
        &[
            Intent::Nuke {
                player: HUMAN,
                kind: UnitKind::AtomBomb,
                tile: t(240, 30),
            },
            Intent::Nuke {
                player: HUMAN,
                kind: UnitKind::AtomBomb,
                tile: t(240, 90),
            },
        ],
    );
    assert_eq!(
        g.unit_count(HUMAN, UnitKind::AtomBomb),
        2,
        "level 2 silo fires two"
    );
}

#[test]
fn sam_shoots_down_atom_bombs_but_not_mirvs() {
    let mut g = arena();
    rich(&mut g, HUMAN);
    rich(&mut g, BOT);
    build(&mut g, HUMAN, UnitKind::MissileSilo, t(60, 60));
    build(&mut g, BOT, UnitKind::SamLauncher, t(240, 60));
    let before = g.player(BOT).unwrap().num_tiles();
    run_turn(
        &mut g,
        &[Intent::Nuke {
            player: HUMAN,
            kind: UnitKind::AtomBomb,
            tile: t(245, 60),
        }],
    );
    for _ in 0..40 {
        run_turn(&mut g, &[]);
    }
    assert_eq!(
        g.player(BOT).unwrap().num_tiles(),
        before,
        "intercepted: no damage"
    );
    // MIRV gets through.
    for _ in 0..95 {
        run_turn(&mut g, &[]);
    }
    let mirv_cost = nuke_cost(&g, UnitKind::Mirv);
    run_turn(
        &mut g,
        &[Intent::Nuke {
            player: HUMAN,
            kind: UnitKind::Mirv,
            tile: t(245, 60),
        }],
    );
    assert_eq!(
        nuke_cost(&g, UnitKind::Mirv),
        mirv_cost + 15_000_000,
        "MIRVs get dearer"
    );
    for _ in 0..60 {
        run_turn(&mut g, &[]);
    }
    let lost = before - g.player(BOT).unwrap().num_tiles();
    assert!(lost > 2_000, "MIRV took only {lost} tiles");
}

#[test]
fn fallout_slows_expansion() {
    let run = |irradiate: bool| {
        let mut g = arena();
        // Free the middle of the bot island; optionally put fallout on it.
        for y in 2..H - 2 {
            for x in 180..=297 {
                g.relinquish(t(x, y));
            }
        }
        g.conquer(BOT, t(180, 60));
        if irradiate {
            rich(&mut g, HUMAN);
            build(&mut g, HUMAN, UnitKind::MissileSilo, t(60, 60));
            run_turn(
                &mut g,
                &[Intent::Nuke {
                    player: HUMAN,
                    kind: UnitKind::HydrogenBomb,
                    tile: t(240, 60),
                }],
            );
            for _ in 0..40 {
                run_turn(&mut g, &[]);
            }
        }
        run_turn(
            &mut g,
            &[Intent::Attack {
                player: BOT,
                target: None,
                troops: Some(5_000.0),
            }],
        );
        for _ in 0..60 {
            run_turn(&mut g, &[]);
        }
        g.player(BOT).unwrap().num_tiles()
    };
    let clean = run(false);
    let hot = run(true);
    assert!(hot < clean, "fallout {hot} vs clean {clean}");
}

#[test]
fn warship_sinks_boats_and_fights() {
    let mut g = arena();
    rich(&mut g, HUMAN);
    rich(&mut g, BOT);
    build(&mut g, HUMAN, UnitKind::Port, t(120, 60));
    run_turn(
        &mut g,
        &[Intent::Warship {
            player: HUMAN,
            tile: t(150, 60),
        }],
    );
    for _ in 0..40 {
        run_turn(&mut g, &[]);
    }
    assert_eq!(g.unit_count(HUMAN, UnitKind::Warship), 1);
    // A bot boat sailing past gets sunk.
    run_turn(
        &mut g,
        &[Intent::Boat {
            player: BOT,
            tile: t(110, 60),
            troops: Some(2_000.0),
        }],
    );
    for _ in 0..120 {
        run_turn(&mut g, &[]);
    }
    assert_eq!(
        g.unit_count(BOT, UnitKind::TransportShip),
        0,
        "boat still at sea"
    );
    let landed = g
        .player(BOT)
        .unwrap()
        .tiles()
        .iter()
        .any(|&x| g.map().x(x) <= 120);
    assert!(!landed, "the boat landed instead of being sunk");
    // Enemy warship: both take damage.
    build(&mut g, BOT, UnitKind::Port, t(180, 60));
    run_turn(
        &mut g,
        &[Intent::Warship {
            player: BOT,
            tile: t(160, 60),
        }],
    );
    for _ in 0..200 {
        run_turn(&mut g, &[]);
    }
    let mine = g
        .player_units(HUMAN, Some(UnitKind::Warship))
        .next()
        .map(|u| u.health())
        .unwrap_or(0);
    let theirs = g
        .player_units(BOT, Some(UnitKind::Warship))
        .next()
        .map(|u| u.health())
        .unwrap_or(0);
    assert!(
        mine < 1000 || theirs < 1000,
        "no shots fired (mine {mine}, theirs {theirs})"
    );
}
