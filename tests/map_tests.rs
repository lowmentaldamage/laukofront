use std::collections::VecDeque;

use openfront_clone::config::{MapSize, MapSizeConfig};
use openfront_clone::core::TerritoryId;
use openfront_clone::map::Map;

mod common;

fn gen(size: MapSize, seed: u64) -> Map {
    let cfg = common::config();
    Map::generate(cfg.map.size(size), cfg.map.candidates_per_point, seed).unwrap()
}

#[test]
fn neighbors_are_symmetric_on_all_sizes() {
    for size in MapSize::ALL {
        for seed in [1, 2, 42, 0xDEAD_BEEF] {
            let m = gen(size, seed);
            for a in m.territory_ids() {
                for b in m.neighbor_ids(a) {
                    assert!(
                        m.is_neighbor(b, a),
                        "{size:?} seed {seed}: {a:?}->{b:?} not symmetric"
                    );
                }
            }
        }
    }
}

#[test]
fn neighbor_lists_are_sorted_unique_and_exclude_self() {
    let m = gen(MapSize::Medium, 7);
    for t in m.territory_ids() {
        let n = m.neighbors(t);
        assert!(n.windows(2).all(|w| w[0] < w[1]), "{t:?} not sorted/unique");
        assert!(!n.contains(&t.0), "{t:?} lists itself");
        assert!(!n.is_empty(), "{t:?} has no neighbours");
    }
}

#[test]
fn same_seed_gives_identical_map() {
    for size in MapSize::ALL {
        let a = gen(size, 12345);
        let b = gen(size, 12345);
        assert_eq!(a.cells(), b.cells());
        assert_eq!(a.content_hash(), b.content_hash());
    }
}

#[test]
fn different_seed_gives_different_map() {
    assert_ne!(
        gen(MapSize::Small, 1).content_hash(),
        gen(MapSize::Small, 2).content_hash()
    );
}

#[test]
fn territory_counts_match_config() {
    let cfg = common::config();
    for size in MapSize::ALL {
        let m = gen(size, 3);
        let s = cfg.map.size(size);
        assert_eq!(m.territory_count(), s.territories as usize);
        assert_eq!(m.cells().len(), s.width as usize * s.height as usize);
        let total: u32 = m.territory_ids().map(|t| m.area(t)).sum();
        assert_eq!(total as usize, m.cells().len());
    }
}

/// Every territory is one 4-connected blob that contains its seed point.
#[test]
fn territories_are_connected() {
    for size in MapSize::ALL {
        for seed in [5, 99] {
            let m = gen(size, seed);
            let (w, h) = (m.width() as i32, m.height() as i32);
            for t in m.territory_ids() {
                let (sx, sy) = m.seed_point(t);
                assert_eq!(m.territory_at(sx as i32, sy as i32), Some(t));
                let mut seen = vec![false; m.cells().len()];
                let mut q = VecDeque::from([(sx as i32, sy as i32)]);
                seen[(sy as i32 * w + sx as i32) as usize] = true;
                let mut count = 0u32;
                while let Some((x, y)) = q.pop_front() {
                    count += 1;
                    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        let (nx, ny) = (x + dx, y + dy);
                        if nx < 0 || ny < 0 || nx >= w || ny >= h {
                            continue;
                        }
                        let i = (ny * w + nx) as usize;
                        if !seen[i] && m.territory_at(nx, ny) == Some(t) {
                            seen[i] = true;
                            q.push_back((nx, ny));
                        }
                    }
                }
                assert_eq!(count, m.area(t), "{size:?} seed {seed}: {t:?} is split");
            }
        }
    }
}

/// The whole graph is connected, so every territory can be reached by attacks.
#[test]
fn territory_graph_is_connected() {
    let m = gen(MapSize::Large, 11);
    let mut seen = vec![false; m.territory_count()];
    let mut q = VecDeque::from([TerritoryId(0)]);
    seen[0] = true;
    while let Some(t) = q.pop_front() {
        for n in m.neighbor_ids(t) {
            if !seen[n.index()] {
                seen[n.index()] = true;
                q.push_back(n);
            }
        }
    }
    assert!(seen.iter().all(|&s| s));
}

#[test]
fn territory_at_outside_map_is_none() {
    let m = gen(MapSize::Small, 1);
    assert_eq!(m.territory_at(-1, 0), None);
    assert_eq!(m.territory_at(0, m.height() as i32), None);
}

#[test]
fn dense_map_fills_every_cell() {
    let size = MapSizeConfig {
        width: 4,
        height: 3,
        territories: 12,
    };
    let m = Map::generate(&size, 10, 9).unwrap();
    for t in m.territory_ids() {
        assert_eq!(m.area(t), 1);
    }
}

#[test]
fn too_many_territories_is_an_error() {
    let size = MapSizeConfig {
        width: 4,
        height: 3,
        territories: 13,
    };
    assert!(Map::generate(&size, 10, 9).is_err());
}
