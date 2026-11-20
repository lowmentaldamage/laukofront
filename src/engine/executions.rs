//! Executions ported so far:
//! - `SpawnTimerExecution` (upstream `SpawnTimerExecution.ts`)
//! - `SpawnExecution` (upstream `SpawnExecution.ts`, without teams)
//! - `PlayerExecution` (upstream `PlayerExecution.ts`, economy part; structures, alliances,
//!   embargoes and cluster removal come with their milestones)

use crate::engine_lib::game_map::TileRef;
use crate::engine_lib::random::PseudoRandom;

use super::clusters;
use super::game::{Execution, Game};
use super::player::{PlayerId, PlayerType};
use super::rules;
use super::util::{simple_hash, spawn_tiles};

/// Ends the spawn phase once `spawn_phase_ticks` have passed.
#[derive(Debug, Default)]
pub struct SpawnTimerExecution {
    active: bool,
}

impl SpawnTimerExecution {
    pub fn new() -> Self {
        Self { active: true }
    }
}

impl Execution for SpawnTimerExecution {
    fn init(&mut self, _game: &mut Game, _ticks: u64) {}

    fn tick(&mut self, game: &mut Game, _ticks: u64) {
        if game.ticks() > game.config().game.spawn_phase_ticks {
            game.end_spawn_phase();
        }
        self.active = game.in_spawn_phase();
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn active_during_spawn_phase(&self) -> bool {
        true
    }
}

/// Places a player: at `tile` (a click) or at a random valid spot. Re-spawning during the spawn
/// phase moves the player.
pub struct SpawnExecution {
    player: PlayerId,
    tile: Option<TileRef>,
    from_intent: bool,
    queued_during_spawn_phase: bool,
    random: Option<PseudoRandom>,
    active: bool,
}

/// Upstream `MAX_SPAWN_TRIES` / `RELAX_MIN_DIST_AT`.
const MAX_SPAWN_TRIES: u32 = 1_000;
const RELAX_MIN_DIST_AT: u32 = 750;

impl SpawnExecution {
    /// `from_intent`: the spawn came from a player's click, so it is only honoured if it was
    /// issued during the spawn phase (upstream security rule).
    pub fn new(player: PlayerId, tile: Option<TileRef>, from_intent: bool) -> Self {
        Self {
            player,
            tile,
            from_intent,
            queued_during_spawn_phase: false,
            random: None,
            active: true,
        }
    }

    fn find_spawn(&mut self, game: &Game) -> Option<(TileRef, Vec<TileRef>)> {
        let map = game.map();
        let radius = game.config().game.spawn_radius;
        if let Some(center) = self.tile {
            let tiles = spawn_tiles(map, center, radius, false)?;
            return (!tiles.is_empty()).then_some((center, tiles));
        }
        let random = self.random.as_mut()?;
        let min_dist = game.config().game.min_spawn_distance;
        for tries in 1..=MAX_SPAWN_TRIES {
            let x = random.next_int(0, map.width() as i64);
            let y = random.next_int(0, map.height() as i64);
            let center = map.tile(x, y)?;
            if !map.is_land(center) || map.has_owner(center) || map.is_border(center) {
                continue;
            }
            if tries <= RELAX_MIN_DIST_AT {
                let crowded = game.players().iter().any(|p| {
                    p.id() != self.player
                        && p.spawn_tile()
                            .is_some_and(|s| map.manhattan_dist(s, center) < min_dist)
                });
                if crowded {
                    continue;
                }
            }
            if let Some(tiles) = spawn_tiles(map, center, radius, true) {
                return Some((center, tiles));
            }
        }
        None
    }
}

impl Execution for SpawnExecution {
    fn init(&mut self, game: &mut Game, _ticks: u64) {
        self.queued_during_spawn_phase = game.in_spawn_phase();
        let seed = game
            .player(self.player)
            .map(|p| simple_hash(&p.info().id))
            .unwrap_or(0)
            + simple_hash(game.game_id());
        self.random = Some(PseudoRandom::new(seed));
    }

    fn tick(&mut self, game: &mut Game, _ticks: u64) {
        self.active = false;
        if let Some(t) = self.tile {
            if !game.map().is_valid(t) {
                return;
            }
        }
        let Some(player) = game.player(self.player) else {
            return;
        };
        if self.from_intent && !self.queued_during_spawn_phase {
            return;
        }
        let had_spawned = player.has_spawned();
        let kind = player.kind();
        let prev: Vec<TileRef> = player.tiles().to_vec();
        for &t in &prev {
            game.relinquish(t);
        }
        let Some((center, tiles)) = self.find_spawn(game) else {
            for &t in &prev {
                game.conquer(self.player, t);
            }
            return;
        };
        for &t in &tiles {
            game.conquer(self.player, t);
        }
        if !had_spawned {
            game.add_execution(Box::new(PlayerExecution::new(self.player)));
        }
        game.set_spawn_tile(self.player, center);
        // Single player: the human picking a spawn ends the spawn phase (upstream rule).
        if kind == PlayerType::Human {
            game.end_spawn_phase();
        }
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn active_during_spawn_phase(&self) -> bool {
        true
    }
}

/// A player's per-tick economy: troop growth and gold income. Stops when the player has no land.
pub struct PlayerExecution {
    player: PlayerId,
    active: bool,
    last_calc: i64,
}

impl PlayerExecution {
    pub fn new(player: PlayerId) -> Self {
        Self {
            player,
            active: true,
            last_calc: 0,
        }
    }
}

impl Execution for PlayerExecution {
    fn init(&mut self, game: &mut Game, ticks: u64) {
        // Upstream staggers each player's cluster check by a hash of its id.
        let every = game.config().attack_logic.cluster_check_ticks as i64;
        let offset = game
            .player(self.player)
            .map_or(0, |p| simple_hash(&p.info().id) % every);
        self.last_calc = ticks as i64 + offset;
    }

    fn tick(&mut self, game: &mut Game, ticks: u64) {
        let Some(p) = game.player(self.player) else {
            self.active = false;
            return;
        };
        if !p.is_alive() {
            self.active = false;
            return;
        }
        let inc = rules::troop_increase_rate(game.config(), p);
        let gold = rules::gold_addition_rate(game.config(), p.kind());
        if let Some(p) = game.player_mut(self.player) {
            p.add_troops(inc);
            p.add_gold(gold);
        }

        let every = game.config().attack_logic.cluster_check_ticks as i64;
        let Some(p) = game.player(self.player) else {
            return;
        };
        if (ticks as i64 - self.last_calc > every || p.num_tiles() < 100)
            && p.last_tile_change() as i64 >= self.last_calc
        {
            self.last_calc = ticks as i64;
            clusters::remove_clusters(game, self.player);
        }
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn active_during_spawn_phase(&self) -> bool {
        false
    }
}
