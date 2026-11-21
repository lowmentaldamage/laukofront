//! Match setup and the intent -> execution step. Port of the single-player parts of upstream
//! `GameRunner.ts` and `ExecutionManager.ts` (`Executor.createExec`).

use crate::config::{Difficulty, GameConfig, MapSize};
use crate::engine_lib::earth::EarthMap;
use crate::engine_lib::game_map::TileRef;
use crate::engine_lib::terrain::{self, MapStyle};

use super::attack::AttackId;
use super::attack_exec::{AttackExecution, RetreatExecution, WinCheckExecution};
use super::boats::{BoatRetreatExecution, TradeExecution, TransportShipExecution};
use super::diplomacy::{
    AllianceExtensionExecution, AllianceReplyExecution, AllianceRequestExecution,
    BreakAllianceExecution, DonateExecution, EmbargoExecution, ReactionExecution,
    TargetPlayerExecution,
};
use super::executions::{SpawnExecution, SpawnTimerExecution};
use super::game::Game;
use super::nations::{nation_name, NationExecution};
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
    /// Ask `to` for an alliance (accepts if `to` already asked).
    AllianceRequest { player: PlayerId, to: PlayerId },
    /// Answer `from`'s alliance request.
    AllianceReply {
        player: PlayerId,
        from: PlayerId,
        accept: bool,
    },
    /// Break the alliance with `with` (you become a traitor).
    BreakAlliance { player: PlayerId, with: PlayerId },
    /// Agree to renew the alliance with `with` (renews when both agree).
    RenewAlliance { player: PlayerId, with: PlayerId },
    /// Start or stop trading with `with`.
    Embargo {
        player: PlayerId,
        with: PlayerId,
        on: bool,
    },
    /// Send troops to an ally (`None`: a third of yours).
    DonateTroops {
        player: PlayerId,
        to: PlayerId,
        troops: Option<f64>,
    },
    /// Send gold to an ally (`None`: a third of yours).
    DonateGold {
        player: PlayerId,
        to: PlayerId,
        gold: Option<i64>,
    },
    /// Mark `target` for your allies.
    Target { player: PlayerId, target: PlayerId },
    /// Show a reaction (index into `diplomacy::REACTIONS`) to one player or everyone.
    Reaction {
        player: PlayerId,
        to: Option<PlayerId>,
        what: u8,
    },
}

/// What a single-player match is created with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchSetup {
    /// Size of a generated map (ignored for real-Earth maps).
    pub size: MapSize,
    pub seed: i64,
    pub bots: u32,
    /// AI nations (stronger than bots: they ally, build, sail and nuke).
    pub nations: u32,
    pub difficulty: Difficulty,
    pub map: MapChoice,
}

/// Which map a match is played on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapChoice {
    /// Generated from the seed in this style.
    Generated(MapStyle),
    /// A real-Earth map (the same every match).
    Earth(EarthMap),
}

impl Default for MapChoice {
    fn default() -> Self {
        MapChoice::Generated(MapStyle::Continents)
    }
}

impl MapChoice {
    /// `continents`, `archipelago`, `pangaea`, `lakes`, or an Earth map id (`world`, `europe`…).
    pub fn parse(s: &str) -> Option<Self> {
        MapStyle::parse(s)
            .map(MapChoice::Generated)
            .or_else(|| EarthMap::parse(s).map(MapChoice::Earth))
    }

    pub fn name(self) -> &'static str {
        match self {
            MapChoice::Generated(s) => s.name(),
            MapChoice::Earth(e) => e.id(),
        }
    }

    /// Every map the command line accepts.
    pub fn all() -> Vec<MapChoice> {
        MapStyle::ALL
            .into_iter()
            .map(MapChoice::Generated)
            .chain(EarthMap::ALL.into_iter().map(MapChoice::Earth))
            .collect()
    }
}

