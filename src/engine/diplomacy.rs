//! Alliances, traitors, relations, embargoes, donations, targets and reactions.
//! Port of upstream `game/AllianceImpl.ts`, `game/AllianceRequestImpl.ts`, the diplomacy parts
//! of `game/PlayerImpl.ts` and `game/GameImpl.ts`, and the executions in
//! `execution/alliance/*`, `DonateGoldExecution.ts`, `DonateTroopExecution.ts`,
//! `EmbargoExecution.ts`, `TargetPlayerExecution.ts` and `EmojiExecution.ts`.
//!
//! Differences (D40): reactions are short words of our own instead of upstream's emoji table;
//! no teams yet (P12), so "friendly" means allied.

use std::collections::BTreeMap;

use crate::config::Difficulty;
use crate::engine_lib::game_map::TileRef;
use crate::engine_lib::random::PseudoRandom;

use super::game::{Execution, Game};
use super::player::{PlayerId, PlayerType};
use super::rules;
use super::units::UnitKind;

/// Reactions players can show above their land (ours, D40).
pub const REACTIONS: [&str; 12] = [
    "Hello!", "Thanks!", "Sorry", "Help!", "Friends?", "Angry!", "Scared", "?!", "Nice", "GG",
    "Traitor!", "Peace?",
];
pub const R_THANKS: u8 = 1;
pub const R_HELP: u8 = 3;
pub const R_FRIENDS: u8 = 4;
pub const R_ANGRY: u8 = 5;
pub const R_SCARED: u8 = 6;
pub const R_CONFUSED: u8 = 7;
pub const R_TRAITOR: u8 = 10;

/// Upstream `AllianceImpl`.
#[derive(Debug, Clone, PartialEq)]
pub struct Alliance {
    pub id: u32,
    pub requestor: PlayerId,
    pub recipient: PlayerId,
    pub created: u64,
    pub expires: u64,
    ext_requestor: bool,
    ext_recipient: bool,
}

impl Alliance {
    pub fn other(&self, p: PlayerId) -> PlayerId {
        if self.requestor == p {
            self.recipient
        } else {
            self.requestor
        }
    }

    pub fn involves(&self, p: PlayerId) -> bool {
        self.requestor == p || self.recipient == p
    }

    pub fn agreed_to_extend(&self, p: PlayerId) -> bool {
        (self.requestor == p && self.ext_requestor) || (self.recipient == p && self.ext_recipient)
    }

    /// Upstream `onlyOneAgreedToExtend`.
    pub fn only_one_agreed(&self) -> bool {
        self.ext_requestor != self.ext_recipient
    }
}

/// Upstream `AllianceRequestImpl` (pending ones only; answered ones are removed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllianceRequest {
    pub requestor: PlayerId,
    pub recipient: PlayerId,
    pub created: u64,
}

/// Diplomacy happenings of the current tick, for the client's notices.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DiploEvent {
    Requested {
        from: PlayerId,
        to: PlayerId,
    },
    Accepted {
        a: PlayerId,
        b: PlayerId,
    },
    Rejected {
        from: PlayerId,
        to: PlayerId,
    },
    Broken {
        traitor: PlayerId,
        betrayed: PlayerId,
    },
    Expired {
        a: PlayerId,
        b: PlayerId,
    },
    Renewed {
        a: PlayerId,
        b: PlayerId,
    },
    RenewAsked {
        from: PlayerId,
        to: PlayerId,
    },
    DonatedTroops {
        from: PlayerId,
        to: PlayerId,
        amount: i64,
    },
    DonatedGold {
        from: PlayerId,
        to: PlayerId,
        amount: i64,
    },
    Embargo {
        from: PlayerId,
        to: PlayerId,
        on: bool,
    },
    Target {
        from: PlayerId,
        to: PlayerId,
    },
    NukesCancelled {
        launcher: PlayerId,
        other: PlayerId,
    },
}

/// A reaction on screen: sender, recipient (`None` = everyone), reaction index, tick sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reaction {
    pub from: PlayerId,
    pub to: Option<PlayerId>,
    pub what: u8,
    pub tick: u64,
}

