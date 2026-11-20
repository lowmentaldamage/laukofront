//! Encircled territory is annexed. A simplified port of upstream `PlayerExecution.removeClusters`
//! (see DECISIONS D25):
//!
//! - The player's land is split into 4-connected pieces.
//! - The **largest** piece is annexed only if it is surrounded by a single enemy: no tile on an
//!   ocean shore or the map edge, no unowned neighbour, every foreign neighbour owned by the same
//!   player, and that player's neighbouring tiles span the piece's bounding box
//!   (upstream `surroundedBySamePlayer`).
//! - **Other** pieces are annexed if no tile touches water or the map edge, no neighbour is
//!   unowned, and enemy neighbours span the piece's bounding box (upstream `classifyCluster`).
//!   The captor is the enemy with the largest attack on this player, else the enemy with the most
//!   bordering tiles (upstream `getCapturingPlayer`).
//! - Annexing the player's last land counts as conquering the player (gold transfer).
//!
//! Upstream's handling of diagonally pinched clusters is not ported yet.

use std::collections::HashMap;

use crate::engine_lib::game_map::TileRef;

use super::game::Game;
use super::player::PlayerId;

#[derive(Clone, Copy)]
struct Bounds {
    min_x: u32,
    min_y: u32,
    max_x: u32,
    max_y: u32,
}

impl Bounds {
    fn empty() -> Self {
        Self {
            min_x: u32::MAX,
            min_y: u32::MAX,
            max_x: 0,
            max_y: 0,
        }
    }

    fn add(&mut self, x: u32, y: u32) {
        self.min_x = self.min_x.min(x);
        self.min_y = self.min_y.min(y);
        self.max_x = self.max_x.max(x);
        self.max_y = self.max_y.max(y);
    }

    fn covers(&self, o: &Bounds) -> bool {
        self.min_x <= o.min_x
            && self.min_y <= o.min_y
            && self.max_x >= o.max_x
            && self.max_y >= o.max_y
    }
}

fn bounds_of(game: &Game, tiles: &[TileRef]) -> Bounds {
    let mut b = Bounds::empty();
    for &t in tiles {
        b.add(game.map().x(t), game.map().y(t));
    }
    b
}

/// Upstream `surroundedBySamePlayer`.
fn surrounded_by_same_player(game: &Game, me: PlayerId, piece: &[TileRef]) -> Option<PlayerId> {
    let map = game.map();
    let mut enemy = 0u16;
    let mut eb = Bounds::empty();
    let mut buf = [0; 4];
    for &t in piece {
        if map.is_ocean_shore(t) || map.is_on_edge_of_map(t) {
            return None;
        }
        let n = map.neighbors4(t, &mut buf);
        for &nb in &buf[..n] {
            let o = map.owner_id(nb);
            if o == me {
                continue;
            }
            if o == 0 {
                return None;
            }
            if enemy == 0 {
                enemy = o;
            } else if enemy != o {
                return None;
            }
            eb.add(map.x(nb), map.y(nb));
        }
    }
    (enemy != 0 && eb.covers(&bounds_of(game, piece))).then_some(enemy)
}

/// Upstream `classifyCluster`: true if the piece is surrounded by enemies.
fn is_surrounded(game: &Game, me: PlayerId, piece: &[TileRef]) -> bool {
    let map = game.map();
    let mut has_enemy = false;
    let mut eb = Bounds::empty();
    let mut buf = [0; 4];
    for &t in piece {
        if map.is_shore(t) || map.is_on_edge_of_map(t) {
            return false;
        }
        let n = map.neighbors4(t, &mut buf);
        for &nb in &buf[..n] {
            let o = map.owner_id(nb);
            if o != 0 && o != me {
                has_enemy = true;
                eb.add(map.x(nb), map.y(nb));
            } else if o == 0 {
                return false;
            }
        }
    }
    has_enemy && eb.covers(&bounds_of(game, piece))
}

/// Upstream `getCapturingPlayer`.
fn capturing_player(game: &Game, me: PlayerId, piece: &[TileRef]) -> Option<PlayerId> {
    let map = game.map();
    let mut counts: HashMap<PlayerId, u32> = HashMap::new();
    let mut order: Vec<PlayerId> = Vec::new();
    let mut buf = [0; 4];
    for &t in piece {
        let n = map.neighbors4(t, &mut buf);
        for &nb in &buf[..n] {
            let o = map.owner_id(nb);
            if o == 0 || o == me {
                continue;
            }
            let c = counts.entry(o).or_insert(0);
            if *c == 0 {
                order.push(o);
            }
            *c += 1;
        }
    }
    if order.is_empty() {
        return None;
    }
    // Largest attack on `me` among those neighbours.
    let mut best: Option<(PlayerId, f64)> = None;
    for &o in &order {
        let Some(p) = game.player(o) else { continue };
        for &a in p.outgoing_attacks() {
            if let Some(at) = game.attack(a) {
                if at.target() == Some(me) && at.troops() > best.map_or(0.0, |b| b.1) {
                    best = Some((o, at.troops()));
                }
            }
        }
    }
    if let Some((o, _)) = best {
        return Some(o);
    }
    // Otherwise the neighbour with the most bordering tiles (first seen wins ties, like `getMode`).
    let mut mode = order[0];
    for &o in &order {
        if counts[&o] > counts[&mode] {
            mode = o;
        }
    }
    Some(mode)
}

/// Annexes `me`'s encircled pieces. Returns how many tiles changed hands.
pub fn remove_clusters(game: &mut Game, me: PlayerId) -> usize {
    let pieces = game.components_of(me);
    if pieces.is_empty() {
        return 0;
    }
    let mut largest = 0;
    for (i, p) in pieces.iter().enumerate() {
        if p.len() > pieces[largest].len() {
            largest = i;
        }
    }
    let mut changed = 0;
    let mut order: Vec<usize> = vec![largest];
    order.extend((0..pieces.len()).filter(|&i| i != largest));
    for i in order {
        if !game.player(me).is_some_and(|p| p.is_alive()) {
            break;
        }
        let piece = &pieces[i];
        if piece.iter().any(|&t| game.map().owner_id(t) != me) {
            continue;
        }
        let captor = if i == largest {
            surrounded_by_same_player(game, me, piece)
        } else if is_surrounded(game, me, piece) {
            capturing_player(game, me, piece)
        } else {
            None
        };
        let Some(c) = captor else { continue };
        if game
            .player(me)
            .is_some_and(|p| p.num_tiles() as usize == piece.len())
        {
            game.conquer_player(c, me);
        }
        for &t in piece {
            if game.conquer(c, t) {
                changed += 1;
            }
        }
    }
    changed
}
