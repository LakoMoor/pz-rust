use pz_sim::wetness::*;
use pz_sim::Player;

fn weather(outside: bool, rain: f32, t: f32) -> Weather {
    Weather { outside, raining: rain > 0.0, rain_intensity: rain, air_temperature: t }
}

fn run(p: &mut Player, ticks: usize, w: Weather) {
    for _ in 0..ticks { p.tick_wetness(w); p.tick_thermo(w.air_temperature, w.air_temperature, 0.5, false, 1.0, !w.outside); }
}

#[test]
fn rain_soaks_a_bare_player_and_shelter_dries_them() {
    let mut p = Player::new(0.0, 0.0);
    run(&mut p, 3000, weather(true, 0.8, 12.0));
    let soaked = p.wetness[6];
    assert!(soaked > 20.0 && p.wetness.iter().all(|&w| (w - soaked).abs() < 1e-3), "{soaked}");
    run(&mut p, 6000, weather(false, 0.0, 22.0));
    assert!(p.wetness[6] < soaked * 0.5, "{} vs {soaked}", p.wetness[6]);
}

#[test]
fn light_drizzle_does_nothing_and_wetness_stays_in_range() {
    let mut p = Player::new(0.0, 0.0);
    run(&mut p, 2000, weather(true, 0.05, 15.0));
    assert_eq!(p.wetness[0], 0.0);
    run(&mut p, 60_000, weather(true, 1.0, 5.0));
    assert!(p.wetness.iter().all(|&w| (0.0..=100.0).contains(&w)));
    assert!(p.wetness[0] > 50.0);
}

#[test]
fn standing_in_cold_rain_eventually_gives_a_cold() {
    let mut p = Player::new(0.0, 0.0);
    run(&mut p, 120_000, weather(true, 1.0, 3.0));
    assert!(p.cold.has_cold || p.cold.catch > 0.0, "wet {} catch {}", p.wetness[6], p.cold.catch);
}