impl Game {
    // ----- reading -----

    pub fn alliances(&self) -> &[Alliance] {
        &self.diplo.alliances
    }

    pub fn alliance_requests(&self) -> &[AllianceRequest] {
        &self.diplo.requests
    }

    pub fn diplo_events(&self) -> &[DiploEvent] {
        &self.diplo.events
    }

    pub fn reactions(&self) -> &[Reaction] {
        &self.diplo.reactions
    }

    pub fn alliance_with(&self, a: PlayerId, b: PlayerId) -> Option<&Alliance> {
        if a == b {
            return None;
        }
        self.diplo
            .alliances
            .iter()
            .find(|x| x.involves(a) && x.involves(b))
    }

    /// Upstream `isAlliedWith`.
    pub fn is_allied(&self, a: PlayerId, b: PlayerId) -> bool {
        self.alliance_with(a, b).is_some()
    }

    /// Upstream `allies()`.
    pub fn allies(&self, p: PlayerId) -> Vec<PlayerId> {
        self.diplo
            .alliances
            .iter()
            .filter(|x| x.involves(p))
            .map(|x| x.other(p))
            .collect()
    }

    /// Upstream `isFriendly` (no teams yet): yourself or an ally.
    pub fn is_friendly(&self, a: PlayerId, b: PlayerId) -> bool {
        a == b || self.is_allied(a, b)
    }

    /// Upstream `getTraitorRemainingTicks`.
    pub fn traitor_remaining(&self, p: PlayerId) -> u64 {
        let Some(t) = self.player(p).and_then(|p| p.traitor_tick) else {
            return 0;
        };
        let elapsed = self.ticks().saturating_sub(t);
        self.config()
            .diplomacy
            .traitor_duration_ticks
            .saturating_sub(elapsed)
    }

    pub fn is_traitor(&self, p: PlayerId) -> bool {
        self.traitor_remaining(p) > 0
    }

    pub fn incoming_requests(&self, p: PlayerId) -> impl Iterator<Item = &AllianceRequest> {
        self.diplo.requests.iter().filter(move |r| r.recipient == p)
    }

    pub fn outgoing_requests(&self, p: PlayerId) -> impl Iterator<Item = &AllianceRequest> {
        self.diplo.requests.iter().filter(move |r| r.requestor == p)
    }

    pub fn has_request(&self, from: PlayerId, to: PlayerId) -> bool {
        self.diplo
            .requests
            .iter()
            .any(|r| r.requestor == from && r.recipient == to)
    }

    /// Upstream `canSendAllianceRequest`.
    pub fn can_send_alliance_request(&self, from: PlayerId, to: PlayerId) -> bool {
        if from == to || self.is_friendly(from, to) {
            return false;
        }
        let (Some(a), Some(b)) = (self.player(from), self.player(to)) else {
            return false;
        };
        if !a.is_alive() || !b.is_alive() {
            return false;
        }
        if self.has_request(from, to) {
            return false;
        }
        if self.has_request(to, from) {
            return true;
        }
        let last = a
            .past_requests
            .iter()
            .filter(|r| r.0 == to)
            .map(|r| r.1)
            .max();
        match last {
            None => true,
            Some(t) => {
                self.ticks().saturating_sub(t)
                    >= self.config().diplomacy.alliance_request_cooldown_ticks
            }
        }
    }

    /// Upstream `allianceInfo().inExtensionWindow`.
    pub fn in_extension_window(&self, a: &Alliance) -> bool {
        a.expires <= self.ticks() + self.config().diplomacy.alliance_extension_prompt_ticks
    }

    /// Upstream `canTrade`: no embargo either way.
    pub fn can_trade(&self, a: PlayerId, b: PlayerId) -> bool {
        if a == b {
            return false;
        }
        let ea = self.player(a).is_some_and(|p| p.has_embargo_against(b));
        let eb = self.player(b).is_some_and(|p| p.has_embargo_against(a));
        !ea && !eb
    }

