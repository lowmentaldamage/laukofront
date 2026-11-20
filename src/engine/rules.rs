//! Rule formulas. Ports of upstream `Config.troopCap`, `Config.troopIncreaseRate`,
//! `EngineConfig.goldAdditionRate`, `EngineConfig.startManpower`, `EngineConfig.attackAmount`.
//! Coefficients come from `assets/config/game.toml`.

use crate::config::GameConfig;
use crate::engine_lib::detmath::pow;

use super::player::{Player, PlayerType};

/// Upstream `startManpower` (non-tutorial, non-cheat; nations use the human value until the
/// nation milestone adds difficulty).
pub fn start_manpower(cfg: &GameConfig, kind: PlayerType) -> i64 {
    match kind {
        PlayerType::Bot => cfg.troops.bot_start,
        PlayerType::Human | PlayerType::Nation => cfg.troops.human_start,
    }
}

/// Upstream `startingGold`: bots get 0.
pub fn starting_gold(cfg: &GameConfig, kind: PlayerType) -> i64 {
    match kind {
        PlayerType::Bot => 0,
        _ => cfg.gold.starting_gold,
    }
}

/// Upstream `troopCap` without cities: `2 * (tiles^0.6 * 1000 + 50000)`, bots divided by 3.
pub fn max_troops(cfg: &GameConfig, kind: PlayerType, tiles: u32) -> f64 {
    let t = &cfg.troops;
    let cap =
        t.cap_multiplier * (pow(tiles as f64, t.cap_tile_exponent) * t.cap_tile_coeff + t.cap_base);
    match kind {
        PlayerType::Bot => cap / t.bot_cap_divisor,
        PlayerType::Human | PlayerType::Nation => cap,
    }
}

/// Upstream `troopIncreaseRate`: `(10 + troops^0.73 / 4) * (1 - troops / cap)`, bots x0.5, never
/// past the cap. Negative when above the cap (troops then drain toward it).
pub fn troop_increase_rate(cfg: &GameConfig, p: &Player) -> f64 {
    let t = &cfg.troops;
    let max = max_troops(cfg, p.kind(), p.num_tiles());
    let troops = p.troops() as f64;
    let mut to_add = t.growth_base + pow(troops, t.growth_exponent) / t.growth_divisor;
    to_add *= 1.0 - troops / max;
    if p.kind() == PlayerType::Bot {
        to_add *= t.bot_growth_multiplier;
    }
    (troops + to_add).min(max) - troops
}

/// Upstream `goldAdditionRate` (gold multiplier 1).
pub fn gold_addition_rate(cfg: &GameConfig, kind: PlayerType) -> i64 {
    match kind {
        PlayerType::Bot => cfg.gold.bot_per_tick,
        _ => cfg.gold.human_per_tick,
    }
}

/// Upstream `attackAmount`: troops a default attack sends (humans 1/5, bots 1/20).
pub fn attack_amount(cfg: &GameConfig, p: &Player) -> f64 {
    let pct = match p.kind() {
        PlayerType::Bot => cfg.attack.bot_default_percent,
        _ => cfg.attack.human_default_percent,
    };
    p.troops() as f64 * pct as f64 / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GAME_CONFIG_PATH;
    use crate::engine::player::PlayerInfo;

    fn cfg() -> GameConfig {
        GameConfig::load(GAME_CONFIG_PATH).unwrap()
    }

    fn player(kind: PlayerType, troops: i64, tiles: u32) -> Player {
        let mut p = Player::new(1, PlayerInfo::new("x", "x", kind), troops, 0);
        p.tiles = (0..tiles).collect();
        p
    }

    #[test]
    fn cap_matches_upstream_formula() {
        let c = cfg();
        // 2 * (1000^0.6 * 1000 + 50000) = 2 * (63095.7... + 50000)
        let cap = max_troops(&c, PlayerType::Human, 1000);
        assert!((cap - 226_191.47).abs() < 0.1, "{cap}");
        assert!((max_troops(&c, PlayerType::Bot, 1000) - cap / 3.0).abs() < 1e-9);
        assert_eq!(max_troops(&c, PlayerType::Human, 0), 100_000.0);
    }

    #[test]
    fn growth_matches_upstream_formula() {
        let c = cfg();
        let p = player(PlayerType::Human, 25_000, 50);
        let cap = max_troops(&c, PlayerType::Human, 50);
        let add = (10.0 + pow(25_000.0, 0.73) / 4.0) * (1.0 - 25_000.0 / cap);
        let expect = (25_000.0 + add).min(cap) - 25_000.0;
        assert_eq!(troop_increase_rate(&c, &p), expect);
        let bot = player(PlayerType::Bot, 10_000, 50);
        assert!(
            troop_increase_rate(&c, &bot)
                < troop_increase_rate(&c, &player(PlayerType::Human, 10_000, 50))
        );
    }

    #[test]
    fn growth_never_passes_cap_and_drains_above_it() {
        let c = cfg();
        let cap = max_troops(&c, PlayerType::Human, 10);
        let near = player(PlayerType::Human, cap as i64 - 1, 10);
        assert!(near.troops() as f64 + troop_increase_rate(&c, &near) <= cap);
        let over = player(PlayerType::Human, cap as i64 + 5000, 10);
        assert!(troop_increase_rate(&c, &over) < 0.0);
    }

    #[test]
    fn gold_and_attack_amounts() {
        let c = cfg();
        assert_eq!(gold_addition_rate(&c, PlayerType::Human), 100);
        assert_eq!(gold_addition_rate(&c, PlayerType::Bot), 50);
        assert_eq!(
            attack_amount(&c, &player(PlayerType::Human, 1000, 1)),
            200.0
        );
        assert_eq!(attack_amount(&c, &player(PlayerType::Bot, 1000, 1)), 50.0);
        assert_eq!(start_manpower(&c, PlayerType::Bot), 10_000);
        assert_eq!(starting_gold(&c, PlayerType::Bot), 0);
    }
}
