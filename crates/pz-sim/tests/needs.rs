use pz_sim::{moodles::*, needs::*};

#[test]
fn hunger_and_thirst_grow_at_the_defines_lua_rates() {
    // Default day length (60 real minutes): delta = 30 / 60 = 0.5. One in-game hour is 3600 s * 30 Hz / 24 ticks...
    let delta = 0.5;
    let mut n = Needs::default();
    n.tick(delta, false, false);
    assert!((n.hunger - HUNGER_INCREASE * delta).abs() < 1e-9);
    assert!((n.thirst - THIRST_INCREASE * delta).abs() < 1e-9);
    // Running and moving: a third of the exercise rate for hunger (not well fed), +20% thirst.
    let mut r = Needs::default();
    r.tick(delta, true, false);
    assert!((r.hunger - HUNGER_INCREASE_EXERCISE / 3.0 * delta).abs() < 1e-9);
    assert!((r.thirst - THIRST_INCREASE * 1.2 * delta).abs() < 1e-9);
    // Asleep: thirst grows at the much slower sleeping rate.
    let mut s = Needs::default();
    s.tick(delta, false, true);
    assert!((s.thirst - THIRST_SLEEPING_INCREASE * delta).abs() < 1e-9);
}

#[test]
fn a_hungrier_character_gets_hungrier_more_slowly_and_everything_clamps() {
    let mut full = Needs::default();
    let mut starving = Needs { hunger: 0.9, thirst: 0.0, ..Default::default() };
    full.tick(1.0, false, false);
    starving.tick(1.0, false, false);
    assert!(starving.hunger - 0.9 < full.hunger, "appetite factor (1 - hunger) slows the climb");
    let mut maxed = Needs { hunger: 1.0, thirst: 1.0, ..Default::default() };
    maxed.tick(1.0, false, false);
    assert_eq!((maxed.hunger, maxed.thirst), (1.0, 1.0));
}

#[test]
fn a_day_without_food_or_water_gets_a_character_into_trouble() {
    let mut n = Needs::default();
    // 30 Hz ticks; the default 1-hour game day is 0.5 delta per tick-multiplier unit... run 24 game hours
    // at 3600 s per day: 3600 * 30 ticks.
    let mut hungry_at = None;
    let mut thirsty_at = None;
    for t in 0..(3600 * 30 * 3) {
        n.tick(0.5, false, false);
        if hungry_at.is_none() && hungry(n.hunger, 100.0) >= 1 { hungry_at = Some(t); }
        if thirsty_at.is_none() && thirst(n.thirst) >= 1 { thirsty_at = Some(t); }
    }
    let (h, t) = (hungry_at.expect("gets hungry"), thirsty_at.expect("gets thirsty"));
    assert!(t < h, "thirst comes on sooner than hunger ({t} vs {h})");
    // Thresholds.
    assert_eq!(hungry(0.15, 100.0), 0);
    assert_eq!(hungry(0.16, 100.0), 1);
    assert_eq!(hungry(0.71, 100.0), 4);
    assert_eq!(hungry(0.9, 0.0), 0);
    assert_eq!(thirst(0.13), 1);
    assert_eq!(thirst(0.85), 4);
}

#[test]
fn a_parched_character_slowly_loses_health() {
    use pz_sim::Player;
    let mut p = Player::new(0.0, 0.0);
    p.needs = Needs { hunger: 0.0, thirst: 0.9, ..Default::default() };
    for _ in 0..300 { p.tick_body(); }
    // 0.0165 / 10 per tick for the thirst moodle at max.
    assert!((100.0 - p.health - 300.0 * 0.0165 / 10.0).abs() < 0.2, "{}", p.health);
    let mut q = Player::new(0.0, 0.0);
    q.needs = Needs { hunger: 0.0, thirst: 0.5, ..Default::default() };
    for _ in 0..300 { q.tick_body(); }
    assert_eq!(q.health, 100.0, "moderate thirst costs no health");
}

#[test]
fn fatigue_builds_while_awake_and_the_tired_moodle_follows() {
    let mut n = Needs::default();
    // Full endurance: floor of 0.3 on the multiplier.
    n.tick(0.5, false, false);
    assert!((n.fatigue - FATIGUE_INCREASE * 0.3 * 0.5).abs() < 1e-9);
    // Less endurance means faster tiring; sleeping stops the climb.
    let mut winded = Needs { endurance: 0.0, ..Default::default() };
    winded.tick(0.5, false, false);
    assert!(winded.fatigue > n.fatigue * 3.0);
    let mut asleep = Needs::default();
    asleep.tick(0.5, false, true);
    assert_eq!(asleep.fatigue, 0.0);
    assert_eq!(tired(0.6, 100.0), 0);
    assert_eq!(tired(0.61, 100.0), 1);
    assert_eq!(tired(0.95, 100.0), 4);
    assert_eq!(tired(0.95, 0.0), 0);
}

#[test]
fn eating_and_drinking_reduce_hunger_and_thirst_and_clamp_at_zero() {
    let mut n = pz_sim::needs::Needs { hunger: 0.5, thirst: 0.4, ..Default::default() };
    n.eat(-0.2, 0.5);
    assert!((n.hunger - 0.4).abs() < 1e-6);
    n.eat(-0.9, 1.0);
    assert_eq!(n.hunger, 0.0);
    n.drink(-0.1, 1.0);
    assert!((n.thirst - 0.3).abs() < 1e-6);
    n.drink(-1.0, 1.0);
    assert_eq!(n.thirst, 0.0);
}
