//! Missiles, SAMs and warships. Simplified ports (D39) of upstream `NukeExecution.ts`,
//! `MIRVExecution.ts`, `MissileSiloExecution.ts`, `SAMLauncherExecution.ts`,
//! `SAMMissileExecution.ts`, `WarshipExecution.ts`, `ShellExecution.ts`,
//! `MoveWarshipExecution.ts`, with upstream's costs, blast radii, kill formula, ranges and
//! cooldowns from `[weapons]` in `game.toml`.

use std::collections::{HashSet, VecDeque};

use crate::engine_lib::detmath::exp;
use crate::engine_lib::game_map::TileRef;
use crate::engine_lib::random::PseudoRandom;

use super::game::{Execution, Game};
use super::player::PlayerId;
use super::rules;
use super::units::{UnitId, UnitKind};

// ----- prices, radii, ranges -----

/// Upstream nuke costs; MIRVs get dearer with every MIRV launched in the match.
pub fn nuke_cost(game: &Game, kind: UnitKind) -> i64 {
    let w = &game.config().weapons;
    match kind {
        UnitKind::AtomBomb => w.atom_cost,
        UnitKind::HydrogenBomb => w.hydrogen_cost,
        UnitKind::Mirv => w.mirv_base_cost + game.mirvs_launched as i64 * w.mirv_cost_step,
        _ => i64::MAX,
    }
}

/// Upstream `nukeMagnitudes`: (inner, outer) blast radius in tiles.
pub fn blast_radius(game: &Game, kind: UnitKind) -> (u32, u32) {
    let w = &game.config().weapons;
    match kind {
        UnitKind::AtomBomb => (w.atom_inner, w.atom_outer),
        UnitKind::HydrogenBomb => (w.hydrogen_inner, w.hydrogen_outer),
        UnitKind::MirvWarhead => (w.warhead_inner, w.warhead_outer),
        _ => (0, 0),
    }
}

/// Upstream `samRange(level)`: 70 at level 1, growing toward 150.
pub fn sam_range(game: &Game, level: u32) -> f64 {
    game.config().weapons.sam_max_range - 480.0 / (level as f64 + 5.0)
}

/// Upstream `nukeDeathFactor`: troops killed per destroyed tile.
pub fn nuke_death_factor(kind: UnitKind, humans: f64, tiles_owned: u32, max_troops: f64) -> f64 {
    if kind != UnitKind::MirvWarhead {
        return 5.0 * humans / tiles_owned.max(1) as f64;
    }
    let target = 0.03 * max_troops;
    let excess = (humans - target).max(0.0);
    500.0 * (1.0 - exp(-2.0 * excess / max_troops.max(1.0)))
}

/// Free missile slots of a silo or SAM: `level` slots, each reloading for `cooldown` ticks.
fn free_slot(game: &Game, unit: UnitId, cooldown: u64) -> bool {
    let now = game.ticks();
    game.unit(unit).is_some_and(|u| {
        u.is_active()
            && !u.under_construction(now)
            && u.reload.iter().filter(|&&t| now < t + cooldown).count() < u.level() as usize
    })
}

fn use_slot(game: &mut Game, unit: UnitId, cooldown: u64) {
    let now = game.ticks();
    if let Some(u) = game.unit_mut(unit) {
        u.reload.retain(|&t| now < t + cooldown);
        u.reload.push(now);
    }
}

/// The silo `p` would launch `kind` at `tile` from, or why it cannot.
pub fn can_launch(
    game: &Game,
    p: PlayerId,
    kind: UnitKind,
    tile: TileRef,
) -> Result<UnitId, &'static str> {
    let map = game.map();
    if !map.is_valid(tile) {
        return Err("Off the map");
    }
    if !matches!(
        kind,
        UnitKind::AtomBomb | UnitKind::HydrogenBomb | UnitKind::Mirv
    ) {
        return Err("Not a missile");
    }
    if kind == UnitKind::Mirv {
        match game.owner(tile) {
            Some(o) if o != p => {}
            _ => return Err("A MIRV must target another player's land"),
        }
    }
    if game.player(p).map_or(0, |pl| pl.gold()) < nuke_cost(game, kind) {
        return Err("Not enough gold");
    }
    let cooldown = game.config().weapons.silo_cooldown_ticks;
    let silos: Vec<(UnitId, TileRef)> = game
        .player_units(p, Some(UnitKind::MissileSilo))
        .map(|u| (u.id(), u.tile()))
        .collect();
    if silos.is_empty() {
        return Err("Build a missile silo first");
    }
    silos
        .into_iter()
        .filter(|&(id, _)| free_slot(game, id, cooldown))
        .min_by_key(|&(id, t)| (map.euclidean_dist_squared(t, tile), id))
        .map(|(id, _)| id)
        .ok_or("All silos are reloading")
}

