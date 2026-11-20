//! Writes a generated map as a BMP image and prints how long generation took.
//!
//!     cargo run --release --bin map_preview -- --width 1200 --height 750 --seed 7 --out map.bmp

use std::process::ExitCode;
use std::time::Instant;

use laukofront::engine_lib::game_map::{GameMap, TerrainType};
use laukofront::engine_lib::terrain::{generate, TerrainParams};

fn color(m: &GameMap, t: u32) -> [u8; 3] {
    let mag = m.magnitude(t) as u32;
    match m.terrain_type(t) {
        TerrainType::Ocean => [28, 52, 84],
        TerrainType::Lake => [44, 84, 120],
        TerrainType::Plains => [(96 + mag * 4) as u8, (150 - mag * 2) as u8, 82],
        TerrainType::Highland => [(150 + mag) as u8, (140 - mag * 2) as u8, 96],
        TerrainType::Mountain => [
            (160 + mag * 2) as u8,
            (160 + mag * 2) as u8,
            (160 + mag * 2) as u8,
        ],
        TerrainType::Impassable => [20, 20, 20],
    }
}

fn write_bmp(path: &str, m: &GameMap) -> std::io::Result<()> {
    let (w, h) = (m.width() as usize, m.height() as usize);
    let row = (w * 3).div_ceil(4) * 4;
    let size = 54 + row * h;
    let mut out = Vec::with_capacity(size);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(size as u32).to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&[0; 24]);
    for y in (0..h).rev() {
        for x in 0..w {
            let [r, g, b] = color(m, (y * w + x) as u32);
            out.extend_from_slice(&[b, g, r]);
        }
        out.resize(out.len() + row - w * 3, 0);
    }
    std::fs::write(path, out)
}

fn run() -> Result<(), String> {
    let (mut w, mut h, mut seed, mut out) = (1200u32, 750u32, 7i64, "map.bmp".to_string());
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let v = it.next().ok_or(format!("{a} needs a value"))?;
        match a.as_str() {
            "--width" => w = v.parse().map_err(|e| format!("--width: {e}"))?,
            "--height" => h = v.parse().map_err(|e| format!("--height: {e}"))?,
            "--seed" => seed = v.parse().map_err(|e| format!("--seed: {e}"))?,
            "--out" => out = v,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let start = Instant::now();
    let map = generate(&TerrainParams::new(w, h), seed).ok_or("map too small")?;
    let ms = start.elapsed().as_secs_f64() * 1e3;
    println!(
        "{w}x{h} seed {seed}: {:.1} ms, land {:.1}%",
        ms,
        100.0 * map.num_land_tiles() as f64 / map.num_tiles() as f64
    );
    write_bmp(&out, &map).map_err(|e| format!("{out}: {e}"))
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("map_preview: {e}");
            ExitCode::FAILURE
        }
    }
}
