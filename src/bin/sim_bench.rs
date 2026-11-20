//! Headless simulation benchmark (Stage 7). No window, no Bevy.
//!
//! Run from the project root:
//!     cargo run --release --bin sim_bench
//!     cargo run --release --bin sim_bench -- --ticks 20000 --seed 7
//!
//! For each map size and AI count it runs a match for `--ticks` ticks and reports ticks per
//! second, mean and worst tick cost. "AIs" are the `RandomAttacker` stand-in until Stage 11;
//! player 0 (the human) stays idle. If a match ends early, a new one starts with the next
//! seed (setup time excluded) so every case measures the same number of live ticks.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use laukofront::ai::RandomAttacker;
use laukofront::config::{GameConfig, MapSize, GAME_CONFIG_PATH};
use laukofront::core::PlayerId;
use laukofront::map::Map;
use laukofront::sim::World;

const AI_COUNTS: [usize; 4] = [0, 1, 3, 5];

struct Args {
    ticks: u64,
    seed: u64,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        ticks: 5_000,
        seed: 1,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut value = |name: &str| -> Result<u64, String> {
            it.next()
                .ok_or(format!("{name} needs a value"))?
                .parse()
                .map_err(|e| format!("{name}: {e}"))
        };
        match a.as_str() {
            "--ticks" => args.ticks = value("--ticks")?.max(1),
            "--seed" => args.seed = value("--seed")?,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(args)
}

struct CaseResult {
    ticks_per_sec: f64,
    mean: Duration,
    max: Duration,
    matches: u32,
}

fn new_match(
    cfg: &GameConfig,
    size: MapSize,
    seed: u64,
    ais: usize,
) -> Result<(World, Vec<RandomAttacker>), String> {
    let world = World::new(cfg.clone(), size, seed, ais + 1).map_err(|e| e.to_string())?;
    let bots = (1..=ais)
        .map(|i| RandomAttacker::new(PlayerId(i as u8), seed))
        .collect();
    Ok((world, bots))
}

fn run_case(
    cfg: &GameConfig,
    size: MapSize,
    ais: usize,
    args: &Args,
) -> Result<CaseResult, String> {
    let mut seed = args.seed;
    let (mut world, mut bots) = new_match(cfg, size, seed, ais)?;
    let mut matches = 1;
    let mut total = Duration::ZERO;
    let mut max = Duration::ZERO;
    for _ in 0..args.ticks {
        if world.is_over() {
            seed += 1;
            (world, bots) = new_match(cfg, size, seed, ais)?;
            matches += 1;
        }
        let start = Instant::now();
        for bot in &mut bots {
            bot.think(&mut world);
        }
        world.tick();
        let dt = start.elapsed();
        total += dt;
        max = max.max(dt);
    }
    Ok(CaseResult {
        ticks_per_sec: args.ticks as f64 / total.as_secs_f64().max(1e-12),
        mean: total / args.ticks as u32,
        max,
        matches,
    })
}

fn us(d: Duration) -> String {
    format!("{:.2}", d.as_secs_f64() * 1e6)
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let cfg = GameConfig::load(GAME_CONFIG_PATH).map_err(|e| e.to_string())?;
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    println!(
        "sim_bench: {} ticks per case, seed {}, {profile} build",
        args.ticks, args.seed
    );
    println!();
    println!("| Map | Territories | Map gen (ms) |");
    println!("|---|---|---|");
    for size in MapSize::ALL {
        let s = cfg.map.size(size);
        let start = Instant::now();
        Map::generate(s, cfg.map.candidates_per_point, args.seed).map_err(|e| e.to_string())?;
        println!(
            "| {size:?} | {} | {:.1} |",
            s.territories,
            start.elapsed().as_secs_f64() * 1e3
        );
    }
    println!();
    println!("| Territories | AIs | Ticks/s | Mean tick (µs) | Max tick (µs) | x real time (10 Hz) | Matches |");
    println!("|---|---|---|---|---|---|---|");
    for size in MapSize::ALL {
        for ais in AI_COUNTS {
            let r = run_case(&cfg, size, ais, &args)?;
            println!(
                "| {} | {ais} | {:.0} | {} | {} | {:.0} | {} |",
                cfg.map.size(size).territories,
                r.ticks_per_sec,
                us(r.mean),
                us(r.max),
                r.ticks_per_sec / cfg.sim.tick_hz as f64,
                r.matches
            );
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sim_bench: {e}");
            ExitCode::FAILURE
        }
    }
}
