//! Loads `assets/config/game.toml` into typed structs. Every key is required and unknown keys are
//! rejected, so a typo or a missing value stops the game with a message naming it.
//! The formulas that use these numbers live in `engine::rules`.

use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::fmt;
use std::path::Path;

use crate::engine_lib::terrain::TerrainParams;

/// Default location of the rules file, relative to the working directory.
pub const GAME_CONFIG_PATH: &str = "assets/config/game.toml";

/// A config file could not be read, parsed or validated.
#[derive(Debug)]
pub struct ConfigError {
    pub source: String,
    pub message: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bad config in {}: {}", self.source, self.message)
    }
}

impl std::error::Error for ConfigError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MapSize {
    Small,
    Medium,
    Large,
}

impl MapSize {
    pub const ALL: [MapSize; 3] = [MapSize::Small, MapSize::Medium, MapSize::Large];
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameConfig {
    pub game: GameRules,
    pub map: MapSizes,
    pub terrain: TerrainConfig,
    pub troops: TroopRules,
    pub gold: GoldRules,
    pub attack: AttackRules,
    pub attack_logic: AttackLogicRules,
    pub boats: BoatRules,
    pub buildings: BuildingRules,
    pub weapons: WeaponRules,
    pub diplomacy: DiplomacyRules,
    /// Nation AI difficulty for this match (set by the match setup, not the file).
    #[serde(skip)]
    pub difficulty: Difficulty,
}

/// Upstream `Difficulty`: how strong and smart nations are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, PartialOrd, Ord, Hash)]
pub enum Difficulty {
    Easy,
    #[default]
    Medium,
    Hard,
    Impossible,
}

impl Difficulty {
    pub const ALL: [Difficulty; 4] = [
        Difficulty::Easy,
        Difficulty::Medium,
        Difficulty::Hard,
        Difficulty::Impossible,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Difficulty::Easy => "easy",
            Difficulty::Medium => "medium",
            Difficulty::Hard => "hard",
            Difficulty::Impossible => "impossible",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|d| d.name() == s.to_ascii_lowercase())
    }

    /// Upstream `maxTroops` nation multiplier.
    pub fn troop_cap_mult(self) -> f64 {
        match self {
            Difficulty::Easy => 0.5,
            Difficulty::Medium => 0.75,
            Difficulty::Hard => 1.0,
            Difficulty::Impossible => 1.25,
        }
    }

    /// Upstream `troopIncreaseRate` nation multiplier.
    pub fn growth_mult(self) -> f64 {
        match self {
            Difficulty::Easy => 0.9,
            Difficulty::Medium => 0.95,
            Difficulty::Hard => 1.0,
            Difficulty::Impossible => 1.05,
        }
    }

