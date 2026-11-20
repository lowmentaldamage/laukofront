//! Buildings. Ports of upstream `ConstructionExecution.ts`, `UpgradeStructureExecution.ts`,
//! the unit costs in `EngineConfig.unitCost`, `structureMinDist`, and (simplified, D36) the
//! factory/train economy (`FactoryExecution`, `TrainExecution`).

use crate::engine_lib::game_map::TileRef;

use super::game::{Execution, Game};
use super::player::PlayerId;
use super::units::{UnitId, UnitKind};

/// Upstream `unitCost` for buildings: city/port/factory `min(max, 2^n * base)` where n counts
/// the player's existing ones (ports and factories share a count); defense posts
/// `min(250000, (n + 1) * 50000)`.
pub fn build_cost(game: &Game, p: PlayerId, kind: UnitKind) -> i64 {
    let b = &game.config().buildings;
    let Some(pl) = game.player(p) else {
        return i64::MAX;
    };
    let count = |k: UnitKind| -> u32 {
        let owned = game.unit_count(p, k) as u32;
        let i = UnitKind::BUILDABLE
            .iter()
            .position(|&x| x == k)
            .unwrap_or(0);
        owned.min(pl.constructed[i])
    };
    match kind {
        UnitKind::City => (b.base_cost << count(UnitKind::City).min(20)).min(b.max_cost),
        UnitKind::Port | UnitKind::Factory => {
            let n = count(UnitKind::Port) + count(UnitKind::Factory);
            (b.base_cost << n.min(20)).min(b.max_cost)
        }
        UnitKind::MissileSilo => b.silo_cost,
        UnitKind::SamLauncher => {
            ((count(UnitKind::SamLauncher) as i64 + 1) * b.sam_cost_step).min(b.sam_max_cost)
        }
        UnitKind::Warship => {
            let w = &game.config().weapons;
            ((game.unit_count(p, UnitKind::Warship) as i64 + 1) * w.warship_cost_step)
                .min(w.warship_max_cost)
        }
        UnitKind::DefensePost => ((count(UnitKind::DefensePost) as i64 + 1)
            * b.defense_post_cost_step)
            .min(b.defense_post_max_cost),
        _ => i64::MAX,
    }
}

/// Where `kind` would be built for a click on `tile`, or why it cannot.
/// Ports snap to the nearest own ocean-coast tile within 10 tiles.
pub fn placement(
    game: &Game,
    p: PlayerId,
    kind: UnitKind,
    tile: TileRef,
) -> Result<TileRef, &'static str> {
    let map = game.map();
    if !map.is_valid(tile) || game.owner(tile) != Some(p) {
        return Err("You can only build on your own land");
    }
    let mut at = tile;
    if kind == UnitKind::Port && !map.is_ocean_shore(tile) {
        let pl = game.player(p).ok_or("No player")?;
        at = pl
            .border_tiles()
            .iter()
            .copied()
            .filter(|&t| map.is_ocean_shore(t) && map.manhattan_dist(t, tile) <= 10)
            .min_by_key(|&t| (map.manhattan_dist(t, tile), t))
            .ok_or("Ports must be built on the coast")?;
    }
    if game.any_structure_within(at, game.config().buildings.min_structure_distance) {
        return Err("Too close to another building");
    }
    let gold = game.player(p).map_or(0, |pl| pl.gold());
    if gold < build_cost(game, p, kind) {
        return Err("Not enough gold");
    }
    Ok(at)
}

/// Upstream `ConstructionExecution`: pays, places the building, and finishes it after its
/// construction time.
pub struct ConstructionExecution {
    player: PlayerId,
    kind: UnitKind,
    tile: TileRef,
    unit: Option<UnitId>,
    active: bool,
}

impl ConstructionExecution {
    pub fn new(player: PlayerId, kind: UnitKind, tile: TileRef) -> Self {
        Self {
            player,
            kind,
            tile,
            unit: None,
            active: true,
        }
    }
}

impl Execution for ConstructionExecution {
    fn init(&mut self, game: &mut Game, _ticks: u64) {
        if !self.kind.is_structure() {
            self.active = false;
            return;
        }
        let Ok(at) = placement(game, self.player, self.kind, self.tile) else {
            self.active = false;
            return;
        };
        let cost = build_cost(game, self.player, self.kind);
        if let Some(p) = game.player_mut(self.player) {
            p.remove_gold(cost);
        }
        self.unit = Some(game.build_unit(self.player, self.kind, at, 0.0));
    }

