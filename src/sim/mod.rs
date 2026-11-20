//! The world state and the tick. Everything that changes the game goes through
//! `World::queue_order` and `World::tick`; everything else only reads.
//!
//! A tick does, in order:
//! 1. clear last tick's events,
//! 2. resolve queued orders in the order they were queued,
//! 3. advance the tick counter,
//! 4. recompute troop caps, then grow troops on growth ticks,
//! 5. refresh player totals and pay gold,
//! 6. eliminate players with no territories, then check victory.
//!
//! The tick reuses its buffers and does not allocate once the buffers have warmed up.

use crate::combat::{self, AttackOutcome};
use crate::config::{GameConfig, MapSize};
use crate::core::{GameError, OrderError, Owner, PlayerId, Rng, TerritoryId};
use crate::economy;
use crate::map::Map;
use crate::territory::{PlayerState, Territory};

/// XOR-ed into the match seed for the setup random stream (neutral troops, start positions),
/// so it is independent of the map stream.
pub const SETUP_SEED_SALT: u64 = 0x5EED_0F5E_7000_0000;

/// An action a player asks the simulation to perform on the next tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    /// Send `troops` from `from` into the neighbouring territory `to`.
    Attack {
        player: PlayerId,
        from: TerritoryId,
        to: TerritoryId,
        troops: u32,
    },
    /// Spend `reinforce.cost_gold` gold to add `reinforce.troops` troops to an own territory.
    Reinforce {
        player: PlayerId,
        territory: TerritoryId,
    },
}

impl Order {
    pub fn player(&self) -> PlayerId {
        match *self {
            Order::Attack { player, .. } | Order::Reinforce { player, .. } => player,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VictoryReason {
    /// Every other player was eliminated.
    Conquest,
    /// `sim.max_match_ticks` was reached; most territories, then most troops, then lowest id.
    TimeLimit,
}

/// Something that happened during the last tick. Read with `World::events`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Attack {
        player: PlayerId,
        from: TerritoryId,
        to: TerritoryId,
        /// Owner of `to` before the attack.
        defender: Owner,
        sent: u32,
        outcome: AttackOutcome,
    },
    Reinforced {
        player: PlayerId,
        territory: TerritoryId,
        added: u32,
    },
    /// The order was valid when queued but no longer was when the tick resolved it.
    OrderDropped {
        order: Order,
    },
    Eliminated {
        player: PlayerId,
    },
    GameOver {
        winner: PlayerId,
        reason: VictoryReason,
    },
}

#[derive(Debug, Clone)]
pub struct World {
    config: GameConfig,
    seed: u64,
    map: Map,
    territories: Vec<Territory>,
    players: Vec<PlayerState>,
    tick: u64,
    winner: Option<(PlayerId, VictoryReason)>,
    orders: Vec<Order>,
    resolving: Vec<Order>,
    events: Vec<Event>,
}

impl World {
    /// Generates the map for `size` and `seed` and sets up a match for `players` players
    /// (player 0 is the human). Fails if `players` is 0, above `sim.max_players`, or above
    /// the territory count.
    pub fn new(
        config: GameConfig,
        size: MapSize,
        seed: u64,
        players: usize,
    ) -> Result<Self, GameError> {
        let map = Map::generate(config.map.size(size), config.map.candidates_per_point, seed)?;
        Self::with_map(config, map, seed, players)
    }

    /// Sets up a match on an existing map.
    pub fn with_map(
        config: GameConfig,
        map: Map,
        seed: u64,
        players: usize,
    ) -> Result<Self, GameError> {
        if players == 0 {
            return Err(GameError::Setup("at least 1 player is required".into()));
        }
        if players > config.sim.max_players as usize {
            return Err(GameError::Setup(format!(
                "{players} players requested, sim.max_players is {}",
                config.sim.max_players
            )));
        }
        let n = map.territory_count();
        if players > n {
            return Err(GameError::Setup(format!(
                "{players} players need at least {players} territories, map has {n}"
            )));
        }

        let mut rng = Rng::new(seed ^ SETUP_SEED_SALT);
        let tc = &config.territory;
        let mut territories: Vec<Territory> = (0..n)
            .map(|_| Territory {
                owner: Owner::Neutral,
                troops: rng.range_inclusive(tc.neutral_troops_min, tc.neutral_troops_max),
                troop_cap: tc.base_cap,
                gold_per_min: tc.gold_per_min,
            })
            .collect();

        for (i, start) in pick_starts(&map, players, &mut rng).into_iter().enumerate() {
            let t = &mut territories[start.index()];
            t.owner = Owner::Player(PlayerId(i as u8));
            t.troops = tc.start_troops;
        }

        let players = (0..players)
            .map(|i| PlayerState {
                id: PlayerId(i as u8),
                gold_milli: 0,
                alive: true,
                territories: 0,
                troops: 0,
            })
            .collect();

        let mut world = Self {
            seed,
            territories,
            players,
            tick: 0,
            winner: None,
            orders: Vec::with_capacity(256),
            resolving: Vec::with_capacity(256),
            events: Vec::with_capacity(n + 64),
            map,
            config,
        };
        economy::update_caps(&world.map, &mut world.territories, &world.config);
        world.refresh_player_totals();
        Ok(world)
    }

