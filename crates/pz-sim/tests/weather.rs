use pz_sim::*;

#[test]
fn vision_radius_follows_the_decompiled_formula() {
    assert_eq!(Weather::default().vision_radius(), 20.0);
    // Darkness 5 vs rain+fog 0: 20 - 5 = 15.
    assert_eq!(Weather { rain: 0.0, fog: 0.0, light: 0.0 }.vision_radius(), 15.0);
    // The worse of the two penalties applies, not their sum: fog 0.5 -> 3.5, rain 1 -> 2.5, total 6.
    let w = Weather { rain: 1.0, fog: 0.5, light: 0.0 };
    assert_eq!(w.vision_radius(), 14.0);
    // Heavy fog clamps at 10.
    assert_eq!(Weather { rain: 1.0, fog: 1.0, light: 1.0 }.vision_radius(), 10.5);
    assert_eq!(Weather { rain: 5.0, fog: 5.0, light: 1.0 }.vision_radius(), 10.0);
}

#[test]
fn clock_cycles_between_day_and_night() {
    let at = |hour| GameClock { hour, ..GameClock::default() };
    assert_eq!(at(12.0).night_strength(), 0.0);
    assert_eq!(at(12.0).weather(0.0, 0.0).vision_radius(), 20.0);
    // Deep night is fully dark: darkness penalty 5.
    let n = at(2.0);
    assert!(n.night_strength() > 0.99, "{}", n.night_strength());
    assert!((n.weather(0.0, 0.0).vision_radius() - 15.0).abs() < 0.05);
    // Dusk is in between and monotonic.
    let (a, b, c) = (at(21.5).night_strength(), at(23.0).night_strength(), at(1.0).night_strength());
    assert!(0.0 < a && a < b && b <= c, "{a} {b} {c}");
}

#[test]
fn clock_advances_one_day_per_day_length() {
    let mut c = GameClock { hour: 0.0, day_seconds: 60.0, ..GameClock::default() };
    for _ in 0..(60.0 * TICK_HZ) as u32 / 2 { c.tick(); }
    assert!((c.hour - 12.0).abs() < 0.01, "{}", c.hour);
}

#[test]
fn sight_setting_and_eating_scale_before_the_clamp() {
    let night = Weather { rain: 0.0, fog: 0.0, light: 0.0 }; // base radius 15
    assert_eq!(night.vision_radius_for(Sight::Eagle, 1.0, false), 20.0); // 26.25 clamps down
    assert_eq!(night.vision_radius_for(Sight::Poor, 1.0, false), 10.0); // 5.25 clamps up
    assert_eq!(night.vision_radius_for(Sight::Normal, 1.0, true), 10.0); // 7.5 clamps up
    assert_eq!(Weather::default().vision_radius_for(Sight::Normal, 1.25, false), 16.0);
}

#[test]
fn a_poor_sighted_zombie_loses_a_distant_player_that_a_normal_one_sees() {
    use pz_world::World;
    let mut w = World::new(std::path::PathBuf::from("."));
    let mk = |sight| { let mut z = Zombie::new(0.0, 0.0, 0); z.sight = sight; z };
    let (mut normal, mut poor) = (mk(Sight::Normal), mk(Sight::Poor));
    let mut p = Player::new(14.0, 0.0);
    let night = Weather { rain: 0.0, fog: 0.0, light: 0.0 }; // radius 15 normal, 10 poor
    for _ in 0..5 { normal.tick_in(&mut p, &mut w, &night); poor.tick_in(&mut p, &mut w, &night); }
    assert!(normal.pos[0] > 0.0, "normal zombie should approach, x={}", normal.pos[0]);
    assert!(poor.pos[0].abs() < normal.pos[0], "poor zombie should not chase: {} vs {}", poor.pos[0], normal.pos[0]);
}
