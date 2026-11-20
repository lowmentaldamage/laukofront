//! Ships. Ports of upstream `TransportShipExecution.ts`, `BoatRetreatExecution.ts`,
//! `TransportShipUtils.ts` and (simplified, D36) `PortExecution.ts`/`TradeShipExecution.ts`.

use std::collections::HashMap;

use crate::engine_lib::detmath::exp;
use crate::engine_lib::game_map::TileRef;
use crate::engine_lib::random::PseudoRandom;

use super::attack_exec::AttackExecution;
use super::game::{Execution, Game};
use super::player::PlayerId;
use super::units::{UnitId, UnitKind};

/// Water bodies `p`'s coast touches.
fn reachable_bodies(game: &Game, p: PlayerId) -> Vec<u32> {
    let map = game.map();
    let mut out: Vec<u32> = Vec::new();
    if let Some(pl) = game.player(p) {
        for &t in pl.border_tiles() {
            if !map.is_shore(t) {
                continue;
            }
            for b in game.water().bodies_at(map, t).into_iter().flatten() {
                if !out.contains(&b) {
                    out.push(b);
                }
            }
        }
    }
    out
}

/// Upstream `targetTransportTile`: the coast tile nearest to `tile` (within the landing search
/// distance) owned by `tile`'s owner and reachable by water from `attacker`'s coast.
pub fn target_transport_tile(game: &Game, attacker: PlayerId, tile: TileRef) -> Option<TileRef> {
    let map = game.map();
    if !map.is_valid(tile) {
        return None;
    }
    let target_id = map.owner_id(tile);
    let reach = reachable_bodies(game, attacker);
    if reach.is_empty() {
        return None;
    }
    let ok = |t: TileRef| {
        map.is_shore(t)
            && map.owner_id(t) == target_id
            && game
                .water()
                .bodies_at(map, t)
                .iter()
                .flatten()
                .any(|b| reach.contains(b))
    };
    if ok(tile) {
        return Some(tile);
    }
    let max = game.config().boats.landing_search_distance as i64;
    let (cx, cy) = (map.x(tile) as i64, map.y(tile) as i64);
    // Rings of growing manhattan distance: nearest first, then lowest index.
    for d in 1..=max {
        let mut best: Option<TileRef> = None;
        for dx in -d..=d {
            let dy_abs = d - dx.abs();
            for dy in [dy_abs, -dy_abs] {
                if let Some(t) = map.tile(cx + dx, cy + dy) {
                    if ok(t) && best.is_none_or(|b| t < b) {
                        best = Some(t);
                    }
                }
                if dy_abs == 0 {
                    break;
                }
            }
        }
        if best.is_some() {
            return best;
        }
    }
    None
}

/// Upstream `closestShoreByWater`: `p`'s coast tile nearest to `dst` that shares a water body
/// with it.
pub fn closest_shore_by_water(game: &Game, p: PlayerId, dst: TileRef) -> Option<TileRef> {
    let map = game.map();
    let pl = game.player(p)?;
    pl.border_tiles()
        .iter()
        .copied()
        .filter(|&t| map.is_shore(t) && game.water().share_body(map, t, dst))
        .min_by_key(|&t| (map.euclidean_dist_squared(t, dst), t))
}

/// Upstream `canBuildTransportShip`: (launch tile, landing tile) if `p` can send a boat at `tile`.
pub fn can_send_boat(game: &Game, p: PlayerId, tile: TileRef) -> Option<(TileRef, TileRef)> {
    if game.unit_count(p, UnitKind::TransportShip)
        >= game.config().boats.max_transport_ships as usize
    {
        return None;
    }
    let other = game.owner(tile);
    if other == Some(p) {
        return None;
    }
    if let Some(o) = other {
        if !game.can_attack_player(p, o) {
            return None;
        }
    }
    let dst = target_transport_tile(game, p, tile)?;
    let src = closest_shore_by_water(game, p, dst)?;
    Some((src, dst))
}

/// Upstream `tradeShipGold`.
pub fn trade_gold(game: &Game, dist: f64) -> i64 {
    let b = &game.config().boats;
    (b.trade_gold_scale / (1.0 + exp(-0.03 * (dist - b.trade_gold_midpoint)))
        + b.trade_gold_per_tile * dist)
        .floor() as i64
}

/// A boat carrying troops to another coast; lands and attacks from the landing tile.
pub struct TransportShipExecution {
    attacker: PlayerId,
    tile: TileRef,
    troops: Option<f64>,
    target: Option<PlayerId>,
    dst: Option<TileRef>,
    boat: Option<UnitId>,
    retreat_planned: bool,
    active: bool,
}

