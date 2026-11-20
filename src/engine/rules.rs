//! Rule formulas. Ports of upstream `Config.troopCap`, `Config.troopIncreaseRate`,
//! `EngineConfig.goldAdditionRate`, `EngineConfig.startManpower`, `EngineConfig.attackAmount`.
//! Coefficients come from `assets/config/game.toml`.

use crate::config::GameConfig;
use crate::engine_lib::detmath::{log, pow, sigmoid, within};
use crate::engine_lib::game_map::TerrainType;

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

/// Upstream `troopCap` with cities: `2 * (tiles^0.6 * 1000 + 50000) + 250000 * city levels`,
/// then bots divided by 3.
pub fn troop_cap(cfg: &GameConfig, p: &Player) -> f64 {
    let t = &cfg.troops;
    let base = t.cap_multiplier
        * (pow(p.num_tiles() as f64, t.cap_tile_exponent) * t.cap_tile_coeff + t.cap_base)
        + cfg.buildings.city_troop_increase * p.city_levels() as f64;
    match p.kind() {
        PlayerType::Bot => base / t.bot_cap_divisor,
        PlayerType::Human | PlayerType::Nation => base,
    }
}

/// Upstream `troopIncreaseRate`: `(10 + troops^0.73 / 4) * (1 - troops / cap)`, bots x0.5, never
/// past the cap. Negative when above the cap (troops then drain toward it).
pub fn troop_increase_rate(cfg: &GameConfig, p: &Player) -> f64 {
    let t = &cfg.troops;
    let max = troop_cap(cfg, p);
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

/// Defender side of an attack (upstream `AttackLogicInput.defender`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DefenderInfo {
    pub kind: PlayerType,
    pub num_tiles: u32,
    pub troops: f64,
    pub is_traitor: bool,
    pub is_disconnected_teammate: bool,
}

/// Upstream `AttackLogicInput`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttackLogicInput {
    pub terrain: TerrainType,
    pub attack_troops: f64,
    pub attacker_kind: PlayerType,
    pub attacker_tiles: u32,
    /// `None` = attacking unowned land (terra nullius).
    pub defender: Option<DefenderInfo>,
    pub defender_has_defense_post: bool,
    /// Share of land with fallout if the tile has fallout.
    pub fallout_ratio: Option<f64>,
    /// Tiles on the attack front this tick (plus jitter).
    pub border_size: f64,
}

/// Upstream `AttackLogicResult`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttackLogicResult {
    pub attacker_troop_loss: f64,
    pub defender_troop_loss: f64,
    /// Share of this tick's conquest budget the tile uses (an attack conquers until these sum to 1).
    pub tick_fraction: f64,
}

/// Upstream `largeTerritoryBonus`: ~1 for small territories, easing to `1 - depth` for huge ones.
pub fn large_territory_bonus(cfg: &GameConfig, num_tiles: u32, depth: f64) -> f64 {
    let l = &cfg.attack_logic;
    1.0 - depth
        * sigmoid(
            log(num_tiles as f64),
            l.large_territory_steepness,
            log(l.large_territory_midpoint),
        )
}

/// Upstream `terrainAttackBase`: (mag, tileCost). `None` for water and impassable land.
pub fn terrain_attack_base(cfg: &GameConfig, terrain: TerrainType) -> Option<(f64, f64)> {
    let l = &cfg.attack_logic;
    match terrain {
        TerrainType::Plains => Some((l.plains_mag, l.plains_cost)),
        TerrainType::Highland => Some((l.highland_mag, l.highland_cost)),
        TerrainType::Mountain => Some((l.mountain_mag, l.mountain_cost)),
        _ => None,
    }
}

/// Upstream `EngineConfig.attackLogic`: troop losses and speed for conquering one tile.
/// Returns `None` for water/impassable tiles (upstream throws).
pub fn attack_logic(cfg: &GameConfig, input: &AttackLogicInput) -> Option<AttackLogicResult> {
    let l = &cfg.attack_logic;
    let (mut mag, mut tile_cost) = terrain_attack_base(cfg, input.terrain)?;
    if input.defender.is_some() && input.defender_has_defense_post {
        mag *= cfg.buildings.defense_post_defense_bonus;
        tile_cost *= cfg.buildings.defense_post_speed_bonus;
    }
    if let Some(r) = input.fallout_ratio {
        let fallout = 5.0 - r * 2.0;
        mag *= fallout;
        tile_cost *= fallout;
    }

    let Some(def) = input.defender else {
        let tick_budget = input.border_size * 2.0;
        let div = if input.attacker_kind == PlayerType::Bot {
            10.0
        } else {
            5.0
        };
        return Some(AttackLogicResult {
            attacker_troop_loss: mag / div,
            defender_troop_loss: 0.0,
            tick_fraction: within(
                l.terra_nullius_cost_scale * tile_cost / input.attack_troops,
                l.terra_nullius_min_cost,
                l.terra_nullius_max_cost,
            ) / tick_budget,
        });
    };

    if def.is_disconnected_teammate {
        mag = 0.0;
    }
    if matches!(input.attacker_kind, PlayerType::Human | PlayerType::Nation)
        && def.kind == PlayerType::Bot
    {
        mag *= l.bot_defender_loss_mult;
    }
    let large_attacker = large_territory_bonus(cfg, input.attacker_tiles, l.large_attacker_depth);
    let large_defender = large_territory_bonus(cfg, def.num_tiles, l.large_defender_depth);
    let traitor_loss = if def.is_traitor {
        TRAITOR_DEFENSE_DEBUFF
    } else {
        1.0
    };
    let traitor_cost = if def.is_traitor {
        TRAITOR_SPEED_DEBUFF
    } else {
        1.0
    };

    let defender_troop_loss = def.troops / def.num_tiles as f64;
    let troop_ratio = def.troops / input.attack_troops;
    let attacker_troop_loss = mag
        * traitor_loss
        * within(troop_ratio, l.cheapest_troop_ratio, 2.0)
        * (l.attacker_loss_base * large_attacker * large_defender
            + l.attacker_loss_per_density * defender_troop_loss);
    let speed_cost = (within(troop_ratio, 0.82, 7.5) * within(troop_ratio / 20.0, 1.0, 50.0))
        / l.speed_cost_divisor;
    let large_attacker_speed =
        large_territory_bonus(cfg, input.attacker_tiles, l.large_attacker_speed_depth);
    Some(AttackLogicResult {
        attacker_troop_loss,
        defender_troop_loss,
        tick_fraction: (speed_cost
            * tile_cost
            * large_attacker_speed
            * large_defender
            * traitor_cost)
            / input.border_size,
    })
}

/// Upstream traitor debuffs (alliances are a later milestone).
const TRAITOR_DEFENSE_DEBUFF: f64 = 0.5;
const TRAITOR_SPEED_DEBUFF: f64 = 0.8;

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
