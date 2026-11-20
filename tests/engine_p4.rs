//! Milestone P4: bot (tribe) AI.

use laukofront::config::{GameConfig, MapSize, GAME_CONFIG_PATH};
use laukofront::engine::{new_match, run_turn, Game, MatchSetup, PlayerType};

fn config() -> GameConfig {
    GameConfig::load(GAME_CONFIG_PATH).expect("game.toml")
}

fn bots_only(seed: i64, bots: u32) -> Game {
    new_match(
        config(),
        MatchSetup {
            size: MapSize::Small,
            seed,
            bots,
        },
    )
    .unwrap()
}

/// Upstream guide requirement: a match between AIs reaches a winner. Seed 3 is a map where the
/// bots share one landmass (boats are a later milestone), so the win is by conquest, not the
/// 170-minute limit.
#[test]
fn bots_only_match_reaches_a_conquest_winner() {
    let mut g = bots_only(3, 10);
    while g.winner().is_none() && g.ticks() < 30_000 {
        run_turn(&mut g, &[]);
    }
    let w = g.winner().expect("no winner within 30 000 ticks");
    let p = g.player(w).unwrap();
    assert_eq!(p.kind(), PlayerType::Bot);
    assert!(p.num_tiles() as u64 * 100 > g.map().num_land_tiles() as u64 * 80);
    assert!(
        g.elapsed_game_seconds() < 170.0 * 60.0,
        "won by time limit, not conquest"
    );
}

#[test]
fn bots_expand_into_free_land_then_fight() {
    let mut g = bots_only(6, 8);
    for _ in 0..200 {
        run_turn(&mut g, &[]);
    }
    // First moves: every bot has grown past its 52-tile spawn disk.
    for p in g.players().iter().filter(|p| p.kind() == PlayerType::Bot) {
        assert!(p.num_tiles() > 52, "{} did not expand", p.name());
    }
    let mut saw_player_attack = false;
    for _ in 0..8_000 {
        run_turn(&mut g, &[]);
        for p in g.alive_players() {
            for &a in p.outgoing_attacks() {
                if g.attack(a).unwrap().target().is_some() {
                    saw_player_attack = true;
                }
            }
        }
    }
    assert!(saw_player_attack, "bots never attacked each other");
}

/// Every new bot attack, at the moment it is created, targets a land neighbour (or free land the
/// bot borders) and carries no more troops than the bot had.
#[test]
fn bot_attacks_target_neighbours_and_never_overdraw() {
    use std::collections::HashSet;
    let mut g = bots_only(2, 12);
    let mut seen: HashSet<u32> = HashSet::new();
    let mut checked = 0;
    for _ in 0..4_000 {
        let before: Vec<i64> = g.players().iter().map(|p| p.troops()).collect();
        run_turn(&mut g, &[]);
        for p in g.alive_players() {
            assert!(p.troops() >= 0);
            for &a in p.outgoing_attacks() {
                if !seen.insert(a) {
                    continue;
                }
                // Created at the end of this tick: no tile has been conquered yet.
                let at = g.attack(a).unwrap();
                assert!(at.troops() >= 1.0);
                assert!(
                    at.troops() <= before[p.id() as usize - 1] as f64,
                    "{} sent {} with {} troops",
                    p.name(),
                    at.troops(),
                    before[p.id() as usize - 1]
                );
                assert!(
                    g.nearby(p.id()).contains(&at.target()),
                    "{} attacked a non-neighbour",
                    p.name()
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 50, "only {checked} bot attacks seen");
}

#[test]
fn bots_only_matches_are_deterministic() {
    let run = || {
        let mut g = bots_only(7, 10);
        for _ in 0..3_000 {
            run_turn(&mut g, &[]);
        }
        g.state_hash()
    };
    assert_eq!(run(), run());
}
