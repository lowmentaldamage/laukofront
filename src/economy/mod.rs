//! Troop caps, troop growth and gold income. Called once per tick by `sim`.
//! None of these functions allocate.

use crate::config::GameConfig;
use crate::core::{Owner, TerritoryId};
use crate::map::Map;
use crate::territory::{PlayerState, Territory};

/// Cap of one territory: `base_cap + cap_per_owned_neighbor * (neighbours with the same owner)`.
/// Neutral territories use the same formula with neutral neighbours, but never grow.
pub fn troop_cap(map: &Map, territories: &[Territory], t: TerritoryId, cfg: &GameConfig) -> u32 {
    let owner = territories[t.index()].owner;
    let same = map
        .neighbors(t)
        .iter()
        .filter(|&&n| territories[n as usize].owner == owner)
        .count() as u32;
    cfg.territory
        .base_cap
        .saturating_add(cfg.territory.cap_per_owned_neighbor.saturating_mul(same))
}

/// Recomputes every territory's cap.
pub fn update_caps(map: &Map, territories: &mut [Territory], cfg: &GameConfig) {
    for i in 0..territories.len() {
        let cap = troop_cap(map, territories, TerritoryId(i as u16), cfg);
        territories[i].troop_cap = cap;
    }
}

/// Troops one growth step adds to `troops`, before the cap: `max(min_amount, troops * percent / 100)`.
pub fn growth_amount(troops: u32, cfg: &GameConfig) -> u32 {
    let pct = (troops as u64 * cfg.growth.percent as u64 / 100) as u32;
    pct.max(cfg.growth.min_amount)
}

/// One growth step for every owned territory below its cap. Growth never passes the cap;
/// troops already above the cap are left alone.
pub fn grow(territories: &mut [Territory], cfg: &GameConfig) {
    for t in territories.iter_mut() {
        if matches!(t.owner, Owner::Neutral) || t.troops >= t.troop_cap {
            continue;
        }
        t.troops = t
            .troops
            .saturating_add(growth_amount(t.troops, cfg))
            .min(t.troop_cap);
    }
}

/// Milli-gold one territory earns per tick: `gold_per_min * 1000 / (60 * tick_hz)`, rounded down.
pub fn income_milli_per_tick(gold_per_min: u32, cfg: &GameConfig) -> u64 {
    gold_per_min as u64 * crate::core::MILLI / (60 * cfg.sim.tick_hz as u64)
}

/// Adds one tick of income: each owned territory pays its owner. Eliminated players get nothing.
pub fn accrue_gold(territories: &[Territory], players: &mut [PlayerState], cfg: &GameConfig) {
    for t in territories {
        if let Owner::Player(p) = t.owner {
            if let Some(ps) = players.get_mut(p.index()) {
                if ps.alive {
                    ps.gold_milli = ps
                        .gold_milli
                        .saturating_add(income_milli_per_tick(t.gold_per_min, cfg));
                }
            }
        }
    }
}
