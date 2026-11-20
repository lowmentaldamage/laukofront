use openfront_clone::ai::RandomAttacker;
use openfront_clone::config::MapSize;
use openfront_clone::core::{OrderError, Owner, PlayerId, TerritoryId};
use openfront_clone::sim::{Event, Order, VictoryReason, World};

mod common;
use common::{P0, P1};

fn run_random(players: usize, seed: u64, ticks: u64) -> World {
    let mut w = World::new(common::config(), MapSize::Small, seed, players).unwrap();
    let mut ais: Vec<_> = (0..players)
        .map(|i| RandomAttacker::new(PlayerId(i as u8), seed))
        .collect();
    for _ in 0..ticks {
        for ai in &mut ais {
            ai.think(&mut w);
        }
        w.tick();
        check_invariants(&w);
    }
    w
}

fn check_invariants(w: &World) {
    let n = w.map().territory_count();
    assert_eq!(w.territories().len(), n);
    let mut owned = 0usize;
    for t in w.territories() {
        match t.owner {
            Owner::Neutral => {}
            Owner::Player(p) => {
                assert!(p.index() < w.players().len(), "owner {p:?} does not exist");
                owned += 1;
            }
        }
    }
    let counted: u32 = w.players().iter().map(|p| p.territories).sum();
    assert_eq!(
        counted as usize, owned,
        "player totals disagree with the board"
    );
    for p in w.players() {
        assert_eq!(p.alive, p.territories > 0 || !p.alive);
        if !p.alive {
            assert_eq!(p.territories, 0, "eliminated player still owns land");
        }
    }
}

/// Guide requirement: a 500-tick run keeps every territory with exactly one owner.
/// `Owner` is an enum, so "exactly one" holds by type; this checks every owner is valid
/// and that the per-player totals always match the board.
#[test]
fn random_500_tick_run_keeps_ownership_consistent() {
    for seed in [1, 2, 3] {
        let w = run_random(4, seed, 500);
        assert_eq!(w.tick_count(), 500.min(w.tick_count()));
        let captured: u32 = w.players().iter().map(|p| p.territories).sum();
        assert!(
            captured > 4,
            "seed {seed}: the run must actually capture land"
        );
    }
}

#[test]
fn same_seed_and_orders_give_identical_world() {
    let a = run_random(4, 77, 500);
    let b = run_random(4, 77, 500);
    assert_eq!(a.territories(), b.territories());
    assert_eq!(a.players(), b.players());
}

#[test]
fn players_start_with_one_territory_and_start_troops() {
    let w = common::world(4, 5);
    for p in w.players() {
        assert_eq!(p.territories, 1);
        assert_eq!(p.troops, 50);
        assert!(p.alive);
    }
}

#[test]
fn neutral_troops_are_in_configured_range() {
    let w = common::world(2, 5);
    for t in w.territories().iter().filter(|t| t.owner == Owner::Neutral) {
        assert!((5..=20).contains(&t.troops));
    }
}

#[test]
fn setup_errors() {
    let cfg = common::config();
    assert!(World::new(cfg.clone(), MapSize::Small, 1, 0).is_err());
    assert!(
        World::new(cfg, MapSize::Small, 1, 7).is_err(),
        "max_players is 6"
    );
}

#[test]
fn invalid_orders_are_refused() {
    let mut w = common::world(2, 9);
    common::clear_board(&mut w, 10);
    let (a, b) = common::adjacent_pair(&w);
    w.set_territory(a, Owner::Player(P0), 40).unwrap();
    w.set_territory(b, Owner::Player(P1), 10).unwrap();

    let attack = |player, from, to, troops| Order::Attack {
        player,
        from,
        to,
        troops,
    };
    assert_eq!(
        w.queue_order(attack(PlayerId(9), a, b, 5)),
        Err(OrderError::UnknownPlayer)
    );
    assert_eq!(
        w.queue_order(attack(P0, a, TerritoryId(9999), 5)),
        Err(OrderError::UnknownTerritory)
    );
    assert_eq!(
        w.queue_order(attack(P0, b, a, 5)),
        Err(OrderError::NotOwner)
    );
    assert_eq!(
        w.queue_order(attack(P0, a, b, 0)),
        Err(OrderError::NoTroopsSent)
    );
    assert_eq!(
        w.queue_order(attack(P0, a, b, 40)),
        Err(OrderError::NotEnoughTroops)
    );

    let c = w.map().neighbor_ids(a).find(|&t| t != b).unwrap();
    w.set_territory(c, Owner::Player(P0), 5).unwrap();
    assert_eq!(
        w.queue_order(attack(P0, a, c, 5)),
        Err(OrderError::OwnTarget)
    );

    assert!(
        w.pending_orders().is_empty(),
        "refused orders must not be queued"
    );
}

