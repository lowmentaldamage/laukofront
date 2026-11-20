//! Nation AI: stronger opponents than bots that ally, betray, build, sail and nuke.
//! Simplified port (D40) of upstream `execution/NationExecution.ts` and the
//! `execution/nation/Nation*Behavior.ts` files: the decision order and difficulty numbers are
//! upstream's; the long tails (beachhead follow-ups, nuke planning against SAM coverage, warship
//! hunting, reaction chatter) are reduced to their core.

use crate::config::Difficulty;
use crate::engine_lib::game_map::TileRef;
use crate::engine_lib::random::PseudoRandom;

use super::bots::AttackBehavior;
use super::diplomacy::{
    nuke_tiles_by_player, AllianceExtensionExecution, AllianceRequestExecution,
    BreakAllianceExecution, DonateExecution, ReactionExecution, R_ANGRY, R_CONFUSED, R_FRIENDS,
    R_SCARED, R_TRAITOR,
};
use super::game::{Execution, Game};
use super::player::{PlayerId, PlayerType, Relation};
use super::rules;
use super::structures::{build_cost, placement, ConstructionExecution};
use super::units::UnitKind;
use super::util::simple_hash;
use super::weapons::{
    blast_radius, can_launch, nuke_cost, warship_spawn, NukeExecution, WarshipExecution,
};

/// Nation names: our own syllable generator (D20: no upstream names or flags).
pub fn nation_name(seed: i64, i: u32) -> String {
    const A: [&str; 16] = [
        "Val", "Kor", "Ost", "Mar", "Bel", "Tor", "Ash", "Ery", "Lun", "Sar", "Dra", "Hel", "Nor",
        "Cal", "Ven", "Rho",
    ];
    const B: [&str; 12] = [
        "and", "ess", "ov", "ar", "in", "eth", "ul", "or", "ad", "is", "em", "ur",
    ];
    const C: [&str; 10] = [
        "ia", "a", "ora", "en", "ica", "ar", "heim", "land", "is", "ova",
    ];
    let mut r = PseudoRandom::new(seed.wrapping_mul(7919).wrapping_add(i as i64 * 104_729));
    let pick = |r: &mut PseudoRandom, n: usize| r.next_int(0, n as i64) as usize;
    format!(
        "{}{}{}",
        A[pick(&mut r, A.len())],
        B[pick(&mut r, B.len())],
        C[pick(&mut r, C.len())]
    )
}

/// Upstream `NationExecution`.
pub struct NationExecution {
    me: PlayerId,
    random: PseudoRandom,
    attack_rate: u64,
    attack_tick: u64,
    behavior: Option<AttackBehavior>,
    trigger_ratio: f64,
    reserve_ratio: f64,
    expand_ratio: f64,
    /// Players whose manual embargo we already took to heart (upstream `embargoMalusApplied`).
    embargo_malus: Vec<PlayerId>,
    active: bool,
}

impl NationExecution {
    /// `id` is the nation's string id, `game_id` the match id (seed the random stream as upstream).
    pub fn new(me: PlayerId, id: &str, game_id: &str, difficulty: Difficulty) -> Self {
        let mut random = PseudoRandom::new(simple_hash(id).wrapping_add(simple_hash(game_id)));
        let trigger_ratio = random.next_int(50, 60) as f64 / 100.0;
        let reserve_ratio = random.next_int(30, 40) as f64 / 100.0;
        let expand_ratio = random.next_int(10, 20) as f64 / 100.0;
        let attack_rate = match difficulty {
            Difficulty::Easy => random.next_int(65, 100),
            Difficulty::Medium => random.next_int(55, 70),
            Difficulty::Hard => random.next_int(45, 60),
            Difficulty::Impossible => random.next_int(30, 50),
        } as u64;
        let attack_tick = random.next_int(0, attack_rate as i64) as u64;
        Self {
            me,
            random,
            attack_rate,
            attack_tick,
            behavior: None,
            trigger_ratio,
            reserve_ratio,
            expand_ratio,
            embargo_malus: Vec::new(),
            active: true,
        }
    }

    fn difficulty(game: &Game) -> Difficulty {
        game.config().difficulty
    }