    /// Upstream `canDonateGold` / `canDonateTroops` (both use the same rules).
    pub fn can_donate(&self, from: PlayerId, to: PlayerId) -> bool {
        if from == to || !self.is_friendly(from, to) {
            return false;
        }
        let (Some(a), Some(b)) = (self.player(from), self.player(to)) else {
            return false;
        };
        if !a.is_alive() || !b.is_alive() {
            return false;
        }
        let cd = self.config().diplomacy.donate_cooldown_ticks;
        !a.donations
            .iter()
            .any(|d| d.0 == to && self.ticks().saturating_sub(d.1) < cd)
    }

    /// Upstream `canTarget`.
    pub fn can_target(&self, from: PlayerId, to: PlayerId) -> bool {
        if from == to || self.is_friendly(from, to) {
            return false;
        }
        let Some(a) = self.player(from) else {
            return false;
        };
        let cd = self.config().diplomacy.target_cooldown_ticks;
        !a.targets
            .iter()
            .any(|t| self.ticks().saturating_sub(t.0) < cd)
    }

    /// Upstream `targets()`: players `p` marked within the target duration.
    pub fn targets(&self, p: PlayerId) -> Vec<PlayerId> {
        let d = self.config().diplomacy.target_duration_ticks;
        self.player(p).map_or(Vec::new(), |pl| {
            pl.targets
                .iter()
                .filter(|t| self.ticks().saturating_sub(t.0) < d)
                .map(|t| t.1)
                .collect()
        })
    }

    // ----- changing -----

    pub(crate) fn push_event(&mut self, e: DiploEvent) {
        self.diplo.events.push(e);
    }

    pub(crate) fn update_relation(&mut self, p: PlayerId, other: PlayerId, delta: f64) {
        if let Some(pl) = self.player_mut(p) {
            pl.update_relation(other, delta);
        }
    }

    /// Upstream `GameImpl.createAllianceRequest` (accepts at once if `to` already asked us).
    pub(crate) fn create_alliance_request(&mut self, from: PlayerId, to: PlayerId) -> bool {
        if self.is_allied(from, to) || self.has_request(from, to) {
            return false;
        }
        if self.has_request(to, from) {
            self.accept_alliance_request(to, from);
            return false;
        }
        self.diplo.requests.push(AllianceRequest {
            requestor: from,
            recipient: to,
            created: self.ticks(),
        });
        self.push_event(DiploEvent::Requested { from, to });
        true
    }

    /// Removes a pending request and remembers when it was made (upstream
    /// `pastOutgoingAllianceRequests`, for the cooldown).
    fn take_request(&mut self, from: PlayerId, to: PlayerId) -> bool {
        let Some(i) = self
            .diplo
            .requests
            .iter()
            .position(|r| r.requestor == from && r.recipient == to)
        else {
            return false;
        };
        let r = self.diplo.requests.remove(i);
        if let Some(p) = self.player_mut(from) {
            p.past_requests.push((to, r.created));
        }
        true
    }

    /// Upstream `acceptAllianceRequest`.
    pub(crate) fn accept_alliance_request(&mut self, from: PlayerId, to: PlayerId) {
        if !self.take_request(from, to) || self.is_allied(from, to) {
            return;
        }
        let t = self.ticks();
        let id = self.diplo.next_alliance_id;
        self.diplo.next_alliance_id += 1;
        self.diplo.alliances.push(Alliance {
            id,
            requestor: from,
            recipient: to,
            created: t,
            expires: t + self.config().diplomacy.alliance_duration_ticks,
            ext_requestor: false,
            ext_recipient: false,
        });
        self.push_event(DiploEvent::Accepted { a: from, b: to });
    }

    /// Upstream `rejectAllianceRequest`.
    pub(crate) fn reject_alliance_request(&mut self, from: PlayerId, to: PlayerId) {
        if self.take_request(from, to) {
            self.push_event(DiploEvent::Rejected { from, to });
        }
    }

    fn remove_alliance(&mut self, a: PlayerId, b: PlayerId) -> Option<Alliance> {
        let i = self
            .diplo
            .alliances
            .iter()
            .position(|x| x.involves(a) && x.involves(b) && a != b)?;
        Some(self.diplo.alliances.remove(i))
    }

