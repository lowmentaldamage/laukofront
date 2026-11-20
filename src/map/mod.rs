//! The territory map: a raster of cells, each belonging to one territory, plus the
//! territory adjacency graph. Generated deterministically from a seed.
//! See `docs/MAP_GENERATION.md` for the algorithm.

use std::collections::VecDeque;

use crate::config::MapSizeConfig;
use crate::core::{GameError, Rng, TerritoryId};

#[derive(Debug, Clone)]
pub struct Map {
    width: u16,
    height: u16,
    /// `width * height` entries, row-major: territory index of each cell.
    cells: Vec<u16>,
    /// Seed point of each territory (cell coordinates).
    seeds: Vec<(u16, u16)>,
    /// Cell count of each territory.
    area: Vec<u32>,
    /// Sum of x and y over each territory's cells (for the centre of mass).
    centroid_sum: Vec<(u64, u64)>,
    /// CSR adjacency: neighbours of `t` are `neighbor_list[neighbor_offsets[t]..neighbor_offsets[t + 1]]`.
    neighbor_offsets: Vec<u32>,
    neighbor_list: Vec<u16>,
}

impl Map {
    /// Generates a map. Same `size`, `candidates_per_point` and `seed` always give the same map.
    pub fn generate(
        size: &MapSizeConfig,
        candidates_per_point: u32,
        seed: u64,
    ) -> Result<Self, GameError> {
        let w = size.width as usize;
        let h = size.height as usize;
        let n = size.territories as usize;
        if w < 2 || h < 2 {
            return Err(GameError::Setup("map must be at least 2x2 cells".into()));
        }
        if n < 1 || n > w * h {
            return Err(GameError::Setup(format!(
                "cannot place {n} territories on {w}x{h} cells"
            )));
        }

        let mut rng = Rng::new(seed);
        let seeds = place_seeds(w, h, n, candidates_per_point.max(1), &mut rng);
        let mut cells = assign_cells(w, h, &seeds);
        repair_connectivity(w, h, &seeds, &mut cells);

        let mut area = vec![0u32; n];
        let mut centroid_sum = vec![(0u64, 0u64); n];
        let mut pairs: Vec<(u16, u16)> = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let t = cells[y * w + x];
                area[t as usize] += 1;
                let cs = &mut centroid_sum[t as usize];
                cs.0 += x as u64;
                cs.1 += y as u64;
                if x + 1 < w {
                    push_pair(&mut pairs, t, cells[y * w + x + 1]);
                }
                if y + 1 < h {
                    push_pair(&mut pairs, t, cells[(y + 1) * w + x]);
                }
            }
        }
        pairs.sort_unstable();
        pairs.dedup();

        let mut degree = vec![0u32; n];
        for &(a, b) in &pairs {
            degree[a as usize] += 1;
            degree[b as usize] += 1;
        }
        let mut neighbor_offsets = Vec::with_capacity(n + 1);
        let mut acc = 0u32;
        neighbor_offsets.push(0);
        for d in &degree {
            acc += d;
            neighbor_offsets.push(acc);
        }
        let mut fill: Vec<u32> = neighbor_offsets[..n].to_vec();
        let mut neighbor_list = vec![0u16; acc as usize];
        // Pairs are sorted by (a, b) with a < b, so each list is filled in ascending order.
        for &(a, b) in &pairs {
            neighbor_list[fill[a as usize] as usize] = b;
            fill[a as usize] += 1;
        }
        for &(a, b) in &pairs {
            neighbor_list[fill[b as usize] as usize] = a;
            fill[b as usize] += 1;
        }
        for t in 0..n {
            let s = neighbor_offsets[t] as usize;
            let e = neighbor_offsets[t + 1] as usize;
            neighbor_list[s..e].sort_unstable();
        }

        Ok(Self {
            width: size.width,
            height: size.height,
            cells,
            seeds,
            area,
            centroid_sum,
            neighbor_offsets,
            neighbor_list,
        })
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn territory_count(&self) -> usize {
        self.seeds.len()
    }

    /// All territory ids, in order.
    pub fn territory_ids(&self) -> impl Iterator<Item = TerritoryId> {
        (0..self.seeds.len() as u16).map(TerritoryId)
    }

    /// Row-major cell raster (territory index per cell).
    pub fn cells(&self) -> &[u16] {
        &self.cells
    }

    /// Territory at cell `(x, y)`, or `None` outside the map.
    pub fn territory_at(&self, x: i32, y: i32) -> Option<TerritoryId> {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return None;
        }
        Some(TerritoryId(
            self.cells[y as usize * self.width as usize + x as usize],
        ))
    }

    /// Neighbours of `t` as raw indices: sorted, no duplicates, never contains `t`.
    pub fn neighbors(&self, t: TerritoryId) -> &[u16] {
        let i = t.index();
        let s = self.neighbor_offsets[i] as usize;
        let e = self.neighbor_offsets[i + 1] as usize;
        &self.neighbor_list[s..e]
    }

    /// Neighbours of `t` as ids.
    pub fn neighbor_ids(&self, t: TerritoryId) -> impl Iterator<Item = TerritoryId> + '_ {
        self.neighbors(t).iter().map(|&n| TerritoryId(n))
    }

    pub fn is_neighbor(&self, a: TerritoryId, b: TerritoryId) -> bool {
        a.index() < self.territory_count() && self.neighbors(a).binary_search(&b.0).is_ok()
    }

    pub fn contains(&self, t: TerritoryId) -> bool {
        t.index() < self.territory_count()
    }

    pub fn seed_point(&self, t: TerritoryId) -> (u16, u16) {
        self.seeds[t.index()]
    }

    pub fn area(&self, t: TerritoryId) -> u32 {
        self.area[t.index()]
    }

    /// Centre of mass of the territory's cells, in cell units (cell centres at +0.5).
    pub fn centroid(&self, t: TerritoryId) -> (f32, f32) {
        let a = self.area[t.index()].max(1) as f64;
        let (sx, sy) = self.centroid_sum[t.index()];
        ((sx as f64 / a + 0.5) as f32, (sy as f64 / a + 0.5) as f32)
    }

    /// FNV-1a hash over the raster and adjacency. Equal maps have equal hashes.
    pub fn content_hash(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |v: u64| {
            for b in v.to_le_bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        };
        eat(self.width as u64);
        eat(self.height as u64);
        for &c in &self.cells {
            eat(c as u64);
        }
        for &o in &self.neighbor_offsets {
            eat(o as u64);
        }
        for &n in &self.neighbor_list {
            eat(n as u64);
        }
        h
    }
}

