//! Water bodies and ship routing. A simplified port of upstream `pathfinding/*` and
//! `pathfinding/spatial/SpatialQuery.ts` (see DECISIONS D35):
//! - every water tile gets a water-body id (4-connected); ships only sail inside one body;
//! - routes are found in two steps, like upstream's hierarchical search: A* on a coarse grid of
//!   8x8-tile cells, then weighted A* over water tiles restricted to a corridor around the coarse
//!   route (falling back to an unrestricted search if the corridor is blocked);
//! - shore helpers pick landing and launch tiles by straight-line distance within a body.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::engine_lib::game_map::{GameMap, TileRef};

const NONE: u32 = u32::MAX;
/// Heuristic weight: 1.0 = optimal A*, higher = faster but less direct routes.
const HEURISTIC_WEIGHT: u64 = 2;
/// Routes longer than this many explored tiles are given up (prevents huge stalls).
const MAX_EXPANSIONS: usize = 2_000_000;
/// Coarse cell size in tiles.
const CELL: u32 = 8;
/// Routes shorter than this (manhattan) skip the coarse step.
const DIRECT_LIMIT: u32 = 96;

/// Water-body ids per tile and reusable search buffers.
#[derive(Debug, Clone)]
pub struct WaterInfo {
    /// Water body of each water tile, NONE for land.
    body: Vec<u32>,
    gen: u32,
    seen: Vec<u32>,
    came_from: Vec<u32>,
    cost: Vec<u32>,
    coarse_w: u32,
    coarse_h: u32,
    /// Coarse cells containing any water.
    coarse_water: Vec<bool>,
    corridor: Vec<bool>,
}

impl WaterInfo {
    pub fn new(map: &GameMap) -> Self {
        let n = map.num_tiles() as usize;
        let mut body = vec![NONE; n];
        let mut next = 0u32;
        let mut stack = Vec::new();
        let mut buf = [0; 4];
        for start in 0..map.num_tiles() {
            if map.is_land(start) || body[start as usize] != NONE {
                continue;
            }
            body[start as usize] = next;
            stack.push(start);
            while let Some(t) = stack.pop() {
                let k = map.neighbors4(t, &mut buf);
                for &nb in &buf[..k] {
                    if map.is_water(nb) && body[nb as usize] == NONE {
                        body[nb as usize] = next;
                        stack.push(nb);
                    }
                }
            }
            next += 1;
        }
        let coarse_w = map.width().div_ceil(CELL);
        let coarse_h = map.height().div_ceil(CELL);
        let mut counts = vec![0u32; (coarse_w * coarse_h) as usize];
        for t in 0..map.num_tiles() {
            if map.is_water(t) {
                counts[((map.y(t) / CELL) * coarse_w + map.x(t) / CELL) as usize] += 1;
            }
        }
        let coarse_water: Vec<bool> = counts.iter().map(|&c| c > 0).collect();
        Self {
            body,
            gen: 0,
            seen: vec![0; n],
            came_from: vec![NONE; n],
            cost: vec![0; n],
            coarse_w,
            coarse_h,
            corridor: vec![false; coarse_water.len()],
            coarse_water,
        }
    }

    fn cell_of(&self, map: &GameMap, t: TileRef) -> usize {
        ((map.y(t) / CELL) * self.coarse_w + map.x(t) / CELL) as usize
    }

