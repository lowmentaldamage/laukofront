//! The game state and tick loop. Port of the core of upstream `packages/engine/src/game/GameImpl.ts`
//! (`executeNextTick`, `conquer`, `relinquish`, border bookkeeping, spawn phase) and the
//! `Execution` contract from `Game.ts`.
//!
//! Differences from upstream (rules-equivalent, see DECISIONS D23): per-player tile sets are
//! vectors with a game-wide slot index (each tile has one owner) instead of upstream's `TileSet`,
//! so iteration order differs; nothing in the rules depends on that order yet.

use std::collections::{HashMap, VecDeque};

use crate::config::GameConfig;
use crate::engine_lib::game_map::{GameMap, TileRef};

use super::attack::{Attack, AttackId};
use super::player::{Player, PlayerId, PlayerInfo, PlayerType};
use super::rules;
use super::units::{Unit, UnitId, UnitKind};
use super::water::WaterInfo;

/// Upstream `Execution`: one ongoing piece of simulation (a spawn, a player's economy, an attack).
pub trait Execution: Send + Sync {
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
    /// Every attack of the match, by id (inactive ones stay so executions can see they ended).
    attacks: Vec<Attack>,
    winner: Option<PlayerId>,
    units: Vec<Unit>,
    structure_at: HashMap<TileRef, UnitId>,
    water: WaterInfo,
    /// Flood-fill scratch: tile -> generation it was visited in.
    visit_gen: Vec<u32>,
    gen: u32,
}

impl Game {
    pub fn new(config: GameConfig, map: GameMap, game_id: impl Into<String>) -> Self {
        let n = map.num_tiles() as usize;
        let water = WaterInfo::new(&map);
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
            attacks: Vec::new(),
            winner: None,
            units: Vec::new(),
            structure_at: HashMap::new(),
            water,
            visit_gen: vec![0; n],
            gen: 0,
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

    /// Ticks since the spawn phase ended (0 during it).
    pub fn ticks_since_start(&self) -> u64 {
        self.start_tick.map_or(0, |s| self.ticks.saturating_sub(s))
    }

    /// Upstream `elapsedGameSeconds`.
    pub fn elapsed_game_seconds(&self) -> f64 {
        self.ticks_since_start() as f64 / self.config.game.tick_hz as f64
    }

    /// Upstream `isSpawnImmunityActive`.
    pub fn is_spawn_immunity_active(&self) -> bool {
        self.in_spawn_phase() || self.ticks_since_start() < self.config.game.spawn_immunity_ticks
    }

    /// Upstream `Player.isImmune`: humans and nations are immune during spawn immunity; bots never.
    pub fn is_immune(&self, p: PlayerId) -> bool {
        match self.player(p).map(|p| p.kind()) {
            Some(PlayerType::Human) | Some(PlayerType::Nation) => self.is_spawn_immunity_active(),
            _ => false,
        }
    }

    /// Upstream `canAttackPlayer` (no alliances or teams yet, so nobody is friendly):
    /// only human attackers respect spawn immunity.
    pub fn can_attack_player(&self, attacker: PlayerId, target: PlayerId) -> bool {
        if attacker == target {
            return false;
        }
        match self.player(attacker).map(|p| p.kind()) {
            Some(PlayerType::Human) => !self.is_immune(target),
            Some(_) => true,
            None => false,
        }
    }

    /// Upstream `sharesBorderWith`. `other = None` means unowned land.
    pub fn shares_border_with(&self, p: PlayerId, other: Option<PlayerId>) -> bool {
        let other_id = other.unwrap_or(0);
        let Some(pl) = self.player(p) else {
            return false;
        };
        let mut buf = [0; 4];
        pl.border.iter().any(|&t| {
            let n = self.map.neighbors4(t, &mut buf);
            buf[..n].iter().any(|&nb| self.map.owner_id(nb) == other_id)
        })
    }

    /// Upstream `Player.nearby` (land neighbours only; river crossings need boats, a later
    /// milestone): every owner of passable land next to `p`'s border, `None` = unowned land,
    /// in first-seen order without duplicates.
    pub fn nearby(&self, p: PlayerId) -> Vec<Option<PlayerId>> {
        let mut out: Vec<Option<PlayerId>> = Vec::new();
        let Some(pl) = self.player(p) else {
            return out;
        };
        let mut buf = [0; 4];
        for &t in &pl.border {
            let n = self.map.neighbors4(t, &mut buf);
            for &nb in &buf[..n] {
                if !self.map.is_passable_land(nb) {
                    continue;
                }
                let o = self.map.owner_id(nb);
                if o == p || (o == 0 && self.map.has_fallout(nb)) {
                    continue;
                }
                let entry = (o != 0).then_some(o);
                if !out.contains(&entry) {
                    out.push(entry);
                }
            }
        }
        // Upstream `shoreReachableNeighbors`: from every 10th coast tile, the land 5 tiles
        // across the water (rivers and straits up to 4 tiles wide).
        let mut shore_idx = 0;
        for &t in &pl.border {
            if !self.map.is_shore(t) {
                continue;
            }
            shore_idx += 1;
            if (shore_idx - 1) % 10 != 0 {
                continue;
            }
            let (bx, by) = (self.map.x(t) as i64, self.map.y(t) as i64);
            for (dx, dy) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
                let Some(first) = self.map.tile(bx + dx, by + dy) else {
                    continue;
                };
                if !self.map.is_water(first) {
                    continue;
                }
                let Some(far) = self.map.tile(bx + dx * 5, by + dy * 5) else {
                    continue;
                };
                if !self.map.is_passable_land(far) {
                    continue;
                }
                let o = self.map.owner_id(far);
                if o == p || (o == 0 && self.map.has_fallout(far)) {
                    continue;
                }
                let entry = (o != 0).then_some(o);
                if !out.contains(&entry) {
                    out.push(entry);
                }
            }
        }
        out
    }

