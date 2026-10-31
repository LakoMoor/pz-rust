use pz_sim::cold::*;
use pz_sim::thermo::{Inputs, Thermoregulator};

fn rested(in_room: bool) -> Rest {
    Rest { in_room, wet_moodle: false, hypothermia_level: 0, fatigue: 0.0, hunger: 0.0, thirst: 0.0 }
}

#[test]
fn chilled_wet_skin_gives_a_cold_delta_and_comfortable_skin_gives_none() {
    let warm = { let mut t = Thermoregulator::default(); for _ in 0..30_000 { t.update(&Inputs::default()); } t };
    assert!(warm.catch_a_cold_delta(&[0.0; 17], 0) < 0.1);
    let mut cold = Thermoregulator::default();
    let i = Inputs { air_temperature: -5.0, air_and_wind_temperature: -5.0, ..Inputs::default() };
    for _ in 0..30_000 { cold.update(&i); }
    let dry = cold.catch_a_cold_delta(&[0.0; 17], 0);
    let wet = cold.catch_a_cold_delta(&[100.0; 17], 0);
    assert!(dry > 0.1 && wet > dry, "dry {dry} wet {wet}");
    assert!(cold.catch_a_cold_delta(&[0.0; 17], 3) > dry * 2.9);
}

#[test]
fn a_cold_builds_up_then_runs_its_course_when_resting_indoors() {
    let mut c = Cold::default();
    let mut ticks = 0;
    while !c.has_cold && ticks < 100_000 { c.tick(2.0, rested(false)); ticks += 1; }
    assert!(c.has_cold && (c.strength - 20.0).abs() < 0.1, "caught after {ticks} ticks");
    // Outdoors it only gets worse; indoors, rested and dry, it fades.
    for _ in 0..1000 { c.tick(0.0, rested(false)); }
    assert!(c.strength > 20.0);
    let peak = c.strength;
    for _ in 0..200 { c.tick(0.0, rested(true)); }
    assert!(c.strength < peak);
    for _ in 0..20_000 { c.tick(0.0, rested(true)); }
    assert!(!c.has_cold && c.strength() == 0.0);
}

#[test]
fn catch_progress_drains_when_the_chill_stops() {
    let mut c = Cold::default();
    for _ in 0..1000 { c.tick(2.0, rested(true)); }
    assert!(c.catch > 5.0);
    for _ in 0..100 { c.tick(0.0, rested(true)); }
    assert_eq!(c.catch, 0.0);
}

#[test]
fn has_a_cold_moodle_levels() {
    assert_eq!([20.0, 21.0, 41.0, 61.0, 76.0].map(|s| has_a_cold(s, 100.0)), [0, 1, 2, 3, 4]);
    assert_eq!(has_a_cold(90.0, 0.0), 0);
}

#[test]
fn a_soaked_player_in_the_cold_comes_down_with_a_cold() {
    let mut p = pz_sim::Player::new(0.0, 0.0);
    p.wetness = [100.0; 17];
    for _ in 0..200_000 {
        p.tick_thermo(2.0, 2.0, 0.8, false, 1.0, false);
        if p.cold.has_cold { break; }
    }
    assert!(p.cold.has_cold, "catch {}", p.cold.catch);
    let mut dry = pz_sim::Player::new(0.0, 0.0);
    for _ in 0..50_000 { dry.tick_thermo(27.0, 27.0, 0.5, false, 1.0, true); }
    assert!(!dry.cold.has_cold && dry.cold.catch == 0.0);
}
