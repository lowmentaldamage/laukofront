//! laukofront entry point.
//!
//!     laukofront [--size small|medium|large] [--bots N] [--seed N]
//!                [--autoplay] [--perf-run SECONDS] [--screenshot PATH] [--screenshot-at SECONDS]
//!
//! Until the menu exists (Stage 12) the match comes from these options: Medium map, 20 bots,
//! random seed. `--autoplay` spawns and attacks for you (for demos, screenshots and perf runs).
//! `--perf-run` opens a 1920x1080 window without vsync, records frame times, prints a summary
//! and exits. `--screenshot` saves a PNG after `--screenshot-at` seconds (default 6) and exits.

use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};

use laukofront::client::{ClientPlugin, ClientSettings, GameState, PendingIntents};
use laukofront::config::{GameConfig, MapSize, GAME_CONFIG_PATH};
use laukofront::engine::util::spawn_tiles;
use laukofront::engine::{new_match, Intent, MatchSetup, HUMAN};
use laukofront::engine_lib::random::PseudoRandom;
use laukofront::input::{click_action, ClickAction, InputPlugin};
use laukofront::render::perf::{record_frames, take_screenshot, PerfRun, ScreenshotRun};
use laukofront::render::MapRenderPlugin;
use laukofront::ui::UiPlugin;

struct Options {
    setup: MatchSetup,
    autoplay: bool,
    perf_run: Option<f32>,
    screenshot: Option<String>,
    screenshot_at: f32,
}

fn parse_args() -> Result<Options, String> {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| (d.as_nanos() % 1_000_000_007) as i64)
        .unwrap_or(1);
    let mut o = Options {
        setup: MatchSetup {
            size: MapSize::Medium,
            seed,
            bots: 20,
        },
        autoplay: false,
        perf_run: None,
        screenshot: None,
        screenshot_at: 6.0,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        if a == "--autoplay" {
            o.autoplay = true;
            continue;
        }
        let v = it.next().ok_or(format!("{a} needs a value"))?;
        match a.as_str() {
            "--size" => {
                o.setup.size = match v.to_ascii_lowercase().as_str() {
                    "small" => MapSize::Small,
                    "medium" => MapSize::Medium,
                    "large" => MapSize::Large,
                    other => return Err(format!("unknown map size {other} (small|medium|large)")),
                }
            }
            "--bots" => o.setup.bots = v.parse().map_err(|e| format!("--bots: {e}"))?,
            "--seed" => o.setup.seed = v.parse().map_err(|e| format!("--seed: {e}"))?,
            "--perf-run" => o.perf_run = Some(v.parse().map_err(|e| format!("--perf-run: {e}"))?),
            "--screenshot" => o.screenshot = Some(v),
            "--screenshot-at" => {
                o.screenshot_at = v.parse().map_err(|e| format!("--screenshot-at: {e}"))?
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(o)
}

/// Plays for the human: spawns on a free spot, then every 2 s attacks a random neighbour with 30%
/// of troops, using the same click rules as the mouse.
#[derive(Resource)]
struct Autoplay {
    rng: PseudoRandom,
    last_tick: u64,
    spawn_sent: bool,
}

fn autoplay(game: Res<GameState>, mut auto: ResMut<Autoplay>, mut pending: ResMut<PendingIntents>) {
    let g = &game.0;
    if g.ticks() == auto.last_tick {
        return;
    }
    auto.last_tick = g.ticks();
    let map = g.map();
    if g.in_spawn_phase() {
        if g.ticks() >= 2 && !g.player(HUMAN).is_some_and(|p| p.has_spawned()) && !auto.spawn_sent {
            auto.spawn_sent = true;
            let n = map.num_tiles();
            let start = auto.rng.next_int(0, n as i64) as u64;
            let tile = (0..n as u64)
                .map(|i| ((start + i * 7919) % n as u64) as u32)
                .find(|&t| spawn_tiles(map, t, 4, true).is_some());
            if let Some(tile) = tile {
                pending.0.push(Intent::Spawn {
                    player: HUMAN,
                    tile,
                });
            }
        }
        return;
    }
    if !g.ticks().is_multiple_of(20) {
        return;
    }
    let Some(me) = g.player(HUMAN) else { return };
    let mut targets: Vec<u32> = Vec::new();
    let mut buf = [0; 4];
    for &b in me.border_tiles().iter().step_by(7) {
        let k = map.neighbors4(b, &mut buf);
        targets.extend(
            buf[..k]
                .iter()
                .copied()
                .filter(|&t| map.is_passable_land(t) && g.owner(t) != Some(HUMAN)),
        );
    }
    if let Some(i) = auto.rng.index(targets.len()) {
        if let ClickAction::Attack { target, troops } = click_action(g, targets[i], 0.3) {
            pending.0.push(Intent::Attack {
                player: HUMAN,
                target,
                troops: Some(troops),
            });
        }
    }
}

fn run() -> Result<(), String> {
    let o = parse_args()?;
    let config = GameConfig::load(GAME_CONFIG_PATH).map_err(|e| e.to_string())?;
    let tick_hz = config.game.tick_hz;
    let game = new_match(config, o.setup)?;
    println!(
        "laukofront: {:?} map, {} bots, seed {} (\u{a9} OpenFront and Contributors; not affiliated)",
        o.setup.size, o.setup.bots, o.setup.seed
    );

    let automated = o.perf_run.is_some() || o.screenshot.is_some();
    let (resolution, present_mode) = match o.perf_run {
        Some(_) => (WindowResolution::new(1920, 1080), PresentMode::AutoNoVsync),
        None => (WindowResolution::new(1440, 900), PresentMode::AutoVsync),
    };
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "laukofront".into(),
            resolution,
            present_mode,
            // Automated runs must not grab the keyboard from whatever the user is typing in.
            focused: !automated,
            ..default()
        }),
        ..default()
    }))
    .insert_resource(GameState(game))
    .add_plugins((
        ClientPlugin { tick_hz },
        MapRenderPlugin,
        InputPlugin,
        UiPlugin,
    ));

    if o.autoplay {
        app.insert_resource(Autoplay {
            rng: PseudoRandom::new(o.setup.seed),
            last_tick: u64::MAX,
            spawn_sent: false,
        })
        .add_systems(Update, autoplay)
        // Demos run at 3x so something happens quickly.
        .insert_resource(ClientSettings {
            speed: 3,
            ..default()
        });
    }
    if automated {
        // Unfocused windows are throttled by default; measurements and screenshots need full speed.
        app.insert_resource(bevy::winit::WinitSettings::continuous());
    }
    if let Some(seconds) = o.perf_run {
        app.insert_resource(PerfRun::new(seconds))
            .add_systems(Update, record_frames);
    }
    if let Some(path) = o.screenshot {
        app.insert_resource(ScreenshotRun::new(path, o.screenshot_at))
            .add_systems(Update, take_screenshot);
    }
    match app.run() {
        AppExit::Success => Ok(()),
        AppExit::Error(code) => Err(format!("exited with code {code}")),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("laukofront: {e}");
            ExitCode::FAILURE
        }
    }
}