    /// Upstream `GameImpl.breakAlliance`: the breaker becomes a traitor unless the other one
    /// already is.
    pub(crate) fn break_alliance(&mut self, breaker: PlayerId, other: PlayerId) {
        if self.remove_alliance(breaker, other).is_none() {
            return;
        }
        if !self.is_traitor(other) {
            let t = self.ticks();
            if let Some(p) = self.player_mut(breaker) {
                p.traitor_tick = Some(t);
                p.betrayals += 1;
            }
        }
        self.push_event(DiploEvent::Broken {
            traitor: breaker,
            betrayed: other,
        });
    }

    /// Upstream `expireAlliance` for every alliance past its end.
    pub(crate) fn expire_alliances(&mut self) {
        let now = self.ticks();
        let ended: Vec<(PlayerId, PlayerId)> = self
            .diplo
            .alliances
            .iter()
            .filter(|a| a.expires <= now)
            .map(|a| (a.requestor, a.recipient))
            .collect();
        for (a, b) in ended {
            self.remove_alliance(a, b);
            self.push_event(DiploEvent::Expired { a, b });
        }
        // Alliances with dead players end too.
        let dead: Vec<(PlayerId, PlayerId)> = self
            .diplo
            .alliances
            .iter()
            .filter(|a| {
                !self.player(a.requestor).is_some_and(|p| p.is_alive())
                    || !self.player(a.recipient).is_some_and(|p| p.is_alive())
            })
            .map(|a| (a.requestor, a.recipient))
            .collect();
        for (a, b) in dead {
            self.remove_alliance(a, b);
        }
        let ended_requests: Vec<(PlayerId, PlayerId)> = self
            .diplo
            .requests
            .iter()
            .filter(|r| {
                !self.player(r.requestor).is_some_and(|p| p.is_alive())
                    || !self.player(r.recipient).is_some_and(|p| p.is_alive())
            })
            .map(|r| (r.requestor, r.recipient))
            .collect();
        for (a, b) in ended_requests {
            self.take_request(a, b);
        }
        let d = self.config().diplomacy.reaction_duration_ticks;
        self.diplo
            .reactions
            .retain(|r| now.saturating_sub(r.tick) < d);
    }

    /// Upstream `addEmbargo`: a manual embargo replaces a temporary one, never the reverse.
    pub(crate) fn add_embargo(&mut self, p: PlayerId, other: PlayerId, temporary: bool) {
        if p == other {
            return;
        }
        let t = self.ticks();
        let Some(pl) = self.player_mut(p) else { return };
        if pl.embargoes.get(&other).is_some_and(|e| !e.1) {
            return;
        }
        let was = pl.embargoes.insert(other, (t, temporary)).is_some();
        if !was {
            self.push_event(DiploEvent::Embargo {
                from: p,
                to: other,
                on: true,
            });
        }
    }

    pub(crate) fn stop_embargo(&mut self, p: PlayerId, other: PlayerId) {
        let Some(pl) = self.player_mut(p) else { return };
        if pl.embargoes.remove(&other).is_some() {
            self.push_event(DiploEvent::Embargo {
                from: p,
                to: other,
                on: false,
            });
        }
    }

    /// Upstream `endTemporaryEmbargo`.
    pub(crate) fn end_temporary_embargo(&mut self, p: PlayerId, other: PlayerId) {
        if self
            .player(p)
            .is_some_and(|x| x.embargoes.get(&other).is_some_and(|e| e.1))
        {
            self.stop_embargo(p, other);
        }
    }

    /// Ends temporary embargoes of `p` older than `temporary_embargo_ticks`.
    pub(crate) fn expire_embargoes(&mut self, p: PlayerId) {
        let now = self.ticks();
        let d = self.config().diplomacy.temporary_embargo_ticks;
        let old: Vec<PlayerId> = self.player(p).map_or(Vec::new(), |pl| {
            pl.embargoes
                .iter()
                .filter(|(_, e)| e.1 && now.saturating_sub(e.0) >= d)
                .map(|(k, _)| *k)
                .collect()
        });
        for o in old {
            self.stop_embargo(p, o);
        }
    }