impl MatchSetup {
    /// A match with only bots (no nations), medium difficulty.
    pub fn bots_only(size: MapSize, seed: i64, bots: u32) -> Self {
        Self {
            size,
            seed,
            bots,
            nations: 0,
            difficulty: Difficulty::Medium,
            map: MapChoice::default(),
        }
    }

    /// Default number of nations for a map size.
    pub fn default_nations(size: MapSize) -> u32 {
        match size {
            MapSize::Small => 3,
            MapSize::Medium => 6,
            MapSize::Large => 10,
        }
    }
}

/// Player id of the human in a match built by `new_match`.
pub const HUMAN: PlayerId = 1;

/// Builds a match: generates the map, adds the human (id 1) and `bots` bots, and queues the
/// spawn timer and a random spawn for every bot. Bots spawn on the first tick; the human spawns
/// by sending `Intent::Spawn` during the spawn phase.
pub fn new_match(mut config: GameConfig, setup: MatchSetup) -> Result<Game, String> {
    config.difficulty = setup.difficulty;
    let map = match setup.map {
        MapChoice::Generated(style) => {
            let mut params = config.terrain_params(setup.size);
            params.style = style;
            terrain::generate(&params, setup.seed).ok_or("map size too small")?
        }
        MapChoice::Earth(e) => e.build().ok_or(format!("map {} is damaged", e.id()))?,
    };
    if (setup.bots + setup.nations) as usize + 1
        > crate::engine_lib::game_map::PLAYER_ID_MASK as usize
    {
        return Err(format!("too many bots: {}", setup.bots));
    }
    // The default map keeps the old id, so seeds replay as before.
    let game_id = match setup.map {
        MapChoice::Generated(MapStyle::Continents) => format!("laukofront-{}", setup.seed),
        m => format!("laukofront-{}-{}", m.name(), setup.seed),
    };
    let mut game = Game::new(config, map, game_id);
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
    let game_id = game.game_id().to_string();
    let mut used: Vec<String> = Vec::new();
    for i in 1..=setup.nations {
        let mut name = nation_name(setup.seed, i);
        let mut k = 0;
        while used.contains(&name) {
            k += 1;
            name = nation_name(setup.seed, i + 1000 * k);
        }
        used.push(name.clone());
        let sid = format!("nation_{i}");
        let id = game.add_player(PlayerInfo::new(sid.clone(), name, PlayerType::Nation));
        game.add_execution(Box::new(SpawnExecution::new(id, None, false)));
        game.add_execution(Box::new(NationExecution::new(
            id,
            &sid,
            &game_id,
            setup.difficulty,
        )));
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
            Intent::AllianceRequest { player, to } => {
                game.add_execution(Box::new(AllianceRequestExecution::new(player, to)));
            }
            Intent::AllianceReply {
                player,
                from,
                accept,
            } => {
                game.add_execution(Box::new(AllianceReplyExecution::new(from, player, accept)));
            }
            Intent::BreakAlliance { player, with } => {
                game.add_execution(Box::new(BreakAllianceExecution::new(player, with)));
            }
            Intent::RenewAlliance { player, with } => {
                game.add_execution(Box::new(AllianceExtensionExecution::new(player, with)));
            }
            Intent::Embargo { player, with, on } => {
                game.add_execution(Box::new(EmbargoExecution::new(player, with, on)));
            }
            Intent::DonateTroops { player, to, troops } => {
                game.add_execution(Box::new(DonateExecution::troops(player, to, troops)));
            }
            Intent::DonateGold { player, to, gold } => {
                game.add_execution(Box::new(DonateExecution::gold(player, to, gold)));
            }
            Intent::Target { player, target } => {
                game.add_execution(Box::new(TargetPlayerExecution::new(player, target)));
            }
            Intent::Reaction { player, to, what } => {
                game.add_execution(Box::new(ReactionExecution::new(player, to, what)));
            }
        }
    }
    game.execute_next_tick();
}
