//! Loads `assets/config/game.toml` and `assets/config/ai.toml` into typed structs.
//! Every key is required (`deny_unknown_fields` also rejects typos). Values are range-checked.

use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::path::Path;

use crate::core::{GameError, MAX_PLAYERS, MAX_TERRITORIES, MILLI};

/// Default location of the balance file, relative to the working directory.
pub const GAME_CONFIG_PATH: &str = "assets/config/game.toml";
/// Default location of the AI file, relative to the working directory.
pub const AI_CONFIG_PATH: &str = "assets/config/ai.toml";

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
    pub sim: SimConfig,
    pub map: MapConfig,
    pub territory: TerritoryConfig,
    pub growth: GrowthConfig,
    pub reinforce: ReinforceConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SimConfig {
    pub tick_hz: u32,
    pub max_players: u32,
    pub max_match_ticks: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapConfig {
    pub candidates_per_point: u32,
    pub small: MapSizeConfig,
    pub medium: MapSizeConfig,
    pub large: MapSizeConfig,
}

impl MapConfig {
    pub fn size(&self, size: MapSize) -> &MapSizeConfig {
        match size {
            MapSize::Small => &self.small,
            MapSize::Medium => &self.medium,
            MapSize::Large => &self.large,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapSizeConfig {
    pub width: u16,
    pub height: u16,
    pub territories: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerritoryConfig {
    pub start_troops: u32,
    pub neutral_troops_min: u32,
    pub neutral_troops_max: u32,
    pub base_cap: u32,
    pub cap_per_owned_neighbor: u32,
    pub gold_per_min: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrowthConfig {
    pub interval_ticks: u32,
    pub percent: u32,
    pub min_amount: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReinforceConfig {
    pub cost_gold: u32,
    pub troops: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiConfig {
    pub easy: AiDifficultyConfig,
    pub normal: AiDifficultyConfig,
    pub hard: AiDifficultyConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiDifficultyConfig {
    pub think_interval_ticks: u32,
    pub attack_ratio: f32,
    pub send_percent: u32,
    pub reserve_troops: u32,
    pub prefer_cap_targets: bool,
}

fn read(path: &Path) -> Result<String, GameError> {
    std::fs::read_to_string(path).map_err(|source| GameError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn parse<T: DeserializeOwned>(text: &str, source: &str) -> Result<T, GameError> {
    toml::from_str(text).map_err(|e| GameError::Config {
        source: source.to_string(),
        message: e.message().to_string(),
    })
}

fn check(ok: bool, source: &str, message: impl Into<String>) -> Result<(), GameError> {
    if ok {
        Ok(())
    } else {
        Err(GameError::Config {
            source: source.to_string(),
            message: message.into(),
        })
    }
}

impl GameConfig {
    /// Loads and validates a game config file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, GameError> {
        let path = path.as_ref();
        Self::from_toml_str(&read(path)?, &path.display().to_string())
    }

    /// Parses and validates config text. `source` names the text in error messages.
    pub fn from_toml_str(text: &str, source: &str) -> Result<Self, GameError> {
        let cfg: GameConfig = parse(text, source)?;
        cfg.validate(source)?;
        Ok(cfg)
    }

    fn validate(&self, src: &str) -> Result<(), GameError> {
        check(self.sim.tick_hz >= 1, src, "sim.tick_hz must be at least 1")?;
        check(
            (2..=MAX_PLAYERS as u32).contains(&self.sim.max_players),
            src,
            "sim.max_players must be 2..=255",
        )?;
        check(
            self.sim.max_match_ticks >= 1,
            src,
            "sim.max_match_ticks must be at least 1",
        )?;
        check(
            self.map.candidates_per_point >= 1,
            src,
            "map.candidates_per_point must be at least 1",
        )?;
        for (name, s) in [
            ("small", &self.map.small),
            ("medium", &self.map.medium),
            ("large", &self.map.large),
        ] {
            let cells = s.width as usize * s.height as usize;
            check(
                s.width >= 2 && s.height >= 2,
                src,
                format!("map.{name}: width and height must be at least 2"),
            )?;
            check(
                s.territories as u32 >= self.sim.max_players,
                src,
                format!("map.{name}.territories must be at least sim.max_players"),
            )?;
            check(
                (s.territories as usize) <= cells && (s.territories as usize) <= MAX_TERRITORIES,
                src,
                format!("map.{name}.territories must not exceed the number of cells"),
            )?;
        }
        let t = &self.territory;
        check(
            t.start_troops >= 1,
            src,
            "territory.start_troops must be at least 1",
        )?;
        check(
            t.neutral_troops_min <= t.neutral_troops_max,
            src,
            "territory.neutral_troops_min must be <= neutral_troops_max",
        )?;
        check(
            t.base_cap >= 1,
            src,
            "territory.base_cap must be at least 1",
        )?;
        check(
            self.growth.interval_ticks >= 1,
            src,
            "growth.interval_ticks must be at least 1",
        )?;
        check(
            self.growth.percent <= 100,
            src,
            "growth.percent must be 0..=100",
        )?;
        check(
            self.reinforce.troops >= 1,
            src,
            "reinforce.troops must be at least 1",
        )?;
        Ok(())
    }

    /// Gold income per owned territory per tick, in milli-gold (integer division).
    pub fn gold_milli_per_tick(&self) -> u64 {
        self.territory.gold_per_min as u64 * MILLI / (60 * self.sim.tick_hz as u64)
    }
}

impl AiConfig {
    /// Loads and validates an AI config file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, GameError> {
        let path = path.as_ref();
        Self::from_toml_str(&read(path)?, &path.display().to_string())
    }

    /// Parses and validates AI config text.
    pub fn from_toml_str(text: &str, source: &str) -> Result<Self, GameError> {
        let cfg: AiConfig = parse(text, source)?;
        for (name, d) in [
            ("easy", &cfg.easy),
            ("normal", &cfg.normal),
            ("hard", &cfg.hard),
        ] {
            check(
                d.think_interval_ticks >= 1,
                source,
                format!("{name}.think_interval_ticks must be at least 1"),
            )?;
            check(
                d.attack_ratio > 0.0,
                source,
                format!("{name}.attack_ratio must be positive"),
            )?;
            check(
                (1..=100).contains(&d.send_percent),
                source,
                format!("{name}.send_percent must be 1..=100"),
            )?;
        }
        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> String {
        std::fs::read_to_string(GAME_CONFIG_PATH).expect("read game.toml")
    }

    #[test]
    fn shipped_files_load() {
        let g = GameConfig::load(GAME_CONFIG_PATH).expect("game.toml");
        assert_eq!(g.sim.tick_hz, 10);
        assert_eq!(g.gold_milli_per_tick(), 100);
        AiConfig::load(AI_CONFIG_PATH).expect("ai.toml");
    }

    #[test]
    fn missing_key_is_named() {
        let broken = shipped().replace("start_troops = 50", "");
        let err = GameConfig::from_toml_str(&broken, "test")
            .unwrap_err()
            .to_string();
        assert!(err.contains("start_troops"), "{err}");
    }

    #[test]
    fn unknown_key_is_rejected() {
        let broken = shipped().replace("[growth]", "[growth]\ntypo_key = 1");
        let err = GameConfig::from_toml_str(&broken, "test")
            .unwrap_err()
            .to_string();
        assert!(err.contains("typo_key"), "{err}");
    }

    #[test]
    fn out_of_range_value_is_rejected() {
        let broken = shipped().replace("percent = 5 ", "percent = 500 ");
        let err = GameConfig::from_toml_str(&broken, "test")
            .unwrap_err()
            .to_string();
        assert!(err.contains("growth.percent"), "{err}");
    }

    #[test]
    fn missing_file_names_path() {
        let err = GameConfig::load("no/such/file.toml")
            .unwrap_err()
            .to_string();
        assert!(err.contains("no/such/file.toml"), "{err}");
    }
}