    pub(crate) fn add_reaction(&mut self, from: PlayerId, to: Option<PlayerId>, what: u8) {
        let what = what.min(REACTIONS.len() as u8 - 1);
        let tick = self.ticks();
        self.diplo
            .reactions
            .retain(|r| !(r.from == from && r.to == to));
        self.diplo.reactions.push(Reaction {
            from,
            to,
            what,
            tick,
        });
    }

    /// Upstream `cancelNukesBetweenAlliedPlayers` (simplified: by the target tile's owner).
    fn cancel_nukes_between(&mut self, a: PlayerId, b: PlayerId) {
        let mut hit: Vec<(PlayerId, PlayerId, u32)> = Vec::new();
        for u in self
            .units()
            .iter()
            .filter(|u| u.is_active() && u.kind().is_nuke())
        {
            let other = if u.owner() == a {
                b
            } else if u.owner() == b {
                a
            } else {
                continue;
            };
            let Some(t) = u.target_tile() else { continue };
            if self.owner(t) == Some(other) {
                hit.push((u.owner(), other, u.id()));
            }
        }
        let mut told = Vec::new();
        for (launcher, other, id) in hit {
            self.delete_unit(id);
            if !told.contains(&launcher) {
                told.push(launcher);
                self.push_event(DiploEvent::NukesCancelled { launcher, other });
            }
        }
    }
}

/// Diplomacy state kept by `Game`.
#[derive(Debug, Default, Clone)]
pub struct DiplomacyState {
    pub(crate) alliances: Vec<Alliance>,
    pub(crate) requests: Vec<AllianceRequest>,
    pub(crate) next_alliance_id: u32,
    pub(crate) events: Vec<DiploEvent>,
    pub(crate) reactions: Vec<Reaction>,
}

/// Tiles of each player a nuke at `tile` would hit, as upstream's `listNukeBreakAlliance`
/// counts them: every tile within the inner radius, half of those in the outer ring.
pub fn nuke_tiles_by_player(
    game: &Game,
    tile: TileRef,
    inner: u32,
    outer: u32,
) -> BTreeMap<PlayerId, u32> {
    let map = game.map();
    let (cx, cy) = (map.x(tile) as i64, map.y(tile) as i64);
    let (r, ri) = (outer as i64, inner as i64);
    let mut out: BTreeMap<PlayerId, u32> = BTreeMap::new();
    for dy in -r..=r {
        for dx in -r..=r {
            let d2 = dx * dx + dy * dy;
            if d2 > r * r {
                continue;
            }
            let Some(t) = map.tile(cx + dx, cy + dy) else {
                continue;
            };
            let o = map.owner_id(t);
            if o == 0 {
                continue;
            }
            let w = if d2 <= ri * ri { 2 } else { 1 };
            *out.entry(o).or_insert(0) += w;
        }
    }
    for v in out.values_mut() {
        *v /= 2;
    }
    out
}

/// Upstream `NukeExecution.maybeBreakAlliances` at launch: allies with at least the threshold of
/// tiles in the blast lose the alliance (the launcher becomes a traitor), and their pending
/// requests to the launcher are rejected. Everyone hit dislikes the launcher.
pub(crate) fn nuke_breaks_alliances(
    game: &mut Game,
    launcher: PlayerId,
    kind: UnitKind,
    tile: TileRef,
) {
    if kind == UnitKind::MirvWarhead {
        return;
    }
    let (inner, outer) = super::weapons::blast_radius(game, kind);
    let threshold = game.config().diplomacy.nuke_alliance_break_threshold;
    let hit = nuke_tiles_by_player(game, tile, inner, outer);
    for (p, n) in hit {
        if p == launcher || n < threshold {
            continue;
        }
        if game.has_request(p, launcher) {
            game.reject_alliance_request(p, launcher);
            continue;
        }
        if game.is_allied(launcher, p) {
            game.break_alliance(launcher, p);
        }
        game.update_relation(p, launcher, -100.0);
    }
}

// ----- executions -----

/// Upstream `AllianceRequestExecution`: ask `to` for an alliance (or accept their request).
pub struct AllianceRequestExecution {
    from: PlayerId,
    to: PlayerId,
    created: Option<u64>,
    active: bool,
}

