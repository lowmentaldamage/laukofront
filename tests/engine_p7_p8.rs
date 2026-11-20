//! Boats (P7) and buildings (P8).

use laukofront::config::{GameConfig, GAME_CONFIG_PATH};
use laukofront::engine::attack_exec::WinCheckExecution;
use laukofront::engine::boats::{can_send_boat, trade_gold, TradeExecution};
use laukofront::engine::executions::PlayerExecution;
use laukofront::engine::rules;
use laukofront::engine::structures::{
    build_cost, can_upgrade, factory_run_gold, placement, FactoryExecution,
};
use laukofront::engine::units::UnitKind;
use laukofront::engine::{run_turn, Game, Intent, PlayerId, PlayerInfo, PlayerType, HUMAN};
use laukofront::engine_lib::game_map::{terrain_byte, GameMap, TileRef};

const W: u32 = 120;
const H: u32 = 60;

fn config() -> GameConfig {
    GameConfig::load(GAME_CONFIG_PATH).expect("game.toml")
}

/// Two islands (x 2..=50 and x 70..=117) in an ocean; a one-tile ocean ring around the map.
fn islands() -> GameMap {
    let mut t = vec![terrain_byte(false, false, true, 0); (W * H) as usize];
    for y in 2..H - 2 {
        for x in 2..W - 2 {
            if x <= 50 || x >= 70 {
                t[(y * W + x) as usize] = terrain_byte(true, false, false, 2);
            }
        }
    }
    // shoreline bits
    let land = |x: i64, y: i64| {
        x >= 0
            && y >= 0
            && x < W as i64
            && y < H as i64
            && t[(y as u32 * W + x as u32) as usize] & 0x80 != 0
    };
    let mut out = t.clone();
    for y in 0..H as i64 {
        for x in 0..W as i64 {
            let me = land(x, y);
            let mixed = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(dx, dy)| {
                x + dx >= 0
                    && y + dy >= 0
                    && x + dx < W as i64
                    && y + dy < H as i64
                    && land(x + dx, y + dy) != me
            });
            if mixed {
                out[(y as u32 * W + x as u32) as usize] |= 1 << 6;
            }
        }
    }
    GameMap::new(W, H, out).unwrap()
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
    g.add_execution(Box::new(FactoryExecution));
    g
}

/// Tile index on the fixed-size test map.
fn t(x: u32, y: u32) -> TileRef {
    y * W + x
}

fn give_rect(g: &mut Game, p: PlayerId, x0: u32, y0: u32, x1: u32, y1: u32) {
    for y in y0..=y1 {
        for x in x0..=x1 {
            g.conquer(p, t(x, y));
        }
    }
}

fn past_immunity(g: &mut Game) {
    while g.is_spawn_immunity_active() {
        run_turn(g, &[]);
    }
}

// ----- boats -----

#[test]
fn boat_crosses_the_sea_and_lands() {
    let mut g = arena();
    give_rect(&mut g, HUMAN, 2, 2, 50, 57); // the whole west island
    past_immunity(&mut g);
    let target = t(90, 30); // free land on the east island
    assert!(can_send_boat(&g, HUMAN, target).is_some());
    let troops_before = g.player(HUMAN).unwrap().troops();
    run_turn(
        &mut g,
        &[Intent::Boat {
            player: HUMAN,
            tile: target,
            troops: Some(5_000.0),
        }],
    );
    assert_eq!(g.unit_count(HUMAN, UnitKind::TransportShip), 1);
    assert!(g.player(HUMAN).unwrap().troops() <= troops_before - 5_000 + 1_000);
    let mut landed = false;
    for _ in 0..200 {
        run_turn(&mut g, &[]);
        if g.player(HUMAN)
            .unwrap()
            .tiles()
            .iter()
            .any(|&t| g.map().x(t) >= 70)
        {
            landed = true;
            break;
        }
    }
    assert!(landed, "boat never landed on the east island");
    assert_eq!(
        g.unit_count(HUMAN, UnitKind::TransportShip),
        0,
        "boat is gone after landing"
    );
    for _ in 0..50 {
        run_turn(&mut g, &[]);
    }
    let east = g
        .player(HUMAN)
        .unwrap()
        .tiles()
        .iter()
        .filter(|&&t| g.map().x(t) >= 70)
        .count();
    assert!(
        east > 50,
        "the landing attack should expand, got {east} tiles"
    );
}

