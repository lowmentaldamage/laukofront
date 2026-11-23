//! Save and load (port milestone P13). Upstream saves full snapshots of every object; laukofront's
//! engine is deterministic, so a save holds the match setup and the human's actions per tick, and
//! loading replays them (D43). The state hash at the save point is stored and checked, so a save
//! from a different rules file or version is refused instead of silently diverging.
//!
//! File: `%APPDATA%\laukofront\save.toml` (versioned TOML).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::config::GameConfig;
use crate::engine::{new_match, run_turn, Game, Intent, MatchSetup};

pub const SAVE_VERSION: u32 = 1;

/// Intents sent each tick of the current match: (tick before the turn, intents).
#[derive(Resource, Default, Debug, Clone)]
pub struct IntentLog(pub Vec<(u64, Vec<Intent>)>);

#[derive(Resource, Default)]
pub struct SaveRequested(pub bool);

#[derive(Resource, Default)]
pub struct LoadRequested(pub bool);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Turn {
    pub tick: u64,
    pub intents: Vec<Intent>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaveFile {
    pub version: u32,
    pub setup: MatchSetup,
    /// Ticks played when saved.
    pub ticks: u64,
    /// `Game::state_hash` when saved, as text (TOML integers are signed).
    pub state_hash: String,
    /// Hash of the rules the match was played with.
    pub rules_hash: String,
    pub turns: Vec<Turn>,
}

fn fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

pub fn rules_hash(cfg: &GameConfig) -> String {
    format!("{:016x}", fnv(&format!("{cfg:?}")))
}

impl SaveFile {
    pub fn capture(game: &Game, setup: MatchSetup, log: &IntentLog) -> Self {
        Self {
            version: SAVE_VERSION,
            setup,
            ticks: game.ticks(),
            state_hash: format!("{:016x}", game.state_hash()),
            rules_hash: rules_hash(game.config()),
            turns: log
                .0
                .iter()
                .map(|(t, i)| Turn {
                    tick: *t,
                    intents: i.clone(),
                })
                .collect(),
        }
    }

    pub fn to_toml(&self) -> Result<String, String> {
        toml::to_string(self).map_err(|e| e.to_string())
    }

    pub fn from_toml(s: &str) -> Result<Self, String> {
        let f: SaveFile = toml::from_str(s).map_err(|e| format!("not a laukofront save: {e}"))?;
        if f.version != SAVE_VERSION {
            return Err(format!("save version {} is not supported", f.version));
        }
        if f.turns.windows(2).any(|w| w[0].tick >= w[1].tick)
            || f.turns.last().is_some_and(|t| t.tick >= f.ticks)
        {
            return Err("damaged save (turns out of order)".into());
        }
        Ok(f)
    }

    /// Replays the match to the save point and checks it matches.
    pub fn replay(&self, cfg: GameConfig) -> Result<(Game, IntentLog), String> {
        if rules_hash(&cfg) != self.rules_hash {
            return Err("the save was made with different game rules".into());
        }
        let mut game = new_match(cfg, self.setup)?;
        let mut log = IntentLog::default();
        let mut next = self.turns.iter().peekable();
        while game.ticks() < self.ticks {
            let t = game.ticks();
            match next.peek() {
                Some(turn) if turn.tick == t => {
                    run_turn(&mut game, &turn.intents);
                    log.0.push((t, turn.intents.clone()));
                    next.next();
                }
                _ => run_turn(&mut game, &[]),
            }
        }
        if format!("{:016x}", game.state_hash()) != self.state_hash {
            return Err("the save does not replay the same (different version?)".into());
        }
        Ok((game, log))
    }
}

pub fn save_path() -> Option<PathBuf> {
    super::SavedSettings::path().map(|p| p.with_file_name("save.toml"))
}

pub fn has_save() -> bool {
    save_path().is_some_and(|p| p.exists())
}

pub fn write_save(f: &SaveFile) -> Result<PathBuf, String> {
    let p = save_path().ok_or("no folder for saves")?;
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    std::fs::write(&p, f.to_toml()?).map_err(|e| e.to_string())?;
    Ok(p)
}

pub fn read_save() -> Result<SaveFile, String> {
    let p = save_path().ok_or("no folder for saves")?;
    let s = std::fs::read_to_string(&p).map_err(|e| format!("no saved game ({e})"))?;
    SaveFile::from_toml(&s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{MapSize, GAME_CONFIG_PATH};
    use crate::engine::HUMAN;

    fn cfg() -> GameConfig {
        GameConfig::load(GAME_CONFIG_PATH).unwrap()
    }

    fn played() -> (Game, MatchSetup, IntentLog) {
        let mut setup = MatchSetup::bots_only(MapSize::Small, 5, 6);
        setup.nations = 2;
        let mut g = new_match(cfg(), setup).unwrap();
        let mut log = IntentLog::default();
        run_turn(&mut g, &[]);
        let map = g.map();
        let tile = (0..map.num_tiles())
            .find(|&t| crate::engine::util::spawn_tiles(map, t, 4, true).is_some())
            .unwrap();
        let mut step = |g: &mut Game, i: Vec<Intent>| {
            if !i.is_empty() {
                log.0.push((g.ticks(), i.clone()));
            }
            run_turn(g, &i);
        };
        step(
            &mut g,
            vec![Intent::Spawn {
                player: HUMAN,
                tile,
            }],
        );
        for k in 0..400 {
            let i = if k % 40 == 0 {
                vec![Intent::Attack {
                    player: HUMAN,
                    target: None,
                    troops: None,
                }]
            } else {
                vec![]
            };
            step(&mut g, i);
        }
        (g, setup, log)
    }

    #[test]
    fn save_and_replay_give_the_same_match() {
        let (g, setup, log) = played();
        let f = SaveFile::capture(&g, setup, &log);
        let text = f.to_toml().unwrap();
        let back = SaveFile::from_toml(&text).unwrap();
        assert_eq!(back, f);
        let (g2, log2) = back.replay(cfg()).unwrap();
        assert_eq!(g2.ticks(), g.ticks());
        assert_eq!(g2.state_hash(), g.state_hash());
        assert_eq!(log2.0, log.0);
    }

    #[test]
    fn broken_or_foreign_saves_are_refused() {
        let (g, setup, log) = played();
        let f = SaveFile::capture(&g, setup, &log);
        assert!(SaveFile::from_toml("hello").is_err());
        let mut v = f.clone();
        v.version = 99;
        assert!(SaveFile::from_toml(&v.to_toml().unwrap()).is_err());
        let mut h = f.clone();
        h.state_hash = "0000000000000000".into();
        assert!(h.replay(cfg()).is_err(), "a wrong hash is detected");
        let mut r = f.clone();
        r.rules_hash = "x".into();
        assert!(r.replay(cfg()).is_err(), "other rules are refused");
    }
}