impl AllianceRequestExecution {
    pub fn new(from: PlayerId, to: PlayerId) -> Self {
        Self {
            from,
            to,
            created: None,
            active: true,
        }
    }
}

impl Execution for AllianceRequestExecution {
    fn init(&mut self, game: &mut Game, _ticks: u64) {
        let (a, b) = (self.from, self.to);
        if !game.can_send_alliance_request(a, b) {
            self.active = false;
            return;
        }
        if game.has_request(b, a) {
            self.active = false;
            game.accept_alliance_request(b, a);
            game.update_relation(a, b, 100.0);
            game.update_relation(b, a, 100.0);
            game.end_temporary_embargo(a, b);
            game.end_temporary_embargo(b, a);
            game.cancel_nukes_between(a, b);
        } else if game.create_alliance_request(a, b) {
            self.created = Some(game.ticks());
        } else {
            self.active = false;
        }
    }

    fn tick(&mut self, game: &mut Game, _ticks: u64) {
        let Some(created) = self.created else {
            self.active = false;
            return;
        };
        if !game.has_request(self.from, self.to) {
            self.active = false;
            return;
        }
        if game.ticks().saturating_sub(created)
            > game.config().diplomacy.alliance_request_duration_ticks
        {
            game.reject_alliance_request(self.from, self.to);
            self.active = false;
        }
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn active_during_spawn_phase(&self) -> bool {
        false
    }
}

/// Upstream `AllianceRejectExecution` (and accepting, which upstream does by sending a request
/// back): `recipient` answers `requestor`'s request.
pub struct AllianceReplyExecution {
    requestor: PlayerId,
    recipient: PlayerId,
    accept: bool,
    active: bool,
}

impl AllianceReplyExecution {
    pub fn new(requestor: PlayerId, recipient: PlayerId, accept: bool) -> Self {
        Self {
            requestor,
            recipient,
            accept,
            active: true,
        }
    }
}

impl Execution for AllianceReplyExecution {
    fn init(&mut self, _game: &mut Game, _ticks: u64) {}

    fn tick(&mut self, game: &mut Game, _ticks: u64) {
        self.active = false;
        let (a, b) = (self.requestor, self.recipient);
        if !game.has_request(a, b) {
            return;
        }
        if self.accept {
            game.accept_alliance_request(a, b);
            game.update_relation(a, b, 100.0);
            game.update_relation(b, a, 100.0);
            game.end_temporary_embargo(a, b);
            game.end_temporary_embargo(b, a);
            game.cancel_nukes_between(a, b);
        } else {
            game.reject_alliance_request(a, b);
        }
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn active_during_spawn_phase(&self) -> bool {
        false
    }
}

/// Upstream `BreakAllianceExecution`.
pub struct BreakAllianceExecution {
    from: PlayerId,
    to: PlayerId,
    active: bool,
}

impl BreakAllianceExecution {
    pub fn new(from: PlayerId, to: PlayerId) -> Self {
        Self {
            from,
            to,
            active: true,
        }
    }
}

impl Execution for BreakAllianceExecution {
    fn init(&mut self, _game: &mut Game, _ticks: u64) {}

