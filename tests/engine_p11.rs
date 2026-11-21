//! Milestone P11: map styles, rivers, peaks, biomes, real-Earth maps.

use laukofront::config::{GameConfig, MapSize, GAME_CONFIG_PATH};
use laukofront::engine::{new_match, run_turn, MapChoice, MatchSetup};
use laukofront::engine_lib::earth::EarthMap;
use laukofront::engine_lib::game_map::{Biome, GameMap, TerrainType, TileRef};
use laukofront::engine_lib::terrain::{generate, MapStyle, TerrainParams};

fn styled(style: MapStyle, seed: i64) -> GameMap {
    let mut p = TerrainParams::new(600, 375);
    p.style = style;
    generate(&p, seed).unwrap()
}

/// Sizes of the 4-connected landmasses, biggest first.
fn landmasses(m: &GameMap) -> Vec<u32> {
    let mut seen = vec![false; m.num_tiles() as usize];
    let mut out = Vec::new();
    let mut buf = [0; 4];
    for s in 0..m.num_tiles() {
        if !m.is_land(s) || seen[s as usize] {
            continue;
        }
        let mut stack = vec![s];
        seen[s as usize] = true;
        let mut n = 0;
        while let Some(t) = stack.pop() {
            n += 1;
            let k = m.neighbors4(t, &mut buf);
            for &nb in &buf[..k] {
                if m.is_land(nb) && !seen[nb as usize] {
                    seen[nb as usize] = true;
                    stack.push(nb);
                }
            }
        }
        out.push(n);
    }
    out.sort_unstable_by(|a, b| b.cmp(a));
    out
}

/// Number of separate lakes.
fn lake_count(m: &GameMap) -> usize {
    let mut seen = vec![false; m.num_tiles() as usize];
    let mut lakes = 0;
    let mut buf = [0; 4];
    for s in 0..m.num_tiles() {
        if m.terrain_type(s) != TerrainType::Lake || seen[s as usize] {
            continue;
        }
        lakes += 1;
        let mut stack = vec![s];
        seen[s as usize] = true;
        while let Some(t) = stack.pop() {
            let k = m.neighbors4(t, &mut buf);
            for &nb in &buf[..k] {
                if m.terrain_type(nb) == TerrainType::Lake && !seen[nb as usize] {
                    seen[nb as usize] = true;
                    stack.push(nb);
                }
            }
        }
    }
    lakes
}

fn count(m: &GameMap, kind: TerrainType) -> usize {
    (0..m.num_tiles())
        .filter(|&t| m.terrain_type(t) == kind)
        .count()
}

#[test]
fn styles_shape_the_map() {
    for seed in [1, 2, 3] {
        let cont = styled(MapStyle::Continents, seed);
        let arch = styled(MapStyle::Archipelago, seed);
        let pang = styled(MapStyle::Pangaea, seed);
        let lakes = styled(MapStyle::Lakes, seed);
        let share = |m: &GameMap| m.num_land_tiles() as f64 / m.num_tiles() as f64;
        assert!(
            share(&arch) < share(&cont),
            "seed {seed}: archipelago has less land"
        );
        assert!(
            landmasses(&arch).len() > landmasses(&cont).len(),
            "seed {seed}: archipelago has more islands"
        );
        let pm = landmasses(&pang);
        let total: u32 = pm.iter().sum();
        assert!(
            pm[0] as f64 > total as f64 * 0.9,
            "seed {seed}: pangaea is one landmass ({pm:?})"
        );
        assert!(
            lake_count(&lakes) > lake_count(&cont) * 2 + 5,
            "seed {seed}: lakes style has lakes ({} vs {})",
            lake_count(&lakes),
            lake_count(&cont)
        );
    }
}

#[test]
fn peaks_are_impassable_and_not_counted() {
    let m = styled(MapStyle::Continents, 4);
    let peaks = count(&m, TerrainType::Impassable);
    assert!(peaks > 50, "only {peaks} peak tiles");
    let passable = (0..m.num_tiles())
        .filter(|&t| m.is_passable_land(t))
        .count();
    assert_eq!(m.num_land_tiles() as usize, passable);
    assert!((0..m.num_tiles()).all(|t| !m.is_impassable(t) || m.is_land(t)));
}

#[test]
fn rivers_reach_the_sea() {
    // A river is a strip of ocean reaching deep into a landmass: some ocean tiles have land on
    // both sides (left and right, or up and down).
    let m = styled(MapStyle::Pangaea, 5);
    let w = m.width() as i64;
    let narrow = (0..m.num_tiles())
        .filter(|&t| m.is_ocean(t))
        .filter(|&t| {
            let (x, y) = (m.x(t) as i64, m.y(t) as i64);
            let land = |dx: i64, dy: i64| m.tile(x + dx, y + dy).is_some_and(|n| m.is_land(n));
            (land(-1, 0) && land(1, 0)) || (land(0, -1) && land(0, 1))
        })
        .count();
    assert!(narrow > 100, "only {narrow} river tiles (w {w})");
}

