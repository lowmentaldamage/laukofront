//! Bevy glue between the engine and the client: holds the `Game` as a resource, runs ticks on
//! Bevy's fixed timestep, queues the human's intents, and collects tiles that changed so the
//! renderer can repaint only those.

use bevy::prelude::*;

use crate::engine::{run_turn, Game, Intent, HUMAN};
use crate::engine_lib::game_map::TileRef;

/// The running match.
#[derive(Resource)]
pub struct GameState(pub Game);

/// Intents the player made since the last tick.
#[derive(Resource, Default)]
pub struct PendingIntents(pub Vec<Intent>);

/// Tiles whose owner changed since the renderer last painted.
#[derive(Resource, Default)]
pub struct TileChanges(pub Vec<TileRef>);

/// Client-side settings and match flow.
#[derive(Resource, Debug, Clone)]
pub struct ClientSettings {
    /// Share of troops an attack sends, 0.1..=0.9 (keys 1-9).
    pub attack_ratio: f64,
    pub paused: bool,
    /// Game speed multiplier (1, 2 or 3).
    pub speed: u32,
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            attack_ratio: 0.2,
            paused: false,
            speed: 1,
        }
    }
}

/// Short messages shown to the player (text, seconds left).
#[derive(Resource, Default)]
pub struct Notices(pub Vec<(String, f32)>);

impl Notices {
    pub fn push(&mut self, text: impl Into<String>) {
        self.0.push((text.into(), 4.0));
        if self.0.len() > 5 {
            self.0.remove(0);
        }
    }
}

pub struct ClientPlugin {
    pub tick_hz: u32,
}

impl Plugin for ClientPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(self.tick_hz as f64))
            .insert_resource(TickRate(self.tick_hz))
            .init_resource::<PendingIntents>()
            .init_resource::<TileChanges>()
            .init_resource::<ClientSettings>()
            .init_resource::<Notices>()
            .add_systems(FixedUpdate, run_tick)
            .add_systems(Update, (apply_speed, age_notices));
    }
}

#[derive(Resource)]
struct TickRate(u32);

fn run_tick(
    mut game: ResMut<GameState>,
    mut pending: ResMut<PendingIntents>,
    mut changes: ResMut<TileChanges>,
    settings: Res<ClientSettings>,
    mut notices: ResMut<Notices>,
    mut announced: Local<bool>,
) {
    if settings.paused {
        return;
    }
    let intents = std::mem::take(&mut pending.0);
    let g = &mut game.0;
    run_turn(g, &intents);
    changes.0.extend_from_slice(g.changed_tiles());
    if let Some(w) = g.winner() {
        if !*announced {
            *announced = true;
            let name = g
                .player(w)
                .map_or("?".to_string(), |p| p.name().to_string());
            notices.push(if w == HUMAN {
                "You won!".to_string()
            } else {
                format!("{name} won the game")
            });
        }
    }
}

fn apply_speed(settings: Res<ClientSettings>, rate: Res<TickRate>, mut time: ResMut<Time<Fixed>>) {
    if settings.is_changed() {
        time.set_timestep_hz((rate.0 * settings.speed.max(1)) as f64);
    }
}

fn age_notices(time: Res<Time>, mut notices: ResMut<Notices>) {
    let dt = time.delta_secs();
    for n in notices.0.iter_mut() {
        n.1 -= dt;
    }
    notices.0.retain(|n| n.1 > 0.0);
}
