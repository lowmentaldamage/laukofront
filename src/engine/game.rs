//! The game state and tick loop. Port of the core of upstream `packages/engine/src/game/GameImpl.ts`
//! (`executeNextTick`, `conquer`, `relinquish`, border bookkeeping, spawn phase) and the
//! `Execution` contract from `Game.ts`.
//!
//! Differences from upstream (rules-equivalent, see DECISIONS D23): per-player tile sets are
//! vectors with a game-wide slot index (each tile has one owner) instead of upstream's `TileSet`,
//! so iteration order differs; nothing in the rules depends on that order yet.

use std::collections::VecDeque;

use crate::config::GameConfig;
use crate::engine_lib::game_map::{GameMap, TileRef};

use super::player::{Player, PlayerId, PlayerInfo};
use super::rules;

/// Upstream `Execution`: one ongoing piece of simulation (a spawn, a player's economy, an attack).
pub trait Execution {
    /// Called once, at the end of the tick it was added in (or the first tick after the spawn
    /// phase if it is not active during the spawn phase).
    fn init(&mut self, game: &mut Game, ticks: u64);
    /// Called every tick while active.
    fn tick(&mut self, game: &mut Game, ticks: u64);
    fn is_active(&self) -> bool;
    fn active_during_spawn_phase(&self) -> bool;
}

const NONE: u32 = u32::MAX;

pub struct Game {
    config: GameConfig,
    game_id: String,
    map: GameMap,
    players: Vec<Player>,
    ticks: u64,
    /// `None` during the spawn phase (upstream `startTick === null`).
    start_tick: Option<u64>,
    execs: Vec<Box<dyn Execution>>,
    uninit: Vec<Box<dyn Execution>>,
    /// Index of each tile in its owner's `tiles` vector, or NONE.
    tile_slot: Vec<u32>,
    /// Index of each tile in its owner's `border` vector, or NONE.
    border_slot: Vec<u32>,
    /// Tiles whose owner changed this tick (for the renderer).
    changed_tiles: Vec<TileRef>,
}

impl Game {
    pub fn new(config: GameConfig, map: GameMap, game_id: impl Into<String>) -> Self {
        let n = map.num_tiles() as usize;
        Self {
            config,
            game_id: game_id.into(),
            map,
            players: Vec::new(),
            ticks: 0,
            start_tick: None,
            execs: Vec::new(),
            uninit: Vec::new(),
            tile_slot: vec![NONE; n],
            border_slot: vec![NONE; n],
            changed_tiles: Vec::new(),
        }
    }

    // ----- reading -----

    pub fn config(&self) -> &GameConfig {
        &self.config
    }

    pub fn game_id(&self) -> &str {
        &self.game_id
    }

    pub fn map(&self) -> &GameMap {
        &self.map
    }

    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    pub fn players(&self) -> &[Player] {
        &self.players
    }

    pub fn player(&self, id: PlayerId) -> Option<&Player> {
        (id as usize)
            .checked_sub(1)
            .and_then(|i| self.players.get(i))
    }

    pub fn player_mut(&mut self, id: PlayerId) -> Option<&mut Player> {
        (id as usize)
            .checked_sub(1)
            .and_then(move |i| self.players.get_mut(i))
    }

    pub fn player_by_str_id(&self, id: &str) -> Option<&Player> {
        self.players.iter().find(|p| p.info.id == id)
    }

    /// Owner of a tile, `None` for terra nullius.
    pub fn owner(&self, t: TileRef) -> Option<PlayerId> {
        let id = self.map.owner_id(t);
        (id != 0).then_some(id)
    }

    pub fn in_spawn_phase(&self) -> bool {
        self.start_tick.is_none()
    }

    /// Tick at which the spawn phase ended.
    pub fn start_tick(&self) -> Option<u64> {
        self.start_tick
    }

    /// Tiles whose owner changed during the last tick.
    pub fn changed_tiles(&self) -> &[TileRef] {
        &self.changed_tiles
    }

    pub fn num_executions(&self) -> usize {
        self.execs.len() + self.uninit.len()
    }

    // ----- writing -----

    /// Upstream `addPlayer`: starts with `startManpower` troops and `startingGold`.
    pub fn add_player(&mut self, info: PlayerInfo) -> PlayerId {
        let id = self.players.len() as PlayerId + 1;
        let troops = rules::start_manpower(&self.config, info.kind);
        let gold = rules::starting_gold(&self.config, info.kind);
        self.players.push(Player::new(id, info, troops, gold));
        id
    }

    /// Upstream `addExecution`: queued and initialised at the end of the current tick.
    pub fn add_execution(&mut self, e: Box<dyn Execution>) {
        self.uninit.push(e);
    }

    pub fn end_spawn_phase(&mut self) {
        if self.start_tick.is_none() {
            self.start_tick = Some(self.ticks);
        }
    }