// ----- nukes -----

/// One missile in flight: atom bomb, hydrogen bomb, MIRV or MIRV warhead.
pub struct NukeExecution {
    player: PlayerId,
    kind: UnitKind,
    target: TileRef,
    src: Option<TileRef>,
    /// Owner of the target tile at launch (MIRV warheads hit this player's land).
    target_player: Option<PlayerId>,
    unit: Option<UnitId>,
    travelled: f64,
    wait: u64,
    active: bool,
}

impl NukeExecution {
    /// A missile launched by `player` from its best silo at `target`.
    pub fn new(player: PlayerId, kind: UnitKind, target: TileRef) -> Self {
        Self {
            player,
            kind,
            target,
            src: None,
            target_player: None,
            unit: None,
            travelled: 0.0,
            wait: 0,
            active: true,
        }
    }

    fn warhead(player: PlayerId, from: TileRef, target: TileRef, wait: u64) -> Self {
        let mut e = Self::new(player, UnitKind::MirvWarhead, target);
        e.src = Some(from);
        e.wait = wait;
        e
    }

    fn speed(&self, game: &Game) -> f64 {
        let w = &game.config().weapons;
        match self.kind {
            UnitKind::Mirv => w.mirv_speed as f64,
            UnitKind::MirvWarhead => 22.0,
            _ => w.nuke_speed as f64,
        }
    }
}

impl Execution for NukeExecution {
    fn init(&mut self, game: &mut Game, _ticks: u64) {
        if self.kind == UnitKind::MirvWarhead {
            let Some(src) = self.src else {
                self.active = false;
                return;
            };
            self.unit = Some(game.build_unit(self.player, self.kind, src, 0.0));
            return;
        }
        let silo = match can_launch(game, self.player, self.kind, self.target) {
            Ok(s) => s,
            Err(_) => {
                self.active = false;
                return;
            }
        };
        let cost = nuke_cost(game, self.kind);
        if let Some(p) = game.player_mut(self.player) {
            p.remove_gold(cost);
        }
        let cooldown = game.config().weapons.silo_cooldown_ticks;
        use_slot(game, silo, cooldown);
        let src = game.unit(silo).map(|u| u.tile()).unwrap_or(self.target);
        self.src = Some(src);
        self.target_player = game.owner(self.target);
        if self.kind == UnitKind::Mirv {
            game.mirvs_launched += 1;
        }
        let id = game.build_unit(self.player, self.kind, src, 0.0);
        if let Some(u) = game.unit_mut(id) {
            u.target_tile = Some(self.target);
            // Launch tile, kept so SAMs can tell whether the nuke is near its launch point.
            u.path = vec![src];
        }
        self.unit = Some(id);
    }

