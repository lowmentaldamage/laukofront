//! Port of upstream `execution/AttackExecution.ts`, `RetreatExecution.ts` and
//! `WinCheckExecution.ts` (free-for-all, no ranked/tutorial/teams).
//!
//! Not ported yet (later milestones): alliances (friendly checks), embargoes and relations,
//! defense posts, fallout, boat attacks (`sourceTile` is kept but never set), stats beyond
//! "attacks sent".

use crate::engine_lib::game_map::{TerrainType, TileRef};
use crate::engine_lib::random::PseudoRandom;

use super::attack::{AttackId, TileHeap};
use super::game::{Execution, Game};
use super::player::PlayerId;
use super::rules::{self, AttackLogicInput, DefenderInfo};

pub struct AttackExecution {
    owner: PlayerId,
    /// `None` = unowned land.
    target: Option<PlayerId>,
    start_troops: Option<f64>,
    source_tile: Option<TileRef>,
    remove_troops: bool,
    to_conquer: TileHeap,
    random: PseudoRandom,
    attack: Option<AttackId>,
    active: bool,
    scratch: Vec<TileRef>,
}

impl AttackExecution {
    /// `troops = None` sends the default share (`rules::attack_amount`).
    pub fn new(owner: PlayerId, target: Option<PlayerId>, troops: Option<f64>) -> Self {
        Self {
            owner,
            target,
            start_troops: troops,
            source_tile: None,
            remove_troops: true,
            to_conquer: TileHeap::new(),
            // Upstream seeds every attack with 123.
            random: PseudoRandom::new(123),
            attack: None,
            active: true,
            scratch: Vec::new(),
        }
    }

    /// An attack started by troops landing from a boat at `source` (upstream
    /// `new AttackExecution(troops, attacker, target, dst, false)`): the troops were already taken
    /// when the boat left, and it is never merged with other attacks.
    pub fn from_boat(
        owner: PlayerId,
        target: Option<PlayerId>,
        troops: f64,
        source: TileRef,
    ) -> Self {
        let mut e = Self::new(owner, target, Some(troops));
        e.source_tile = Some(source);
        e.remove_troops = false;
        e
    }

    /// The attack this execution drives, once initialised.
    pub fn attack_id(&self) -> Option<AttackId> {
        self.attack
    }

    fn target_small_id(&self) -> u16 {
        self.target.unwrap_or(0)
    }

    fn refresh_to_conquer(&mut self, game: &mut Game) {
        self.to_conquer.clear();
        if let Some(a) = self.attack.and_then(|id| game.attack_mut(id)) {
            a.border.clear();
        }
        self.scratch.clear();
        if let Some(p) = game.player(self.owner) {
            self.scratch.extend_from_slice(p.border_tiles());
        }
        let tiles = std::mem::take(&mut self.scratch);
        for &t in &tiles {
            self.add_neighbors(game, t);
        }
        self.scratch = tiles;
    }

    /// Upstream `addNeighbors`: queue the target-owned land next to `tile`, prioritised by how
    /// many of its neighbours the attacker already owns, terrain, a little randomness and time.
    fn add_neighbors(&mut self, game: &mut Game, tile: TileRef) {
        let Some(attack_id) = self.attack else { return };
        let ticks = game.ticks() as f64;
        let target_id = self.target_small_id();
        let mut buf = [0; 4];
        let mut buf2 = [0; 4];
        let n = game.map().neighbors4(tile, &mut buf);
        for &nb in &buf[..n] {
            let map = game.map();
            if map.is_water(nb) || map.is_impassable(nb) || map.owner_id(nb) != target_id {
                continue;
            }
            let k = map.neighbors4(nb, &mut buf2);
            let owned_by_me = buf2[..k]
                .iter()
                .filter(|&&x| map.owner_id(x) == self.owner)
                .count() as f64;
            let mag = match map.terrain_type(nb) {
                TerrainType::Plains => 1.0,
                TerrainType::Highland => 1.5,
                TerrainType::Mountain => 2.0,
                _ => 0.0,
            };
            if let Some(a) = game.attack_mut(attack_id) {
                a.border.insert(nb);
            }
            let priority = (self.random.next_int(0, 7) as f64 + 10.0)
                * (1.0 - owned_by_me * 0.5 + mag / 2.0)
                + ticks;
            self.to_conquer.push(nb, priority);
        }
    }

