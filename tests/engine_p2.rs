//! Milestone P2: players, spawn phase, spawning, economy, ownership bookkeeping.

use std::collections::HashSet;

use laukofront::config::{GameConfig, MapSize, GAME_CONFIG_PATH};
use laukofront::engine::rules;
use laukofront::engine::util::spawn_tiles;
use laukofront::engine::{new_match, run_turn, Game, Intent, MatchSetup, PlayerType, HUMAN};
use laukofront::engine_lib::game_map::TileRef;

fn config() -> GameConfig {
    GameConfig::load(GAME_CONFIG_PATH).expect("game.toml")
}

/// Sends intents, then runs one more tick: executions created this tick are initialised at its end
/// and act on the next tick, exactly as upstream.
fn turn_and_settle(g: &mut Game, intents: &[Intent]) {
    run_turn(g, intents);
    run_turn(g, &[]);
}

fn game(seed: i64, bots: u32) -> Game {
    new_match(
        config(),
        MatchSetup {
            size: MapSize::Small,
            seed,
            bots,
        },
    )
    .expect("match")
}

/// Ownership bookkeeping must always agree with the tile map.
fn check_invariants(g: &Game) {
    let map = g.map();
    let mut owned_count = vec![0u32; g.players().len() + 1];
    for t in 0..map.num_tiles() {
        let o = map.owner_id(t) as usize;
        if o != 0 {
            assert!(
                map.is_passable_land(t),
                "water or impassable tile {t} is owned"
            );
            owned_count[o] += 1;
        }
    }
    for p in g.players() {
        assert_eq!(
            p.num_tiles(),
            owned_count[p.id() as usize],
            "{} tile count",
            p.name()
        );
        let tiles: HashSet<TileRef> = p.tiles().iter().copied().collect();
        assert_eq!(tiles.len(), p.tiles().len(), "duplicate tiles");
        assert!(tiles.iter().all(|&t| map.owner_id(t) == p.id()));
        let border: HashSet<TileRef> = p.border_tiles().iter().copied().collect();
        let expect: HashSet<TileRef> = tiles
            .iter()
            .copied()
            .filter(|&t| map.is_border(t))
            .collect();
        assert_eq!(border, expect, "{} border set", p.name());
        assert!(p.troops() >= 0);
    }
}

/// A tile where a full spawn disk fits, far from every spawned player.
fn free_spawn_tile(g: &Game) -> TileRef {
    let map = g.map();
    (0..map.num_tiles())
        .step_by(97)
        .find(|&t| {
            spawn_tiles(map, t, 4, true).is_some()
                && g.players()
                    .iter()
                    .filter_map(|p| p.spawn_tile())
                    .all(|s| map.manhattan_dist(s, t) > 60)
        })
        .expect("free land")
}

#[test]
fn bots_spawn_one_tick_after_setup_with_full_disks_apart() {
    let mut g = game(1, 8);
    turn_and_settle(&mut g, &[]);
    let map = g.map();
    let spawns: Vec<TileRef> = g
        .players()
        .iter()
        .skip(1)
        .filter_map(|p| p.spawn_tile())
        .collect();
    assert_eq!(spawns.len(), 8);
    for p in g.players().iter().skip(1) {
        assert_eq!(p.kind(), PlayerType::Bot);
        assert_eq!(p.num_tiles(), 52, "{}", p.name());
    }
    for (i, &a) in spawns.iter().enumerate() {
        for &b in &spawns[i + 1..] {
            assert!(map.manhattan_dist(a, b) >= 30);
        }
    }
    assert!(g.in_spawn_phase(), "bots do not end the spawn phase");
    check_invariants(&g);
}

#[test]
fn spawn_phase_ends_by_timer_without_a_human_spawn() {
    let mut g = game(2, 2);
    while g.in_spawn_phase() {
        run_turn(&mut g, &[]);
        assert!(g.ticks() < 200, "spawn phase never ended");
    }
    // Timer ends it on the first tick where ticks() > 100 (tick 101), as upstream.
    assert_eq!(g.start_tick(), Some(101));
}

#[test]
fn human_spawn_ends_spawn_phase_and_claims_disk() {
    let mut g = game(3, 3);
    turn_and_settle(&mut g, &[]);
    let t = free_spawn_tile(&g);
    turn_and_settle(
        &mut g,
        &[Intent::Spawn {
            player: HUMAN,
            tile: t,
        }],
    );
    let human = g.player(HUMAN).unwrap();
    assert_eq!(human.spawn_tile(), Some(t));
    assert_eq!(human.num_tiles(), 52);
    assert!(!g.in_spawn_phase());
    check_invariants(&g);
}

#[test]
fn respawn_moves_the_player() {
    let mut g = game(4, 0);
    // Make the spawn phase last: spawn a bot-free map, use the human twice before it ends.
    let first = free_spawn_tile(&g);
    // A human spawn ends the phase at once, so queue both in the same tick: the second moves it.
    let map = g.map();
    let second = (0..map.num_tiles())
        .step_by(89)
        .find(|&t| spawn_tiles(map, t, 4, true).is_some() && map.manhattan_dist(t, first) > 100)
        .unwrap();
    turn_and_settle(
        &mut g,
        &[
            Intent::Spawn {
                player: HUMAN,
                tile: first,
            },
            Intent::Spawn {
                player: HUMAN,
                tile: second,
            },
        ],
    );
    let human = g.player(HUMAN).unwrap();
    assert_eq!(human.spawn_tile(), Some(second));
    assert_eq!(human.num_tiles(), 52);
    assert!(human
        .tiles()
        .iter()
        .all(|&t| g.map().manhattan_dist(t, second) <= 8));
    check_invariants(&g);
}

