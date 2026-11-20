//! Bevy glue between the engine and the client: holds the `Game` as a resource, runs ticks on
//! Bevy's fixed timestep, queues the human's intents, collects changed tiles for the renderer,
//! turns game changes into notifications, keeps match statistics, and restarts matches.

pub mod settings;

use bevy::prelude::*;

use crate::engine::{new_match, run_turn, Game, Intent, MatchSetup, PlayerId, HUMAN};
use crate::engine_lib::game_map::TileRef;

pub use settings::SavedSettings;

/// The running match.
#[derive(Resource)]
pub struct GameState(pub Game);

/// How the running match was created (used to restart it).
#[derive(Resource, Clone, Copy)]
pub struct CurrentSetup(pub MatchSetup);

/// Intents the player made since the last tick.
#[derive(Resource, Default)]
pub struct PendingIntents(pub Vec<Intent>);

/// Tiles whose owner changed since the renderer last painted.
#[derive(Resource, Default)]
pub struct TileChanges(pub Vec<TileRef>);

/// Tiles whose owner changed since the effects layer last looked (capture animation).
#[derive(Resource, Default)]
pub struct CaptureEvents(pub Vec<(TileRef, PlayerId)>);

/// Short-lived weapon effects collected from ticks: explosions (tile, radius, age) and
/// shots (from, to, is_sam, age). Ages in seconds; the renderer ages and removes them.
#[derive(Resource, Default)]
pub struct WeaponFx {
    pub blasts: Vec<(TileRef, u32, f32)>,
    pub shots: Vec<(TileRef, TileRef, bool, f32)>,
}

/// Set when a new match replaced the old one; the renderer rebuilds its textures.
#[derive(Resource, Default)]
pub struct MapRebuild(pub bool);

/// Client-side settings and match flow.
#[derive(Resource, Debug, Clone)]
pub struct ClientSettings {
    /// Share of troops an attack sends, 0.1..=0.9 (keys 1-9).
    pub attack_ratio: f64,
    pub paused: bool,
    /// Game speed multiplier (1, 2 or 3).
    pub speed: u32,
    /// Key-help card visible (H).
    pub show_help: bool,
    /// Building chosen in the build bar; the next map click places it.
    pub build: Option<crate::engine::units::UnitKind>,
    /// Your warship chosen by clicking it; the next water click moves it.
    pub selected_ship: Option<crate::engine::units::UnitId>,
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            attack_ratio: 0.2,
            paused: false,
            speed: 1,
            show_help: false,
            build: None,
            selected_ship: None,
        }
    }
}

/// What a notification is about; decides its colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeKind {
    Info,
    Good,
    Warning,
    Danger,
}

#[derive(Debug, Clone)]
pub struct Notice {
    pub text: String,
    pub kind: NoticeKind,
    pub ttl: f32,
}

/// Short messages shown to the player, newest last.
#[derive(Resource, Default)]
pub struct Notices(pub Vec<Notice>);

impl Notices {
    pub fn push(&mut self, text: impl Into<String>) {
        self.push_kind(text, NoticeKind::Info);
    }

    pub fn push_kind(&mut self, text: impl Into<String>, kind: NoticeKind) {
        self.0.push(Notice {
            text: text.into(),
            kind,
            ttl: 6.0,
        });
        if self.0.len() > 6 {
            self.0.remove(0);
        }
    }
}

/// Statistics of the running match, for the HUD and end screen.
#[derive(Resource, Default, Debug, Clone)]
pub struct MatchStats {
    /// Highest share of land the human held (0..1).
    pub peak_land: f64,
    /// Real seconds since the human spawned (drives the "YOU" highlight).
    pub since_spawn: Option<f32>,
    /// The match is over for the human (won, lost or eliminated); shows the end screen.
    pub ended: bool,
    /// Your measured gold income per second over the last 10 s of game time (base + trade +
    /// factories).
    pub gold_rate: f64,
    gold_history: std::collections::VecDeque<i64>,
}

/// Restart with the next seed when set (end screen button).
#[derive(Resource, Default)]
pub struct RestartRequested(pub bool);

pub struct ClientPlugin {
    pub tick_hz: u32,
}