#[test]
fn at_most_three_boats() {
    let mut g = arena();
    give_rect(&mut g, HUMAN, 2, 2, 50, 57);
    past_immunity(&mut g);
    let intents: Vec<Intent> = (0..5)
        .map(|i| Intent::Boat {
            player: HUMAN,
            tile: t(90, 10 + i * 8),
            troops: Some(1_000.0),
        })
        .collect();
    run_turn(&mut g, &intents);
    assert_eq!(g.unit_count(HUMAN, UnitKind::TransportShip), 3);
}

#[test]
fn boat_cannot_target_own_land_or_without_a_coast() {
    let mut g = arena();
    give_rect(&mut g, HUMAN, 20, 20, 30, 30); // inland, no coast
    past_immunity(&mut g);
    assert!(
        can_send_boat(&g, HUMAN, t(90, 30)).is_none(),
        "no coast, no boat"
    );
    assert!(can_send_boat(&g, HUMAN, t(25, 25)).is_none(), "own land");
}

#[test]
fn retreating_boat_returns_troops_minus_malus() {
    let mut g = arena();
    give_rect(&mut g, HUMAN, 2, 2, 50, 57);
    past_immunity(&mut g);
    run_turn(
        &mut g,
        &[Intent::Boat {
            player: HUMAN,
            tile: t(100, 30),
            troops: Some(10_000.0),
        }],
    );
    let boat = g
        .player_units(HUMAN, Some(UnitKind::TransportShip))
        .next()
        .unwrap()
        .id();
    for _ in 0..5 {
        run_turn(&mut g, &[]);
    }
    run_turn(
        &mut g,
        &[Intent::CancelBoat {
            player: HUMAN,
            unit: boat,
        }],
    );
    for _ in 0..100 {
        if !g.unit(boat).unwrap().is_active() {
            break;
        }
        run_turn(&mut g, &[]);
    }
    assert!(!g.unit(boat).unwrap().is_active(), "boat came home");
    assert!(
        !g.player(HUMAN)
            .unwrap()
            .tiles()
            .iter()
            .any(|&t| g.map().x(t) >= 70),
        "never landed"
    );
}

#[test]
fn trade_gold_formula() {
    let g = arena();
    let near = trade_gold(&g, 50.0);
    let far = trade_gold(&g, 600.0);
    assert!(far > near);
    // 75000 / (1 + e^(-0.03 * 300)) + 50 * 600
    assert!((far as f64 - (75_000.0 / (1.0 + (-9.0f64).exp()) + 30_000.0)).abs() < 2.0);
}

// ----- buildings -----

#[test]
fn costs_double_and_cap_like_upstream() {
    let mut g = arena();
    give_rect(&mut g, HUMAN, 2, 2, 50, 57);
    past_immunity(&mut g);
    g.player_mut(HUMAN).unwrap().add_gold(10_000_000);
    assert_eq!(build_cost(&g, HUMAN, UnitKind::City), 125_000);
    assert_eq!(build_cost(&g, HUMAN, UnitKind::DefensePost), 50_000);
    run_turn(
        &mut g,
        &[Intent::Build {
            player: HUMAN,
            kind: UnitKind::City,
            tile: t(10, 10),
        }],
    );
    for _ in 0..25 {
        run_turn(&mut g, &[]);
    }
    assert_eq!(
        build_cost(&g, HUMAN, UnitKind::City),
        250_000,
        "second city costs double"
    );
    for i in 0..5 {
        run_turn(
            &mut g,
            &[Intent::Build {
                player: HUMAN,
                kind: UnitKind::City,
                tile: t(10 + (i % 2) * 20, 30 + (i / 2) * 9),
            }],
        );
        for _ in 0..25 {
            run_turn(&mut g, &[]);
        }
    }
    assert!(
        build_cost(&g, HUMAN, UnitKind::City) <= 1_000_000,
        "capped at 1M"
    );
}