    fn tick(&mut self, game: &mut Game, ticks: u64) {
        let (Some(id), Some(src)) = (self.unit, self.src) else {
            self.active = false;
            return;
        };
        if !game.unit(id).is_some_and(|u| u.is_active()) {
            // Shot down by a SAM.
            self.active = false;
            return;
        }
        if self.wait > 0 {
            self.wait -= 1;
            return;
        }
        let map = game.map();
        let (sx, sy) = (map.x(src) as f64, map.y(src) as f64);
        let (tx, ty) = (map.x(self.target) as f64, map.y(self.target) as f64);
        let dist = ((tx - sx).powi(2) + (ty - sy).powi(2)).sqrt();
        self.travelled += self.speed(game);
        if self.travelled < dist {
            let f = self.travelled / dist;
            let pos = map.tile(
                (sx + (tx - sx) * f).round() as i64,
                (sy + (ty - sy) * f).round() as i64,
            );
            if let (Some(pos), Some(u)) = (pos, game.unit_mut(id)) {
                u.tile = pos;
            }
            return;
        }
        if let Some(u) = game.unit_mut(id) {
            u.tile = self.target;
        }
        if self.kind == UnitKind::Mirv {
            split_mirv(game, self.player, self.target, self.target_player, ticks);
        } else {
            detonate(game, self.player, self.kind, self.target, ticks);
        }
        game.delete_unit(id);
        self.active = false;
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn active_during_spawn_phase(&self) -> bool {
        false
    }
}

/// MIRV separation: `warhead_count` warheads at random tiles of the target player's land near the
/// target, each after a short random delay (upstream waits 0-15 ticks).
fn split_mirv(
    game: &mut Game,
    player: PlayerId,
    target: TileRef,
    target_player: Option<PlayerId>,
    ticks: u64,
) {
    let w = game.config().weapons.clone();
    let Some(victim) = target_player else { return };
    let map = game.map();
    let land: Vec<TileRef> = game
        .player(victim)
        .map_or_else(Vec::new, |p| p.tiles().to_vec())
        .into_iter()
        .filter(|&t| map.manhattan_dist(t, target) <= w.mirv_spread)
        .collect();
    if land.is_empty() {
        return;
    }
    let mut rng = PseudoRandom::new(ticks as i64 ^ 0x317);
    for _ in 0..w.warhead_count {
        let Some(i) = rng.index(land.len()) else {
            break;
        };
        let wait = rng.next_int(0, 15) as u64;
        game.add_execution(Box::new(NukeExecution::warhead(
            player, target, land[i], wait,
        )));
    }
}

/// Upstream `detonate`: every tile in the inner radius and about half in the outer ring
/// (breadth-first from the target) loses its owner and gets fallout; owners lose troops per tile
/// (`nuke_death_factor`), and so do their attacks and boats; units in the blast are destroyed.
pub fn detonate(game: &mut Game, attacker: PlayerId, kind: UnitKind, target: TileRef, ticks: u64) {
    let (inner, outer) = blast_radius(game, kind);
    let (inner2, outer2) = ((inner as u64).pow(2), (outer as u64).pow(2));
    let mut rng = PseudoRandom::new(ticks as i64);
    let tiles: Vec<TileRef> = {
        let map = game.map();
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let mut queue = VecDeque::from([target]);
        seen.insert(target);
        let mut buf = [0; 4];
        let include = |t: TileRef, rng: &mut PseudoRandom| {
            let d2 = map.euclidean_dist_squared(target, t);
            d2 <= outer2 && (d2 <= inner2 || rng.chance(2)) && !map.is_impassable(t)
        };
        if include(target, &mut rng) {
            while let Some(t) = queue.pop_front() {
                out.push(t);
                let k = map.neighbors4(t, &mut buf);
                for &nb in &buf[..k] {
                    if seen.insert(nb) && include(nb, &mut rng) {
                        queue.push_back(nb);
                    }
                }
            }
        }
        out
    };
    let mut hit: Vec<(PlayerId, u32)> = Vec::new();
    for &t in &tiles {
        if let Some(o) = game.owner(t) {
            game.relinquish(t);
            match hit.iter_mut().find(|(p, _)| *p == o) {
                Some(e) => e.1 += 1,
                None => hit.push((o, 1)),
            }
        }
        if game.map().is_land(t) {
            game.set_fallout(t, true);
        }
    }
    for (p, n) in hit {
        let Some(pl) = game.player(p) else { continue };
        let before = pl.num_tiles() + n;
        let max = rules::troop_cap(game.config(), pl);
        let attacks: Vec<u32> = pl.outgoing_attacks().to_vec();
        let boats: Vec<UnitId> = game
            .player_units(p, Some(UnitKind::TransportShip))
            .map(|u| u.id())
            .collect();
        for i in 0..n {
            let left = before - i;
            let troops = game.player(p).map_or(0.0, |x| x.troops() as f64);
            let d = nuke_death_factor(kind, troops, left, max);
            if let Some(x) = game.player_mut(p) {
                x.remove_troops(d);
            }
            for &a in &attacks {
                if let Some(at) = game.attack_mut(a) {
                    let t = at.troops();
                    at.set_troops(t - nuke_death_factor(kind, t, left, max));
                }
            }
            for &b in &boats {
                if let Some(u) = game.unit_mut(b) {
                    u.troops = (u.troops - nuke_death_factor(kind, u.troops, left, max)).max(0.0);
                }
            }
        }
    }
    let doomed: Vec<UnitId> = game
        .units()
        .iter()
        .filter(|u| u.is_active() && !u.kind().is_nuke())
        .filter(|u| game.map().euclidean_dist_squared(target, u.tile()) < outer2)
        .map(|u| u.id())
        .collect();
    for id in doomed {
        game.delete_unit(id);
    }
    if let Some(p) = game.player_mut(attacker) {
        p.attacks_sent += 1;
    }
    game.record_detonation(target, outer);
}

// ----- SAMs -----

/// Every tick: each finished SAM with a free slot shoots down the first enemy atom or hydrogen
/// bomb in its range that is "targetable" (within `nuke_targetable_range` of its launch or target
/// tile, as upstream). MIRVs and their warheads are not intercepted (D39).
#[derive(Default)]
pub struct SamExecution;

impl Execution for SamExecution {
    fn init(&mut self, _game: &mut Game, _ticks: u64) {}