impl TransportShipExecution {
    /// `troops = None` sends the default share (a fifth of the troops).
    pub fn new(attacker: PlayerId, tile: TileRef, troops: Option<f64>) -> Self {
        Self {
            attacker,
            tile,
            troops,
            target: None,
            dst: None,
            boat: None,
            retreat_planned: false,
            active: true,
        }
    }

    fn finish(&mut self, game: &mut Game, refund: f64) {
        if let Some(b) = self.boat {
            if refund > 0.0 {
                if let Some(p) = game.player_mut(self.attacker) {
                    p.add_troops(refund);
                }
            }
            game.delete_unit(b);
        }
        self.active = false;
    }
}

impl Execution for TransportShipExecution {
    fn init(&mut self, game: &mut Game, _ticks: u64) {
        if !game.map().is_valid(self.tile) || game.player(self.attacker).is_none() {
            self.active = false;
            return;
        }
        let Some((src, dst)) = can_send_boat(game, self.attacker, self.tile) else {
            self.active = false;
            return;
        };
        self.target = game.owner(self.tile);
        let have = game
            .player(self.attacker)
            .map_or(0.0, |p| p.troops() as f64);
        let wanted = self
            .troops
            .unwrap_or(have * game.config().boats.attack_share);
        let troops = wanted.min(have);
        if troops < 1.0 {
            self.active = false;
            return;
        }
        let Some(path) = game.find_water_path(src, dst) else {
            self.active = false;
            return;
        };
        let id = game.build_unit(self.attacker, UnitKind::TransportShip, src, troops);
        if let Some(u) = game.unit_mut(id) {
            u.target_tile = Some(dst);
            u.path = path;
            u.path_index = 0;
        }
        if let Some(p) = game.player_mut(self.attacker) {
            p.attacks_sent += 1;
        }
        self.boat = Some(id);
        self.dst = Some(dst);
    }

    fn tick(&mut self, game: &mut Game, _ticks: u64) {
        let Some(id) = self.boat else {
            self.active = false;
            return;
        };
        let Some(boat) = game.unit(id) else {
            self.active = false;
            return;
        };
        if !boat.is_active() {
            self.active = false;
            return;
        }
        let troops = boat.troops();
        // Retreat: head for our nearest coast once.
        if boat.is_retreating() && !self.retreat_planned {
            self.retreat_planned = true;
            let here = boat.tile();
            match closest_shore_by_water(game, self.attacker, here)
                .and_then(|home| game.find_water_path(here, home).map(|p| (home, p)))
            {
                Some((home, path)) => {
                    self.dst = Some(home);
                    if let Some(u) = game.unit_mut(id) {
                        u.target_tile = Some(home);
                        u.path = path;
                        u.path_index = 0;
                    }
                }
                None => {
                    self.finish(game, troops);
                    return;
                }
            }
        }
        let Some(dst) = self.dst else {
            self.finish(game, troops);
            return;
        };
        // One step per tick.
        let next = game
            .unit(id)
            .and_then(|u| u.path.get(u.path_index).copied());
        match next {
            Some(t) => {
                if let Some(u) = game.unit_mut(id) {
                    u.tile = t;
                    u.path_index += 1;
                }
                if t != dst {
                    return;
                }
            }
            None => {
                self.finish(game, troops);
                return;
            }
        }
        // Arrived.
        if game.owner(dst) == Some(self.attacker) {
            let malus = game.config().boats.retreat_malus_percent / 100.0;
            self.finish(game, troops * (1.0 - malus));
            return;
        }
        if let Some(o) = game.owner(dst) {
            if game.is_friendly(self.attacker, o) {
                // Upstream lands on the friend's coast and hands the troops back home.
                self.finish(game, troops);
                return;
            }
        }
        game.conquer(self.attacker, dst);
        game.add_execution(Box::new(AttackExecution::from_boat(
            self.attacker,
            self.target,
            troops,
            dst,
        )));
        self.finish(game, 0.0);
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn active_during_spawn_phase(&self) -> bool {
        false
    }
}

/// Orders a transport ship home (upstream `BoatRetreatExecution`).
pub struct BoatRetreatExecution {
    player: PlayerId,
    unit: UnitId,
    done: bool,
}

impl BoatRetreatExecution {
    pub fn new(player: PlayerId, unit: UnitId) -> Self {
        Self {
            player,
            unit,
            done: false,
        }
    }
}

impl Execution for BoatRetreatExecution {
    fn init(&mut self, _game: &mut Game, _ticks: u64) {}

