//! Bevy glue between the engine and the client: holds the `Game` as a resource, runs ticks on
//! Bevy's fixed timestep, queues the human's intents, collects changed tiles for the renderer,
//! turns game changes into notifications, keeps match statistics, and restarts matches.

pub mod settings;

use bevy::prelude::*;

use crate::engine::{new_match, run_turn, Game, Intent, MatchSetup, PlayerId, HUMAN};
use crate::engine_lib::game_map::TileRef;

pub use settings::SavedSettings;

pub mod audio;
pub mod save;

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

/// Where each moving unit (ships, missiles) was before the latest tick, so the renderer can slide
/// it smoothly between ticks instead of jumping 10 times a second.
#[derive(Resource, Default)]
pub struct UnitMotion {
    pub prev: std::collections::HashMap<crate::engine::units::UnitId, TileRef>,
}

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
    /// Tile whose action menu is open (right click): attack, boat, build, diplomacy…
    pub menu_tile: Option<TileRef>,
    /// Screen position the menu opened at.
    pub menu_pos: Vec2,
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
            menu_tile: None,
            menu_pos: Vec2::ZERO,
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
    /// Your share of the land every 10 s of game time (end-screen chart).
    pub land_history: Vec<f32>,
    pub peak_troops: i64,
    pub peak_buildings: usize,
}

/// Restart with the next seed when set (end screen button).
#[derive(Resource, Default)]
pub struct RestartRequested(pub bool);

/// Start exactly this match (menu "Start", loading a save).
#[derive(Resource, Default)]
pub struct StartMatch(pub Option<MatchSetup>);

/// Which screen is in front. Menus show over a live match (a bots-only one behind the main
/// menu); the map only takes input while `Playing`.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Screen {
    #[default]
    Playing,
    MainMenu,
    Setup,
    Settings,
    Credits,
    Paused,
}

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
            .init_resource::<UnitMotion>()
            .init_resource::<MapRebuild>()
            .init_resource::<ClientSettings>()
            .init_resource::<Notices>()
            .init_resource::<MatchStats>()
            .init_resource::<RestartRequested>()
            .init_resource::<StartMatch>()
            .add_plugins(audio::AudioFxPlugin)
            .init_resource::<save::IntentLog>()
            .init_resource::<save::SaveRequested>()
            .init_resource::<save::LoadRequested>()
            .init_resource::<Screen>()
            .add_systems(FixedUpdate, run_tick)
            .add_systems(
                Update,
                (
                    apply_speed,
                    age_notices,
                    track_spawn_time,
                    restart,
                    save_and_load,
                ),
            );
    }
}

