//! Boat attacks from real clicks (regression for the 1.0.0 report "boat attacks don't work"):
//! on a coastal start, clicking other players' or free land across the sea sends a boat, even
//! deep inland, and the boats land.

use laukofront::config::{GameConfig, MapSize, GAME_CONFIG_PATH};
use laukofront::engine::units::UnitKind;
use laukofront::engine::util::spawn_tiles;
use laukofront::engine::{new_match, run_turn, Game, Intent, MapChoice, MatchSetup, HUMAN};
use laukofront::input::{click_action, ClickAction};

/// A match where you start on an ocean coast and have expanded for a minute.
fn coastal_start(map: MapChoice, seed: i64) -> Game {
    let mut s = MatchSetup::bots_only(MapSize::Medium, seed, 12);
    s.nations = 4;
    s.map = map;
    let mut g = new_match(GameConfig::load(GAME_CONFIG_PATH).unwrap(), s).unwrap();
    run_turn(&mut g, &[]);
    let m = g.map();
    let n = m.num_tiles();
    let tile = (0..n)
        .map(|i| (i as u64 * 7919 % n as u64) as u32)
        .find(|&t| {
            spawn_tiles(m, t, 4, true).is_some()
                && (-8i64..=8).any(|d| {
                    m.tile(m.x(t) as i64 + d, m.y(t) as i64)
                        .is_some_and(|w| m.is_ocean(w))
                })
        })
        .unwrap();
    run_turn(
        &mut g,
        &[Intent::Spawn {
            player: HUMAN,
            tile,
        }],
    );
    while g.in_spawn_phase() {
        run_turn(&mut g, &[]);
    }
    for _ in 0..600 {
        let i = if g.ticks().is_multiple_of(30) {
            vec![Intent::Attack {
                player: HUMAN,
                target: None,
                troops: None,
            }]
        } else {
            vec![]
        };
        run_turn(&mut g, &i);
    }
    g
}

#[test]
fn clicks_across_the_sea_send_boats_that_land() {
    for (map, seed) in [
        (MapChoice::Generated(Default::default()), 3),
        (MapChoice::parse("archipelago").unwrap(), 2),
        (MapChoice::parse("europe").unwrap(), 1),
    ] {
        let mut g = coastal_start(map, seed);
        let m = g.map().clone();
        let (mut boat, mut attack, mut refused) = (0, 0, 0);
        let mut boat_tiles = Vec::new();
        for i in 0..400u64 {
            let t = (i * 104_729 % m.num_tiles() as u64) as u32;
            if !m.is_passable_land(t) || g.owner(t) == Some(HUMAN) {
                continue;
            }
            match click_action(&g, t, 0.2) {
                ClickAction::Boat { .. } => {
                    boat += 1;
                    boat_tiles.push(t);
                }
                ClickAction::Attack { .. } => attack += 1,
                _ => refused += 1,
            }
        }
        let name = map.name();
        // Refusals left are land on lakes or other seas your coast does not touch.
        assert!(
            refused * 2 < boat + attack,
            "{name}: {refused} refused vs {boat} boats, {attack} attacks"
        );
        assert!(boat > 50, "{name}: only {boat} clicks send a boat");
        // Three boats sail and land.
        let tiles = g.player(HUMAN).unwrap().num_tiles();
        for &t in boat_tiles.iter().take(3) {
            if let ClickAction::Boat { tile, troops } = click_action(&g, t, 0.2) {
                run_turn(
                    &mut g,
                    &[Intent::Boat {
                        player: HUMAN,
                        tile,
                        troops: Some(troops),
                    }],
                );
            }
        }
        assert_eq!(g.unit_count(HUMAN, UnitKind::TransportShip), 3, "{name}");
        for _ in 0..2_000 {
            run_turn(&mut g, &[]);
        }
        assert_eq!(
            g.unit_count(HUMAN, UnitKind::TransportShip),
            0,
            "{name}: boats still at sea after 200 s"
        );
        assert!(
            g.player(HUMAN).unwrap().num_tiles() > tiles,
            "{name}: no land taken"
        );
    }
}