    fn tick(&mut self, game: &mut Game, _ticks: u64) {
        let cooldown = game.config().weapons.sam_cooldown_ticks;
        let reach = game.config().weapons.nuke_targetable_range as u64;
        let map = game.map();
        let nukes: Vec<(UnitId, PlayerId, TileRef)> = game
            .units()
            .iter()
            .filter(|u| {
                u.is_active() && matches!(u.kind(), UnitKind::AtomBomb | UnitKind::HydrogenBomb)
            })
            .filter(|u| {
                let near = |t: TileRef| map.euclidean_dist_squared(t, u.tile()) <= reach * reach;
                u.target_tile().is_some_and(near) || u.path.first().copied().is_some_and(near)
            })
            .map(|u| (u.id(), u.owner(), u.tile()))
            .collect();
        if nukes.is_empty() {
            return;
        }
        let sams: Vec<(UnitId, PlayerId, TileRef, u32)> = game
            .units()
            .iter()
            .filter(|u| u.is_active() && u.kind() == UnitKind::SamLauncher)
            .map(|u| (u.id(), u.owner(), u.tile(), u.level()))
            .collect();
        for (nuke, owner, at) in nukes {
            for &(sam, sam_owner, sam_tile, level) in &sams {
                if sam_owner == owner || !free_slot(game, sam, cooldown) {
                    continue;
                }
                let r = sam_range(game, level);
                if game.map().euclidean_dist_squared(sam_tile, at) as f64 <= r * r {
                    use_slot(game, sam, cooldown);
                    game.delete_unit(nuke);
                    game.record_sam_shot(sam_tile, at);
                    break;
                }
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

// ----- warships -----

/// Where a warship ordered to `tile` would start: water next to one of `p`'s finished ports on
/// the same water as `tile`.
pub fn warship_spawn(game: &Game, p: PlayerId, tile: TileRef) -> Result<TileRef, &'static str> {
    let map = game.map();
    if !map.is_valid(tile) || !map.is_water(tile) {
        return Err("Click water to place a warship");
    }
    let now = game.ticks();
    let ports: Vec<TileRef> = game
        .player_units(p, Some(UnitKind::Port))
        .filter(|u| !u.under_construction(now))
        .map(|u| u.tile())
        .collect();
    if ports.is_empty() {
        return Err("Warships need a port");
    }
    let mut buf = [0; 4];
    let mut best: Option<(u64, TileRef)> = None;
    for port in ports {
        let k = map.neighbors4(port, &mut buf);
        for &nb in &buf[..k] {
            if map.is_water(nb) && game.water().share_body(map, nb, tile) {
                let d = map.euclidean_dist_squared(nb, tile);
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, nb));
                }
            }
        }
    }
    let spawn = best
        .map(|b| b.1)
        .ok_or("None of your ports is on that water")?;
    if game.player(p).map_or(0, |pl| pl.gold())
        < super::structures::build_cost(game, p, UnitKind::Warship)
    {
        return Err("Not enough gold");
    }
    Ok(spawn)
}

/// A warship: patrols around its patrol point, chases enemy ships within range, shoots transport
/// ships (sunk in one hit) and warships (damage 200-275 per shot), captures trade ships.
pub struct WarshipExecution {
    player: PlayerId,
    patrol: TileRef,
    unit: Option<UnitId>,
    target: Option<UnitId>,
    last_shot: u64,
    random: PseudoRandom,
    active: bool,
}

impl WarshipExecution {
    pub fn new(player: PlayerId, patrol: TileRef) -> Self {
        Self {
            player,
            patrol,
            unit: None,
            target: None,
            last_shot: 0,
            random: PseudoRandom::new(patrol as i64 ^ player as i64),
            active: true,
        }
    }