impl Plugin for ClientPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(self.tick_hz as f64))
            .insert_resource(TickRate(self.tick_hz))
            .insert_resource(SavedSettings::load())
            .init_resource::<PendingIntents>()
            .init_resource::<TileChanges>()
            .init_resource::<CaptureEvents>()
            .init_resource::<WeaponFx>()
            .init_resource::<MapRebuild>()
            .init_resource::<ClientSettings>()
            .init_resource::<Notices>()
            .init_resource::<MatchStats>()
            .init_resource::<RestartRequested>()
            .add_systems(FixedUpdate, run_tick)
            .add_systems(
                Update,
                (apply_speed, age_notices, track_spawn_time, restart),
            );
    }
}

#[derive(Resource)]
struct TickRate(u32);

/// Snapshot of what the notifications compare across a tick.
struct Before {
    alive: Vec<PlayerId>,
    incoming: Vec<u32>,
    my_tiles: u32,
    nukes: Vec<u32>,
}

/// Ids of nukes in flight aimed at the human's land.
fn nukes_at_me(g: &Game) -> Vec<u32> {
    g.units()
        .iter()
        .filter(|u| u.is_active() && u.kind().is_nuke() && u.owner() != HUMAN)
        .filter(|u| u.kind() != crate::engine::units::UnitKind::MirvWarhead)
        .filter(|u| u.target_tile().and_then(|t| g.owner(t)) == Some(HUMAN))
        .map(|u| u.id())
        .collect()
}

fn snapshot(g: &Game) -> Before {
    Before {
        alive: g.alive_players().map(|p| p.id()).collect(),
        incoming: g.incoming_attacks(HUMAN),
        my_tiles: g.player(HUMAN).map_or(0, |p| p.num_tiles()),
        nukes: nukes_at_me(g),
    }
}