    /// Upstream `retreat`: give the surviving troops back and end the attack.
    fn retreat(&mut self, game: &mut Game, malus_percent: f64) {
        let Some(id) = self.attack else {
            self.active = false;
            return;
        };
        let troops = game.attack(id).map_or(0.0, |a| a.troops());
        let deaths = troops * (malus_percent / 100.0);
        let mut troops = troops;
        if !self.remove_troops && self.source_tile.is_none() {
            troops = (troops - self.start_troops.unwrap_or(0.0)).max(0.0);
        }
        let survivors = troops - deaths;
        if let Some(p) = game.player_mut(self.owner) {
            p.add_troops(survivors);
        }
        game.delete_attack(id);
        self.active = false;
    }

    /// Upstream `handleDeadDefender`: a defender down to < 100 tiles is finished off. Its gold
    /// goes to the attacker; tiles touching the attacker go to the attacker, others to another
    /// enemy neighbour, repeated until nothing changes.
    fn handle_dead_defender(&mut self, game: &mut Game) {
        let Some(target) = self.target else { return };
        let cutoff = game.config().attack_logic.conquer_bot_defender_cutoff_tiles;
        if game.player(target).is_none_or(|p| p.num_tiles() >= cutoff) {
            return;
        }
        game.conquer_player(self.owner, target);
        let mut buf = [0; 4];
        for _pass in 0..100 {
            let mut progressed = false;
            let tiles: Vec<TileRef> = game
                .player(target)
                .map_or_else(Vec::new, |p| p.tiles().to_vec());
            for t in tiles {
                if game.map().owner_id(t) != target {
                    continue;
                }
                let n = game.map().neighbors4(t, &mut buf);
                if buf[..n]
                    .iter()
                    .any(|&x| game.map().owner_id(x) == self.owner)
                {
                    game.conquer(self.owner, t);
                    progressed = true;
                } else if let Some(&other) = buf[..n].iter().find(|&&x| {
                    let o = game.map().owner_id(x);
                    o != 0 && o != target
                }) {
                    let o = game.map().owner_id(other);
                    game.conquer(o, t);
                    progressed = true;
                }
            }
            if !progressed {
                break;
            }
        }
    }
}