#[test]
fn biomes_follow_latitude() {
    let m = styled(MapStyle::Continents, 6);
    let mut seen = std::collections::HashSet::new();
    for t in 0..m.num_tiles() {
        if m.is_land(t) {
            seen.insert(m.biome(t));
        }
    }
    assert!(seen.len() >= 4, "only {seen:?}");
    // Generated maps span 70 N to 70 S: no snow in the middle third.
    let h = m.height();
    for t in 0..m.num_tiles() {
        let y = m.y(t);
        if m.is_land(t) && y > h / 3 && y < 2 * h / 3 {
            assert_ne!(m.biome(t), Biome::Snow, "snow near the equator at row {y}");
        }
    }
}

/// The tile at a longitude and latitude of an Earth map.
fn at(e: EarthMap, m: &GameMap, lon: f64, lat: f64) -> TileRef {
    let r = e.region();
    let x = ((lon - r.lon_west) / (r.lon_east - r.lon_west) * r.width as f64) as i64;
    let y = ((r.lat_north - lat) / (r.lat_north - r.lat_south) * r.height as f64) as i64;
    m.tile(x, y).unwrap()
}

#[test]
fn earth_maps_put_land_and_sea_in_the_right_places() {
    let world = EarthMap::World.build().unwrap();
    let w = EarthMap::World;
    for (name, lon, lat) in [
        ("Sahara", 10.0, 23.0),
        ("Siberia", 100.0, 62.0),
        ("Brazil", -50.0, -10.0),
        ("Australia", 134.0, -25.0),
        ("Canada", -100.0, 55.0),
    ] {
        assert!(world.is_land(at(w, &world, lon, lat)), "{name} is not land");
    }
    for (name, lon, lat) in [
        ("Atlantic", -40.0, 30.0),
        ("Pacific", -150.0, 0.0),
        ("Indian Ocean", 80.0, -20.0),
    ] {
        assert!(
            world.is_ocean(at(w, &world, lon, lat)),
            "{name} is not ocean"
        );
    }
    let asia = EarthMap::Asia.build().unwrap();
    let caspian = at(EarthMap::Asia, &asia, 50.5, 42.0);
    assert!(
        !asia.is_land(caspian) && !asia.is_ocean(caspian),
        "the Caspian is a lake"
    );
    let uk = EarthMap::BritishIsles.build().unwrap();
    assert!(
        uk.is_land(at(EarthMap::BritishIsles, &uk, -1.5, 52.5)),
        "England"
    );
    assert!(
        uk.is_land(at(EarthMap::BritishIsles, &uk, -8.0, 53.3)),
        "Ireland"
    );
    assert!(
        uk.is_ocean(at(EarthMap::BritishIsles, &uk, -5.0, 50.0)),
        "the Channel approaches"
    );
    // Snow in the far north, none on the equator.
    assert_eq!(
        world.biome(at(w, &world, -40.0, 75.0)),
        Biome::Snow,
        "Greenland"
    );
    assert_ne!(world.biome(at(w, &world, 20.0, 0.0)), Biome::Snow);
}

#[test]
fn earth_maps_are_the_same_every_time() {
    let a = EarthMap::Europe.build().unwrap();
    let b = EarthMap::Europe.build().unwrap();
    assert_eq!(a.terrain_bytes(), b.terrain_bytes());
}

#[test]
fn every_map_name_parses_and_plays() {
    let cfg = GameConfig::load(GAME_CONFIG_PATH).unwrap();
    for choice in MapChoice::all() {
        assert_eq!(MapChoice::parse(choice.name()), Some(choice));
        let mut setup = MatchSetup::bots_only(MapSize::Small, 3, 6);
        setup.nations = 2;
        setup.map = choice;
        let mut g = new_match(cfg.clone(), setup).unwrap();
        for _ in 0..400 {
            run_turn(&mut g, &[]);
        }
        let alive = g.alive_players().count();
        assert!(
            alive >= 7,
            "{}: only {alive} players alive after spawning",
            choice.name()
        );
    }
    assert_eq!(MapChoice::parse("atlantis"), None);
}

#[test]
fn ships_can_cross_the_atlantic() {
    let cfg = GameConfig::load(GAME_CONFIG_PATH).unwrap();
    let mut setup = MatchSetup::bots_only(MapSize::Small, 1, 0);
    setup.map = MapChoice::Earth(EarthMap::World);
    let mut g = new_match(cfg, setup).unwrap();
    let w = EarthMap::World;
    let map = g.map().clone();
    // Nearest ocean tiles to Lisbon and New York.
    let near_ocean = |lon: f64, lat: f64| {
        let c = at(w, &map, lon, lat);
        (0..map.num_tiles())
            .filter(|&t| map.is_ocean(t) && map.manhattan_dist(t, c) < 30)
            .min_by_key(|&t| map.manhattan_dist(t, c))
            .unwrap()
    };
    let (a, b) = (near_ocean(-9.5, 38.7), near_ocean(-73.5, 40.5));
    let t0 = std::time::Instant::now();
    let p = g
        .find_water_path(a, b)
        .expect("a route across the Atlantic");
    let ms = t0.elapsed().as_secs_f64() * 1e3;
    assert!(p.iter().all(|&t| map.is_water(t)));
    assert!(p.len() < 700, "route too long: {}", p.len());
    assert!(ms < 50.0, "route took {ms:.1} ms");
}
