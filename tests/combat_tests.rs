use openfront_clone::combat::{resolve, AttackOutcome};
use openfront_clone::core::{OrderError, Owner};
use openfront_clone::sim::{Event, Order};
use openfront_clone::territory::Territory;

mod common;
use common::{P0, P1};

fn target(owner: Owner, troops: u32) -> Territory {
    Territory {
        owner,
        troops,
        troop_cap: 100,
        gold_per_min: 60,
    }
}

#[test]
fn attacker_wins_and_captures() {
    let mut t = target(Owner::Player(P1), 10);
    assert_eq!(
        resolve(P0, 15, &mut t),
        AttackOutcome::Captured { remaining: 5 }
    );
    assert_eq!(t.owner, Owner::Player(P0));
    assert_eq!(t.troops, 5);
}

#[test]
fn attacker_loses() {
    let mut t = target(Owner::Player(P1), 10);
    assert_eq!(
        resolve(P0, 4, &mut t),
        AttackOutcome::Repelled { defender_left: 6 }
    );
    assert_eq!(t.owner, Owner::Player(P1));
    assert_eq!(t.troops, 6);
}

#[test]
fn tie_leaves_defender_with_zero() {
    let mut t = target(Owner::Player(P1), 10);
    assert_eq!(resolve(P0, 10, &mut t), AttackOutcome::Tie);
    assert_eq!(t.owner, Owner::Player(P1));
    assert_eq!(t.troops, 0);
}

#[test]
fn one_troop_captures_empty_neutral() {
    let mut t = target(Owner::Neutral, 0);
    assert_eq!(
        resolve(P0, 1, &mut t),
        AttackOutcome::Captured { remaining: 1 }
    );
    assert_eq!(t.owner, Owner::Player(P0));
}

/// Full path through the World: the order is queued, resolves on the next tick, and the
/// source keeps the troops it did not send.
#[test]
fn capture_through_world_resolves_next_tick() {
    let mut w = common::world(2, 1);
    common::clear_board(&mut w, 10);
    let (a, b) = common::adjacent_pair(&w);
    w.set_territory(a, Owner::Player(P0), 40).unwrap();
    let far = common::far_from(&w, a, b);
    w.set_territory(far, Owner::Player(P1), 10).unwrap();

    w.queue_order(Order::Attack {
        player: P0,
        from: a,
        to: b,
        troops: 30,
    })
    .unwrap();
    assert_eq!(
        w.territory(b).owner,
        Owner::Neutral,
        "must not resolve before the tick"
    );

    w.tick();
    assert_eq!(w.territory(b).owner, Owner::Player(P0));
    assert_eq!(w.territory(b).troops, 20);
    assert_eq!(w.territory(a).troops, 10);
    assert!(w.events().iter().any(|e| matches!(
        e,
        Event::Attack {
            outcome: AttackOutcome::Captured { remaining: 20 },
            ..
        }
    )));
}

#[test]
fn attacking_a_non_neighbor_is_refused() {
    let mut w = common::world(2, 1);
    let (a, _) = common::adjacent_pair(&w);
    w.set_territory(a, Owner::Player(P0), 40).unwrap();
    let far = common::non_neighbor(&w, a);
    let err = w.queue_order(Order::Attack {
        player: P0,
        from: a,
        to: far,
        troops: 5,
    });
    assert_eq!(err, Err(OrderError::NotNeighbor));
}

#[test]
fn sending_every_troop_is_refused() {
    let mut w = common::world(2, 1);
    let (a, b) = common::adjacent_pair(&w);
    w.set_territory(a, Owner::Player(P0), 40).unwrap();
    w.set_territory(b, Owner::Neutral, 5).unwrap();
    let err = w.queue_order(Order::Attack {
        player: P0,
        from: a,
        to: b,
        troops: 40,
    });
    assert_eq!(err, Err(OrderError::NotEnoughTroops));
    assert!(w
        .queue_order(Order::Attack {
            player: P0,
            from: a,
            to: b,
            troops: 39
        })
        .is_ok());
}
