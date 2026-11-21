//! Builds the real-Earth map masks in `assets/maps/` from Natural Earth shapefiles (public
//! domain, https://www.naturalearthdata.com). The shapefiles are not part of the repository:
//!
//!     cargo run --release --bin earth_import -- --land ne_50m_land.shp --lakes ne_50m_lakes.shp
//!
//! Every polygon ring is projected (equirectangular, per map bounds in `EarthMap::region`) and
//! filled by even-odd scanlines at tile centres; lakes are cut out of the land.

use std::process::ExitCode;

use laukofront::engine_lib::earth::{encode, EarthMap, EarthRegion, Mask};

/// Polygon rings in longitude / latitude.
type Rings = Vec<Vec<(f64, f64)>>;

/// Reads the rings of every polygon (shape type 5) in a .shp file.
fn read_shp(path: &str) -> Result<Rings, String> {
    let b = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    if b.len() < 100 || i32::from_be_bytes([b[0], b[1], b[2], b[3]]) != 9994 {
        return Err(format!("{path}: not a shapefile"));
    }
    let le_i32 = |i: usize| i32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    let le_f64 = |i: usize| {
        let mut a = [0u8; 8];
        a.copy_from_slice(&b[i..i + 8]);
        f64::from_le_bytes(a)
    };
    let mut rings = Vec::new();
    let mut i = 100;
    while i + 8 <= b.len() {
        let len = i32::from_be_bytes([b[i + 4], b[i + 5], b[i + 6], b[i + 7]]) as usize * 2;
        let c = i + 8;
        if c + len > b.len() {
            return Err(format!("{path}: truncated record"));
        }
        if len >= 44 && le_i32(c) == 5 {
            let parts = le_i32(c + 36) as usize;
            let points = le_i32(c + 40) as usize;
            let pstart = c + 44;
            let xy = pstart + parts * 4;
            if xy + points * 16 > c + len {
                return Err(format!("{path}: bad polygon"));
            }
            for p in 0..parts {
                let a = le_i32(pstart + p * 4) as usize;
                let z = if p + 1 < parts {
                    le_i32(pstart + (p + 1) * 4) as usize
                } else {
                    points
                };
                let ring: Vec<(f64, f64)> = (a..z)
                    .map(|k| (le_f64(xy + k * 16), le_f64(xy + k * 16 + 8)))
                    .collect();
                rings.push(ring);
            }
        }
        i = c + len;
    }
    Ok(rings)
}

/// Even-odd fill of `rings` over the map grid (tile centres).
fn rasterize(rings: &Rings, r: &EarthRegion) -> Vec<bool> {
    let (w, h) = (r.width as usize, r.height as usize);
    let px = |lon: f64| (lon - r.lon_west) / (r.lon_east - r.lon_west) * w as f64;
    let py = |lat: f64| (r.lat_north - lat) / (r.lat_north - r.lat_south) * h as f64;
    // Edges in pixel space, bucketed by the rows they cross.
    let mut by_row: Vec<Vec<(f64, f64, f64, f64)>> = vec![Vec::new(); h];
    for ring in rings {
        for k in 0..ring.len() {
            let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
            let (x0, y0, x1, y1) = (px(a.0), py(a.1), px(b.0), py(b.1));
            if y0 == y1 {
                continue;
            }
            let (lo, hi) = (y0.min(y1), y0.max(y1));
            let first = (lo - 0.5).ceil().max(0.0) as usize;
            let last = (((hi - 0.5).ceil() as i64).min(h as i64)).max(0) as usize;
            for edges in by_row.iter_mut().take(last).skip(first) {
                edges.push((x0, y0, x1, y1));
            }
        }
    }
    let mut out = vec![false; w * h];
    let mut xs: Vec<f64> = Vec::new();
    for (row, edges) in by_row.iter().enumerate() {
        let yc = row as f64 + 0.5;
        xs.clear();
        for &(x0, y0, x1, y1) in edges {
            if (y0 <= yc && yc < y1) || (y1 <= yc && yc < y0) {
                xs.push(x0 + (yc - y0) / (y1 - y0) * (x1 - x0));
            }
        }
        xs.sort_by(|a, b| a.total_cmp(b));
        for pair in xs.chunks(2) {
            if pair.len() < 2 {
                break;
            }
            let a = (pair[0] - 0.5).ceil().max(0.0) as usize;
            let b = ((pair[1] - 0.5).ceil().max(0.0) as usize).min(w);
            for x in a..b {
                out[row * w + x] = !out[row * w + x];
            }
        }
    }
    out
}

fn run() -> Result<(), String> {
    let (mut land_path, mut lakes_path, mut out_dir) = (None, None, "assets/maps".to_string());
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let v = it.next().ok_or(format!("{a} needs a value"))?;
        match a.as_str() {
            "--land" => land_path = Some(v),
            "--lakes" => lakes_path = Some(v),
            "--out" => out_dir = v,
            o => return Err(format!("unknown argument {o}")),
        }
    }
    let land = read_shp(&land_path.ok_or("--land is required")?)?;
    let lakes = match lakes_path {
        Some(p) => read_shp(&p)?,
        None => Vec::new(),
    };
    println!("{} land rings, {} lake rings", land.len(), lakes.len());
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("{out_dir}: {e}"))?;
    for m in EarthMap::ALL {
        let r = m.region();
        let mut mask = rasterize(&land, &r);
        for (l, lake) in mask.iter_mut().zip(rasterize(&lakes, &r)) {
            if lake {
                *l = false;
            }
        }
        let share = mask.iter().filter(|&&l| l).count() as f64 / mask.len() as f64;
        let bytes = encode(&Mask {
            width: r.width,
            height: r.height,
            lat_north: r.lat_north as f32,
            lat_south: r.lat_south as f32,
            land: mask,
        });
        let path = format!("{out_dir}/{}.lkmap", m.id());
        std::fs::write(&path, &bytes).map_err(|e| format!("{path}: {e}"))?;
        println!(
            "{path}: {}x{}, land {:.1}%, {} bytes",
            r.width,
            r.height,
            share * 100.0,
            bytes.len()
        );
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("earth_import: {e}");
            ExitCode::FAILURE
        }
    }
}