    pub(crate) fn set_spawn_tile(&mut self, p: PlayerId, t: TileRef) {
        if let Some(pl) = self.player_mut(p) {
            pl.spawn_tile = Some(t);
        }
    }

    /// Upstream `conquer`: gives `tile` to `owner`. Returns false (and changes nothing) for water,
    /// impassable land, an unknown player, or a tile the player already owns.
    pub fn conquer(&mut self, owner: PlayerId, tile: TileRef) -> bool {
        if !self.map.is_valid(tile)
            || !self.map.is_passable_land(tile)
            || self.player(owner).is_none()
        {
            return false;
        }
        let prev = self.map.owner_id(tile);
        if prev == owner {
            return false;
        }
        if prev != 0 {
            self.remove_tile_from(prev, tile);
        }
        self.map.set_owner_id(tile, owner);
        let ticks = self.ticks;
        let slot = &mut self.tile_slot[tile as usize];
        let pl = &mut self.players[owner as usize - 1];
        *slot = pl.tiles.len() as u32;
        pl.tiles.push(tile);
        pl.last_tile_change = ticks;
        self.update_borders(tile);
        self.changed_tiles.push(tile);
        true
    }

    /// Upstream `relinquish`: makes an owned tile terra nullius. Returns false if unowned.
    pub fn relinquish(&mut self, tile: TileRef) -> bool {
        if !self.map.is_valid(tile) {
            return false;
        }
        let prev = self.map.owner_id(tile);
        if prev == 0 {
            return false;
        }
        self.remove_tile_from(prev, tile);
        self.map.set_owner_id(tile, 0);
        self.update_borders(tile);
        self.changed_tiles.push(tile);
        true
    }

    fn remove_tile_from(&mut self, owner: PlayerId, tile: TileRef) {
        let ticks = self.ticks;
        let pl = &mut self.players[owner as usize - 1];
        pl.last_tile_change = ticks;
        swap_remove(&mut pl.tiles, &mut self.tile_slot, tile);
        swap_remove(&mut pl.border, &mut self.border_slot, tile);
    }

    fn update_borders(&mut self, tile: TileRef) {
        self.update_border_status(tile);
        let mut buf = [0; 4];
        let n = self.map.neighbors4(tile, &mut buf);
        for &nb in &buf[..n] {
            self.update_border_status(nb);
        }
    }

    fn update_border_status(&mut self, t: TileRef) {
        let owner = self.map.owner_id(t);
        if owner == 0 {
            return;
        }
        let is_border = self.map.is_border(t);
        let pl = &mut self.players[owner as usize - 1];
        if is_border {
            if self.border_slot[t as usize] == NONE {
                self.border_slot[t as usize] = pl.border.len() as u32;
                pl.border.push(t);
            }
        } else {
            swap_remove(&mut pl.border, &mut self.border_slot, t);
        }
    }

    /// Upstream `executeNextTick`: tick active executions, initialise new ones, drop finished ones,
    /// then advance the tick counter.
    pub fn execute_next_tick(&mut self) {
        self.changed_tiles.clear();
        let ticks = self.ticks;

        let mut execs = std::mem::take(&mut self.execs);
        for e in execs.iter_mut() {
            if (!self.in_spawn_phase() || e.active_during_spawn_phase()) && e.is_active() {
                e.tick(self, ticks);
            }
        }
        self.execs = execs;

        // Initialise new executions in FIFO order, including ones added while initialising
        // (upstream iterates the growing array).
        let mut queue: VecDeque<Box<dyn Execution>> = std::mem::take(&mut self.uninit).into();
        let mut kept = Vec::new();
        while let Some(mut e) = queue.pop_front() {
            if !self.in_spawn_phase() || e.active_during_spawn_phase() {
                e.init(self, ticks);
                self.execs.push(e);
            } else {
                kept.push(e);
            }
            queue.extend(self.uninit.drain(..));
        }
        self.uninit = kept;

        let spawn = self.in_spawn_phase();
        self.execs.retain(|e| {
            if spawn {
                !e.active_during_spawn_phase() || e.is_active()
            } else {
                e.is_active()
            }
        });

        self.ticks += 1;
    }

    /// Order-independent hash of ownership, troops and gold (for determinism tests).
    pub fn state_hash(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |v: u64| {
            for b in v.to_le_bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        };
        eat(self.ticks);
        for t in 0..self.map.num_tiles() {
            eat(self.map.owner_id(t) as u64);
        }
        for p in &self.players {
            eat(p.troops as u64);
            eat(p.gold as u64);
            eat(p.tiles.len() as u64);
        }
        h
    }
}

fn swap_remove(list: &mut Vec<TileRef>, slots: &mut [u32], t: TileRef) {
    let i = slots[t as usize];
    if i == NONE {
        return;
    }
    let last = list.pop().unwrap_or(t);
    if last != t {
        list[i as usize] = last;
        slots[last as usize] = i;
    }
    slots[t as usize] = NONE;
}
