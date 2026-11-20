//! Match setup and the intent -> execution step. Port of the single-player parts of upstream
//! `GameRunner.ts` and `ExecutionManager.ts` (`Executor.createExec`).

use crate::config::{GameConfig, MapSize};
use crate::engine_lib::game_map::TileRef;
use crate::engine_lib::terrain;

use super::executions::{SpawnExecution, SpawnTimerExecution};
use super::game::Game;
use super::player::{PlayerId, PlayerInfo, PlayerType};

/// A player action, as upstream's intents. More kinds arrive with later milestones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    /// Pick (or move) a spawn point during the spawn phase.
    Spawn { player: PlayerId, tile: TileRef },
}

/// What a single-player match is created with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchSetup {
    pub size: MapSize,
    pub seed: i64,
    pub bots: u32,
}

/// Player id of the human in a match built by `new_match`.
pub const HUMAN: PlayerId = 1;

/// Builds a match: generates the map, adds the human (id 1) and `bots` bots, and queues the
/// spawn timer and a random spawn for every bot. Bots spawn on the first tick; the human spawns
/// by sending `Intent::Spawn` during the spawn phase.
pub fn new_match(config: GameConfig, setup: MatchSetup) -> Result<Game, String> {
    let params = config.terrain_params(setup.size);
    let map = terrain::generate(&params, setup.seed).ok_or("map size too small")?;
    if setup.bots as usize + 1 > crate::engine_lib::game_map::PLAYER_ID_MASK as usize {
        return Err(format!("too many bots: {}", setup.bots));
    }
    let mut game = Game::new(config, map, format!("laukofront-{}", setup.seed));
    let human = game.add_player(PlayerInfo::new("human", "You", PlayerType::Human));
    debug_assert_eq!(human, HUMAN);
    game.add_execution(Box::new(SpawnTimerExecution::new()));
    for i in 1..=setup.bots {
        let id = game.add_player(PlayerInfo::new(
            format!("bot_{i}"),
            format!("Bot {i}"),
            PlayerType::Bot,
        ));
        game.add_execution(Box::new(SpawnExecution::new(id, None, false)));
    }
    Ok(game)
}

/// Turns intents into executions (upstream `Executor.createExec`) and runs one tick.
pub fn run_turn(game: &mut Game, intents: &[Intent]) {
    for intent in intents {
        match *intent {
            Intent::Spawn { player, tile } => {
                game.add_execution(Box::new(SpawnExecution::new(player, Some(tile), true)));
            }
        }
    }
    game.execute_next_tick();
}