    /// A* over coarse cells (8 directions) from the cell of `src` to the cell of `dst`.
    fn coarse_route(&self, map: &GameMap, src: TileRef, dst: TileRef) -> Option<Vec<usize>> {
        let (cw, ch) = (self.coarse_w as i64, self.coarse_h as i64);
        let start = self.cell_of(map, src);
        let goal = self.cell_of(map, dst);
        let (gx, gy) = ((goal as i64) % cw, (goal as i64) / cw);
        let h = |c: usize| {
            let dx = ((c as i64) % cw - gx).unsigned_abs();
            let dy = ((c as i64) / cw - gy).unsigned_abs();
            let (lo, hi) = if dx < dy { (dx, dy) } else { (dy, dx) };
            hi * 10 + lo * 4
        };
        let n = self.coarse_water.len();
        let mut cost = vec![u64::MAX; n];
        let mut from = vec![usize::MAX; n];
        let mut open = BinaryHeap::new();
        cost[start] = 0;
        open.push(Reverse((h(start), start)));
        while let Some(Reverse((_, c))) = open.pop() {
            if c == goal {
                let mut route = vec![c];
                let mut cur = c;
                while from[cur] != usize::MAX {
                    cur = from[cur];
                    route.push(cur);
                }
                route.reverse();
                return Some(route);
            }
            let (x, y) = ((c as i64) % cw, (c as i64) / cw);
            for (dx, dy, step) in [
                (1, 0, 10),
                (-1, 0, 10),
                (0, 1, 10),
                (0, -1, 10),
                (1, 1, 14),
                (1, -1, 14),
                (-1, 1, 14),
                (-1, -1, 14),
            ] {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= cw || ny >= ch {
                    continue;
                }
                let nb = (ny * cw + nx) as usize;
                if !self.coarse_water[nb] && nb != goal {
                    continue;
                }
                let c2 = cost[c] + step;
                if c2 < cost[nb] {
                    cost[nb] = c2;
                    from[nb] = c;
                    open.push(Reverse((c2 + h(nb), nb)));
                }
            }
        }
        None
    }

    /// Water body of a water tile.
    pub fn body_of_water(&self, t: TileRef) -> Option<u32> {
        let b = self.body[t as usize];
        (b != NONE).then_some(b)
    }

    /// Water bodies a tile touches: itself if water, its water neighbours if land (shore).
    pub fn bodies_at(&self, map: &GameMap, t: TileRef) -> [Option<u32>; 4] {
        if map.is_water(t) {
            return [self.body_of_water(t), None, None, None];
        }
        let mut out = [None; 4];
        let mut buf = [0; 4];
        let k = map.neighbors4(t, &mut buf);
        for (i, &nb) in buf[..k].iter().enumerate() {
            out[i] = self.body_of_water(nb);
        }
        out
    }

    /// True if both tiles touch a common water body.
    pub fn share_body(&self, map: &GameMap, a: TileRef, b: TileRef) -> bool {
        let ba = self.bodies_at(map, a);
        let bb = self.bodies_at(map, b);
        ba.iter()
            .flatten()
            .any(|x| bb.iter().flatten().any(|y| x == y))
    }

    /// Route for a ship from `src` (land shore or water) to `dst` (land shore or water): the
    /// water tiles in between, then `dst`. `None` if no route.
    pub fn find_path(&mut self, map: &GameMap, src: TileRef, dst: TileRef) -> Option<Vec<TileRef>> {
        if src == dst {
            return Some(vec![dst]);
        }
        if map.manhattan_dist(src, dst) > DIRECT_LIMIT {
            if let Some(p) = self.waypoint_path(map, src, dst) {
                return Some(p);
            }
        }
        self.search(map, src, dst, false)
    }

    /// Follows the coarse route: short tile searches between water tiles near every 4th coarse
    /// cell, joined. `None` if any leg fails (the caller then searches the whole sea).
    fn waypoint_path(&mut self, map: &GameMap, src: TileRef, dst: TileRef) -> Option<Vec<TileRef>> {
        let route = self.coarse_route(map, src, dst)?;
        let cw = self.coarse_w;
        let mut waypoints: Vec<TileRef> = Vec::new();
        for (i, &c) in route.iter().enumerate() {
            if i == 0 || i % 4 != 0 || i + 2 >= route.len() {
                continue;
            }
            // The water tile in this cell nearest its centre.
            let (cx, cy) = (
                (c as u32 % cw) * CELL + CELL / 2,
                (c as u32 / cw) * CELL + CELL / 2,
            );
            let mut best: Option<(u32, TileRef)> = None;
            for y in (cy.saturating_sub(CELL / 2))..(cy + CELL / 2).min(map.height()) {
                for x in (cx.saturating_sub(CELL / 2))..(cx + CELL / 2).min(map.width()) {
                    let t = y * map.width() + x;
                    if map.is_water(t) {
                        let d = x.abs_diff(cx) + y.abs_diff(cy);
                        if best.is_none_or(|(bd, _)| d < bd) {
                            best = Some((d, t));
                        }
                    }
                }
            }
            if let Some((_, t)) = best {
                waypoints.push(t);
            }
        }
        waypoints.push(dst);
        let mut path: Vec<TileRef> = Vec::new();
        let mut from = src;
        for wp in waypoints {
            let leg = self.search(map, from, wp, false)?;
            path.extend(
                leg.iter()
                    .copied()
                    .skip(usize::from(!path.is_empty() && leg.first() == path.last())),
            );
            from = wp;
        }
        Some(path)
    }