/// Handles "Save game" and "Load" (pause menu, main menu, F5 / F9).
#[allow(clippy::too_many_arguments)]
fn save_and_load(
    mut save_req: ResMut<save::SaveRequested>,
    mut load_req: ResMut<save::LoadRequested>,
    mut game: ResMut<GameState>,
    mut setup: ResMut<CurrentSetup>,
    mut log: ResMut<save::IntentLog>,
    mut notices: ResMut<Notices>,
    mut stats: ResMut<MatchStats>,
    mut rebuild: ResMut<MapRebuild>,
    mut changes: ResMut<TileChanges>,
    mut captures: ResMut<CaptureEvents>,
) {
    if std::mem::take(&mut save_req.0) {
        let f = save::SaveFile::capture(&game.0, setup.0, &log);
        match save::write_save(&f) {
            Ok(p) => notices.push_kind(format!("Game saved ({})", p.display()), NoticeKind::Good),
            Err(e) => notices.push_kind(format!("Could not save: {e}"), NoticeKind::Danger),
        }
    }
    if std::mem::take(&mut load_req.0) {
        let result = save::read_save().and_then(|f| {
            let s = f.setup;
            f.replay(game.0.config().clone()).map(|(g, l)| (g, l, s))
        });
        match result {
            Ok((g, l, s)) => {
                game.0 = g;
                *log = l;
                setup.0 = s;
                changes.0.clear();
                captures.0.clear();
                notices.0.clear();
                *stats = MatchStats::default();
                rebuild.0 = true;
                notices.push_kind("Saved game loaded", NoticeKind::Good);
            }
            Err(e) => notices.push_kind(format!("Could not load: {e}"), NoticeKind::Danger),
        }
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

/// Notifications for this tick's diplomacy events that concern the human (betrayals between
/// others too, since they change who is safe to trust). Pure so it can be tested.
pub fn diplo_notices(g: &Game) -> Vec<(String, NoticeKind)> {
    use crate::engine::diplomacy::DiploEvent as E;
    let name = |p: PlayerId| {
        g.player(p)
            .map_or("?".to_string(), |x| x.name().to_string())
    };
    let fmt = |n: i64| crate::ui::fmt_num(n as f64);
    let mut out = Vec::new();
    for e in g.diplo_events() {
        let n = match *e {
            E::Requested { from, to } if to == HUMAN => Some((
                format!("{} asks for an alliance (top right)", name(from)),
                NoticeKind::Good,
            )),
            E::Accepted { a, b } if a == HUMAN || b == HUMAN => {
                let o = if a == HUMAN { b } else { a };
                Some((
                    format!("You are now allied with {}", name(o)),
                    NoticeKind::Good,
                ))
            }
            E::Rejected { from, to } if from == HUMAN => Some((
                format!("{} rejected your alliance request", name(to)),
                NoticeKind::Warning,
            )),
            E::Broken { traitor, betrayed } if betrayed == HUMAN => Some((
                format!(
                    "{} betrayed you! They defend at half strength for 30 s",
                    name(traitor)
                ),
                NoticeKind::Danger,
            )),
            E::Broken { traitor, betrayed } if traitor != HUMAN => Some((
                format!("{} betrayed {}", name(traitor), name(betrayed)),
                NoticeKind::Info,
            )),
            E::Expired { a, b } if a == HUMAN || b == HUMAN => {
                let o = if a == HUMAN { b } else { a };
                Some((
                    format!("Alliance with {} ended", name(o)),
                    NoticeKind::Warning,
                ))
            }
            E::Renewed { a, b } if a == HUMAN || b == HUMAN => {
                let o = if a == HUMAN { b } else { a };
                Some((
                    format!("Alliance with {} renewed", name(o)),
                    NoticeKind::Good,
                ))
            }
            E::RenewAsked { from, to } if to == HUMAN => Some((
                format!("{} wants to renew your alliance", name(from)),
                NoticeKind::Info,
            )),
            E::DonatedTroops { from, to, amount } if to == HUMAN => Some((
                format!("{} sent you {} troops", name(from), fmt(amount)),
                NoticeKind::Good,
            )),
            E::DonatedGold { from, to, amount } if to == HUMAN => Some((
                format!("{} sent you {} gold", name(from), fmt(amount)),
                NoticeKind::Good,
            )),
            E::Embargo { from, to, on } if to == HUMAN => Some((
                if on {
                    format!("{} stopped trading with you", name(from))
                } else {
                    format!("{} trades with you again", name(from))
                },
                NoticeKind::Info,
            )),
            E::Target { from, to } if g.is_allied(from, HUMAN) => Some((
                format!("Your ally {} marked {} as a target", name(from), name(to)),
                NoticeKind::Info,
            )),
            E::Target { from, to } if to == HUMAN => Some((
                format!("{} marked you as a target", name(from)),
                NoticeKind::Warning,
            )),
            E::NukesCancelled { launcher, other } if launcher == HUMAN || other == HUMAN => Some((
                "Missiles between new allies were called off".to_string(),
                NoticeKind::Info,
            )),
            _ => None,
        };
        out.extend(n);
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
    mut motion: ResMut<UnitMotion>,
    mut log: ResMut<save::IntentLog>,
    mut sounds: ResMut<audio::SoundQueue>,
) {
    if settings.paused {
        return;
    }
    use audio::Sfx;
    let intents = std::mem::take(&mut pending.0);
    let g = &mut game.0;
    if !intents.is_empty() {
        log.0.push((g.ticks(), intents.clone()));
    }
    for i in &intents {
        match i {
            Intent::Attack { .. } | Intent::Boat { .. } => sounds.0.push(Sfx::Attack),
            Intent::Build { .. } | Intent::Upgrade { .. } | Intent::Warship { .. } => {
                sounds.0.push(Sfx::Build)
            }
            Intent::Nuke { .. } => sounds.0.push(Sfx::Launch),
            _ => {}
        }
    }
    let before = snapshot(g);
    motion.prev.clear();
    for u in g
        .units()
        .iter()
        .filter(|u| u.is_active() && u.kind().is_mobile())
    {
        motion.prev.insert(u.id(), u.tile());
    }
    let had_winner = g.winner().is_some();
    run_turn(g, &intents);

    for &(t, r) in g.detonations() {
        fx.blasts.push((t, r, 0.0));
    }
    if g.detonations()
        .iter()
        .any(|&(_, r)| r > g.config().weapons.warhead_outer)
    {
        sounds.0.push(Sfx::Explosion);
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
        if kind == NoticeKind::Danger {
            sounds.0.push(Sfx::Warning);
        }
        notices.push_kind(text, kind);
    }
    for (text, kind) in diplo_notices(g) {
        match kind {
            NoticeKind::Good => sounds.0.push(Sfx::Alliance),
            NoticeKind::Danger => sounds.0.push(Sfx::Warning),
            _ => {}
        }
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
        stats.peak_troops = stats.peak_troops.max(me.troops());
        if g.ticks().is_multiple_of(100) && me.has_spawned() && !stats.ended {
            stats
                .land_history
                .push((me.num_tiles() as f64 / land) as f32);
            let built = g
                .player_units(HUMAN, None)
                .filter(|u| u.kind().is_structure())
                .count();
            stats.peak_buildings = stats.peak_buildings.max(built);
        }
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
            let my_team = g.player(HUMAN).and_then(|p| p.team());
            if w == HUMAN || (my_team.is_some() && g.winner_team() == my_team) {
                sounds.0.push(Sfx::Victory);
                notices.push_kind("You won!", NoticeKind::Good);
            } else {
                sounds.0.push(Sfx::Defeat);
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
    mut settings: ResMut<ClientSettings>,
    mut start: ResMut<StartMatch>,
    mut log: ResMut<save::IntentLog>,
) {
    let next = if let Some(s) = start.0.take() {
        s
    } else if req.0 {
        let mut n = setup.0;
        n.seed = n.seed.wrapping_add(1);
        n
    } else {
        return;
    };
    req.0 = false;
    settings.menu_tile = None;
    settings.selected_ship = None;
    settings.build = None;
    settings.paused = false;
    match new_match(game.0.config().clone(), next) {
        Ok(g) => {
            game.0 = g;
            setup.0 = next;
            log.0.clear();
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
        let mut g = new_match(cfg, MatchSetup::bots_only(MapSize::Small, 3, 10)).unwrap();
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
    fn alliance_requests_and_answers_become_notices() {
        let cfg = GameConfig::load(GAME_CONFIG_PATH).unwrap();
        let mut g = new_match(cfg, MatchSetup::bots_only(MapSize::Small, 4, 2)).unwrap();
        run_turn(&mut g, &[]);
        let map = g.map();
        let tile = (0..map.num_tiles())
            .find(|&t| crate::engine::util::spawn_tiles(map, t, 4, true).is_some())
            .unwrap();
        run_turn(
            &mut g,
            &[Intent::Spawn {
                player: HUMAN,
                tile,
            }],
        );
        while g.in_spawn_phase() {
            run_turn(&mut g, &[]);
        }
        let mut texts = Vec::new();
        let mut step = |g: &mut Game, i: &[Intent]| {
            run_turn(g, i);
            texts.extend(diplo_notices(g).into_iter().map(|n| n.0));
        };
        step(
            &mut g,
            &[Intent::AllianceRequest {
                player: 2,
                to: HUMAN,
            }],
        );
        step(&mut g, &[]);
        step(
            &mut g,
            &[Intent::AllianceReply {
                player: HUMAN,
                from: 2,
                accept: true,
            }],
        );
        step(&mut g, &[]);
        step(
            &mut g,
            &[Intent::BreakAlliance {
                player: 2,
                with: HUMAN,
            }],
        );
        step(&mut g, &[]);
        let all = texts.join(" | ");
        assert!(all.contains("asks for an alliance"), "{all}");
        assert!(all.contains("You are now allied with"), "{all}");
        assert!(all.contains("betrayed you"), "{all}");
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