    fn react(&self, game: &mut Game, to: PlayerId, what: u8) {
        game.add_execution(Box::new(ReactionExecution::new(self.me, Some(to), what)));
    }

    // ----- alliances (upstream NationAllianceBehavior) -----

    fn is_confused(&mut self, d: Difficulty) -> bool {
        match d {
            Difficulty::Easy => self.random.chance(10),
            Difficulty::Medium => self.random.chance(20),
            Difficulty::Hard => self.random.chance(40),
            Difficulty::Impossible => false,
        }
    }

    /// Upstream `findRunawayLeader` (simplified): a player with over 40% of the land.
    fn runaway_leader(game: &Game) -> Option<PlayerId> {
        let land = game.map().num_land_tiles().max(1) as f64;
        game.alive_players()
            .find(|p| p.num_tiles() as f64 / land > 0.4)
            .map(|p| p.id())
    }

    fn is_threat(game: &Game, d: Difficulty, me: PlayerId, other: PlayerId) -> bool {
        let (Some(a), Some(b)) = (game.player(me), game.player(other)) else {
            return false;
        };
        let cap = |p| rules::troop_cap(game.config(), p);
        let (mt, ot) = (a.troops() as f64, b.troops() as f64);
        match d {
            Difficulty::Easy => false,
            Difficulty::Medium => ot > mt * 2.5,
            Difficulty::Hard => ot > mt && cap(b) > cap(a) * 2.0,
            Difficulty::Impossible => {
                ot > mt * 1.5
                    || (ot > mt && cap(b) > cap(a) * 1.5)
                    || (ot > mt && b.num_tiles() as f64 > a.num_tiles() as f64 * 1.5)
            }
        }
    }

    /// Upstream `getAllianceDecision`.
    fn alliance_decision(&mut self, game: &mut Game, other: PlayerId, is_response: bool) -> bool {
        let d = Self::difficulty(game);
        let me = self.me;
        if self.is_confused(d) {
            return self.random.chance(2);
        }
        if game.is_traitor(other) && self.random.next_int(0, 100) >= 10 {
            if is_response && self.random.chance(3) {
                self.react(game, other, R_CONFUSED);
            }
            return false;
        }
        if matches!(d, Difficulty::Hard | Difficulty::Impossible) {
            let total = game
                .players()
                .iter()
                .filter(|p| p.kind() != PlayerType::Bot)
                .count() as f64;
            let theirs =
                game.allies(other).len() as f64 - if game.is_allied(other, me) { 1.0 } else { 0.0 };
            let limit = if d == Difficulty::Hard { 0.5 } else { 0.25 };
            if theirs >= total * limit {
                return false;
            }
        }
        if d != Difficulty::Easy && Self::runaway_leader(game) == Some(other) {
            return false;
        }
        if Self::is_threat(game, d, me, other) {
            if !is_response && self.random.chance(6) {
                self.react(game, other, R_SCARED);
            }
            return true;
        }
        let rel = game
            .player(me)
            .map_or(Relation::Neutral, |p| p.relation(other));
        if rel < Relation::Neutral {
            if is_response && self.random.chance(3) {
                self.react(game, other, R_CONFUSED);
            }
            return false;
        }
        if rel == Relation::Friendly {
            let ok = match d {
                Difficulty::Easy | Difficulty::Medium => true,
                Difficulty::Hard => self.random.next_int(0, 100) >= 17,
                Difficulty::Impossible => self.random.next_int(0, 100) >= 33,
            };
            if ok {
                return true;
            }
        }
        let mine = game.allies(me).len() as i64;
        let enough = match d {
            Difficulty::Easy => false,
            Difficulty::Medium => mine >= self.random.next_int(4, 6),
            Difficulty::Hard => mine >= self.random.next_int(3, 5),
            Difficulty::Impossible => mine >= self.random.next_int(2, 4),
        };
        if enough {
            return false;
        }
        let spawn = game.config().game.spawn_phase_ticks;
        let t = game.ticks();
        let early = match d {
            Difficulty::Easy => t < 3000 + spawn && self.random.next_int(0, 100) >= 10,
            Difficulty::Medium => t < 1800 + spawn && self.random.next_int(0, 100) >= 30,
            Difficulty::Hard => t < 1800 + spawn && self.random.next_int(0, 100) >= 50,
            Difficulty::Impossible => t < 600 + spawn && self.random.next_int(0, 100) >= 70,
        };
        if early {
            return true;
        }
        // Upstream `isAlliancePartnerSimilarlyStrong`: within a factor of the troops.
        let (Some(a), Some(b)) = (game.player(me), game.player(other)) else {
            return false;
        };
        let (mt, ot) = (a.troops() as f64, b.troops() as f64);
        ot >= mt * 0.7 && ot <= mt * 1.3
    }