#[test]
fn city_raises_troop_cap_after_construction() {
    let mut g = arena();
    give_rect(&mut g, HUMAN, 2, 2, 50, 57);
    past_immunity(&mut g);
    g.player_mut(HUMAN).unwrap().add_gold(1_000_000);
    let cap0 = rules::troop_cap(g.config(), g.player(HUMAN).unwrap());
    let gold0 = g.player(HUMAN).unwrap().gold();
    run_turn(
        &mut g,
        &[Intent::Build {
            player: HUMAN,
            kind: UnitKind::City,
            tile: t(20, 20),
        }],
    );
    assert!(g.player(HUMAN).unwrap().gold() < gold0, "paid");
    assert_eq!(
        rules::troop_cap(g.config(), g.player(HUMAN).unwrap()),
        cap0,
        "not before it is finished"
    );
    for _ in 0..25 {
        run_turn(&mut g, &[]);
    }
    let cap1 = rules::troop_cap(g.config(), g.player(HUMAN).unwrap());
    assert_eq!(cap1, cap0 + 250_000.0);
}

#[test]
fn placement_rules() {
    let mut g = arena();
    give_rect(&mut g, HUMAN, 2, 2, 50, 57);
    past_immunity(&mut g);
    assert_eq!(
        placement(&g, HUMAN, UnitKind::City, t(20, 20)),
        Err("Not enough gold")
    );
    g.player_mut(HUMAN).unwrap().add_gold(5_000_000);
    assert!(placement(&g, HUMAN, UnitKind::City, t(20, 20)).is_ok());
    assert_eq!(
        placement(&g, HUMAN, UnitKind::City, t(90, 20)),
        Err("You can only build on your own land")
    );
    // Ports snap to the coast.
    let port = placement(&g, HUMAN, UnitKind::Port, t(46, 20)).unwrap();
    assert!(g.map().is_ocean_shore(port));
    assert_eq!(
        placement(&g, HUMAN, UnitKind::Port, t(25, 30)),
        Err("Ports must be built on the coast")
    );
    run_turn(
        &mut g,
        &[Intent::Build {
            player: HUMAN,
            kind: UnitKind::City,
            tile: t(20, 20),
        }],
    );
    assert_eq!(
        placement(&g, HUMAN, UnitKind::Factory, t(25, 22)),
        Err("Too close to another building")
    );
}

#[test]
fn buildings_are_captured_with_their_land_and_defense_posts_destroyed() {
    let mut g = arena();
    give_rect(&mut g, HUMAN, 2, 2, 50, 57);
    past_immunity(&mut g);
    g.player_mut(HUMAN).unwrap().add_gold(5_000_000);
    run_turn(
        &mut g,
        &[
            Intent::Build {
                player: HUMAN,
                kind: UnitKind::City,
                tile: t(10, 10),
            },
            Intent::Build {
                player: HUMAN,
                kind: UnitKind::DefensePost,
                tile: t(10, 40),
            },
        ],
    );
    for _ in 0..60 {
        run_turn(&mut g, &[]);
    }
    let city = g.structure_at(t(10, 10)).unwrap().id();
    let post = g.structure_at(t(10, 40)).unwrap().id();
    let (c, p) = (t(10, 10), t(10, 40));
    g.conquer(2, c);
    g.conquer(2, p);
    assert_eq!(g.unit(city).unwrap().owner(), 2, "city captured");
    assert!(!g.unit(post).unwrap().is_active(), "defense post destroyed");
    assert_eq!(g.player(2).unwrap().city_levels(), 1);
    assert_eq!(g.player(HUMAN).unwrap().city_levels(), 0);
}

#[test]
fn defense_post_slows_and_hurts_attackers() {
    // Same attack twice, once against a defended border: the defended one takes fewer tiles.
    let run = |with_post: bool| {
        let mut g = arena();
        give_rect(&mut g, HUMAN, 2, 2, 25, 57);
        give_rect(&mut g, 2, 26, 2, 50, 57);
        g.player_mut(2).unwrap().set_troops(50_000.0);
        if with_post {
            g.player_mut(2).unwrap().add_gold(1_000_000);
            let post = t(30, 30);
            run_turn(
                &mut g,
                &[Intent::Build {
                    player: 2,
                    kind: UnitKind::DefensePost,
                    tile: post,
                }],
            );
            for _ in 0..55 {
                run_turn(&mut g, &[]);
            }
        }
        past_immunity(&mut g);
        run_turn(
            &mut g,
            &[Intent::Attack {
                player: HUMAN,
                target: Some(2),
                troops: Some(20_000.0),
            }],
        );
        for _ in 0..30 {
            run_turn(&mut g, &[]);
        }
        g.player(HUMAN).unwrap().num_tiles()
    };
    let open = run(false);
    let defended = run(true);
    assert!(defended < open, "defended {defended} vs open {open}");
}

