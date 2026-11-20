#![allow(dead_code)]

use laukofront::config::{GameConfig, MapSize, GAME_CONFIG_PATH};
use laukofront::core::{Owner, PlayerId, TerritoryId};
use laukofront::sim::World;

pub fn config() -> GameConfig {
    GameConfig::load(GAME_CONFIG_PATH).expect("load game.toml")
}

/// Config with one piece of text replaced, e.g. `config_with("max_match_ticks = 36000", "max_match_ticks = 50")`.
pub fn config_with(from: &str, to: &str) -> GameConfig {
    let text = std::fs::read_to_string(GAME_CONFIG_PATH).unwrap();
    assert!(text.contains(from), "config does not contain {from:?}");
    GameConfig::from_toml_str(&text.replace(from, to), "test").unwrap()
}

pub fn world(players: usize, seed: u64) -> World {
    World::new(config(), MapSize::Small, seed, players).unwrap()
}

pub const P0: PlayerId = PlayerId(0);
pub const P1: PlayerId = PlayerId(1);

/// Two adjacent territories `(a, b)` on the world's map.
pub fn adjacent_pair(w: &World) -> (TerritoryId, TerritoryId) {
    let a = TerritoryId(0);
    let b = w.map().neighbor_ids(a).next().unwrap();
    (a, b)
}

/// A territory that is neither `a` nor adjacent to `a`.
pub fn non_neighbor(w: &World, a: TerritoryId) -> TerritoryId {
    w.map()
        .territory_ids()
        .find(|&t| t != a && !w.map().is_neighbor(a, t))
        .unwrap()
}

/// A territory adjacent to neither `a` nor `b`, and equal to neither.
pub fn far_from(w: &World, a: TerritoryId, b: TerritoryId) -> TerritoryId {
    let m = w.map();
    m.territory_ids()
        .find(|&t| t != a && t != b && !m.is_neighbor(a, t) && !m.is_neighbor(b, t))
        .unwrap()
}

/// Makes every territory neutral with `troops`. Players with no territory left will be
/// eliminated on the next tick, so callers give them land afterwards.
pub fn clear_board(w: &mut World, troops: u32) {
    let ids: Vec<_> = w.map().territory_ids().collect();
    for t in ids {
        w.set_territory(t, Owner::Neutral, troops).unwrap();
    }
}