    fn handle_alliance_requests(&mut self, game: &mut Game) {
        let me = self.me;
        let spawn = game.config().game.spawn_phase_ticks;
        let incoming: Vec<(PlayerId, u64)> = game
            .incoming_requests(me)
            .map(|r| (r.requestor, r.created))
            .collect();
        for (from, created) in incoming {
            if created <= spawn + 1 {
                game.reject_alliance_request(from, me);
                continue;
            }
            if self.alliance_decision(game, from, true) {
                game.add_execution(Box::new(AllianceRequestExecution::new(me, from)));
            } else {
                game.reject_alliance_request(from, me);
            }
        }
        // Renewals: agree when the other side asked and we still like them.
        let asks: Vec<PlayerId> = game
            .alliances()
            .iter()
            .filter(|a| a.involves(me) && a.only_one_agreed() && !a.agreed_to_extend(me))
            .map(|a| a.other(me))
            .collect();
        for other in asks {
            if self.alliance_decision(game, other, true) {
                game.add_execution(Box::new(AllianceExtensionExecution::new(me, other)));
            }
        }
    }

    fn maybe_send_alliance_requests(&mut self, game: &mut Game, enemies: &[PlayerId]) {
        let me = self.me;
        let d = Self::difficulty(game);
        for &e in enemies {
            let kind = game.player(e).map(|p| p.kind());
            let ok_kind = kind != Some(PlayerType::Bot) || d == Difficulty::Easy;
            if self.random.chance(30)
                && ok_kind
                && game.can_send_alliance_request(me, e)
                && self.alliance_decision(game, e, false)
            {
                game.add_execution(Box::new(AllianceRequestExecution::new(me, e)));
            }
        }
    }

    /// Upstream `maybeBetray` (simplified): a much weaker ally that we no longer like.
    fn maybe_betray(&mut self, game: &mut Game, friends: &[PlayerId]) -> Option<PlayerId> {
        let d = Self::difficulty(game);
        let me = self.me;
        let mine = game.player(me)?.troops() as f64;
        for &f in friends {
            let Some(p) = game.player(f) else { continue };
            let weak = (p.troops() as f64) < mine * 0.2;
            let rel = game.player(me).map_or(Relation::Neutral, |x| x.relation(f));
            let odds = match d {
                Difficulty::Easy => 60,
                Difficulty::Medium => 30,
                Difficulty::Hard => 15,
                Difficulty::Impossible => 8,
            };
            if weak && rel < Relation::Friendly && self.random.chance(odds) {
                game.add_execution(Box::new(BreakAllianceExecution::new(me, f)));
                return Some(f);
            }
        }
        None
    }

    // ----- attacks (upstream NationAttackBehavior) -----