    /// Weighted A* over water tiles; with `in_corridor`, only tiles in marked coarse cells.
    fn search(
        &mut self,
        map: &GameMap,
        src: TileRef,
        dst: TileRef,
        in_corridor: bool,
    ) -> Option<Vec<TileRef>> {
        let w = map.width() as i64;
        let goal_ok = |t: TileRef| -> bool {
            // Arrived next to dst (dst itself is land) or at dst (water).
            t == dst || (map.is_land(dst) && map.manhattan_dist(t, dst) == 1)
        };
        let (dx_goal, dy_goal) = (map.x(dst) as i64, map.y(dst) as i64);
        let h = |t: TileRef| -> u64 {
            let dx = (map.x(t) as i64 - dx_goal).unsigned_abs();
            let dy = (map.y(t) as i64 - dy_goal).unsigned_abs();
            let (lo, hi) = if dx < dy { (dx, dy) } else { (dy, dx) };
            // octile distance x 10 (diagonal 14)
            (hi * 10 + lo * 4) * HEURISTIC_WEIGHT
        };
        self.gen = self.gen.wrapping_add(1);
        if self.gen == 0 {
            self.seen.fill(0);
            self.gen = 1;
        }
        let gen = self.gen;
        let mut open: BinaryHeap<Reverse<(u64, TileRef)>> = BinaryHeap::new();
        // Start: src if water, otherwise every water neighbour of src.
        let mut starts: Vec<TileRef> = Vec::new();
        if map.is_water(src) {
            starts.push(src);
        } else {
            let mut buf = [0; 4];
            let k = map.neighbors4(src, &mut buf);
            starts.extend(buf[..k].iter().copied().filter(|&t| map.is_water(t)));
        }
        for &s in &starts {
            self.seen[s as usize] = gen;
            self.cost[s as usize] = 10;
            self.came_from[s as usize] = NONE;
            open.push(Reverse((10 + h(s), s)));
        }
        let mut expansions = 0;
        while let Some(Reverse((_, t))) = open.pop() {
            if goal_ok(t) {
                let mut path = vec![t];
                let mut cur = t;
                while self.came_from[cur as usize] != NONE {
                    cur = self.came_from[cur as usize];
                    path.push(cur);
                }
                path.reverse();
                if path.last() != Some(&dst) {
                    path.push(dst);
                }
                return Some(path);
            }
            expansions += 1;
            if expansions > MAX_EXPANSIONS {
                return None;
            }
            let (x, y) = (map.x(t) as i64, map.y(t) as i64);
            let base = self.cost[t as usize];
            for (ddx, ddy, step) in [
                (1, 0, 10u32),
                (-1, 0, 10),
                (0, 1, 10),
                (0, -1, 10),
                (1, 1, 14),
                (1, -1, 14),
                (-1, 1, 14),
                (-1, -1, 14),
            ] {
                let (nx, ny) = (x + ddx, y + ddy);
                let Some(nb) = map.tile(nx, ny) else { continue };
                if map.is_land(nb) && nb != dst {
                    continue;
                }
                if in_corridor && !self.corridor[self.cell_of(map, nb)] {
                    continue;
                }
                if nb == dst && map.is_land(dst) {
                    // Land destination is entered only orthogonally from an adjacent water tile.
                    continue;
                }
                if ddx != 0 && ddy != 0 {
                    // No corner cutting between two land tiles.
                    let a = (y * w + nx) as TileRef;
                    let b = (ny * w + x) as TileRef;
                    if map.is_land(a) || map.is_land(b) {
                        continue;
                    }
                }
                let c = base + step;
                if self.seen[nb as usize] != gen || c < self.cost[nb as usize] {
                    self.seen[nb as usize] = gen;
                    self.cost[nb as usize] = c;
                    self.came_from[nb as usize] = t;
                    open.push(Reverse((c as u64 + h(nb), nb)));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine_lib::game_map::terrain_byte;

    /// 20x10 map: land columns 0-3 and 16-19, water between; a land wall at x=10 except a gap at
    /// y=8; a separate lake inside the left land at (1,1).
    fn map() -> GameMap {
        let (w, h) = (20u32, 10u32);
        let mut t = vec![terrain_byte(false, false, true, 0); (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                if x <= 3 || x >= 16 || (x == 10 && y != 8) {
                    t[i] = terrain_byte(true, false, false, 1);
                }
            }
        }
        t[(w + 1) as usize] = terrain_byte(false, false, false, 0); // lake
        GameMap::new(w, h, t).unwrap()
    }

    #[test]
    fn bodies_are_separate() {
        let m = map();
        let w = WaterInfo::new(&m);
        let sea = w.body_of_water(m.tile(5, 5).unwrap()).unwrap();
        let lake = w.body_of_water(m.tile(1, 1).unwrap()).unwrap();
        assert_ne!(sea, lake);
        assert_eq!(
            w.body_of_water(m.tile(14, 5).unwrap()),
            Some(sea),
            "connected through the gap"
        );
        assert!(w.share_body(&m, m.tile(3, 5).unwrap(), m.tile(16, 2).unwrap()));
        assert!(!w.share_body(&m, m.tile(0, 0).unwrap(), m.tile(16, 2).unwrap()));
    }

    #[test]
    fn path_goes_through_the_gap_and_stays_on_water() {
        let m = map();
        let mut w = WaterInfo::new(&m);
        let src = m.tile(3, 1).unwrap();
        let dst = m.tile(16, 1).unwrap();
        let p = w.find_path(&m, src, dst).unwrap();
        assert_eq!(*p.last().unwrap(), dst);
        assert!(p[..p.len() - 1].iter().all(|&t| m.is_water(t)));
        assert!(
            p.contains(&m.tile(10, 8).unwrap()),
            "must pass the only gap"
        );
        for pair in p.windows(2) {
            let d = m.x(pair[0]).abs_diff(m.x(pair[1])) + m.y(pair[0]).abs_diff(m.y(pair[1]));
            assert!(d == 1 || d == 2, "steps are to neighbours");
        }
    }

    #[test]
    fn no_path_into_a_lake() {
        let m = map();
        let mut w = WaterInfo::new(&m);
        assert!(w
            .find_path(&m, m.tile(3, 5).unwrap(), m.tile(1, 1).unwrap())
            .is_none());
    }
}

#[cfg(test)]
mod timing {
    use super::*;
    use crate::engine_lib::terrain::{generate, TerrainParams};

    /// Run with `cargo test --release -- --ignored water_route_timing --nocapture`.
    #[test]
    #[ignore]
    fn water_route_timing() {
        let map = generate(&TerrainParams::new(1600, 1000), 3).unwrap();
        let mut w = WaterInfo::new(&map);
        let shores: Vec<TileRef> = (0..map.num_tiles()).filter(|&t| map.is_shore(t)).collect();
        let mut worst = 0.0f64;
        for i in 0..40 {
            let a = shores[(i * 7919) % shores.len()];
            let b = shores[(i * 104_729 + 17) % shores.len()];
            if !w.share_body(&map, a, b) {
                continue;
            }
            let t0 = std::time::Instant::now();
            let p = w.find_path(&map, a, b);
            let ms = t0.elapsed().as_secs_f64() * 1e3;
            worst = worst.max(ms);
            println!("route {i}: {:.2} ms, {:?} steps", ms, p.map(|p| p.len()));
        }
        println!("worst {worst:.2} ms");
    }
}