impl Execution for AttackExecution {
    fn init(&mut self, game: &mut Game, _ticks: u64) {
        if !self.active {
            return;
        }
        if let Some(t) = self.target {
            if game.player(t).is_none() || t == self.owner || !game.can_attack_player(self.owner, t)
            {
                self.active = false;
                return;
            }
        }
        let Some(owner) = game.player(self.owner) else {
            self.active = false;
            return;
        };
        let wanted = self
            .start_troops
            .unwrap_or_else(|| rules::attack_amount(game.config(), owner));
        let mut start = wanted;
        if self.remove_troops {
            start = wanted.min(owner.troops() as f64);
            start = game
                .player_mut(self.owner)
                .map_or(0, |p| p.remove_troops(start)) as f64;
        }
        self.start_troops = Some(start);
        let id = game.create_attack(self.owner, self.target, start, self.source_tile);
        self.attack = Some(id);
        if let Some(p) = game.player_mut(self.owner) {
            p.attacks_sent += 1;
        }
        // Upstream: the target dislikes the attacker (more on harder difficulties).
        if let Some(t) = self.target {
            let change = match game.config().difficulty {
                crate::config::Difficulty::Easy => -60.0,
                crate::config::Difficulty::Medium => -70.0,
                crate::config::Difficulty::Hard => -80.0,
                crate::config::Difficulty::Impossible => -100.0,
            };
            game.update_relation(t, self.owner, change);
        }
        match self.source_tile {
            Some(t) => self.add_neighbors(game, t),
            None => self.refresh_to_conquer(game),
        }

        // Opposing attacks cancel out.
        for incoming in game.incoming_attacks(self.owner) {
            let Some(inc) = game.attack(incoming) else {
                continue;
            };
            if Some(inc.attacker()) != self.target {
                continue;
            }
            let (inc_troops, my_troops) =
                (inc.troops(), game.attack(id).map_or(0.0, |a| a.troops()));
            if inc_troops > my_troops {
                if let Some(a) = game.attack_mut(incoming) {
                    a.set_troops(inc_troops - my_troops);
                }
                game.delete_attack(id);
                self.active = false;
                return;
            }
            if let Some(a) = game.attack_mut(id) {
                a.set_troops(my_troops - inc_troops);
            }
            game.delete_attack(incoming);
        }
        // Merge with earlier land attacks on the same target.
        let outgoing: Vec<AttackId> = game
            .player(self.owner)
            .map_or_else(Vec::new, |p| p.outgoing_attacks().to_vec());
        for other in outgoing {
            if other == id || self.source_tile.is_some() {
                continue;
            }
            let Some(o) = game.attack(other) else {
                continue;
            };
            if o.target() != self.target {
                continue;
            }
            let extra = o.troops();
            if let Some(a) = game.attack_mut(id) {
                let t = a.troops() + extra;
                a.set_troops(t);
            }
            game.delete_attack(other);
        }

        if self.to_conquer.is_empty() {
            self.retreat(game, 0.0);
        }
    }

    fn tick(&mut self, game: &mut Game, _ticks: u64) {
        let Some(id) = self.attack else {
            self.active = false;
            return;
        };
        let Some(attack) = game.attack(id) else {
            self.active = false;
            return;
        };
        if !attack.is_active() {
            self.active = false;
            return;
        }
        if attack.retreated() {
            let malus = if self.target.is_some() {
                game.config().attack_logic.retreat_malus_percent
            } else {
                0.0
            };
            self.retreat(game, malus);
            return;
        }
        if attack.retreating() {
            return;
        }
        // An alliance made after the attack started: pull back.
        if self.target.is_some_and(|t| game.is_friendly(self.owner, t)) {
            self.retreat(game, 0.0);
            return;
        }
        let mut troop_count = attack.troops();
        let border_size = attack.border_size() as f64 + self.random.next_int(0, 5) as f64;
        let target_id = self.target_small_id();
        let mut tick_budget = 1.0;
        let mut buf = [0; 4];

        while tick_budget > 0.0 {
            if troop_count < 1.0 {
                game.delete_attack(id);
                self.active = false;
                return;
            }
            let Some(tile) = self.to_conquer.pop() else {
                self.refresh_to_conquer(game);
                self.retreat(game, 0.0);
                return;
            };
            if let Some(a) = game.attack_mut(id) {
                a.border.remove(&tile);
            }
            let map = game.map();
            let n = map.neighbors4(tile, &mut buf);
            let on_border = buf[..n].iter().any(|&x| map.owner_id(x) == self.owner);
            if map.owner_id(tile) != target_id || !on_border {
                continue;
            }
            if !map.is_land(tile) || map.is_impassable(tile) {
                continue;
            }
            self.add_neighbors(game, tile);
            let input = AttackLogicInput {
                terrain: game.map().terrain_type(tile),
                attack_troops: troop_count,
                attacker_kind: game
                    .player(self.owner)
                    .map_or(super::player::PlayerType::Bot, |p| p.kind()),
                attacker_tiles: game.player(self.owner).map_or(0, |p| p.num_tiles()),
                defender: self
                    .target
                    .and_then(|t| game.player(t))
                    .map(|d| DefenderInfo {
                        kind: d.kind(),
                        num_tiles: d.num_tiles(),
                        troops: d.troops() as f64,
                        is_traitor: game.is_traitor(d.id()),
                        is_disconnected_teammate: false,
                    }),
                defender_has_defense_post: self.target.is_some_and(|d| {
                    game.has_structure_near(
                        tile,
                        game.config().buildings.defense_post_range,
                        super::units::UnitKind::DefensePost,
                        d,
                    )
                }),
                fallout_ratio: game.map().has_fallout(tile).then(|| {
                    game.map().num_tiles_with_fallout() as f64
                        / game.map().num_land_tiles().max(1) as f64
                }),
                border_size,
            };
            let Some(result) = rules::attack_logic(game.config(), &input) else {
                continue;
            };
            tick_budget -= result.tick_fraction;
            troop_count -= result.attacker_troop_loss;
            if let Some(a) = game.attack_mut(id) {
                a.set_troops(troop_count);
            }
            if let Some(t) = self.target {
                if let Some(d) = game.player_mut(t) {
                    d.remove_troops(result.defender_troop_loss);
                }
            }
            game.conquer(self.owner, tile);
            self.handle_dead_defender(game);
        }
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn active_during_spawn_phase(&self) -> bool {
        false
    }
}

/// Orders a retreat at once and completes it `retreat_delay_ticks` later.
pub struct RetreatExecution {
    player: PlayerId,
    attack: AttackId,
    ordered: bool,
    start_tick: u64,
    active: bool,
}

impl RetreatExecution {
    pub fn new(player: PlayerId, attack: AttackId) -> Self {
        Self {
            player,
            attack,
            ordered: false,
            start_tick: 0,
            active: true,
        }
    }
}

impl Execution for RetreatExecution {
    fn init(&mut self, game: &mut Game, _ticks: u64) {
        self.start_tick = game.ticks();
    }