fn push_pair(pairs: &mut Vec<(u16, u16)>, a: u16, b: u16) {
    if a != b {
        pairs.push((a.min(b), a.max(b)));
    }
}

#[inline]
fn dist2(a: (u16, u16), b: (u16, u16)) -> u64 {
    let dx = a.0 as i64 - b.0 as i64;
    let dy = a.1 as i64 - b.1 as i64;
    (dx * dx + dy * dy) as u64
}

/// Best-candidate sampling: each new point is the candidate farthest from all existing points.
/// Points are always on distinct cells.
fn place_seeds(w: usize, h: usize, n: usize, candidates: u32, rng: &mut Rng) -> Vec<(u16, u16)> {
    let mut used = vec![false; w * h];
    let mut seeds: Vec<(u16, u16)> = Vec::with_capacity(n);
    while seeds.len() < n {
        let mut best: Option<((u16, u16), u64)> = None;
        for _ in 0..candidates {
            let p = (rng.below(w as u32) as u16, rng.below(h as u32) as u16);
            if used[p.1 as usize * w + p.0 as usize] {
                continue;
            }
            let d = seeds.iter().map(|&s| dist2(s, p)).min().unwrap_or(u64::MAX);
            if best.is_none_or(|(_, bd)| d > bd) {
                best = Some((p, d));
            }
        }
        let p = match best {
            Some((p, _)) => p,
            None => {
                // Every candidate hit a used cell (very dense map): take the next free cell
                // after a random start. A free cell exists because n <= w * h.
                let start = rng.below((w * h) as u32) as usize;
                let mut i = start;
                while used[i] {
                    i = (i + 1) % (w * h);
                }
                ((i % w) as u16, (i / w) as u16)
            }
        };
        used[p.1 as usize * w + p.0 as usize] = true;
        seeds.push(p);
    }
    seeds
}

/// Each cell goes to the nearest seed (integer squared distance); ties go to the lower id.
fn assign_cells(w: usize, h: usize, seeds: &[(u16, u16)]) -> Vec<u16> {
    let mut cells = vec![0u16; w * h];
    for y in 0..h {
        for x in 0..w {
            let p = (x as u16, y as u16);
            let mut best = 0usize;
            let mut best_d = u64::MAX;
            for (i, &s) in seeds.iter().enumerate() {
                let d = dist2(s, p);
                if d < best_d {
                    best_d = d;
                    best = i;
                }
            }
            cells[y * w + x] = best as u16;
        }
    }
    cells
}

/// Makes every territory a single 4-connected blob. Cells not reachable from their own seed
/// (rare tie artefacts) are handed to an adjacent territory that does reach them.
fn repair_connectivity(w: usize, h: usize, seeds: &[(u16, u16)], cells: &mut [u16]) {
    let mut reached = vec![false; w * h];
    let mut queue: VecDeque<usize> = VecDeque::with_capacity(w * h);
    for &(sx, sy) in seeds {
        let i = sy as usize * w + sx as usize;
        reached[i] = true;
        queue.push_back(i);
    }
    while let Some(i) = queue.pop_front() {
        let id = cells[i];
        for nb in neighbors4(i, w, h).into_iter().flatten() {
            if !reached[nb] && cells[nb] == id {
                reached[nb] = true;
                queue.push_back(nb);
            }
        }
    }
    loop {
        let mut pending = false;
        for i in 0..w * h {
            if reached[i] {
                continue;
            }
            let donor = neighbors4(i, w, h)
                .into_iter()
                .flatten()
                .find(|&nb| reached[nb]);
            match donor {
                Some(nb) => {
                    cells[i] = cells[nb];
                    reached[i] = true;
                }
                None => pending = true,
            }
        }
        if !pending {
            break;
        }
    }
}

#[inline]
fn neighbors4(i: usize, w: usize, h: usize) -> [Option<usize>; 4] {
    let x = i % w;
    let y = i / w;
    [
        (x > 0).then(|| i - 1),
        (x + 1 < w).then(|| i + 1),
        (y > 0).then(|| i - w),
        (y + 1 < h).then(|| i + w),
    ]
}
