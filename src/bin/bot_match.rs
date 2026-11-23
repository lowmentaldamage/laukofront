//! Runs a bots-only match (the human never spawns) until someone wins or a tick limit.
//!
//!     cargo run --release --bin bot_match -- --size small --bots 10 --seed 1 --max-ticks 60000
//!
//! Prints the leader every 3000 ticks (5 game minutes) and the result.

use std::process::ExitCode;
use std::time::Instant;

use laukofront::config::{Difficulty, GameConfig, MapSize, GAME_CONFIG_PATH};
use laukofront::engine::{new_match, run_turn, Game, MapChoice, MatchSetup};

fn leader(g: &Game) -> String {
    let land = g.map().num_land_tiles() as f64;
    let alive = g.alive_players().count();
    match g.alive_players().max_by_key(|p| p.num_tiles()) {
        Some(p) => format!(
            "{} {:.1}% of land, {} troops, {alive} alive",
            p.name(),
            100.0 * p.num_tiles() as f64 / land,
            p.troops()
        ),
        None => "nobody alive".into(),
    }
}

fn run() -> Result<(), String> {
    let (mut size, mut bots, mut seed, mut max_ticks) = (MapSize::Small, 10u32, 1i64, 60_000u64);
    let (mut nations, mut difficulty) = (0u32, Difficulty::Medium);
    let mut map = MapChoice::default();
    let mut teams = 0u8;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let v = it.next().ok_or(format!("{a} needs a value"))?;
        match a.as_str() {
            "--size" => {
                size = match v.as_str() {
                    "small" => MapSize::Small,
                    "medium" => MapSize::Medium,
                    "large" => MapSize::Large,
                    o => return Err(format!("unknown size {o}")),
                }
            }
            "--bots" => bots = v.parse().map_err(|e| format!("--bots: {e}"))?,
            "--seed" => seed = v.parse().map_err(|e| format!("--seed: {e}"))?,
            "--teams" => teams = v.parse().map_err(|e| format!("--teams: {e}"))?,
            "--map" => map = MapChoice::parse(&v).ok_or(format!("unknown map {v}"))?,
            "--nations" => nations = v.parse().map_err(|e| format!("--nations: {e}"))?,
            "--difficulty" => difficulty = Difficulty::parse(&v).ok_or("unknown difficulty")?,
            "--max-ticks" => max_ticks = v.parse().map_err(|e| format!("--max-ticks: {e}"))?,
            o => return Err(format!("unknown argument {o}")),
        }
    }
    let cfg = GameConfig::load(GAME_CONFIG_PATH).map_err(|e| e.to_string())?;
    let mut g = new_match(
        cfg,
        MatchSetup {
            size,
            seed,
            bots,
            nations,
            difficulty,
            map,
            teams,
        },
    )?;
    let start = Instant::now();
    while g.winner().is_none() && g.ticks() < max_ticks {
        run_turn(&mut g, &[]);
        if g.ticks() % 3000 == 0 {
            println!(
                "tick {:>6} ({:>3} min): {}",
                g.ticks(),
                g.ticks() / 600,
                leader(&g)
            );
        }
    }
    let secs = start.elapsed().as_secs_f64();
    for p in g.alive_players() {
        let near: Vec<String> = g
            .nearby(p.id())
            .iter()
            .map(|n| {
                n.and_then(|id| g.player(id))
                    .map_or("unowned land".into(), |q| q.name().to_string())
            })
            .collect();
        let allies: Vec<&str> = g
            .allies(p.id())
            .into_iter()
            .filter_map(|a| g.player(a).map(|q| q.name()))
            .collect();
        println!(
            "  {}: {} tiles, {} troops, {} gold, {} units, land neighbours: {:?}, allies: {:?}",
            p.name(),
            p.num_tiles(),
            p.troops(),
            p.gold(),
            p.units().len(),
            near,
            allies
        );
    }
    if std::env::var("BOT_MATCH_DEBUG").is_ok() {
        use laukofront::engine::units::UnitKind;
        for p in g.alive_players() {
            let kinds: Vec<String> = [
                UnitKind::City,
                UnitKind::Port,
                UnitKind::Factory,
                UnitKind::DefensePost,
                UnitKind::MissileSilo,
                UnitKind::SamLauncher,
                UnitKind::Warship,
                UnitKind::TransportShip,
            ]
            .iter()
            .map(|&k| format!("{k:?}={}", g.unit_count(p.id(), k)))
            .collect();
            println!(
                "  {} units: {}; outgoing attacks {}",
                p.name(),
                kinds.join(" "),
                p.outgoing_attacks().len()
            );
        }
        println!("  fallout tiles: {}", g.map().num_tiles_with_fallout());
    }
    let owned: u32 = g.players().iter().map(|p| p.num_tiles()).sum();
    println!(
        "  unowned land: {} of {} tiles",
        g.map().num_land_tiles() - owned,
        g.map().num_land_tiles()
    );
    match g.winner().and_then(|w| g.player(w)) {
        Some(w) => println!(
            "WINNER {}{} at tick {} ({:.1} game minutes); {:.1} s real time",
            w.name(),
            g.winner_team()
                .map_or(String::new(), |t| format!(" (team {})", t + 1)),
            g.ticks(),
            g.elapsed_game_seconds() / 60.0,
            secs
        ),
        None => println!(
            "no winner after {} ticks; {}; {secs:.1} s real time",
            g.ticks(),
            leader(&g)
        ),
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("bot_match: {e}");
            ExitCode::FAILURE
        }
    }
}