    fn tick(&mut self, game: &mut Game, _ticks: u64) {
        let owned = game
            .player(self.player)
            .is_some_and(|p| p.outgoing_attacks().contains(&self.attack));
        if !self.ordered {
            if owned {
                if let Some(a) = game.attack_mut(self.attack) {
                    a.retreating = true;
                }
            }
            self.ordered = true;
        }
        if game.ticks() >= self.start_tick + game.config().attack_logic.retreat_delay_ticks {
            if owned {
                if let Some(a) = game.attack_mut(self.attack) {
                    a.retreated = true;
                }
            }
            self.active = false;
        }
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn active_during_spawn_phase(&self) -> bool {
        false
    }
}

/// Every 10 ticks: the player with the most tiles wins once they own more than `win_percent` of
/// the land, or when the hard time limit is reached.
#[derive(Debug, Default)]
pub struct WinCheckExecution {
    active: bool,
}

impl WinCheckExecution {
    pub fn new() -> Self {
        Self { active: true }
    }
}

impl Execution for WinCheckExecution {
    fn init(&mut self, _game: &mut Game, _ticks: u64) {}

    fn tick(&mut self, game: &mut Game, ticks: u64) {
        if !ticks.is_multiple_of(10) {
            return;
        }
        // Most tiles first; ties keep the lower id (stable sort of players in id order, as upstream).
        let Some(top) = game
            .alive_players()
            .fold(None::<(PlayerId, u32)>, |best, p| match best {
                Some((_, n)) if n >= p.num_tiles() => best,
                _ => Some((p.id(), p.num_tiles())),
            })
        else {
            return;
        };
        let elapsed = game.elapsed_game_seconds();
        let rules = &game.config().game;
        let won = elapsed >= game.config().attack_logic.hard_time_limit_seconds as f64
            || top.1 as u64 * 1000
                > game.map().num_land_tiles() as u64 * rules.win_percent as u64 * 10;
        if won {
            game.set_winner(top.0);
            self.active = false;
        }
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn active_during_spawn_phase(&self) -> bool {
        false
    }
}
