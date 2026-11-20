//! Bot AI. Port of upstream `execution/TribeExecution.ts`, `TribeAttackBehavior.ts` and the
//! land-attack parts of `utils/AiAttackBehavior.ts`.
//!
//! Not ported yet (later milestones): boat attacks and river-crossing neighbours (transport
//! ships), alliances and traitors, structure deletion, the tutorial variant.

use crate::engine_lib::random::PseudoRandom;

use super::attack_exec::AttackExecution;
use super::boats::{can_send_boat, TransportShipExecution};
use super::game::{Execution, Game};
use super::player::{PlayerId, PlayerType};
use super::rules;
use super::util::simple_hash;

/// A bot's attack decisions (upstream `TribeAttackBehavior` on `AiAttackBehavior`).
struct AttackBehavior {
    trigger_ratio: f64,
    reserve_ratio: f64,
    expand_ratio: f64,
    /// (tick, troops) already sent this tick, so several sends in one tick share the pool.
    sent_this_tick: Option<(u64, f64)>,
}

impl AttackBehavior {
    fn home_troops(&self, game: &Game, me: PlayerId) -> f64 {
        let troops = game.player(me).map_or(0.0, |p| p.troops() as f64);
        match self.sent_this_tick {
            Some((t, sent)) if t == game.ticks() => (troops - sent).max(0.0),
            _ => troops,
        }
    }

    fn reserve_basis(&self, game: &Game, me: PlayerId) -> f64 {
        game.player(me).map_or(0.0, |p| {
            rules::max_troops(game.config(), p.kind(), p.num_tiles())
        })
    }

    fn has_trigger_ratio_troops(&self, game: &Game, me: PlayerId) -> bool {
        self.home_troops(game, me) / self.reserve_basis(game, me) >= self.trigger_ratio
    }

    /// Upstream `reserveTroops`: keep `reserve_ratio` of the cap when attacking a player,
    /// `expand_ratio` when expanding into free land.
    fn reserve_troops(&self, game: &Game, me: PlayerId, target: Option<PlayerId>) -> f64 {
        let ratio = if target.is_some() {
            self.reserve_ratio
        } else {
            self.expand_ratio
        };
        self.reserve_basis(game, me) * ratio
    }

    fn send_land_attack(
        &mut self,
        game: &mut Game,
        me: PlayerId,
        target: Option<PlayerId>,
    ) -> bool {
        let troops = self.home_troops(game, me) - self.reserve_troops(game, me, target);
        if troops < 1.0 {
            return false;
        }
        let tick = game.ticks();
        let home = self.home_troops(game, me);
        let already = match self.sent_this_tick {
            Some((t, s)) if t == tick => s,
            _ => 0.0,
        };
        self.sent_this_tick = Some((tick, already + troops.min(home)));
        game.add_execution(Box::new(AttackExecution::new(me, target, Some(troops))));
        true
    }

    /// Upstream `sendAttack`: land attack if we share a border, otherwise a boat.
    fn send_attack(&mut self, game: &mut Game, me: PlayerId, target: Option<PlayerId>) -> bool {
        if game.shares_border_with(me, target) {
            return self.send_land_attack(game, me, target);
        }
        match target {
            Some(t) => self.send_boat_attack(game, me, t),
            None => self.send_boat_to_free_land(game, me),
        }
    }

    fn record_send(&mut self, game: &Game, troops: f64) {
        let tick = game.ticks();
        let already = match self.sent_this_tick {
            Some((t, s)) if t == tick => s,
            _ => 0.0,
        };
        self.sent_this_tick = Some((tick, already + troops));
    }

