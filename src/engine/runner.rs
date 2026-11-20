//! Match setup and the intent -> execution step. Port of the single-player parts of upstream
//! `GameRunner.ts` and `ExecutionManager.ts` (`Executor.createExec`).

use crate::config::{GameConfig, MapSize};
use crate::engine_lib::game_map::TileRef;
use crate::engine_lib::terrain;

use super::attack::AttackId;
use super::attack_exec::{AttackExecution, RetreatExecution, WinCheckExecution};
use super::boats::{BoatRetreatExecution, TradeExecution, TransportShipExecution};
use super::executions::{SpawnExecution, SpawnTimerExecution};
use super::game::Game;
use super::player::{PlayerId, PlayerInfo, PlayerType};
use super::structures::{ConstructionExecution, FactoryExecution, UpgradeExecution};
use super::units::{UnitId, UnitKind};
use super::weapons::{MoveWarshipExecution, NukeExecution, SamExecution, WarshipExecution};

/// A player action, as upstream's intents. More kinds arrive with later milestones.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Intent {
    /// Pick (or move) a spawn point during the spawn phase.
    Spawn { player: PlayerId, tile: TileRef },
    /// Send troops against a neighbour (`target = None` = unowned land). `troops = None` sends the
    /// default share (20% for humans, 5% for bots).
    Attack {
        player: PlayerId,
        target: Option<PlayerId>,
        troops: Option<f64>,
    },
    /// Pull back one of the player's attacks (completes after the retreat delay).
    CancelAttack { player: PlayerId, attack: AttackId },
    /// Send a transport ship to the coast near `tile` (`troops = None`: a fifth of the troops).
    Boat {
        player: PlayerId,
        tile: TileRef,
        troops: Option<f64>,
    },
    /// Turn a transport ship around.
    CancelBoat { player: PlayerId, unit: UnitId },
    /// Build a building at (or, for ports, near) `tile`.
    Build {
        player: PlayerId,
        kind: UnitKind,
        tile: TileRef,
    },
    /// Raise a building's level.
    Upgrade { player: PlayerId, unit: UnitId },
    /// Launch an atom bomb, hydrogen bomb or MIRV from the best silo at `tile`.
    Nuke {
        player: PlayerId,
        kind: UnitKind,
        tile: TileRef,
    },
    /// Build a warship at a port; it patrols around `tile` (water).
    Warship { player: PlayerId, tile: TileRef },
    /// Send a warship to patrol around `tile`.
    MoveWarship {
        player: PlayerId,
        unit: UnitId,
        tile: TileRef,
    },
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
    game.add_execution(Box::new(WinCheckExecution::new()));
    game.add_execution(Box::new(TradeExecution::new(setup.seed)));
    game.add_execution(Box::new(FactoryExecution));
    game.add_execution(Box::new(SamExecution));
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
            Intent::Attack {
                player,
                target,
                troops,
            } => {
                game.add_execution(Box::new(AttackExecution::new(player, target, troops)));
            }
            Intent::CancelAttack { player, attack } => {
                game.add_execution(Box::new(RetreatExecution::new(player, attack)));
            }
            Intent::Boat {
                player,
                tile,
                troops,
            } => {
                game.add_execution(Box::new(TransportShipExecution::new(player, tile, troops)));
            }
            Intent::CancelBoat { player, unit } => {
                game.add_execution(Box::new(BoatRetreatExecution::new(player, unit)));
            }
            Intent::Build { player, kind, tile } => {
                game.add_execution(Box::new(ConstructionExecution::new(player, kind, tile)));
            }
            Intent::Upgrade { player, unit } => {
                game.add_execution(Box::new(UpgradeExecution::new(player, unit)));
            }
            Intent::Nuke { player, kind, tile } => {
                game.add_execution(Box::new(NukeExecution::new(player, kind, tile)));
            }
            Intent::Warship { player, tile } => {
                game.add_execution(Box::new(WarshipExecution::new(player, tile)));
            }
            Intent::MoveWarship { player, unit, tile } => {
                game.add_execution(Box::new(MoveWarshipExecution::new(player, unit, tile)));
            }
        }
    }
    game.execute_next_tick();
}