    fn tick(&mut self, game: &mut Game, ticks: u64) {
        let Some(id) = self.unit else {
            self.active = false;
            return;
        };
        let Some(u) = game.unit(id) else {
            self.active = false;
            return;
        };
        if !u.is_active() {
            self.active = false;
            return;
        }
        if u.under_construction(ticks) {
            return;
        }
        let owner = u.owner();
        if let Some(u) = game.unit_mut(id) {
            u.under_construction_until = None;
        }
        game.note_constructed(owner, self.kind);
        game.refresh_city_levels(owner);
        self.active = false;
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn active_during_spawn_phase(&self) -> bool {
        false
    }
}

/// Upstream `UpgradeStructureExecution`: pays the building's current cost and raises its level.
pub struct UpgradeExecution {
    player: PlayerId,
    unit: UnitId,
    done: bool,
}

impl UpgradeExecution {
    pub fn new(player: PlayerId, unit: UnitId) -> Self {
        Self {
            player,
            unit,
            done: false,
        }
    }
}

/// Whether `p` can upgrade building `unit` now (owner, finished, upgradable, gold).
pub fn can_upgrade(game: &Game, p: PlayerId, unit: UnitId) -> Result<i64, &'static str> {
    let u = game
        .unit(unit)
        .filter(|u| u.is_active())
        .ok_or("No building")?;
    if u.owner() != p {
        return Err("Not your building");
    }
    if !u.kind().upgradable() {
        return Err("This building cannot be upgraded");
    }
    if u.under_construction(game.ticks()) {
        return Err("Still under construction");
    }
    let cost = build_cost(game, p, u.kind());
    if game.player(p).map_or(0, |pl| pl.gold()) < cost {
        return Err("Not enough gold");
    }
    Ok(cost)
}

impl Execution for UpgradeExecution {
    fn init(&mut self, _game: &mut Game, _ticks: u64) {}

    fn tick(&mut self, game: &mut Game, _ticks: u64) {
        self.done = true;
        let Ok(cost) = can_upgrade(game, self.player, self.unit) else {
            return;
        };
        if let Some(p) = game.player_mut(self.player) {
            p.remove_gold(cost);
        }
        if let Some(u) = game.unit_mut(self.unit) {
            u.level += 1;
        }
        game.refresh_city_levels(self.player);
    }

    fn is_active(&self) -> bool {
        !self.done
    }

    fn active_during_spawn_phase(&self) -> bool {
        false
    }
}

/// Own finished cities and ports a factory's trains reach (within `factory_station_range`).
pub fn factory_stations(game: &Game, factory: UnitId) -> Vec<UnitId> {
    let Some(f) = game.unit(factory) else {
        return Vec::new();
    };
    let range = game.config().buildings.factory_station_range;
    let now = game.ticks();
    let map = game.map();
    game.player_units(f.owner(), None)
        .filter(|u| matches!(u.kind(), UnitKind::City | UnitKind::Port))
        .filter(|u| !u.under_construction(now) && map.manhattan_dist(u.tile(), f.tile()) <= range)
        .map(|u| u.id())
        .collect()
}

/// Gold for one train run of a factory: upstream `trainGold("self", stop)` summed over its
/// stations, times the factory level.
pub fn factory_run_gold(game: &Game, factory: UnitId) -> i64 {
    let b = &game.config().buildings;
    let level = game.unit(factory).map_or(1, |u| u.level()) as i64;
    let stops = factory_stations(game, factory).len() as u32;
    let mut gold = 0;
    for i in 0..stops {
        let penalised = (i + 1).saturating_sub(b.train_gold_free_stops) as i64;
        gold += (b.train_gold_self - penalised * b.train_gold_stop_penalty).max(b.train_gold_min);
    }
    gold * level
}

/// Factories pay their owners every `factory_interval_ticks` (simplified trains, D36).
#[derive(Default)]
pub struct FactoryExecution;

impl Execution for FactoryExecution {
    fn init(&mut self, _game: &mut Game, _ticks: u64) {}

    fn tick(&mut self, game: &mut Game, ticks: u64) {
        let every = game.config().buildings.factory_interval_ticks.max(1);
        let factories: Vec<(UnitId, PlayerId)> = game
            .units()
            .iter()
            .filter(|u| {
                u.is_active() && u.kind() == UnitKind::Factory && !u.under_construction(ticks)
            })
            .filter(|u| (ticks + u.id() as u64 * 13).is_multiple_of(every))
            .map(|u| (u.id(), u.owner()))
            .collect();
        for (f, owner) in factories {
            let gold = factory_run_gold(game, f);
            if let Some(p) = game.player_mut(owner) {
                p.add_gold(gold);
            }
        }
    }

    fn is_active(&self) -> bool {
        true
    }

    fn active_during_spawn_phase(&self) -> bool {
        false
    }
}