#[test]
fn spawn_after_spawn_phase_is_ignored() {
    let mut g = game(5, 0);
    let t = free_spawn_tile(&g);
    while g.in_spawn_phase() {
        run_turn(&mut g, &[]);
    }
    turn_and_settle(
        &mut g,
        &[Intent::Spawn {
            player: HUMAN,
            tile: t,
        }],
    );
    turn_and_settle(&mut g, &[]);
    assert!(!g.player(HUMAN).unwrap().has_spawned());
    assert_eq!(g.player(HUMAN).unwrap().num_tiles(), 0);
}

#[test]
fn spawn_on_water_takes_only_valid_tiles_or_nothing() {
    let mut g = game(6, 0);
    let map = g.map();
    let deep = (0..map.num_tiles())
        .find(|&t| spawn_tiles(map, t, 4, false).is_some_and(|v| v.is_empty()))
        .unwrap();
    turn_and_settle(
        &mut g,
        &[Intent::Spawn {
            player: HUMAN,
            tile: deep,
        }],
    );
    assert!(
        !g.player(HUMAN).unwrap().has_spawned(),
        "open ocean: no spawn"
    );
    let map = g.map();
    let coast = (0..map.num_tiles())
        .find(|&t| spawn_tiles(map, t, 4, false).is_some_and(|v| (10..40).contains(&v.len())))
        .unwrap();
    let expect = spawn_tiles(map, coast, 4, false).unwrap().len() as u32;
    turn_and_settle(
        &mut g,
        &[Intent::Spawn {
            player: HUMAN,
            tile: coast,
        }],
    );
    assert_eq!(g.player(HUMAN).unwrap().num_tiles(), expect);
    check_invariants(&g);
}

#[test]
fn no_growth_during_spawn_phase_then_upstream_formula() {
    let cfg = config();
    let mut g = game(7, 1);
    turn_and_settle(&mut g, &[]); // bot spawns
    let bot = 2;
    let before = g.player(bot).unwrap().troops();
    for _ in 0..20 {
        run_turn(&mut g, &[]);
    }
    assert_eq!(
        g.player(bot).unwrap().troops(),
        before,
        "no growth in spawn phase"
    );
    assert_eq!(g.player(bot).unwrap().gold(), 0);

    let t = free_spawn_tile(&g);
    turn_and_settle(
        &mut g,
        &[Intent::Spawn {
            player: HUMAN,
            tile: t,
        }],
    ); // ends spawn phase
       // Spawning ended the spawn phase; every PlayerExecution was initialised in that tick.
    for _ in 0..50 {
        let snapshot: Vec<_> = g.players().to_vec();
        run_turn(&mut g, &[]);
        for (old, new) in snapshot.iter().zip(g.players()) {
            let mut expect = old.clone();
            expect.add_troops(rules::troop_increase_rate(&cfg, old));
            expect.add_gold(rules::gold_addition_rate(&cfg, old.kind()));
            assert_eq!(new.troops(), expect.troops(), "{}", new.name());
            assert_eq!(new.gold(), expect.gold(), "{}", new.name());
        }
    }
    assert_eq!(g.player(HUMAN).unwrap().gold(), 50 * 100);
    assert_eq!(g.player(bot).unwrap().gold(), 50 * 50);
}

#[test]
fn troops_grow_to_cap_and_stop() {
    let cfg = config();
    let mut g = game(8, 0);
    let t = free_spawn_tile(&g);
    turn_and_settle(
        &mut g,
        &[Intent::Spawn {
            player: HUMAN,
            tile: t,
        }],
    );
    for _ in 0..20_000 {
        run_turn(&mut g, &[]);
    }
    let p = g.player(HUMAN).unwrap();
    let cap = rules::max_troops(&cfg, p.kind(), p.num_tiles());
    assert!(p.troops() as f64 <= cap);
    assert!(
        p.troops() as f64 > cap * 0.99,
        "troops {} cap {cap}",
        p.troops()
    );
}

#[test]
fn same_seed_same_state() {
    let run = || {
        let mut g = game(9, 6);
        turn_and_settle(&mut g, &[]);
        let t = free_spawn_tile(&g);
        turn_and_settle(
            &mut g,
            &[Intent::Spawn {
                player: HUMAN,
                tile: t,
            }],
        );
        for _ in 0..300 {
            run_turn(&mut g, &[]);
        }
        g.state_hash()
    };
    assert_eq!(run(), run());
}

#[test]
fn conquer_and_relinquish_keep_bookkeeping() {
    let mut g = game(10, 2);
    turn_and_settle(&mut g, &[]);
    let a = 2;
    let b = 3;
    // Hand a slice of b's land to a, then free some of it.
    let b_tiles: Vec<TileRef> = g.player(b).unwrap().tiles().to_vec();
    for &t in b_tiles.iter().take(20) {
        assert!(g.conquer(a, t));
    }
    check_invariants(&g);
    for &t in b_tiles.iter().take(10) {
        assert!(g.relinquish(t));
    }
    check_invariants(&g);
    assert_eq!(g.player(a).unwrap().num_tiles(), 52 + 10);
    assert_eq!(g.player(b).unwrap().num_tiles(), 52 - 20);
    // Water cannot be conquered.
    let water = (0..g.map().num_tiles())
        .find(|&t| g.map().is_water(t))
        .unwrap();
    assert!(!g.conquer(a, water));
    // Changed tiles are reported for the renderer.
    assert!(!g.changed_tiles().is_empty());
}