    fn choose_attack(&mut self, game: &mut Game, b: &mut AttackBehavior) {
        let me = self.me;
        let d = Self::difficulty(game);
        let near = game.nearby(me);
        let free_land = near.contains(&None);
        let mut players: Vec<PlayerId> = near.into_iter().flatten().collect();
        players.sort_by_key(|&p| game.player(p).map_or(0, |x| x.troops()));
        let friends: Vec<PlayerId> = players
            .iter()
            .copied()
            .filter(|&p| game.is_friendly(me, p))
            .collect();
        let enemies: Vec<PlayerId> = players
            .iter()
            .copied()
            .filter(|&p| !game.is_friendly(me, p))
            .collect();
        if free_land && b.send_attack(game, me, None) {
            return;
        }
        if enemies.is_empty() {
            // Upstream `attackWithRandomBoat` / `findNearestIslandEnemy`: nobody next to us, so
            // sail to the weakest player we are not friends with.
            if self.random.chance(3) && b.has_trigger_ratio_troops(game, me) {
                let mut targets: Vec<(i64, PlayerId)> = game
                    .alive_players()
                    .filter(|p| p.id() != me && !game.is_friendly(me, p.id()))
                    .map(|p| (p.troops(), p.id()))
                    .collect();
                targets.sort();
                for (_, t) in targets.into_iter().take(3) {
                    if b.send_boat_attack(game, me, t) {
                        return;
                    }
                }
                if self.boat_to_random_free_land(game, b) {
                    return;
                }
            }
        } else {
            self.maybe_send_alliance_requests(game, &enemies);
        }
        self.attack_best_target(game, b, &friends, &enemies, d);
    }

    /// Upstream `findRandomBoatTarget` (reduced): try a few random unowned land tiles anywhere
    /// (fallout included, it is still free land) and sail to the first reachable one.
    fn boat_to_random_free_land(&mut self, game: &mut Game, b: &mut AttackBehavior) -> bool {
        let me = self.me;
        let n = game.map().num_tiles() as i64;
        for _ in 0..40 {
            let t = self.random.next_int(0, n) as TileRef;
            let map = game.map();
            if !map.is_passable_land(t) || map.has_owner(t) {
                continue;
            }
            if super::boats::can_send_boat(game, me, t).is_none() {
                continue;
            }
            let troops = b.home_troops(game, me) / 5.0;
            if troops < 1.0 {
                return false;
            }
            b.record_send(game, troops);
            game.add_execution(Box::new(super::boats::TransportShipExecution::new(
                me,
                t,
                Some(troops),
            )));
            return true;
        }
        false
    }

    fn retaliate(&self, game: &mut Game, b: &mut AttackBehavior) -> bool {
        match b.find_incoming_attack_player(game, self.me) {
            Some(a) if !game.is_friendly(self.me, a) => b.send_attack(game, self.me, Some(a)),
            _ => false,
        }
    }

    /// Upstream `isAttackTooWeak`: Hard and Impossible keep troops home rather than send less
    /// than a fifth of the target's troops (unless under attack).
    fn too_weak(&self, game: &Game, b: &AttackBehavior, target: PlayerId, d: Difficulty) -> bool {
        if !matches!(d, Difficulty::Hard | Difficulty::Impossible) {
            return false;
        }
        if !game.incoming_attacks(self.me).is_empty() {
            return false;
        }
        let send = b.home_troops(game, self.me) - b.reserve_troops(game, self.me, Some(target));
        send < game.player(target).map_or(0, |p| p.troops()) as f64 * 0.2
    }

    fn attack(&self, game: &mut Game, b: &mut AttackBehavior, t: PlayerId, d: Difficulty) -> bool {
        if game.is_friendly(self.me, t) || self.too_weak(game, b, t, d) {
            return false;
        }
        b.send_attack(game, self.me, Some(t))
    }