    /// Upstream `Player.canAttack(tile)` (without immunity): an owned tile can be attacked if
    /// `p` borders its owner; an unowned one if the free land it belongs to (searched up to 200
    /// tiles away, manhattan) touches `p`'s land.
    pub fn can_attack_tile(&self, p: PlayerId, tile: TileRef) -> bool {
        let map = &self.map;
        if !map.is_valid(tile) || !map.is_passable_land(tile) {
            return false;
        }
        let owner = map.owner_id(tile);
        if owner == p {
            return false;
        }
        if owner != 0 {
            return self.shares_border_with(p, Some(owner));
        }
        let mut seen = std::collections::HashSet::new();
        let mut queue = VecDeque::from([tile]);
        seen.insert(tile);
        let mut buf = [0; 4];
        while let Some(t) = queue.pop_front() {
            let k = map.neighbors4(t, &mut buf);
            for &nb in &buf[..k] {
                if map.owner_id(nb) == p {
                    return true;
                }
                if !seen.contains(&nb)
                    && map.is_passable_land(nb)
                    && !map.has_owner(nb)
                    && map.manhattan_dist(nb, tile) <= 200
                {
                    seen.insert(nb);
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    pub fn attack(&self, id: AttackId) -> Option<&Attack> {
        self.attacks.get(id as usize)
    }

    pub fn attack_mut(&mut self, id: AttackId) -> Option<&mut Attack> {
        self.attacks.get_mut(id as usize)
    }

    /// Upstream `incomingAttacks`: attacks against `p` whose attacker is still alive.
    pub fn incoming_attacks(&self, p: PlayerId) -> Vec<AttackId> {
        self.player(p).map_or_else(Vec::new, |pl| {
            pl.incoming
                .iter()
                .copied()
                .filter(|&a| {
                    self.attack(a)
                        .and_then(|at| self.player(at.attacker))
                        .is_some_and(|ap| ap.is_alive())
                })
                .collect()
        })
    }

    /// The winner, once the win check has found one.
    pub fn winner(&self) -> Option<PlayerId> {
        self.winner
    }

    pub(crate) fn set_winner(&mut self, p: PlayerId) {
        if self.winner.is_none() {
            self.winner = Some(p);
        }
    }

    /// Players that still own land.
    pub fn alive_players(&self) -> impl Iterator<Item = &Player> {
        self.players.iter().filter(|p| p.is_alive())
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

    /// The player's land split into 4-connected pieces.
    pub fn components_of(&mut self, p: PlayerId) -> Vec<Vec<TileRef>> {
        let Some(pl) = self.player(p) else {
            return Vec::new();
        };
        let tiles = pl.tiles.clone();
        self.gen = self.gen.wrapping_add(1);
        if self.gen == 0 {
            self.visit_gen.fill(0);
            self.gen = 1;
        }
        let gen = self.gen;
        let mut pieces = Vec::new();
        let mut buf = [0; 4];
        for start in tiles {
            if self.visit_gen[start as usize] == gen {
                continue;
            }
            self.visit_gen[start as usize] = gen;
            let mut piece = vec![start];
            let mut head = 0;
            while head < piece.len() {
                let t = piece[head];
                head += 1;
                let n = self.map.neighbors4(t, &mut buf);
                for &nb in &buf[..n] {
                    if self.visit_gen[nb as usize] != gen && self.map.owner_id(nb) == p {
                        self.visit_gen[nb as usize] = gen;
                        piece.push(nb);
                    }
                }
            }
            pieces.push(piece);
        }
        pieces
    }

    // ----- units -----

    pub fn units(&self) -> &[Unit] {
        &self.units
    }

    pub fn unit(&self, id: UnitId) -> Option<&Unit> {
        self.units.get(id as usize)
    }

    pub fn unit_mut(&mut self, id: UnitId) -> Option<&mut Unit> {
        self.units.get_mut(id as usize)
    }

    pub fn water(&self) -> &WaterInfo {
        &self.water
    }

    /// Routes a ship (see `WaterInfo::find_path`).
    pub fn find_water_path(&mut self, src: TileRef, dst: TileRef) -> Option<Vec<TileRef>> {
        self.water.find_path(&self.map, src, dst)
    }

    /// Active units of a player, optionally of one kind.
    pub fn player_units(
        &self,
        p: PlayerId,
        kind: Option<UnitKind>,
    ) -> impl Iterator<Item = &Unit> + '_ {
        let ids: &[UnitId] = self.player(p).map_or(&[], |pl| pl.units.as_slice());
        ids.iter()
            .filter_map(move |&id| self.unit(id))
            .filter(move |u| u.active && kind.is_none_or(|k| u.kind == k))
    }

    /// Upstream `unitCount`.
    pub fn unit_count(&self, p: PlayerId, kind: UnitKind) -> usize {
        self.player_units(p, Some(kind)).count()
    }

    /// The building on a tile, if any.
    pub fn structure_at(&self, t: TileRef) -> Option<&Unit> {
        self.structure_at
            .get(&t)
            .and_then(|&id| self.unit(id))
            .filter(|u| u.active)
    }

    /// True if `owner` has a finished `kind` building within `range` tiles (euclidean) of `t`.
    pub fn has_structure_near(
        &self,
        t: TileRef,
        range: u32,
        kind: UnitKind,
        owner: PlayerId,
    ) -> bool {
        let r2 = (range as u64) * (range as u64);
        let now = self.ticks;
        self.player_units(owner, Some(kind))
            .any(|u| !u.under_construction(now) && self.map.euclidean_dist_squared(u.tile, t) <= r2)
    }

    /// Any building (of anyone) within `range` tiles (manhattan) of `t`.
    pub fn any_structure_within(&self, t: TileRef, range: u32) -> bool {
        self.structure_at.iter().any(|(&st, &id)| {
            self.unit(id).is_some_and(|u| u.active) && self.map.manhattan_dist(st, t) <= range
        })
    }

    /// Upstream `buildUnit`: creates a unit for `owner` (taking `troops` from it).
    pub(crate) fn build_unit(
        &mut self,
        owner: PlayerId,
        kind: UnitKind,
        tile: TileRef,
        troops: f64,
    ) -> UnitId {
        let id = self.units.len() as UnitId;
        let now = self.ticks;
        let taken = self
            .player_mut(owner)
            .map_or(0, |p| p.remove_troops(troops)) as f64;
        let build_ticks = kind.construction_ticks();
        self.units.push(Unit {
            id,
            kind,
            owner,
            tile,
            active: true,
            level: 1,
            under_construction_until: (build_ticks > 0).then_some(now + build_ticks),
            troops: taken,
            target_tile: None,
            path: Vec::new(),
            path_index: 0,
            retreating: false,
        });
        if kind.is_structure() {
            self.structure_at.insert(tile, id);
        }
        if let Some(p) = self.player_mut(owner) {
            p.units.push(id);
        }
        id
    }

    /// Upstream `Unit.delete`.
    pub(crate) fn delete_unit(&mut self, id: UnitId) {
        let Some(u) = self.units.get_mut(id as usize) else {
            return;
        };
        if !u.active {
            return;
        }
        u.active = false;
        let (owner, tile, kind) = (u.owner, u.tile, u.kind);
        if kind.is_structure() && self.structure_at.get(&tile) == Some(&id) {
            self.structure_at.remove(&tile);
        }
        if let Some(p) = self.player_mut(owner) {
            p.units.retain(|&x| x != id);
        }
        self.refresh_city_levels(owner);
    }

    /// Upstream `captureUnit`: the building changes owner.
    fn capture_unit(&mut self, id: UnitId, new_owner: PlayerId) {
        let Some(u) = self.units.get_mut(id as usize) else {
            return;
        };
        let old = u.owner;
        u.owner = new_owner;
        if let Some(p) = self.player_mut(old) {
            p.units.retain(|&x| x != id);
        }
        if let Some(p) = self.player_mut(new_owner) {
            p.units.push(id);
        }
        self.refresh_city_levels(old);
        self.refresh_city_levels(new_owner);
    }

    /// Recounts a player's finished city levels (troop cap bonus).
    pub(crate) fn refresh_city_levels(&mut self, p: PlayerId) {
        let now = self.ticks;
        let levels: u32 = self
            .player_units(p, Some(UnitKind::City))
            .filter(|u| !u.under_construction(now))
            .map(|u| u.level)
            .sum();
        if let Some(pl) = self.player_mut(p) {
            pl.city_levels = levels;
        }
    }

    /// Counts a finished building for cost scaling.
    pub(crate) fn note_constructed(&mut self, p: PlayerId, kind: UnitKind) {
        if let Some(i) = UnitKind::BUILDABLE.iter().position(|&k| k == kind) {
            if let Some(pl) = self.player_mut(p) {
                pl.constructed[i] += 1;
            }
        }
    }

    /// Buildings on a tile change hands with it: defense posts are destroyed, others captured
    /// (upstream PlayerExecution structure capture).
    fn transfer_structure(&mut self, tile: TileRef, new_owner: u16) {
        let Some(&id) = self.structure_at.get(&tile) else {
            return;
        };
        let Some(u) = self.unit(id) else { return };
        if !u.active || u.owner == new_owner {
            return;
        }
        if new_owner == 0 || u.kind == UnitKind::DefensePost {
            self.delete_unit(id);
        } else {
            self.capture_unit(id, new_owner);
        }
    }

    /// Upstream `createAttack`: registers an attack in both players' lists.
    pub(crate) fn create_attack(
        &mut self,
        attacker: PlayerId,
        target: Option<PlayerId>,
        troops: f64,
        source_tile: Option<TileRef>,
    ) -> AttackId {
        let id = self.attacks.len() as AttackId;
        self.attacks
            .push(Attack::new(id, attacker, target, troops, source_tile));
        if let Some(p) = self.player_mut(attacker) {
            p.outgoing.push(id);
        }
        if let Some(t) = target {
            if let Some(p) = self.player_mut(t) {
                p.incoming.push(id);
            }
        }
        id
    }

    /// Upstream `Attack.delete`: removes the attack from both players' lists and deactivates it.
    pub(crate) fn delete_attack(&mut self, id: AttackId) {
        let Some(a) = self.attacks.get_mut(id as usize) else {
            return;
        };
        a.active = false;
        let (attacker, target) = (a.attacker, a.target);
        if let Some(p) = self.player_mut(attacker) {
            p.outgoing.retain(|&x| x != id);
        }
        if let Some(t) = target {
            if let Some(p) = self.player_mut(t) {
                p.incoming.retain(|&x| x != id);
            }
        }
    }

    /// Upstream `conquerPlayer`: the conqueror takes the conquered player's gold (all of a bot's or
    /// nation's, half of a human's; nothing from a human who never attacked).
    pub(crate) fn conquer_player(&mut self, conqueror: PlayerId, conquered: PlayerId) {
        let Some(c) = self.player(conquered) else {
            return;
        };
        if c.attacks_sent == 0 && c.kind() == PlayerType::Human {
            return;
        }
        let gold = c.gold();
        let captured = match c.kind() {
            PlayerType::Bot | PlayerType::Nation => gold,
            PlayerType::Human => gold / 2,
        };
        if let Some(p) = self.player_mut(conqueror) {
            p.add_gold(captured);
        }
        if let Some(p) = self.player_mut(conquered) {
            p.remove_gold(gold);
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
        self.transfer_structure(tile, owner);
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
        self.transfer_structure(tile, 0);
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