    fn tick(&mut self, game: &mut Game, _ticks: u64) {
        self.active = false;
        if !game.is_allied(self.from, self.to) {
            return;
        }
        game.break_alliance(self.from, self.to);
        game.update_relation(self.to, self.from, -100.0);
        let neighbors: Vec<PlayerId> = game.nearby(self.from).into_iter().flatten().collect();
        for n in neighbors {
            if n != self.to {
                game.update_relation(n, self.from, -40.0);
            }
        }
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn active_during_spawn_phase(&self) -> bool {
        false
    }
}

/// Upstream `AllianceExtensionExecution`: renews when both sides agreed.
pub struct AllianceExtensionExecution {
    from: PlayerId,
    to: PlayerId,
    active: bool,
}

impl AllianceExtensionExecution {
    pub fn new(from: PlayerId, to: PlayerId) -> Self {
        Self {
            from,
            to,
            active: true,
        }
    }
}

impl Execution for AllianceExtensionExecution {
    fn init(&mut self, game: &mut Game, _ticks: u64) {
        self.active = false;
        let (a, b) = (self.from, self.to);
        let window = game.config().diplomacy.alliance_extension_prompt_ticks;
        let duration = game.config().diplomacy.alliance_duration_ticks;
        let now = game.ticks();
        let Some(al) = game
            .diplo
            .alliances
            .iter_mut()
            .find(|x| x.involves(a) && x.involves(b) && a != b)
        else {
            return;
        };
        if al.expires > now + window {
            return;
        }
        if al.requestor == a {
            al.ext_requestor = true;
        } else {
            al.ext_recipient = true;
        }
        if al.ext_requestor && al.ext_recipient {
            al.ext_requestor = false;
            al.ext_recipient = false;
            al.expires = now + duration;
            game.push_event(DiploEvent::Renewed { a, b });
        } else {
            game.push_event(DiploEvent::RenewAsked { from: a, to: b });
        }
    }

    fn tick(&mut self, _game: &mut Game, _ticks: u64) {}

    fn is_active(&self) -> bool {
        self.active
    }

    fn active_during_spawn_phase(&self) -> bool {
        false
    }
}

/// Upstream `EmbargoExecution`.
pub struct EmbargoExecution {
    from: PlayerId,
    to: PlayerId,
    on: bool,
    active: bool,
}

impl EmbargoExecution {
    pub fn new(from: PlayerId, to: PlayerId, on: bool) -> Self {
        Self {
            from,
            to,
            on,
            active: true,
        }
    }
}

impl Execution for EmbargoExecution {
    fn init(&mut self, _game: &mut Game, _ticks: u64) {}

    fn tick(&mut self, game: &mut Game, _ticks: u64) {
        self.active = false;
        if self.on {
            game.add_embargo(self.from, self.to, false);
        } else {
            game.stop_embargo(self.from, self.to);
        }
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn active_during_spawn_phase(&self) -> bool {
        false
    }
}

/// Upstream `DonateTroopExecution` and `DonateGoldExecution` in one: `troops = Some(None)`
/// sends a third of the troops, `gold = Some(None)` a third of the gold.
pub struct DonateExecution {
    from: PlayerId,
    to: PlayerId,
    troops: Option<Option<f64>>,
    gold: Option<Option<i64>>,
    active: bool,
}

impl DonateExecution {
    pub fn troops(from: PlayerId, to: PlayerId, amount: Option<f64>) -> Self {
        Self {
            from,
            to,
            troops: Some(amount),
            gold: None,
            active: true,
        }
    }

    pub fn gold(from: PlayerId, to: PlayerId, amount: Option<i64>) -> Self {
        Self {
            from,
            to,
            troops: None,
            gold: Some(amount),
            active: true,
        }
    }
}

fn gold_chunk(d: Difficulty) -> f64 {
    match d {
        Difficulty::Easy => 2_500.0,
        Difficulty::Medium => 5_000.0,
        Difficulty::Hard => 12_500.0,
        Difficulty::Impossible => 25_000.0,
    }
}

/// Upstream `getMinTroopsForRelationUpdate`: max troops / (13..11, 11..9, 9..7, 7..5).
fn min_troops_for_relation(d: Difficulty, max: f64, random: &mut PseudoRandom) -> f64 {
    let (a, b) = match d {
        Difficulty::Easy => (13.0, 11.0),
        Difficulty::Medium => (11.0, 9.0),
        Difficulty::Hard => (9.0, 7.0),
        Difficulty::Impossible => (7.0, 5.0),
    };
    random.next_int((max / a) as i64, (max / b) as i64 + 1) as f64
}

impl Execution for DonateExecution {
    fn init(&mut self, _game: &mut Game, _ticks: u64) {}