    fn attack_best_target(
        &mut self,
        game: &mut Game,
        b: &mut AttackBehavior,
        friends: &[PlayerId],
        enemies: &[PlayerId],
        d: Difficulty,
    ) {
        let me = self.me;
        if matches!(d, Difficulty::Hard | Difficulty::Impossible) && self.retaliate(game, b) {
            return;
        }
        let reserve_ok = b.home_troops(game, me) / b.reserve_basis(game, me) >= self.reserve_ratio;
        if !reserve_ok {
            return;
        }
        if d == Difficulty::Medium && self.retaliate(game, b) {
            return;
        }
        if !b.has_trigger_ratio_troops(game, me) && !self.random.chance(10) {
            return;
        }
        let my_troops = game.player(me).map_or(0, |p| p.troops()) as f64;
        let troops = |g: &Game, p: PlayerId| g.player(p).map_or(0, |x| x.troops()) as f64;
        let kind = |g: &Game, p: PlayerId| g.player(p).map(|x| x.kind());
        // Strategy order per difficulty, as upstream's `getAttackStrategies`.
        #[derive(Clone, Copy)]
        enum S {
            Bots,
            Retaliate,
            Assist,
            Betray,
            Hated,
            Traitor,
            Weakest,
            VeryWeak,
            Victim,
        }
        use S::*;
        let order: &[S] = match d {
            Difficulty::Easy => &[Bots, Retaliate, Assist, Betray, Hated, Weakest],
            Difficulty::Medium => &[Bots, Assist, Betray, Hated, Traitor, Weakest],
            Difficulty::Hard => &[
                Bots, Assist, Betray, Traitor, Hated, VeryWeak, Victim, Weakest,
            ],
            Difficulty::Impossible => &[
                Bots, VeryWeak, Betray, Assist, Victim, Traitor, Hated, Weakest,
            ],
        };
        for &s in order {
            let done = match s {
                Bots => {
                    let bots: Vec<PlayerId> = enemies
                        .iter()
                        .copied()
                        .filter(|&e| kind(game, e) == Some(PlayerType::Bot))
                        .collect();
                    bots.into_iter().any(|e| b.send_attack(game, me, Some(e)))
                }
                Retaliate => self.retaliate(game, b),
                Assist => {
                    let wanted: Vec<PlayerId> = friends
                        .iter()
                        .flat_map(|&f| game.targets(f))
                        .filter(|t| enemies.contains(t))
                        .collect();
                    wanted.into_iter().any(|t| self.attack(game, b, t, d))
                }
                Betray => match self.maybe_betray(game, friends) {
                    // The alliance breaks next tick; attack then.
                    Some(f) => {
                        self.react(game, f, R_ANGRY);
                        true
                    }
                    None => false,
                },
                Hated => {
                    let hated: Vec<PlayerId> = enemies
                        .iter()
                        .copied()
                        .filter(|&e| {
                            game.player(me)
                                .is_some_and(|p| p.relation(e) == Relation::Hostile)
                                && troops(game, e) <= my_troops * 3.0
                        })
                        .collect();
                    hated.into_iter().any(|e| self.attack(game, b, e, d))
                }
                Traitor => {
                    let t: Vec<PlayerId> = enemies
                        .iter()
                        .copied()
                        .filter(|&e| game.is_traitor(e) && troops(game, e) < my_troops * 1.2)
                        .collect();
                    t.into_iter().any(|e| self.attack(game, b, e, d))
                }
                VeryWeak => {
                    let w: Vec<PlayerId> = enemies
                        .iter()
                        .copied()
                        .filter(|&e| {
                            game.player(e).is_some_and(|p| {
                                (p.troops() as f64) < rules::troop_cap(game.config(), p) * 0.15
                            }) && troops(game, e) < my_troops * 1.2
                        })
                        .collect();
                    w.into_iter().any(|e| self.attack(game, b, e, d))
                }
                Victim => {
                    let v: Vec<PlayerId> = enemies
                        .iter()
                        .copied()
                        .filter(|&e| {
                            let inc: f64 = game
                                .incoming_attacks(e)
                                .iter()
                                .filter_map(|&a| game.attack(a))
                                .map(|a| a.troops())
                                .sum();
                            troops(game, e) <= my_troops * 1.2 && inc > troops(game, e) * 0.5
                        })
                        .collect();
                    v.into_iter().any(|e| self.attack(game, b, e, d))
                }
                Weakest => {
                    let w = enemies
                        .iter()
                        .copied()
                        .find(|&e| troops(game, e) < my_troops);
                    w.is_some_and(|e| self.attack(game, b, e, d))
                }
            };
            if done {
                return;
            }
        }
    }

    // ----- structures (upstream NationStructureBehavior, reduced) -----

