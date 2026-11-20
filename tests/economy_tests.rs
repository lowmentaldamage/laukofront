use openfront_clone::core::{OrderError, Owner, TerritoryId};
use openfront_clone::economy::growth_amount;
use openfront_clone::sim::Order;

mod common;
use common::{P0, P1};

#[test]
fn growth_amount_formula() {
    let cfg = common::config();
    assert_eq!(growth_amount(0, &cfg), 1, "minimum +1");
    assert_eq!(
        growth_amount(19, &cfg),
        1,
        "5% of 19 rounds down to 0, min 1"
    );
    assert_eq!(growth_amount(100, &cfg), 5);
}

#[test]
fn growth_stops_at_the_cap() {
    let mut w = common::world(1, 3);
    common::clear_board(&mut w, 10);
    let t = TerritoryId(0);
    w.set_territory(t, Owner::Player(P0), 50).unwrap();
    let cap = w.territory(t).troop_cap;
    assert_eq!(cap, 100, "lone territory has the base cap");
    for _ in 0..1000 {
        w.tick();
        assert!(w.territory(t).troops <= cap);
    }
    assert_eq!(w.territory(t).troops, cap);
}

#[test]
fn growth_happens_once_per_interval() {
    let mut w = common::world(1, 3);
    common::clear_board(&mut w, 10);
    let t = TerritoryId(0);
    w.set_territory(t, Owner::Player(P0), 50).unwrap();
    for _ in 0..9 {
        w.tick();
    }
    assert_eq!(w.territory(t).troops, 50, "no growth before tick 10");
    w.tick();
    assert_eq!(w.territory(t).troops, 52, "50 + 5% (2)");
}

#[test]
fn troops_above_cap_are_kept_but_do_not_grow() {
    let mut w = common::world(1, 3);
    common::clear_board(&mut w, 10);
    let t = TerritoryId(0);
    w.set_territory(t, Owner::Player(P0), 500).unwrap();
    for _ in 0..50 {
        w.tick();
    }
    assert_eq!(w.territory(t).troops, 500);
}

#[test]
fn neutral_territories_never_grow() {
    let mut w = common::world(1, 3);
    let neutral: Vec<_> = w
        .map()
        .territory_ids()
        .filter(|&t| w.territory(t).owner == Owner::Neutral)
        .map(|t| (t, w.territory(t).troops))
        .collect();
    for _ in 0..100 {
        w.tick();
    }
    for (t, troops) in neutral {
        assert_eq!(w.territory(t).troops, troops);
    }
}

#[test]
fn cap_grows_with_owned_neighbors() {
    let mut w = common::world(1, 3);
    common::clear_board(&mut w, 10);
    let t = TerritoryId(0);
    w.set_territory(t, Owner::Player(P0), 50).unwrap();
    let nbs: Vec<_> = w.map().neighbor_ids(t).take(2).collect();
    for &n in &nbs {
        w.set_territory(n, Owner::Player(P0), 10).unwrap();
    }
    assert_eq!(w.territory(t).troop_cap, 100 + 20 * 2);
}

#[test]
fn gold_accrues_at_configured_rate() {
    let mut w = common::world(1, 3);
    common::clear_board(&mut w, 10);
    w.set_territory(TerritoryId(0), Owner::Player(P0), 10)
        .unwrap();
    // 1 territory x 60 gold/min: 600 ticks (1 minute at 10 Hz) gives exactly 60 gold.
    for _ in 0..600 {
        w.tick();
    }
    assert_eq!(w.player(P0).unwrap().gold(), 60);
    assert_eq!(w.player(P0).unwrap().gold_milli, 60_000);
}

#[test]
fn gold_scales_with_territory_count() {
    let mut w = common::world(1, 3);
    common::clear_board(&mut w, 10);
    for i in 0..3 {
        w.set_territory(TerritoryId(i), Owner::Player(P0), 10)
            .unwrap();
    }
    for _ in 0..600 {
        w.tick();
    }
    assert_eq!(w.player(P0).unwrap().gold(), 180);
}

#[test]
fn reinforce_costs_gold_and_respects_cap() {
    let mut w = common::world(2, 3);
    common::clear_board(&mut w, 10);
    let t = TerritoryId(0);
    w.set_territory(t, Owner::Player(P0), 90).unwrap();
    let far = common::non_neighbor(&w, t);
    w.set_territory(far, Owner::Player(P1), 10).unwrap();
    w.set_gold(P0, 450).unwrap();

    w.queue_order(Order::Reinforce {
        player: P0,
        territory: t,
    })
    .unwrap();
    w.tick();
    assert_eq!(w.territory(t).troops, 100, "90 + 25 clamped to cap 100");
    // 450 - 200, plus one tick of income (0.1 gold = 100 milli).
    assert_eq!(w.player(P0).unwrap().gold_milli, 250_000 + 100);

    let err = w.queue_order(Order::Reinforce {
        player: P0,
        territory: t,
    });
    assert_eq!(err, Err(OrderError::AtCap));
}

#[test]
fn reinforce_without_gold_is_refused() {
    let mut w = common::world(1, 3);
    let t = w
        .map()
        .territory_ids()
        .find(|&t| w.territory(t).owner == Owner::Player(P0))
        .unwrap();
    let err = w.queue_order(Order::Reinforce {
        player: P0,
        territory: t,
    });
    assert_eq!(err, Err(OrderError::NotEnoughGold));
}