    fn tick(&mut self, game: &mut Game, _ticks: u64) {
        if let Some(u) = game.unit_mut(self.unit) {
            if u.owner == self.player && u.kind == UnitKind::TransportShip && u.active {
                u.retreating = true;
            }
        }
        self.done = true;
    }

    fn is_active(&self) -> bool {
        !self.done
    }

    fn active_during_spawn_phase(&self) -> bool {
        false
    }
}

/// Trade (simplified port, D36): every finished port sends a trade ship about every
/// `trade_ship_interval_ticks` to a random other player's port on the same water. On arrival both
/// owners get `trade_gold(route length)`.
pub struct TradeExecution {
    random: PseudoRandom,
    /// (ship, destination port).
    ships: Vec<(UnitId, UnitId)>,
    route_cache: HashMap<(TileRef, TileRef), Vec<TileRef>>,
}

impl TradeExecution {
    pub fn new(seed: i64) -> Self {
        Self {
            random: PseudoRandom::new(seed ^ 0x7a4e),
            ships: Vec::new(),
            route_cache: HashMap::new(),
        }
    }

    fn launch(&mut self, game: &mut Game, port: UnitId) {
        let Some(p) = game.unit(port).cloned() else {
            return;
        };
        let map = game.map();
        let candidates: Vec<UnitId> = game
            .units()
            .iter()
            .filter(|u| {
                u.is_active()
                    && u.kind() == UnitKind::Port
                    && u.owner() != p.owner()
                    && game.can_trade(u.owner(), p.owner())
                    && !u.under_construction(game.ticks())
                    && game.water().share_body(map, u.tile(), p.tile())
            })
            .map(|u| u.id())
            .collect();
        let Some(i) = self.random.index(candidates.len()) else {
            return;
        };
        let dest = candidates[i];
        let Some(dest_tile) = game.unit(dest).map(|u| u.tile()) else {
            return;
        };
        let key = (p.tile(), dest_tile);
        let path = match self.route_cache.get(&key) {
            Some(r) => r.clone(),
            None => {
                let Some(r) = game.find_water_path(p.tile(), dest_tile) else {
                    return;
                };
                if self.route_cache.len() > 512 {
                    self.route_cache.clear();
                }
                self.route_cache.insert(key, r.clone());
                r
            }
        };
        let ship = game.build_unit(p.owner(), UnitKind::TradeShip, p.tile(), 0.0);
        if let Some(u) = game.unit_mut(ship) {
            u.target_tile = Some(dest_tile);
            u.path = path;
            u.path_index = 0;
        }
        self.ships.push((ship, dest));
    }
}

impl Execution for TradeExecution {
    fn init(&mut self, _game: &mut Game, _ticks: u64) {}

    fn tick(&mut self, game: &mut Game, ticks: u64) {
        let every = game.config().boats.trade_ship_interval_ticks.max(1);
        let ports: Vec<UnitId> = game
            .units()
            .iter()
            .filter(|u| u.is_active() && u.kind() == UnitKind::Port && !u.under_construction(ticks))
            .filter(|u| (ticks + u.id() as u64 * 37).is_multiple_of(every))
            .map(|u| u.id())
            .collect();
        for port in ports {
            self.launch(game, port);
        }
        // Move ships; pay on arrival.
        let mut keep = Vec::with_capacity(self.ships.len());
        for (ship, dest) in std::mem::take(&mut self.ships) {
            let Some(u) = game.unit(ship) else { continue };
            if !u.is_active() {
                continue;
            }
            let next = u.path.get(u.path_index).copied();
            let len = u.path.len();
            match next {
                Some(t) => {
                    if let Some(u) = game.unit_mut(ship) {
                        u.tile = t;
                        u.path_index += 1;
                    }
                    let arrived = game
                        .unit(ship)
                        .is_some_and(|u| u.path_index >= u.path.len());
                    if !arrived {
                        keep.push((ship, dest));
                        continue;
                    }
                    let owner = game.unit(ship).map(|u| u.owner());
                    let dest_owner = game.unit(dest).filter(|d| d.is_active()).map(|d| d.owner());
                    if let (Some(o), Some(d)) = (owner, dest_owner) {
                        if o != d {
                            let gold = trade_gold(game, len as f64);
                            if let Some(p) = game.player_mut(o) {
                                p.add_gold(gold);
                            }
                            if let Some(p) = game.player_mut(d) {
                                p.add_gold(gold);
                            }
                        }
                    }
                    game.delete_unit(ship);
                }
                None => game.delete_unit(ship),
            }
        }
        self.ships = keep;
    }

    fn is_active(&self) -> bool {
        true
    }

    fn active_during_spawn_phase(&self) -> bool {
        false
    }
}