    fn handle_structures(&mut self, game: &mut Game) {
        let me = self.me;
        let d = Self::difficulty(game);
        let Some(p) = game.player(me) else { return };
        let gold = p.gold();
        let count = |k: UnitKind| game.unit_count(me, k) as f64;
        let cities = count(UnitKind::City);
        let coastal = p
            .border_tiles()
            .iter()
            .any(|&t| game.map().is_ocean_shore(t));
        let sam_ratio = match d {
            Difficulty::Easy => 0.15,
            Difficulty::Medium => 0.2,
            Difficulty::Hard => 0.25,
            Difficulty::Impossible => 0.3,
        };
        // Upstream ratios per city; the first one below its share is built, else a city.
        let mut wanted: Vec<UnitKind> = Vec::new();
        if coastal && count(UnitKind::Port) < (cities * 0.75).ceil().max(1.0) {
            wanted.push(UnitKind::Port);
        }
        if count(UnitKind::Factory) < (cities * 0.75).floor() {
            wanted.push(UnitKind::Factory);
        }
        if d != Difficulty::Easy && cities >= 2.0 {
            if count(UnitKind::MissileSilo) < (cities * 0.2).ceil() {
                wanted.push(UnitKind::MissileSilo);
            }
            if count(UnitKind::SamLauncher) < (cities * sam_ratio).floor() {
                wanted.push(UnitKind::SamLauncher);
            }
        }
        // Defense posts when under a heavy attack (Medium and up).
        let incoming: f64 = game
            .incoming_attacks(me)
            .iter()
            .filter_map(|&a| game.attack(a))
            .map(|a| a.troops())
            .sum();
        if d != Difficulty::Easy && incoming > p.troops() as f64 * 0.3 {
            wanted.insert(0, UnitKind::DefensePost);
        }
        wanted.push(UnitKind::City);
        for kind in wanted {
            if gold < build_cost(game, me, kind) {
                // Save up for the most wanted one rather than spending on cheaper ones.
                return;
            }
            if let Some(t) = self.pick_site(game, kind) {
                game.add_execution(Box::new(ConstructionExecution::new(me, kind, t)));
                return;
            }
        }
    }

    /// A random valid spot: ports on the coast, defense posts on the attacked border, the rest
    /// inland where possible.
    fn pick_site(&mut self, game: &Game, kind: UnitKind) -> Option<TileRef> {
        let me = self.me;
        let p = game.player(me)?;
        let pool: Vec<TileRef> = match kind {
            UnitKind::Port => p
                .border_tiles()
                .iter()
                .copied()
                .filter(|&t| game.map().is_ocean_shore(t))
                .collect(),
            UnitKind::DefensePost => {
                let attackers: Vec<PlayerId> = game
                    .incoming_attacks(me)
                    .iter()
                    .filter_map(|&a| game.attack(a))
                    .map(|a| a.attacker())
                    .collect();
                let map = game.map();
                let mut buf = [0; 4];
                p.border_tiles()
                    .iter()
                    .copied()
                    .filter(|&t| {
                        let n = map.neighbors4(t, &mut buf);
                        buf[..n]
                            .iter()
                            .any(|&x| attackers.iter().any(|&a| map.owner_id(x) == a))
                    })
                    .collect()
            }
            _ => p.tiles().to_vec(),
        };
        if pool.is_empty() {
            return None;
        }
        for _ in 0..12 {
            let t = pool[self.random.index(pool.len())?];
            if kind != UnitKind::Port
                && kind != UnitKind::DefensePost
                && p.border_tiles().len() < p.tiles().len()
                && p.border_tiles().contains(&t)
            {
                continue;
            }
            if let Ok(at) = placement(game, me, kind, t) {
                return Some(at);
            }
        }
        None
    }

    // ----- warships (upstream NationWarshipBehavior, reduced) -----

    fn maybe_spawn_warship(&mut self, game: &mut Game) {
        let me = self.me;
        let d = Self::difficulty(game);
        if d == Difficulty::Easy || !self.random.chance(3) {
            return;
        }
        let ports = game.unit_count(me, UnitKind::Port);
        if ports == 0 || game.unit_count(me, UnitKind::Warship) >= ports {
            return;
        }
        let cost = build_cost(game, me, UnitKind::Warship);
        if game.player(me).map_or(0, |p| p.gold()) < cost + 500_000 {
            return;
        }
        let port = game
            .player_units(me, Some(UnitKind::Port))
            .map(|u| u.tile())
            .next();
        let Some(port) = port else { return };
        let map = game.map();
        let (px, py) = (map.x(port) as i64, map.y(port) as i64);
        for _ in 0..10 {
            let dx = self.random.next_int(-20, 21);
            let dy = self.random.next_int(-20, 21);
            let Some(t) = map.tile(px + dx, py + dy) else {
                continue;
            };
            if map.is_ocean(t) && warship_spawn(game, me, t).is_ok() {
                game.add_execution(Box::new(WarshipExecution::new(me, t)));
                return;
            }
        }
    }