#[test]
fn stale_order_is_dropped() {
    let mut w = common::world(2, 9);
    common::clear_board(&mut w, 10);
    let (a, b) = common::adjacent_pair(&w);
    w.set_territory(a, Owner::Player(P0), 40).unwrap();
    w.set_territory(b, Owner::Player(P1), 100).unwrap();
    let c = w.map().neighbor_ids(a).find(|&t| t != b).unwrap();

    // P1 captures `a` first in the same tick, so P0's order from `a` is no longer valid.
    w.queue_order(Order::Attack {
        player: P1,
        from: b,
        to: a,
        troops: 90,
    })
    .unwrap();
    w.queue_order(Order::Attack {
        player: P0,
        from: a,
        to: c,
        troops: 30,
    })
    .unwrap();
    w.tick();
    assert_eq!(w.territory(a).owner, Owner::Player(P1));
    assert!(w
        .events()
        .iter()
        .any(|e| matches!(e, Event::OrderDropped { .. })));
    assert_eq!(w.territory(c).owner, Owner::Neutral);
}

#[test]
fn losing_last_territory_eliminates_and_ends_match() {
    let mut w = common::world(2, 9);
    common::clear_board(&mut w, 10);
    let (a, b) = common::adjacent_pair(&w);
    w.set_territory(a, Owner::Player(P0), 100).unwrap();
    w.set_territory(b, Owner::Player(P1), 10).unwrap();

    w.queue_order(Order::Attack {
        player: P0,
        from: a,
        to: b,
        troops: 50,
    })
    .unwrap();
    w.tick();

    assert!(!w.player(P1).unwrap().alive);
    assert_eq!(w.winner(), Some((P0, VictoryReason::Conquest)));
    assert!(w.events().contains(&Event::Eliminated { player: P1 }));
    assert!(w
        .events()
        .iter()
        .any(|e| matches!(e, Event::GameOver { winner: P0, .. })));

    // Nothing changes after the match is over, and new orders are refused.
    let before = w.territories().to_vec();
    w.tick();
    assert_eq!(w.territories(), &before[..]);
    let err = w.queue_order(Order::Reinforce {
        player: P0,
        territory: a,
    });
    assert_eq!(err, Err(OrderError::GameOver));
}

#[test]
fn eliminated_player_cannot_act() {
    let mut w = common::world(3, 9);
    common::clear_board(&mut w, 10);
    let (a, b) = common::adjacent_pair(&w);
    w.set_territory(a, Owner::Player(P0), 100).unwrap();
    w.set_territory(b, Owner::Player(P1), 10).unwrap();
    let far = common::far_from(&w, a, b);
    w.set_territory(far, Owner::Player(PlayerId(2)), 10)
        .unwrap();

    w.queue_order(Order::Attack {
        player: P0,
        from: a,
        to: b,
        troops: 50,
    })
    .unwrap();
    w.tick();
    assert!(!w.player(P1).unwrap().alive);
    assert!(!w.is_over(), "player 2 is still alive");
    let err = w.queue_order(Order::Reinforce {
        player: P1,
        territory: b,
    });
    assert_eq!(err, Err(OrderError::PlayerEliminated));
}

#[test]
fn time_limit_picks_the_player_with_most_territories() {
    let cfg = common::config_with("max_match_ticks = 36000", "max_match_ticks = 20");
    let mut w = World::new(cfg, MapSize::Small, 4, 2).unwrap();
    common::clear_board(&mut w, 10);
    let ids: Vec<_> = w.map().territory_ids().collect();
    w.set_territory(ids[0], Owner::Player(P0), 10).unwrap();
    w.set_territory(ids[1], Owner::Player(P1), 10).unwrap();
    w.set_territory(ids[2], Owner::Player(P1), 10).unwrap();
    for _ in 0..19 {
        w.tick();
    }
    assert!(!w.is_over());
    w.tick();
    assert_eq!(w.winner(), Some((P1, VictoryReason::TimeLimit)));
}

#[test]
fn single_player_match_never_ends_by_conquest() {
    let mut w = common::world(1, 2);
    for _ in 0..200 {
        w.tick();
    }
    assert!(!w.is_over());
}
