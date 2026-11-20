//! Headless engine benchmark (replaces the Stage 7 `sim_bench`).
//!
//!     cargo run --release --bin engine_bench -- --ticks 2000 --seed 1
//!
//! For each map size and bot count: build the match (timed), spawn everyone, then time
//! `--ticks` ticks after the spawn phase. Every 10 ticks each living player launches a default
//! attack at a random other player or at unowned land (a stand-in for AI until P4), so ticks
//! include economy, attacks, takeovers and encirclement checks. Max tick = worst single tick.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use laukofront::config::{GameConfig, MapSize, GAME_CONFIG_PATH};
use laukofront::engine::util::spawn_tiles;
use laukofront::engine::{new_match, run_turn, Intent, MatchSetup, PlayerId, HUMAN};
use laukofront::engine_lib::random::PseudoRandom;

const BOTS: [u32; 3] = [0, 10, 50];

fn run() -> Result<(), String> {
    let (mut ticks, mut seed) = (2_000u64, 1i64);
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let v = it.next().ok_or(format!("{a} needs a value"))?;
        match a.as_str() {
            "--ticks" => ticks = v.parse().map_err(|e| format!("--ticks: {e}"))?,
            "--seed" => seed = v.parse().map_err(|e| format!("--seed: {e}"))?,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let cfg = GameConfig::load(GAME_CONFIG_PATH).map_err(|e| e.to_string())?;
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    println!("engine_bench: {ticks} ticks per case, seed {seed}, {profile} build\n");
    println!("| Map | Tiles | Bots | Setup (ms) | Ticks/s | Mean tick (µs) | Max tick (µs) | Owned tiles at end |");
    println!("|---|---|---|---|---|---|---|---|");
    for size in MapSize::ALL {
        for bots in BOTS {
            let start = Instant::now();
            let mut g = new_match(cfg.clone(), MatchSetup::bots_only(size, seed, bots))?;
            run_turn(&mut g, &[]);
            run_turn(&mut g, &[]);
            let setup = start.elapsed();
            let map = g.map();
            let tile = (0..map.num_tiles())
                .step_by(101)
                .find(|&t| spawn_tiles(map, t, 4, true).is_some())
                .ok_or("no free land for the human")?;
            run_turn(
                &mut g,
                &[Intent::Spawn {
                    player: HUMAN,
                    tile,
                }],
            );
            run_turn(&mut g, &[]);
            let (mut total, mut max) = (Duration::ZERO, Duration::ZERO);
            let mut rng = PseudoRandom::new(seed);
            let mut intents = Vec::new();
            for i in 0..ticks {
                intents.clear();
                if i % 10 == 0 {
                    let alive: Vec<PlayerId> = g.alive_players().map(|p| p.id()).collect();
                    for &p in &alive {
                        let target = if rng.chance(3) {
                            None
                        } else {
                            rng.index(alive.len()).map(|k| alive[k]).filter(|&q| q != p)
                        };
                        intents.push(Intent::Attack {
                            player: p,
                            target,
                            troops: None,
                        });
                    }
                }
                let t0 = Instant::now();
                run_turn(&mut g, &intents);
                let dt = t0.elapsed();
                total += dt;
                max = max.max(dt);
            }
            println!(
                "| {size:?} | {} | {bots} | {:.1} | {:.0} | {:.2} | {:.2} | {} |",
                g.map().num_tiles(),
                setup.as_secs_f64() * 1e3,
                ticks as f64 / total.as_secs_f64().max(1e-12),
                total.as_secs_f64() * 1e6 / ticks as f64,
                max.as_secs_f64() * 1e6,
                g.players().iter().map(|p| p.num_tiles()).sum::<u32>()
            );
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("engine_bench: {e}");
            ExitCode::FAILURE
        }
    }
}
