//! Units: ships and buildings. Port of the parts of upstream `game/UnitImpl.ts` and the unit
//! bookkeeping in `GameImpl`/`PlayerImpl` that boats and structures need.

use crate::engine_lib::game_map::TileRef;

use super::player::PlayerId;

pub type UnitId = u32;

/// Upstream `UnitType` (the kinds ported so far).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum UnitKind {
    TransportShip,
    TradeShip,
    City,
    Port,
    Factory,
    DefensePost,
    MissileSilo,
    SamLauncher,
    Warship,
    AtomBomb,
    HydrogenBomb,
    Mirv,
    MirvWarhead,
}

impl UnitKind {
    /// Buildings (stay on one tile, change hands with it).
    pub fn is_structure(self) -> bool {
        matches!(
            self,
            UnitKind::City
                | UnitKind::Port
                | UnitKind::Factory
                | UnitKind::DefensePost
                | UnitKind::MissileSilo
                | UnitKind::SamLauncher
        )
    }

    /// Ships and missiles (things that move every tick).
    pub fn is_mobile(self) -> bool {
        matches!(
            self,
            UnitKind::TransportShip | UnitKind::TradeShip | UnitKind::Warship
        ) || self.is_nuke()
    }

    /// Missiles in flight.
    pub fn is_nuke(self) -> bool {
        matches!(
            self,
            UnitKind::AtomBomb | UnitKind::HydrogenBomb | UnitKind::Mirv | UnitKind::MirvWarhead
        )
    }

    /// Buildings the player can place from the build menu.
    pub const BUILDABLE: [UnitKind; 6] = [
        UnitKind::City,
        UnitKind::Port,
        UnitKind::Factory,
        UnitKind::DefensePost,
        UnitKind::MissileSilo,
        UnitKind::SamLauncher,
    ];

    pub fn name(self) -> &'static str {
        match self {
            UnitKind::TransportShip => "Transport ship",
            UnitKind::TradeShip => "Trade ship",
            UnitKind::City => "City",
            UnitKind::Port => "Port",
            UnitKind::Factory => "Factory",
            UnitKind::DefensePost => "Defense post",
            UnitKind::MissileSilo => "Missile silo",
            UnitKind::SamLauncher => "SAM launcher",
            UnitKind::Warship => "Warship",
            UnitKind::AtomBomb => "Atom bomb",
            UnitKind::HydrogenBomb => "Hydrogen bomb",
            UnitKind::Mirv => "MIRV",
            UnitKind::MirvWarhead => "MIRV warhead",
        }
    }

    /// Upstream `constructionDuration` in ticks (0 for ships).
    pub fn construction_ticks(self) -> u64 {
        match self {
            UnitKind::City | UnitKind::Factory => 2 * 10,
            UnitKind::Port | UnitKind::DefensePost => 5 * 10,
            UnitKind::MissileSilo => 10 * 10,
            UnitKind::SamLauncher => 30 * 10,
            _ => 0,
        }
    }

    /// Upstream `upgradable`.
    pub fn upgradable(self) -> bool {
        matches!(
            self,
            UnitKind::City
                | UnitKind::Port
                | UnitKind::Factory
                | UnitKind::MissileSilo
                | UnitKind::SamLauncher
        )
    }
}

#[derive(Debug, Clone)]
pub struct Unit {
    pub(crate) id: UnitId,
    pub(crate) kind: UnitKind,
    pub(crate) owner: PlayerId,
    pub(crate) tile: TileRef,
    pub(crate) active: bool,
    /// Structure level (1 when built; upgrades add 1).
    pub(crate) level: u32,
    /// Tick construction finishes (structures); `None` once built.
    pub(crate) under_construction_until: Option<u64>,
    /// Troops carried (transport ships).
    pub(crate) troops: f64,
    /// Destination (ships).
    pub(crate) target_tile: Option<TileRef>,
    /// Remaining route (ships), next step first.
    pub path: Vec<TileRef>,
    pub(crate) path_index: usize,
    pub(crate) retreating: bool,
    /// Warship health (0 for other units).
    pub(crate) health: i64,
    /// Ticks at which silo/SAM slots were last used (reload queue).
    pub(crate) reload: Vec<u64>,
}

impl Unit {
    pub fn id(&self) -> UnitId {
        self.id
    }
    pub fn kind(&self) -> UnitKind {
        self.kind
    }
    pub fn owner(&self) -> PlayerId {
        self.owner
    }
    pub fn tile(&self) -> TileRef {
        self.tile
    }
    pub fn is_active(&self) -> bool {
        self.active
    }
    pub fn level(&self) -> u32 {
        self.level
    }
    pub fn troops(&self) -> f64 {
        self.troops
    }
    pub fn target_tile(&self) -> Option<TileRef> {
        self.target_tile
    }
    pub fn is_retreating(&self) -> bool {
        self.retreating
    }
    pub fn health(&self) -> i64 {
        self.health
    }
    /// True while the building is still going up.
    pub fn under_construction(&self, now: u64) -> bool {
        self.under_construction_until.is_some_and(|t| now < t)
    }
    /// Construction progress 0..1 (1 when built).
    pub fn construction_progress(&self, now: u64) -> f32 {
        match self.under_construction_until {
            Some(end) if now < end => {
                let total = self.kind.construction_ticks().max(1) as f32;
                1.0 - (end - now) as f32 / total
            }
            _ => 1.0,
        }
    }
    /// Remaining route of a ship (for drawing).
    pub fn remaining_path(&self) -> &[TileRef] {
        &self.path[self.path_index.min(self.path.len())..]
    }
}