    fn tick(&mut self, game: &mut Game, ticks: u64) {
        self.active = false;
        let (a, b) = (self.from, self.to);
        if !game.can_donate(a, b) {
            return;
        }
        let diff = game.config().difficulty;
        if let Some(amount) = self.troops {
            let (Some(s), Some(r)) = (game.player(a), game.player(b)) else {
                return;
            };
            let want = amount.unwrap_or(s.troops() as f64 / 3.0);
            let room = (rules::troop_cap(game.config(), r) - r.troops() as f64).max(0.0);
            let want = want.min(room);
            let max = rules::troop_cap(game.config(), r);
            let to_nation = r.kind() == PlayerType::Nation;
            if want < 1.0 {
                return;
            }
            let removed = game.player_mut(a).map_or(0, |p| p.remove_troops(want));
            if removed == 0 {
                return;
            }
            if let Some(r) = game.player_mut(b) {
                r.add_troops(removed as f64);
            }
            if let Some(s) = game.player_mut(a) {
                s.donations.push((b, ticks));
            }
            let mut random = PseudoRandom::new(ticks as i64);
            if removed as f64 >= min_troops_for_relation(diff, max, &mut random) {
                game.update_relation(b, a, 50.0);
                if to_nation {
                    game.add_reaction(b, Some(a), R_THANKS);
                }
            }
            game.push_event(DiploEvent::DonatedTroops {
                from: a,
                to: b,
                amount: removed,
            });
        }
        if let Some(amount) = self.gold {
            let Some(s) = game.player(a) else { return };
            let want = amount.unwrap_or(s.gold() / 3);
            if want <= 0 {
                return;
            }
            let removed = game.player_mut(a).map_or(0, |p| p.remove_gold(want));
            if removed == 0 {
                return;
            }
            if let Some(r) = game.player_mut(b) {
                r.add_gold(removed);
            }
            if let Some(s) = game.player_mut(a) {
                s.donations.push((b, ticks));
            }
            // Upstream `calculateRelationUpdate`: 5 per chunk (chunk grows every 5 minutes), max 100.
            let chunk = gold_chunk(diff);
            let mult = ticks as f64 / (3000 + game.config().game.spawn_phase_ticks) as f64;
            let adjusted = (chunk + chunk * mult).round().max(1.0) as i64;
            let rel = ((removed / adjusted) * 5).min(100);
            if rel > 0 {
                game.update_relation(b, a, rel as f64);
            }
            game.push_event(DiploEvent::DonatedGold {
                from: a,
                to: b,
                amount: removed,
            });
        }
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn active_during_spawn_phase(&self) -> bool {
        false
    }
}

/// Upstream `TargetPlayerExecution`: mark a player as a target for your allies.
pub struct TargetPlayerExecution {
    from: PlayerId,
    to: PlayerId,
    active: bool,
}

impl TargetPlayerExecution {
    pub fn new(from: PlayerId, to: PlayerId) -> Self {
        Self {
            from,
            to,
            active: true,
        }
    }
}

impl Execution for TargetPlayerExecution {
    fn init(&mut self, _game: &mut Game, _ticks: u64) {}

    fn tick(&mut self, game: &mut Game, ticks: u64) {
        self.active = false;
        if !game.can_target(self.from, self.to) {
            return;
        }
        if let Some(p) = game.player_mut(self.from) {
            p.targets.push((ticks, self.to));
        }
        game.push_event(DiploEvent::Target {
            from: self.from,
            to: self.to,
        });
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn active_during_spawn_phase(&self) -> bool {
        false
    }
}

/// Upstream `EmojiExecution` with our reactions (D40).
pub struct ReactionExecution {
    from: PlayerId,
    to: Option<PlayerId>,
    what: u8,
    active: bool,
}

impl ReactionExecution {
    pub fn new(from: PlayerId, to: Option<PlayerId>, what: u8) -> Self {
        Self {
            from,
            to,
            what,
            active: true,
        }
    }
}

impl Execution for ReactionExecution {
    fn init(&mut self, _game: &mut Game, _ticks: u64) {}

    fn tick(&mut self, game: &mut Game, _ticks: u64) {
        self.active = false;
        if game.player(self.from).is_some_and(|p| p.is_alive()) {
            game.add_reaction(self.from, self.to, self.what);
        }
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn active_during_spawn_phase(&self) -> bool {
        false
    }
}
