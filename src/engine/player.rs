//! Player state. Port of the data half of upstream `packages/engine/src/game/PlayerImpl.ts`.

use crate::engine_lib::game_map::TileRef;

use super::attack::AttackId;
use super::units::UnitId;

/// 1-based small id as upstream: 0 means "nobody" (terra nullius) in the tile state.
pub type PlayerId = u16;

/// Upstream `PlayerType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlayerType {
    Human,
    /// Upstream "tribe": a weak filler AI.
    Bot,
    /// Upstream nation AI (later milestone).
    Nation,
}

/// Upstream `PlayerInfo` (the parts laukofront uses).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerInfo {
    /// Stable string id; seeds per-player random streams like upstream.
    pub id: String,
    pub name: String,
    pub kind: PlayerType,
}

impl PlayerInfo {
    pub fn new(id: impl Into<String>, name: impl Into<String>, kind: PlayerType) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Player {
    pub(crate) small_id: PlayerId,
    pub(crate) info: PlayerInfo,
    /// Upstream stores troops as a bigint; integer here too.
    pub(crate) troops: i64,
    pub(crate) gold: i64,
    /// Owned tiles (order is not meaningful). Membership index lives in `Game`.
    pub(crate) tiles: Vec<TileRef>,
    /// Owned tiles with a 4-neighbour of a different owner.
    pub(crate) border: Vec<TileRef>,
    pub(crate) spawn_tile: Option<TileRef>,
    pub(crate) last_tile_change: u64,
    pub(crate) outgoing: Vec<AttackId>,
    pub(crate) incoming: Vec<AttackId>,
    /// Attacks launched (upstream stats `attacks[ATTACK_INDEX_SENT]`); decides conquest gold.
    pub(crate) attacks_sent: u32,
    /// Ids of this player's units (ships and buildings), active ones only.
    pub(crate) units: Vec<UnitId>,
    /// Sum of levels of finished cities (raises the troop cap).
    pub(crate) city_levels: u32,
    /// Units of each buildable kind ever finished (upstream `unitsConstructed`, for costs).
    pub(crate) constructed: [u32; 4],
}

impl Player {
    pub(crate) fn new(small_id: PlayerId, info: PlayerInfo, troops: i64, gold: i64) -> Self {
        Self {
            small_id,
            info,
            troops,
            gold,
            tiles: Vec::new(),
            border: Vec::new(),
            spawn_tile: None,
            last_tile_change: 0,
            outgoing: Vec::new(),
            incoming: Vec::new(),
            attacks_sent: 0,
            units: Vec::new(),
            city_levels: 0,
            constructed: [0; 4],
        }
    }

    pub fn id(&self) -> PlayerId {
        self.small_id
    }

    pub fn info(&self) -> &PlayerInfo {
        &self.info
    }

    pub fn name(&self) -> &str {
        &self.info.name
    }

    pub fn kind(&self) -> PlayerType {
        self.info.kind
    }

    pub fn troops(&self) -> i64 {
        self.troops
    }

    pub fn gold(&self) -> i64 {
        self.gold
    }

    pub fn num_tiles(&self) -> u32 {
        self.tiles.len() as u32
    }

    pub fn tiles(&self) -> &[TileRef] {
        &self.tiles
    }

    pub fn border_tiles(&self) -> &[TileRef] {
        &self.border
    }

    /// Upstream `isAlive`: owns at least one tile.
    pub fn is_alive(&self) -> bool {
        !self.tiles.is_empty()
    }

    pub fn has_spawned(&self) -> bool {
        self.spawn_tile.is_some()
    }

    pub fn spawn_tile(&self) -> Option<TileRef> {
        self.spawn_tile
    }

    pub fn last_tile_change(&self) -> u64 {
        self.last_tile_change
    }

    /// Ids of this player's active outgoing attacks.
    pub fn outgoing_attacks(&self) -> &[AttackId] {
        &self.outgoing
    }

    /// Ids of active attacks against this player (all attackers, alive or not).
    pub fn incoming_attack_ids(&self) -> &[AttackId] {
        &self.incoming
    }

    /// Ids of this player's active units.
    pub fn units(&self) -> &[UnitId] {
        &self.units
    }

    /// Sum of levels of finished cities.
    pub fn city_levels(&self) -> u32 {
        self.city_levels
    }

    pub fn attacks_sent(&self) -> u32 {
        self.attacks_sent
    }

    /// Upstream `removeGold`: removes up to `g`, returns the amount removed.
    pub fn remove_gold(&mut self, g: i64) -> i64 {
        let r = self.gold.min(g.max(0));
        self.gold -= r;
        r
    }

    /// Upstream `addTroops`: adds `floor(x)`; a negative amount removes instead.
    pub fn add_troops(&mut self, x: f64) {
        if x < 0.0 {
            self.remove_troops(-x);
            return;
        }
        self.troops = self.troops.saturating_add(to_int(x));
    }

    /// Upstream `removeTroops`: removes `min(troops, floor(x))`, returns how many were removed.
    pub fn remove_troops(&mut self, x: f64) -> i64 {
        if x <= 0.0 {
            return 0;
        }
        let r = self.troops.min(to_int(x));
        self.troops -= r;
        r
    }

    pub fn set_troops(&mut self, x: f64) {
        self.troops = to_int(x);
    }

    pub fn add_gold(&mut self, g: i64) {
        self.gold = self.gold.saturating_add(g);
    }
}

/// Upstream `toInt`: `floor`, with infinities clamped to +-(2^53 - 1).
pub fn to_int(x: f64) -> i64 {
    const MAX_SAFE: i64 = 9_007_199_254_740_991;
    if x == f64::INFINITY {
        MAX_SAFE
    } else if x == f64::NEG_INFINITY {
        -MAX_SAFE
    } else if x.is_nan() {
        0
    } else {
        x.floor() as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> Player {
        Player::new(1, PlayerInfo::new("a", "A", PlayerType::Human), 100, 0)
    }

    #[test]
    fn add_troops_floors() {
        let mut a = p();
        a.add_troops(10.9);
        assert_eq!(a.troops(), 110);
    }

    #[test]
    fn negative_add_removes_floor_of_amount() {
        let mut a = p();
        a.add_troops(-3.7);
        assert_eq!(a.troops(), 97, "removeTroops(3.7) removes floor(3.7) = 3");
    }

    #[test]
    fn remove_never_goes_below_zero() {
        let mut a = p();
        assert_eq!(a.remove_troops(1000.0), 100);
        assert_eq!(a.troops(), 0);
    }

    #[test]
    fn to_int_edges() {
        assert_eq!(to_int(-0.5), -1);
        assert_eq!(to_int(f64::INFINITY), 9_007_199_254_740_991);
    }
}