    // ----- reading -----

    pub fn config(&self) -> &GameConfig {
        &self.config
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn map(&self) -> &Map {
        &self.map
    }

    /// Ticks completed so far.
    pub fn tick_count(&self) -> u64 {
        self.tick
    }

    pub fn territories(&self) -> &[Territory] {
        &self.territories
    }

    /// Panics if `t` is not on the map; check with `map().contains(t)` for untrusted ids.
    pub fn territory(&self, t: TerritoryId) -> &Territory {
        &self.territories[t.index()]
    }

    pub fn players(&self) -> &[PlayerState] {
        &self.players
    }

    pub fn player(&self, p: PlayerId) -> Option<&PlayerState> {
        self.players.get(p.index())
    }

    /// Events produced by the most recent tick.
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// Orders waiting for the next tick.
    pub fn pending_orders(&self) -> &[Order] {
        &self.orders
    }

    pub fn winner(&self) -> Option<(PlayerId, VictoryReason)> {
        self.winner
    }

    pub fn is_over(&self) -> bool {
        self.winner.is_some()
    }

    // ----- writing -----

    /// Checks an order against the current state and queues it for the next tick.
    pub fn queue_order(&mut self, order: Order) -> Result<(), OrderError> {
        self.validate(&order)?;
        self.orders.push(order);
        Ok(())
    }

    /// Checks an order without queuing it.
    pub fn validate(&self, order: &Order) -> Result<(), OrderError> {
        if self.winner.is_some() {
            return Err(OrderError::GameOver);
        }
        let player = order.player();
        let ps = self.player(player).ok_or(OrderError::UnknownPlayer)?;
        if !ps.alive {
            return Err(OrderError::PlayerEliminated);
        }
        match *order {
            Order::Attack {
                from, to, troops, ..
            } => {
                if !self.map.contains(from) || !self.map.contains(to) {
                    return Err(OrderError::UnknownTerritory);
                }
                let src = &self.territories[from.index()];
                if src.owner != Owner::Player(player) {
                    return Err(OrderError::NotOwner);
                }
                if !self.map.is_neighbor(from, to) {
                    return Err(OrderError::NotNeighbor);
                }
                if self.territories[to.index()].owner == Owner::Player(player) {
                    return Err(OrderError::OwnTarget);
                }
                if troops == 0 {
                    return Err(OrderError::NoTroopsSent);
                }
                if troops > src.sendable() {
                    return Err(OrderError::NotEnoughTroops);
                }
            }
            Order::Reinforce { territory, .. } => {
                if !self.map.contains(territory) {
                    return Err(OrderError::UnknownTerritory);
                }
                let t = &self.territories[territory.index()];
                if t.owner != Owner::Player(player) {
                    return Err(OrderError::NotOwner);
                }
                if ps.gold_milli < self.reinforce_cost_milli() {
                    return Err(OrderError::NotEnoughGold);
                }
                if t.troops >= t.troop_cap {
                    return Err(OrderError::AtCap);
                }
            }
        }
        Ok(())
    }

    /// Overwrites one territory's owner and troops, then refreshes caps and player totals.
    /// For scenario setup, tests and save loading; not used during normal play.
    pub fn set_territory(
        &mut self,
        t: TerritoryId,
        owner: Owner,
        troops: u32,
    ) -> Result<(), GameError> {
        if !self.map.contains(t) {
            return Err(GameError::Setup(format!(
                "territory {} is not on the map",
                t.0
            )));
        }
        if let Owner::Player(p) = owner {
            if p.index() >= self.players.len() {
                return Err(GameError::Setup(format!("player {} does not exist", p.0)));
            }
        }
        let terr = &mut self.territories[t.index()];
        terr.owner = owner;
        terr.troops = troops;
        economy::update_caps(&self.map, &mut self.territories, &self.config);
        self.refresh_player_totals();
        Ok(())
    }

    /// Sets a player's gold (in whole gold). For scenario setup, tests and save loading.
    pub fn set_gold(&mut self, p: PlayerId, gold: u64) -> Result<(), GameError> {
        let ps = self
            .players
            .get_mut(p.index())
            .ok_or_else(|| GameError::Setup(format!("player {} does not exist", p.0)))?;
        ps.gold_milli = gold.saturating_mul(crate::core::MILLI);
        Ok(())
    }

    /// Advances the simulation by one tick. Does nothing once the match is over.
    pub fn tick(&mut self) {
        if self.winner.is_some() {
            return;
        }
        self.events.clear();

        std::mem::swap(&mut self.orders, &mut self.resolving);
        for i in 0..self.resolving.len() {
            let order = self.resolving[i];
            self.apply(order);
        }
        self.resolving.clear();

        self.tick += 1;

        economy::update_caps(&self.map, &mut self.territories, &self.config);
        if self
            .tick
            .is_multiple_of(self.config.growth.interval_ticks as u64)
        {
            economy::grow(&mut self.territories, &self.config);
        }

        self.refresh_player_totals();
        economy::accrue_gold(&self.territories, &mut self.players, &self.config);

        self.check_eliminations();
        self.check_victory();
    }

    // ----- internals -----

    fn reinforce_cost_milli(&self) -> u64 {
        self.config.reinforce.cost_gold as u64 * crate::core::MILLI
    }

    fn apply(&mut self, order: Order) {
        match order {
            Order::Attack {
                player,
                from,
                to,
                troops,
            } => {
                let me = Owner::Player(player);
                let src = self.territories[from.index()];
                let defender = self.territories[to.index()].owner;
                let sent = troops.min(src.sendable());
                if src.owner != me || defender == me || sent == 0 {
                    self.events.push(Event::OrderDropped { order });
                    return;
                }
                self.territories[from.index()].troops -= sent;
                let outcome = combat::resolve(player, sent, &mut self.territories[to.index()]);
                self.events.push(Event::Attack {
                    player,
                    from,
                    to,
                    defender,
                    sent,
                    outcome,
                });
            }
            Order::Reinforce { player, territory } => {
                let cost = self.reinforce_cost_milli();
                let t = self.territories[territory.index()];
                let gold = self.players[player.index()].gold_milli;
                if t.owner != Owner::Player(player) || gold < cost || t.troops >= t.troop_cap {
                    self.events.push(Event::OrderDropped { order });
                    return;
                }
                let new_troops = t
                    .troops
                    .saturating_add(self.config.reinforce.troops)
                    .min(t.troop_cap);
                self.territories[territory.index()].troops = new_troops;
                self.players[player.index()].gold_milli -= cost;
                self.events.push(Event::Reinforced {
                    player,
                    territory,
                    added: new_troops - t.troops,
                });
            }
        }
    }

    fn refresh_player_totals(&mut self) {
        for p in self.players.iter_mut() {
            p.territories = 0;
            p.troops = 0;
        }
        for t in &self.territories {
            if let Owner::Player(p) = t.owner {
                let ps = &mut self.players[p.index()];
                ps.territories += 1;
                ps.troops += t.troops as u64;
            }
        }
    }

    fn check_eliminations(&mut self) {
        for i in 0..self.players.len() {
            let p = &mut self.players[i];
            if p.alive && p.territories == 0 {
                p.alive = false;
                let player = p.id;
                self.events.push(Event::Eliminated { player });
            }
        }
    }

    fn check_victory(&mut self) {
        let mut alive = self.players.iter().filter(|p| p.alive);
        let first = alive.next().copied();
        let more = alive.next().is_some();

        let result = if self.players.len() >= 2 && !more {
            first.map(|p| (p.id, VictoryReason::Conquest))
        } else if self.tick >= self.config.sim.max_match_ticks {
            self.players
                .iter()
                .filter(|p| p.alive)
                .max_by(|a, b| {
                    a.territories
                        .cmp(&b.territories)
                        .then(a.troops.cmp(&b.troops))
                        .then(b.id.cmp(&a.id))
                })
                .map(|p| (p.id, VictoryReason::TimeLimit))
        } else {
            None
        };

        if let Some((winner, reason)) = result {
            self.winner = Some((winner, reason));
            self.events.push(Event::GameOver { winner, reason });
        }
    }
}

/// Start territories: the first is random; each next one maximises the smallest squared
/// distance (between seed points) to the starts already chosen. Ties go to the lower id.
fn pick_starts(map: &Map, players: usize, rng: &mut Rng) -> Vec<TerritoryId> {
    let n = map.territory_count();
    let mut starts = Vec::with_capacity(players);
    starts.push(TerritoryId(rng.below(n as u32) as u16));
    let dist2 = |a: TerritoryId, b: TerritoryId| {
        let (ax, ay) = map.seed_point(a);
        let (bx, by) = map.seed_point(b);
        let dx = ax as i64 - bx as i64;
        let dy = ay as i64 - by as i64;
        dx * dx + dy * dy
    };
    while starts.len() < players {
        let mut best: Option<(TerritoryId, i64)> = None;
        for t in map.territory_ids() {
            if starts.contains(&t) {
                continue;
            }
            let d = starts
                .iter()
                .map(|&s| dist2(s, t))
                .min()
                .unwrap_or(i64::MAX);
            if best.is_none_or(|(_, bd)| d > bd) {
                best = Some((t, d));
            }
        }
        match best {
            Some((t, _)) => starts.push(t),
            None => break, // unreachable: players <= territory count was checked
        }
    }
    starts
}