    /// Upstream `sendBoatAttack`: a fifth of home troops to the coast of `target` nearest ours.
    fn send_boat_attack(&mut self, game: &mut Game, me: PlayerId, target: PlayerId) -> bool {
        let map = game.map();
        let Some(t) = game.player(target) else {
            return false;
        };
        let Some(mine) = game.player(me) else {
            return false;
        };
        // Closest pair of coast tiles (sampled) between us and them.
        let ours: Vec<u32> = mine
            .border_tiles()
            .iter()
            .copied()
            .filter(|&x| map.is_shore(x))
            .collect();
        let theirs: Vec<u32> = t
            .border_tiles()
            .iter()
            .copied()
            .filter(|&x| map.is_shore(x))
            .collect();
        // At most ~150 samples per side, so big empires cost the same as small ones.
        let sample = |v: &Vec<u32>| -> Vec<u32> {
            let step = (v.len() / 150).max(1);
            v.iter().copied().step_by(step).collect()
        };
        let (ours, theirs) = (sample(&ours), sample(&theirs));
        let mut best: Option<(u64, u32)> = None;
        for &a in &ours {
            for &b in &theirs {
                let d = map.euclidean_dist_squared(a, b);
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, b));
                }
            }
        }
        let Some((_, landing)) = best else {
            return false;
        };
        if can_send_boat(game, me, landing).is_none() {
            return false;
        }
        let troops = self.home_troops(game, me) / 5.0;
        if troops < 1.0 {
            return false;
        }
        self.record_send(game, troops);
        game.add_execution(Box::new(TransportShipExecution::new(
            me,
            landing,
            Some(troops),
        )));
        true
    }

    /// Upstream `sendBoatAttackToNearbyTerraNullius`: from every 10th coast tile, look 5 tiles
    /// across the water for free land and send an expansion boat there.
    fn send_boat_to_free_land(&mut self, game: &mut Game, me: PlayerId) -> bool {
        let map = game.map();
        let Some(mine) = game.player(me) else {
            return false;
        };
        let shores: Vec<u32> = mine
            .border_tiles()
            .iter()
            .copied()
            .filter(|&x| map.is_shore(x))
            .collect();
        for &b in shores.iter().step_by(10) {
            let (bx, by) = (map.x(b) as i64, map.y(b) as i64);
            for (dx, dy) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
                let Some(first) = map.tile(bx + dx, by + dy) else {
                    continue;
                };
                if !map.is_water(first) {
                    continue;
                }
                let Some(t) = map.tile(bx + dx * 5, by + dy * 5) else {
                    continue;
                };
                if !map.is_passable_land(t) || map.has_owner(t) || map.has_fallout(t) {
                    continue;
                }
                if can_send_boat(game, me, t).is_none() {
                    continue;
                }
                let troops = self.home_troops(game, me) / 5.0;
                if troops < 1.0 {
                    return false;
                }
                self.record_send(game, troops);
                game.add_execution(Box::new(TransportShipExecution::new(me, t, Some(troops))));
                return true;
            }
        }
        false
    }

    /// Upstream `findIncomingAttackPlayer`: the attacker with the largest incoming attack.
    fn find_incoming_attack_player(&self, game: &Game, me: PlayerId) -> Option<PlayerId> {
        let mut best: Option<(PlayerId, f64)> = None;
        for id in game.incoming_attacks(me) {
            if let Some(a) = game.attack(id) {
                if a.troops() > best.map_or(0.0, |b| b.1) {
                    best = Some((a.attacker(), a.troops()));
                }
            }
        }
        best.map(|b| b.0)
    }

    /// Upstream `TribeAttackBehavior.attackRandomTarget`.
    fn attack_random_target(&mut self, game: &mut Game, me: PlayerId, random: &mut PseudoRandom) {
        if !self.has_trigger_ratio_troops(game, me) {
            return;
        }
        if let Some(attacker) = self.find_incoming_attack_player(game, me) {
            if self.send_attack(game, me, Some(attacker)) {
                return;
            }
        }
        let mut neighbors = game.nearby(me);
        random.shuffle(&mut neighbors);
        for n in neighbors {
            let Some(p) = n else { continue };
            let kind = game.player(p).map(|x| x.kind());
            if matches!(kind, Some(PlayerType::Nation) | Some(PlayerType::Human))
                && random.chance(2)
            {
                continue;
            }
            if self.send_attack(game, me, Some(p)) {
                return;
            }
        }
    }
}

/// Upstream `TribeExecution`: every `attack_rate` ticks (40-80, per bot), expand into free land or
/// attack a neighbour.
pub struct TribeExecution {
    bot: PlayerId,
    random: PseudoRandom,
    attack_rate: u64,
    attack_tick: u64,
    behavior: Option<AttackBehavior>,
    trigger_ratio: f64,
    reserve_ratio: f64,
    expand_ratio: f64,
    neighbors_terra_nullius: bool,
    active: bool,
}

impl TribeExecution {
    /// `id` is the bot's string id (seeds its random stream, as upstream).
    pub fn new(bot: PlayerId, id: &str) -> Self {
        let mut random = PseudoRandom::new(simple_hash(id));
        let attack_rate = random.next_int(40, 80) as u64;
        let attack_tick = random.next_int(0, attack_rate as i64) as u64;
        let trigger_ratio = random.next_int(50, 60) as f64 / 100.0;
        let reserve_ratio = random.next_int(30, 40) as f64 / 100.0;
        let expand_ratio = random.next_int(10, 20) as f64 / 100.0;
        Self {
            bot,
            random,
            attack_rate,
            attack_tick,
            behavior: None,
            trigger_ratio,
            reserve_ratio,
            expand_ratio,
            neighbors_terra_nullius: true,
            active: true,
        }
    }
}

impl Execution for TribeExecution {
    fn init(&mut self, _game: &mut Game, _ticks: u64) {}

    fn tick(&mut self, game: &mut Game, ticks: u64) {
        if ticks % self.attack_rate != self.attack_tick {
            return;
        }
        if !game.player(self.bot).is_some_and(|p| p.is_alive()) {
            self.active = false;
            return;
        }
        let me = self.bot;
        let Some(mut behavior) = self.behavior.take() else {
            let mut b = AttackBehavior {
                trigger_ratio: self.trigger_ratio,
                reserve_ratio: self.reserve_ratio,
                expand_ratio: self.expand_ratio,
                sent_this_tick: None,
            };
            // Upstream sends an attack into free land on the first turn.
            b.send_attack(game, me, None);
            self.behavior = Some(b);
            return;
        };
        // maybeAttack (no traitors without alliances).
        let mut done = false;
        if self.neighbors_terra_nullius {
            if game.nearby(me).contains(&None) {
                done = behavior.send_attack(game, me, None);
            } else {
                self.neighbors_terra_nullius = false;
            }
        }
        if !done {
            behavior.attack_random_target(game, me, &mut self.random);
        }
        self.behavior = Some(behavior);
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn active_during_spawn_phase(&self) -> bool {
        false
    }
}
