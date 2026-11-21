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

/// Water-body ids per tile, the coarse routing graph and reusable search buffers.
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
    /// Coarse node of each water tile: one node per connected piece of water inside an 8x8 cell
    /// (upstream's abstract graph).
    node_of: Vec<u32>,
    /// Per node: its cell and a representative water tile (nearest the piece's middle).
    nodes: Vec<(u32, TileRef)>,
    /// Per node: neighbouring nodes (pieces that touch across a cell edge) and step cost.
    edges: Vec<Vec<(u32, u32)>>,
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
        let cell_of = |t: TileRef| (map.y(t) / CELL) * coarse_w + map.x(t) / CELL;

        // Nodes: flood fill water inside each cell.
        let mut node_of = vec![NONE; n];
        let mut nodes: Vec<(u32, TileRef)> = Vec::new();
        let mut piece: Vec<TileRef> = Vec::new();
        for start in 0..map.num_tiles() {
            if body[start as usize] == NONE || node_of[start as usize] != NONE {
                continue;
            }
            let id = nodes.len() as u32;
            let cell = cell_of(start);
            piece.clear();
            node_of[start as usize] = id;
            stack.push(start);
            while let Some(t) = stack.pop() {
                piece.push(t);
                let k = map.neighbors4(t, &mut buf);
                for &nb in &buf[..k] {
                    if body[nb as usize] != NONE
                        && node_of[nb as usize] == NONE
                        && cell_of(nb) == cell
                    {
                        node_of[nb as usize] = id;
                        stack.push(nb);
                    }
                }
            }
            let (sx, sy) = piece.iter().fold((0u64, 0u64), |(a, b), &t| {
                (a + map.x(t) as u64, b + map.y(t) as u64)
            });
            let (cx, cy) = (sx / piece.len() as u64, sy / piece.len() as u64);
            let rep = piece
                .iter()
                .copied()
                .min_by_key(|&t| (map.x(t) as u64).abs_diff(cx) + (map.y(t) as u64).abs_diff(cy))
                .unwrap_or(start);
            nodes.push((cell, rep));
        }
        // Edges: pieces whose tiles touch across a cell edge (right and down neighbours).
        let mut edges: Vec<Vec<(u32, u32)>> = vec![Vec::new(); nodes.len()];
        for t in 0..map.num_tiles() {
            let a = node_of[t as usize];
            if a == NONE {
                continue;
            }
            let (x, y) = (map.x(t), map.y(t));
            for nb in [
                (x + 1 < map.width()).then(|| t + 1),
                (y + 1 < map.height()).then(|| t + map.width()),
            ]
            .into_iter()
            .flatten()
            {
                let b = node_of[nb as usize];
                if b == NONE || b == a || edges[a as usize].iter().any(|e| e.0 == b) {
                    continue;
                }
                let (ra, rb) = (nodes[a as usize].1, nodes[b as usize].1);
                let c = map.manhattan_dist(ra, rb) * 10;
                edges[a as usize].push((b, c));
                edges[b as usize].push((a, c));
            }
        }
        Self {
            body,
            gen: 0,
            seen: vec![0; n],
            came_from: vec![NONE; n],
            cost: vec![0; n],
            coarse_w,
            coarse_h,
            node_of,
            nodes,
            edges,
            corridor: vec![false; (coarse_w * coarse_h) as usize],
        }
    }

    fn cell_of(&self, map: &GameMap, t: TileRef) -> usize {
        ((map.y(t) / CELL) * self.coarse_w + map.x(t) / CELL) as usize
    }

    /// Coarse nodes a tile starts or ends in: its own if water, its water neighbours' if shore.
    fn nodes_at(&self, map: &GameMap, t: TileRef) -> Vec<u32> {
        if map.is_water(t) {
            return vec![self.node_of[t as usize]];
        }
        let mut buf = [0; 4];
        let k = map.neighbors4(t, &mut buf);
        buf[..k]
            .iter()
            .map(|&nb| self.node_of[nb as usize])
            .filter(|&x| x != NONE)
            .collect()
    }

    /// A* over the coarse graph from the pieces of water at `src` to those at `dst`.
    fn coarse_route(&self, map: &GameMap, src: TileRef, dst: TileRef) -> Option<Vec<u32>> {
        let starts = self.nodes_at(map, src);
        let goals = self.nodes_at(map, dst);
        if starts.is_empty() || goals.is_empty() {
            return None;
        }
        let (gx, gy) = (map.x(dst) as i64, map.y(dst) as i64);
        let h = |nd: u32| {
            let r = self.nodes[nd as usize].1;
            let dx = (map.x(r) as i64 - gx).unsigned_abs();
            let dy = (map.y(r) as i64 - gy).unsigned_abs();
            let (lo, hi) = if dx < dy { (dx, dy) } else { (dy, dx) };
            hi * 10 + lo * 4
        };
        let n = self.nodes.len();
        let mut cost = vec![u64::MAX; n];
        let mut from = vec![NONE; n];
        let mut open = BinaryHeap::new();
        for &s in &starts {
            cost[s as usize] = 0;
            open.push(Reverse((h(s), s)));
        }
        while let Some(Reverse((f, c))) = open.pop() {
            if f > cost[c as usize].saturating_add(h(c)) {
                continue;
            }
            if goals.contains(&c) {
                let mut route = vec![c];
                let mut cur = c;
                while from[cur as usize] != NONE {
                    cur = from[cur as usize];
                    route.push(cur);
                }
                route.reverse();
                return Some(route);
            }
            for &(nb, step) in &self.edges[c as usize] {
                let c2 = cost[c as usize] + step as u64;
                if c2 < cost[nb as usize] {
                    cost[nb as usize] = c2;
                    from[nb as usize] = c;
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
        if !self.share_body(map, src, dst) {
            return None;
        }
        if map.manhattan_dist(src, dst) > DIRECT_LIMIT {
            if let Some(route) = self.coarse_route(map, src, dst) {
                if let Some(p) = self.waypoint_path(map, src, dst, &route) {
                    return Some(p);
                }
                // A leg was blocked: search near the coarse route before the whole sea.
                self.mark_corridor(&route);
                if let Some(p) = self.search(map, src, dst, true) {
                    return Some(p);
                }
            }
        }
        self.search(map, src, dst, false)
    }

    /// Marks the coarse cells within 2 of the route's cells as the search corridor.
    fn mark_corridor(&mut self, route: &[u32]) {
        self.corridor.fill(false);
        let (cw, ch) = (self.coarse_w as i64, self.coarse_h as i64);
        for &nd in route {
            let c = self.nodes[nd as usize].0 as i64;
            let (x, y) = (c % cw, c / cw);
            for dy in -2..=2 {
                for dx in -2..=2 {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx >= 0 && ny >= 0 && nx < cw && ny < ch {
                        self.corridor[(ny * cw + nx) as usize] = true;
                    }
                }
            }
        }
    }

    /// Follows the coarse route: short tile searches between the representative tiles of every
    /// 3rd node, joined. `None` if any leg fails.
    fn waypoint_path(
        &mut self,
        map: &GameMap,
        src: TileRef,
        dst: TileRef,
        route: &[u32],
    ) -> Option<Vec<TileRef>> {
        let mut waypoints: Vec<TileRef> = route
            .iter()
            .enumerate()
            .filter(|&(i, _)| i > 0 && i % 3 == 0 && i + 2 < route.len())
            .map(|(_, &nd)| self.nodes[nd as usize].1)
            .collect();
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
        let map = match std::env::var("ROUTE_MAP")
            .ok()
            .and_then(|m| crate::engine_lib::earth::EarthMap::parse(&m))
        {
            Some(e) => e.build().unwrap(),
            None => generate(&TerrainParams::new(1600, 1000), 3).unwrap(),
        };
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