    fn set_path(game: &mut Game, id: UnitId, to: TileRef) {
        let Some(from) = game.unit(id).map(|u| u.tile()) else {
            return;
        };
        if let Some(path) = game.find_water_path(from, to) {
            if let Some(u) = game.unit_mut(id) {
                u.path = path;
                u.path_index = 0;
            }
        }
    }
}

impl Execution for WarshipExecution {
    fn init(&mut self, game: &mut Game, _ticks: u64) {
        let Ok(spawn) = warship_spawn(game, self.player, self.patrol) else {
            self.active = false;
            return;
        };
        let cost = super::structures::build_cost(game, self.player, UnitKind::Warship);
        if let Some(p) = game.player_mut(self.player) {
            p.remove_gold(cost);
        }
        let id = game.build_unit(self.player, UnitKind::Warship, spawn, 0.0);
        let hp = game.config().weapons.warship_health;
        if let Some(u) = game.unit_mut(id) {
            u.health = hp;
            u.target_tile = Some(self.patrol);
        }
        self.unit = Some(id);
    }

    fn tick(&mut self, game: &mut Game, ticks: u64) {
        let Some(id) = self.unit else {
            self.active = false;
            return;
        };
        let Some(me) = game.unit(id).cloned() else {
            self.active = false;
            return;
        };
        if !me.is_active() {
            self.active = false;
            return;
        }
        // A move order changed the patrol point.
        if let Some(p) = me.target_tile() {
            if p != self.patrol {
                self.patrol = p;
                self.target = None;
                Self::set_path(game, id, p);
            }
        }
        let w = game.config().weapons.clone();
        let map = game.map();
        let r2 = (w.warship_target_range as u64).pow(2);
        // Pick a target every 5 ticks (or keep the current one while it lives and is in range).
        let keep = self.target.and_then(|t| game.unit(t)).is_some_and(|t| {
            t.is_active()
                && t.owner() != me.owner()
                && map.euclidean_dist_squared(t.tile(), me.tile()) <= r2
        });
        if !keep {
            self.target = None;
        }
        if self.target.is_none() && ticks.is_multiple_of(5) {
            self.target = game
                .units()
                .iter()
                .filter(|u| {
                    u.is_active()
                        && u.owner() != me.owner()
                        && matches!(
                            u.kind(),
                            UnitKind::TransportShip | UnitKind::Warship | UnitKind::TradeShip
                        )
                        && map.euclidean_dist_squared(u.tile(), me.tile()) <= r2
                })
                .min_by_key(|u| {
                    let rank = match u.kind() {
                        UnitKind::TransportShip => 0,
                        UnitKind::Warship => 1,
                        _ => 2,
                    };
                    (
                        rank,
                        map.euclidean_dist_squared(u.tile(), me.tile()),
                        u.id(),
                    )
                })
                .map(|u| u.id());
            if let Some(t) = self.target.and_then(|t| game.unit(t)).map(|u| u.tile()) {
                Self::set_path(game, id, t);
            }
        }
        if let Some(tid) = self.target {
            let Some(t) = game.unit(tid).cloned() else {
                return;
            };
            let d2 = game.map().euclidean_dist_squared(t.tile(), me.tile());
            if t.kind() == UnitKind::TradeShip && d2 <= (w.warship_capture_range as u64).pow(2) {
                game.capture_unit(tid, me.owner());
                self.target = None;
            } else if t.kind() != UnitKind::TradeShip
                && d2 <= (w.warship_shot_range as u64).pow(2)
                && ticks >= self.last_shot + w.warship_shot_interval
            {
                self.last_shot = ticks;
                game.record_shell(me.tile(), t.tile());
                if t.kind() == UnitKind::TransportShip {
                    game.delete_unit(tid);
                    self.target = None;
                } else {
                    let roll = self.random.next_int(1, 5);
                    let dmg = ((roll - 1) * 25 + 200) * w.shell_damage / 250;
                    if let Some(u) = game.unit_mut(tid) {
                        u.health -= dmg;
                    }
                    if game.unit(tid).is_some_and(|u| u.health <= 0) {
                        game.delete_unit(tid);
                        self.target = None;
                    }
                }
            } else if ticks.is_multiple_of(10) {
                Self::set_path(game, id, t.tile());
            }
        } else if me.remaining_path().is_empty() {
            // Patrol: a random water tile near the patrol point.
            let map = game.map();
            let (px, py) = (map.x(self.patrol) as i64, map.y(self.patrol) as i64);
            let range = w.warship_patrol_range as i64;
            for _ in 0..20 {
                let x = px + self.random.next_int(-range, range + 1);
                let y = py + self.random.next_int(-range, range + 1);
                if let Some(t) = map.tile(x, y) {
                    if map.is_water(t) && game.water().share_body(map, t, me.tile()) {
                        Self::set_path(game, id, t);
                        break;
                    }
                }
            }
        }
        // One step per tick (never onto land).
        let next = game
            .unit(id)
            .and_then(|u| u.path.get(u.path_index).copied());
        if let Some(n) = next {
            if game.map().is_water(n) {
                if let Some(u) = game.unit_mut(id) {
                    u.tile = n;
                    u.path_index += 1;
                }
            } else if let Some(u) = game.unit_mut(id) {
                u.path.clear();
                u.path_index = 0;
            }
        }
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn active_during_spawn_phase(&self) -> bool {
        false
    }
}

/// Upstream `MoveWarshipExecution`: send an own warship to patrol somewhere else on its water.
pub fn order_warship(game: &mut Game, p: PlayerId, unit: UnitId, tile: TileRef) -> bool {
    let ok = game.unit(unit).is_some_and(|u| {
        u.is_active()
            && u.owner() == p
            && u.kind() == UnitKind::Warship
            && game.map().is_water(tile)
            && game.water().share_body(game.map(), u.tile(), tile)
    });
    if ok {
        if let Some(u) = game.unit_mut(unit) {
            u.target_tile = Some(tile);
        }
    }
    ok
}

/// Applies a warship move order on the next tick.
pub struct MoveWarshipExecution {
    player: PlayerId,
    unit: UnitId,
    tile: TileRef,
    done: bool,
}

impl MoveWarshipExecution {
    pub fn new(player: PlayerId, unit: UnitId, tile: TileRef) -> Self {
        Self {
            player,
            unit,
            tile,
            done: false,
        }
    }
}

impl Execution for MoveWarshipExecution {
    fn init(&mut self, _game: &mut Game, _ticks: u64) {}

    fn tick(&mut self, game: &mut Game, _ticks: u64) {
        order_warship(game, self.player, self.unit, self.tile);
        self.done = true;
    }

    fn is_active(&self) -> bool {
        !self.done
    }

    fn active_during_spawn_phase(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn death_factor_matches_upstream() {
        assert_eq!(
            nuke_death_factor(UnitKind::AtomBomb, 10_000.0, 100, 1e6),
            500.0
        );
        // MIRV: nothing below 3% of the cap, saturating toward 500.
        assert_eq!(
            nuke_death_factor(UnitKind::MirvWarhead, 20_000.0, 100, 1e6),
            0.0
        );
        let big = nuke_death_factor(UnitKind::MirvWarhead, 900_000.0, 100, 1e6);
        assert!(big > 400.0 && big < 500.0, "{big}");
    }
}