#[test]
fn upgrades_cost_gold_and_raise_level() {
    let mut g = arena();
    give_rect(&mut g, HUMAN, 2, 2, 50, 57);
    past_immunity(&mut g);
    g.player_mut(HUMAN).unwrap().add_gold(5_000_000);
    run_turn(
        &mut g,
        &[Intent::Build {
            player: HUMAN,
            kind: UnitKind::City,
            tile: t(20, 20),
        }],
    );
    let city = g.structure_at(t(20, 20)).unwrap().id();
    assert_eq!(
        can_upgrade(&g, HUMAN, city),
        Err("Still under construction")
    );
    for _ in 0..25 {
        run_turn(&mut g, &[]);
    }
    assert!(can_upgrade(&g, HUMAN, city).is_ok());
    run_turn(
        &mut g,
        &[Intent::Upgrade {
            player: HUMAN,
            unit: city,
        }],
    );
    run_turn(&mut g, &[]);
    assert_eq!(g.unit(city).unwrap().level(), 2);
    assert_eq!(g.player(HUMAN).unwrap().city_levels(), 2);
}

#[test]
fn factory_pays_for_connected_cities_and_ports() {
    let mut g = arena();
    give_rect(&mut g, HUMAN, 2, 2, 50, 57);
    past_immunity(&mut g);
    g.player_mut(HUMAN).unwrap().add_gold(10_000_000);
    run_turn(
        &mut g,
        &[
            Intent::Build {
                player: HUMAN,
                kind: UnitKind::Factory,
                tile: t(25, 30),
            },
            Intent::Build {
                player: HUMAN,
                kind: UnitKind::City,
                tile: t(5, 5),
            },
            Intent::Build {
                player: HUMAN,
                kind: UnitKind::City,
                tile: t(5, 50),
            },
        ],
    );
    for _ in 0..60 {
        run_turn(&mut g, &[]);
    }
    let factory = g.structure_at(t(25, 30)).unwrap().id();
    assert_eq!(factory_run_gold(&g, factory), 20_000, "two cities x 10 000");
    let before = g.player(HUMAN).unwrap().gold();
    for _ in 0..100 {
        run_turn(&mut g, &[]);
    }
    let gained = g.player(HUMAN).unwrap().gold() - before;
    // 100 ticks of base income (100 x 100) plus one factory run (20 000).
    assert_eq!(gained, 10_000 + 20_000);
}

#[test]
fn ports_trade_with_other_players() {
    let mut g = arena();
    give_rect(&mut g, HUMAN, 2, 2, 50, 57);
    give_rect(&mut g, 2, 70, 2, 117, 57);
    past_immunity(&mut g);
    g.player_mut(HUMAN).unwrap().add_gold(1_000_000);
    g.player_mut(2).unwrap().add_gold(1_000_000);
    run_turn(
        &mut g,
        &[
            Intent::Build {
                player: HUMAN,
                kind: UnitKind::Port,
                tile: t(50, 30),
            },
            Intent::Build {
                player: 2,
                kind: UnitKind::Port,
                tile: t(70, 30),
            },
        ],
    );
    let mut saw_ship = false;
    let base_rate =
        |p: PlayerId, g: &Game| rules::gold_addition_rate(g.config(), g.player(p).unwrap().kind());
    let start = g.player(HUMAN).unwrap().gold();
    let ticks = 900;
    for _ in 0..ticks {
        run_turn(&mut g, &[]);
        saw_ship |=
            g.unit_count(HUMAN, UnitKind::TradeShip) + g.unit_count(2, UnitKind::TradeShip) > 0;
    }
    assert!(saw_ship, "no trade ship launched");
    let gained = g.player(HUMAN).unwrap().gold() - start;
    assert!(
        gained > base_rate(HUMAN, &g) * ticks as i64,
        "trade added gold: {gained}"
    );
}