    // ----- nukes (upstream NationNukeBehavior / NationMIRVBehavior, reduced) -----

    fn maybe_send_nuke(&mut self, game: &mut Game, enemies_hint: &[PlayerId]) {
        let me = self.me;
        let d = Self::difficulty(game);
        if d == Difficulty::Easy || game.unit_count(me, UnitKind::MissileSilo) == 0 {
            return;
        }
        let gold = game.player(me).map_or(0, |p| p.gold());
        // MIRV the runaway leader (Hard and up).
        if matches!(d, Difficulty::Hard | Difficulty::Impossible) {
            if let Some(leader) = Self::runaway_leader(game) {
                if leader != me
                    && !game.is_friendly(me, leader)
                    && gold >= nuke_cost(game, UnitKind::Mirv)
                {
                    if let Some(t) = self.target_tile_of(game, leader) {
                        if can_launch(game, me, UnitKind::Mirv, t).is_ok() {
                            game.add_execution(Box::new(NukeExecution::new(me, UnitKind::Mirv, t)));
                            return;
                        }
                    }
                }
            }
        }
        // Otherwise the most hated (or strongest) enemy, with the biggest bomb we can afford.
        let kind = if d != Difficulty::Medium && gold >= nuke_cost(game, UnitKind::HydrogenBomb) {
            UnitKind::HydrogenBomb
        } else if gold >= nuke_cost(game, UnitKind::AtomBomb) {
            UnitKind::AtomBomb
        } else {
            return;
        };
        let mut candidates: Vec<PlayerId> = enemies_hint
            .iter()
            .copied()
            .filter(|&e| {
                !game.is_friendly(me, e)
                    && game.player(e).is_some_and(|p| p.kind() != PlayerType::Bot)
            })
            .collect();
        candidates.sort_by(|&a, &b| {
            let ra = game.player(me).map_or(0.0, |p| p.relation_value(a));
            let rb = game.player(me).map_or(0.0, |p| p.relation_value(b));
            ra.total_cmp(&rb)
        });
        let Some(&target) = candidates.first() else {
            return;
        };
        // Only nuke players we really dislike (Medium) or anyone hostile-ish (Hard and up).
        let rel = game
            .player(me)
            .map_or(Relation::Neutral, |p| p.relation(target));
        let ok = match d {
            Difficulty::Medium => rel == Relation::Hostile,
            _ => rel <= Relation::Distrustful,
        };
        if !ok {
            return;
        }
        let Some(t) = self.target_tile_of(game, target) else {
            return;
        };
        // Never hit our own land or an ally's.
        let (inner, outer) = blast_radius(game, kind);
        let hit = nuke_tiles_by_player(game, t, inner, outer);
        if hit.keys().any(|&p| game.is_friendly(me, p)) {
            return;
        }
        if can_launch(game, me, kind, t).is_ok() {
            game.add_execution(Box::new(NukeExecution::new(me, kind, t)));
            self.react(game, target, R_ANGRY);
        }
    }

    /// A good tile to hit: one of the player's cities if any, else a random tile of theirs.
    fn target_tile_of(&mut self, game: &Game, p: PlayerId) -> Option<TileRef> {
        let cities: Vec<TileRef> = game
            .player_units(p, Some(UnitKind::City))
            .map(|u| u.tile())
            .collect();
        if let Some(i) = self.random.index(cities.len()) {
            return Some(cities[i]);
        }
        let tiles = game.player(p)?.tiles();
        let i = self.random.index(tiles.len())?;
        Some(tiles[i])
    }

    // ----- embargoes and relations -----