/// Notifications for what changed during one tick. Pure so it can be tested.
pub fn tick_notices(
    g: &Game,
    before_alive: &[PlayerId],
    before_incoming: &[u32],
    before_tiles: u32,
) -> Vec<(String, NoticeKind)> {
    let mut out = Vec::new();
    for id in g.incoming_attacks(HUMAN) {
        if before_incoming.contains(&id) {
            continue;
        }
        if let Some(a) = g.attack(id) {
            let name = g.player(a.attacker()).map_or("?", |p| p.name());
            out.push((
                format!("{name} attacks you with {:.0} troops", a.troops()),
                NoticeKind::Danger,
            ));
        }
    }
    for &id in before_alive {
        if g.player(id).is_some_and(|p| !p.is_alive()) {
            if id == HUMAN {
                out.push(("You have been eliminated".to_string(), NoticeKind::Danger));
            } else {
                let name = g.player(id).map_or("?", |p| p.name());
                out.push((format!("{name} was eliminated"), NoticeKind::Info));
            }
        }
    }
    let now = g.player(HUMAN).map_or(0, |p| p.num_tiles());
    if before_tiles > 200 && now + now / 10 < before_tiles {
        out.push((
            format!("You lost {} tiles", before_tiles - now),
            NoticeKind::Warning,
        ));
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn run_tick(
    mut game: ResMut<GameState>,
    mut pending: ResMut<PendingIntents>,
    mut changes: ResMut<TileChanges>,
    mut captures: ResMut<CaptureEvents>,
    settings: Res<ClientSettings>,
    mut notices: ResMut<Notices>,
    mut stats: ResMut<MatchStats>,
    mut fx: ResMut<WeaponFx>,
) {
    if settings.paused {
        return;
    }
    let intents = std::mem::take(&mut pending.0);
    let g = &mut game.0;
    let before = snapshot(g);
    let had_winner = g.winner().is_some();
    run_turn(g, &intents);

    for &(t, r) in g.detonations() {
        fx.blasts.push((t, r, 0.0));
    }
    for &(a, b) in g.sam_shots() {
        fx.shots.push((a, b, true, 0.0));
        notices.push_kind("A SAM shot down a nuke", NoticeKind::Info);
    }
    for &(a, b) in g.shell_shots() {
        fx.shots.push((a, b, false, 0.0));
    }
    let map = g.map();
    for &t in g.changed_tiles() {
        changes.0.push(t);
        captures.0.push((t, map.owner_id(t)));
    }
    if captures.0.len() > 200_000 {
        captures.0.drain(..100_000);
    }
    for (text, kind) in tick_notices(g, &before.alive, &before.incoming, before.my_tiles) {
        notices.push_kind(text, kind);
    }
    for id in nukes_at_me(g) {
        if !before.nukes.contains(&id) {
            if let Some(u) = g.unit(id) {
                let who = g.player(u.owner()).map_or("?", |p| p.name());
                notices.push_kind(
                    format!(
                        "{who} launched a {} at you!",
                        u.kind().name().to_lowercase()
                    ),
                    NoticeKind::Danger,
                );
            }
        }
    }
    let land = g.map().num_land_tiles().max(1) as f64;
    let hz = g.config().game.tick_hz as f64;
    if let Some(me) = g.player(HUMAN) {
        stats.gold_history.push_back(me.gold());
        if stats.gold_history.len() > 101 {
            stats.gold_history.pop_front();
        }
        if let (Some(&first), Some(&last)) = (stats.gold_history.front(), stats.gold_history.back())
        {
            let n = (stats.gold_history.len() - 1).max(1) as f64;
            // Spending makes the delta negative; show income only.
            stats.gold_rate = ((last - first) as f64 / n * hz).max(0.0);
        }
        stats.peak_land = stats.peak_land.max(me.num_tiles() as f64 / land);
        if me.has_spawned() && !me.is_alive() {
            stats.ended = true;
        }
    }
    if let Some(w) = g.winner() {
        stats.ended = true;
        if !had_winner {
            let name = g
                .player(w)
                .map_or("?".to_string(), |p| p.name().to_string());
            if w == HUMAN {
                notices.push_kind("You won!", NoticeKind::Good);
            } else {
                notices.push_kind(format!("{name} won the game"), NoticeKind::Danger);
            }
        }
    }
}

fn track_spawn_time(time: Res<Time>, game: Res<GameState>, mut stats: ResMut<MatchStats>) {
    let spawned = game.0.player(HUMAN).is_some_and(|p| p.has_spawned());
    match (spawned, stats.since_spawn) {
        (true, None) => stats.since_spawn = Some(0.0),
        (true, Some(t)) => stats.since_spawn = Some(t + time.delta_secs()),
        (false, Some(_)) => stats.since_spawn = None,
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn restart(
    mut req: ResMut<RestartRequested>,
    mut setup: ResMut<CurrentSetup>,
    mut game: ResMut<GameState>,
    mut changes: ResMut<TileChanges>,
    mut captures: ResMut<CaptureEvents>,
    mut notices: ResMut<Notices>,
    mut stats: ResMut<MatchStats>,
    mut rebuild: ResMut<MapRebuild>,
    mut pending: ResMut<PendingIntents>,
) {
    if !req.0 {
        return;
    }
    req.0 = false;
    let mut next = setup.0;
    next.seed = next.seed.wrapping_add(1);
    match new_match(game.0.config().clone(), next) {
        Ok(g) => {
            game.0 = g;
            setup.0 = next;
            changes.0.clear();
            captures.0.clear();
            pending.0.clear();
            notices.0.clear();
            *stats = MatchStats::default();
            rebuild.0 = true;
            notices.push(format!("New match, seed {}", next.seed));
        }
        Err(e) => notices.push_kind(format!("Cannot restart: {e}"), NoticeKind::Danger),
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
        n.ttl -= dt;
    }
    notices.0.retain(|n| n.ttl > 0.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GameConfig, MapSize, GAME_CONFIG_PATH};

    #[test]
    fn incoming_attacks_and_eliminations_become_notices() {
        let cfg = GameConfig::load(GAME_CONFIG_PATH).unwrap();
        let mut g = new_match(
            cfg,
            MatchSetup {
                size: MapSize::Small,
                seed: 3,
                bots: 10,
            },
        )
        .unwrap();
        let mut seen_elimination = false;
        for _ in 0..8_000 {
            let b = snapshot(&g);
            run_turn(&mut g, &[]);
            for (text, _) in tick_notices(&g, &b.alive, &b.incoming, b.my_tiles) {
                if text.ends_with("was eliminated") {
                    seen_elimination = true;
                }
            }
        }
        assert!(
            seen_elimination,
            "bots eliminate each other within 8000 ticks on seed 3"
        );
    }

    #[test]
    fn notices_keep_the_newest_six() {
        let mut n = Notices::default();
        for i in 0..10 {
            n.push(format!("n{i}"));
        }
        assert_eq!(n.0.len(), 6);
        assert_eq!(n.0.last().unwrap().text, "n9");
    }
}