    /// Upstream `startManpower` for nations.
    pub fn start_troops(self) -> i64 {
        match self {
            Difficulty::Easy => 12_500,
            Difficulty::Medium => 18_750,
            Difficulty::Hard => 25_000,
            Difficulty::Impossible => 31_250,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiplomacyRules {
    pub alliance_duration_ticks: u64,
    pub alliance_request_duration_ticks: u64,
    pub alliance_request_cooldown_ticks: u64,
    pub alliance_extension_prompt_ticks: u64,
    pub traitor_duration_ticks: u64,
    pub donate_cooldown_ticks: u64,
    pub target_duration_ticks: u64,
    pub target_cooldown_ticks: u64,
    pub temporary_embargo_ticks: u64,
    pub nuke_alliance_break_threshold: u32,
    pub reaction_duration_ticks: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameRules {
    pub tick_hz: u32,
    pub spawn_phase_ticks: u64,
    pub spawn_immunity_ticks: u64,
    pub min_spawn_distance: u32,
    pub spawn_radius: u32,
    pub win_percent: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapSizes {
    pub small: MapDims,
    pub medium: MapDims,
    pub large: MapDims,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapDims {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainConfig {
    pub land_fraction: f64,
    pub octaves: u32,
    pub base_cells: f64,
    pub edge_fade: f64,
    pub mountain_exponent: f64,
    pub min_island_tiles: u32,
    pub min_lake_tiles: u32,
    pub peak_share: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TroopRules {
    pub human_start: i64,
    pub bot_start: i64,
    pub cap_multiplier: f64,
    pub cap_tile_coeff: f64,
    pub cap_tile_exponent: f64,
    pub cap_base: f64,
    pub bot_cap_divisor: f64,
    pub growth_base: f64,
    pub growth_exponent: f64,
    pub growth_divisor: f64,
    pub bot_growth_multiplier: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoldRules {
    pub human_per_tick: i64,
    pub bot_per_tick: i64,
    pub starting_gold: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttackRules {
    pub human_default_percent: u32,
    pub bot_default_percent: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttackLogicRules {
    pub plains_mag: f64,
    pub plains_cost: f64,
    pub highland_mag: f64,
    pub highland_cost: f64,
    pub mountain_mag: f64,
    pub mountain_cost: f64,
    pub large_territory_midpoint: f64,
    pub large_territory_steepness: f64,
    pub large_attacker_depth: f64,
    pub large_defender_depth: f64,
    pub large_attacker_speed_depth: f64,
    pub bot_defender_loss_mult: f64,
    pub terra_nullius_cost_scale: f64,
    pub terra_nullius_min_cost: f64,
    pub terra_nullius_max_cost: f64,
    pub attacker_loss_base: f64,
    pub cheapest_troop_ratio: f64,
    pub attacker_loss_per_density: f64,
    pub speed_cost_divisor: f64,
    pub retreat_malus_percent: f64,
    pub retreat_delay_ticks: u64,
    pub conquer_bot_defender_cutoff_tiles: u32,
    pub cluster_check_ticks: u64,
    pub hard_time_limit_seconds: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoatRules {
    pub max_transport_ships: u32,
    pub attack_share: f64,
    pub retreat_malus_percent: f64,
    pub landing_search_distance: u32,
    pub trade_ship_interval_ticks: u64,
    pub trade_gold_scale: f64,
    pub trade_gold_midpoint: f64,
    pub trade_gold_per_tile: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildingRules {
    pub city_troop_increase: f64,
    pub base_cost: i64,
    pub max_cost: i64,
    pub defense_post_cost_step: i64,
    pub defense_post_max_cost: i64,
    pub defense_post_range: u32,
    pub defense_post_defense_bonus: f64,
    pub defense_post_speed_bonus: f64,
    pub min_structure_distance: u32,
    pub factory_interval_ticks: u64,
    pub factory_station_range: u32,
    pub train_gold_self: i64,
    pub train_gold_min: i64,
    pub train_gold_free_stops: u32,
    pub train_gold_stop_penalty: i64,
    pub silo_cost: i64,
    pub sam_cost_step: i64,
    pub sam_max_cost: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeaponRules {
    pub atom_cost: i64,
    pub hydrogen_cost: i64,
    pub mirv_base_cost: i64,
    pub mirv_cost_step: i64,
    pub atom_inner: u32,
    pub atom_outer: u32,
    pub hydrogen_inner: u32,
    pub hydrogen_outer: u32,
    pub warhead_inner: u32,
    pub warhead_outer: u32,
    pub nuke_speed: u32,
    pub mirv_speed: u32,
    pub warhead_count: u32,
    pub mirv_spread: u32,
    pub silo_cooldown_ticks: u64,
    pub sam_cooldown_ticks: u64,
    pub sam_max_range: f64,
    pub nuke_targetable_range: u32,
    pub warship_cost_step: i64,
    pub warship_max_cost: i64,
    pub warship_health: i64,
    pub warship_patrol_range: u32,
    pub warship_target_range: u32,
    pub warship_shot_range: u32,
    pub warship_shot_interval: u64,
    pub warship_capture_range: u32,
    pub shell_damage: i64,
}

fn err(source: &str, message: impl Into<String>) -> ConfigError {
    ConfigError {
        source: source.to_string(),
        message: message.into(),
    }
}

fn check(ok: bool, source: &str, message: &str) -> Result<(), ConfigError> {
    if ok {
        Ok(())
    } else {
        Err(err(source, message))
    }
}

fn parse<T: DeserializeOwned>(text: &str, source: &str) -> Result<T, ConfigError> {
    toml::from_str(text).map_err(|e| err(source, e.message()))
}

impl GameConfig {
    /// Loads and validates a config file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let name = path.display().to_string();
        let text =
            std::fs::read_to_string(path).map_err(|e| err(&name, format!("cannot read: {e}")))?;
        Self::from_toml_str(&text, &name)
    }

    /// Parses and validates config text. `source` names the text in error messages.
    pub fn from_toml_str(text: &str, source: &str) -> Result<Self, ConfigError> {
        let cfg: GameConfig = parse(text, source)?;
        cfg.validate(source)?;
        Ok(cfg)
    }

    fn validate(&self, s: &str) -> Result<(), ConfigError> {
        let g = &self.game;
        check(g.tick_hz >= 1, s, "game.tick_hz must be at least 1")?;
        check(
            (1..=100).contains(&g.win_percent),
            s,
            "game.win_percent must be 1..=100",
        )?;
        check(
            g.spawn_radius >= 1,
            s,
            "game.spawn_radius must be at least 1",
        )?;
        for (name, d) in [
            ("small", self.map.small),
            ("medium", self.map.medium),
            ("large", self.map.large),
        ] {
            check(
                d.width >= 32 && d.height >= 32 && d.width <= 4096 && d.height <= 4096,
                s,
                &format!("map.{name}: width and height must be 32..=4096"),
            )?;
        }
        let t = &self.terrain;
        check(
            t.land_fraction > 0.05 && t.land_fraction < 0.95,
            s,
            "terrain.land_fraction must be between 0.05 and 0.95",
        )?;
        check(t.octaves >= 1, s, "terrain.octaves must be at least 1")?;
        let tr = &self.troops;
        check(
            tr.human_start >= 0 && tr.bot_start >= 0,
            s,
            "troops.*_start must not be negative",
        )?;
        check(tr.cap_base > 0.0, s, "troops.cap_base must be positive")?;
        check(
            tr.bot_cap_divisor > 0.0,
            s,
            "troops.bot_cap_divisor must be positive",
        )?;
        check(
            tr.growth_divisor > 0.0,
            s,
            "troops.growth_divisor must be positive",
        )?;
        let a = &self.attack;
        check(
            (1..=100).contains(&a.human_default_percent)
                && (1..=100).contains(&a.bot_default_percent),
            s,
            "attack.*_default_percent must be 1..=100",
        )?;
        let l = &self.attack_logic;
        check(
            l.speed_cost_divisor > 0.0 && l.large_territory_midpoint > 1.0,
            s,
            "attack_logic.speed_cost_divisor and large_territory_midpoint must be positive",
        )?;
        check(
            l.cluster_check_ticks >= 1,
            s,
            "attack_logic.cluster_check_ticks must be at least 1",
        )?;
        Ok(())
    }

    pub fn map_dims(&self, size: MapSize) -> MapDims {
        match size {
            MapSize::Small => self.map.small,
            MapSize::Medium => self.map.medium,
            MapSize::Large => self.map.large,
        }
    }

    /// Terrain generator settings for a map size.
    pub fn terrain_params(&self, size: MapSize) -> TerrainParams {
        let d = self.map_dims(size);
        let t = &self.terrain;
        TerrainParams {
            width: d.width,
            height: d.height,
            land_fraction: t.land_fraction,
            octaves: t.octaves,
            base_cells: t.base_cells,
            edge_fade: t.edge_fade,
            mountain_exponent: t.mountain_exponent,
            min_island_tiles: t.min_island_tiles,
            min_lake_tiles: t.min_lake_tiles,
            style: Default::default(),
            peak_share: t.peak_share,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> String {
        std::fs::read_to_string(GAME_CONFIG_PATH).expect("read game.toml")
    }

    #[test]
    fn shipped_file_loads() {
        let c = GameConfig::load(GAME_CONFIG_PATH).expect("game.toml");
        assert_eq!(c.game.tick_hz, 10);
        assert_eq!(c.troops.human_start, 25_000);
        assert_eq!(
            c.map_dims(MapSize::Medium),
            MapDims {
                width: 1200,
                height: 750
            }
        );
    }

    #[test]
    fn missing_key_is_named() {
        let broken = shipped().replace("human_start = 25000", "");
        let e = GameConfig::from_toml_str(&broken, "t")
            .unwrap_err()
            .to_string();
        assert!(e.contains("human_start"), "{e}");
    }

    #[test]
    fn unknown_key_is_rejected() {
        let broken = shipped().replace("[gold]", "[gold]\ntypo = 1");
        let e = GameConfig::from_toml_str(&broken, "t")
            .unwrap_err()
            .to_string();
        assert!(e.contains("typo"), "{e}");
    }

    #[test]
    fn out_of_range_is_rejected() {
        let broken = shipped().replace("win_percent = 80", "win_percent = 0");
        let e = GameConfig::from_toml_str(&broken, "t")
            .unwrap_err()
            .to_string();
        assert!(e.contains("win_percent"), "{e}");
    }

    #[test]
    fn missing_file_names_path() {
        let e = GameConfig::load("no/such.toml").unwrap_err().to_string();
        assert!(e.contains("no/such.toml"), "{e}");
    }
}