    fn update_relations_from_embargoes(&mut self, game: &mut Game) {
        let me = self.me;
        let others: Vec<PlayerId> = game
            .players()
            .iter()
            .map(|p| p.id())
            .filter(|&p| p != me)
            .collect();
        for o in others {
            let has = game
                .player(o)
                .is_some_and(|p| p.has_manual_embargo_against(me));
            let applied = self.embargo_malus.contains(&o);
            if has && !applied {
                game.update_relation(me, o, -20.0);
                self.embargo_malus.push(o);
            } else if !has && applied {
                game.update_relation(me, o, 20.0);
                self.embargo_malus.retain(|&x| x != o);
            }
        }
    }

    fn handle_embargoes(&mut self, game: &mut Game) {
        let me = self.me;
        let others: Vec<PlayerId> = game
            .players()
            .iter()
            .map(|p| p.id())
            .filter(|&p| p != me)
            .collect();
        for o in others {
            let rel = game.player(me).map_or(Relation::Neutral, |p| p.relation(o));
            let has = game.player(me).is_some_and(|p| p.has_embargo_against(o));
            if rel <= Relation::Hostile && !has {
                game.add_embargo(me, o, true);
            } else if rel >= Relation::Neutral && has {
                game.end_temporary_embargo(me, o);
            }
        }
    }

    /// Upstream `donateTroops` (reduced): help an ally that is under heavy attack.
    fn maybe_donate(&mut self, game: &mut Game) {
        let me = self.me;
        if !self.random.chance(4) {
            return;
        }
        let Some(mine) = game.player(me).map(|p| p.troops() as f64) else {
            return;
        };
        for ally in game.allies(me) {
            let Some(a) = game.player(ally) else { continue };
            let inc: f64 = game
                .incoming_attacks(ally)
                .iter()
                .filter_map(|&x| game.attack(x))
                .map(|x| x.troops())
                .sum();
            if inc > a.troops() as f64 && mine > a.troops() as f64 && game.can_donate(me, ally) {
                game.add_execution(Box::new(DonateExecution::troops(
                    me,
                    ally,
                    Some(mine / 5.0),
                )));
                return;
            }
        }
    }
}

impl Execution for NationExecution {
    fn init(&mut self, _game: &mut Game, _ticks: u64) {}

    fn tick(&mut self, game: &mut Game, ticks: u64) {
        let me = self.me;
        if !game.player(me).is_some_and(|p| p.is_alive()) {
            if self.behavior.is_some() {
                self.active = false;
            }
            return;
        }
        let Some(mut b) = self.behavior.take() else {
            let mut b = AttackBehavior {
                trigger_ratio: self.trigger_ratio,
                reserve_ratio: self.reserve_ratio,
                expand_ratio: self.expand_ratio,
                sent_this_tick: None,
            };
            b.send_attack(game, me, None);
            self.behavior = Some(b);
            return;
        };
        let offset = ticks % self.attack_rate;
        if offset != self.attack_tick {
            let third = (self.attack_tick + self.attack_rate / 3) % self.attack_rate;
            let two = (self.attack_tick + self.attack_rate * 2 / 3) % self.attack_rate;
            if offset == third || offset == two {
                self.handle_structures(game);
            }
            self.behavior = Some(b);
            return;
        }
        self.update_relations_from_embargoes(game);
        self.handle_alliance_requests(game);
        self.handle_structures(game);
        self.maybe_spawn_warship(game);
        self.handle_embargoes(game);
        self.choose_attack(game, &mut b);
        self.maybe_donate(game);
        let enemies: Vec<PlayerId> = game
            .alive_players()
            .map(|p| p.id())
            .filter(|&p| p != me)
            .collect();
        self.maybe_send_nuke(game, &enemies);
        // Call out traitors next to us now and then.
        if self.random.chance(40) {
            if let Some(t) = game
                .nearby(me)
                .into_iter()
                .flatten()
                .find(|&p| game.is_traitor(p))
            {
                self.react(game, t, R_TRAITOR);
            } else if let Some(&f) = game.allies(me).first() {
                self.react(game, f, R_FRIENDS);
            }
        }
        self.behavior = Some(b);
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn active_during_spawn_phase(&self) -> bool {
        false
    }
}
