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

#[test]
fn crawlers_are_roughly_half_as_fast() {
    use pz_world::World;
    let mut w = World::new(std::path::PathBuf::from("."));
    let mut p = Player::new(15.0, 0.0);
    let (mut walker, mut crawler) = (Zombie::new(0.0, 0.0, 0), Zombie::new(0.0, 0.0, 0));
    crawler.set_crawler();
    assert!(crawler.is_crawler() && !walker.is_crawler());
    for _ in 0..60 { walker.tick(&mut p, &mut w); crawler.tick(&mut p, &mut w); }
    let ratio = crawler.pos[0] / walker.pos[0];
    assert!((0.48..=0.75).contains(&ratio), "ratio {ratio}");
}

#[test]
fn morning_fog_builds_peaks_and_burns_off() {
    let day = |hour| GameClock { hour, fog: Some(FogDay { strength: 1.0, duration: 4.0 }), ..GameClock::default() };
    // Dawn is 7:00: fog starts at 5:00 and is gone at 11:00.
    assert_eq!(day(4.9).fog_intensity(), 0.0);
    assert_eq!(day(11.1).fog_intensity(), 0.0);
    let (early, peak, late) = (day(5.5).fog_intensity(), day(8.0).fog_intensity(), day(10.5).fog_intensity());
    assert!(0.0 < early && early < peak && late < peak, "{early} {peak} {late}");
    // Full strength maps to 1.0 and is reached mid-window.
    assert!((peak - 1.0).abs() < 1e-3, "{peak}");
    // No fog today: always clear. And fog really shortens the zombies' sight.
    assert_eq!(GameClock { hour: 8.0, ..GameClock::default() }.fog_intensity(), 0.0);
    assert_eq!(day(8.0).weather_now(0.0).vision_radius(), 13.0);
}

#[test]
fn endless_day_and_night_pin_the_light() {
    let at = |hour, cycle| GameClock { hour, cycle, ..GameClock::default() };
    for h in [2.0, 12.0, 23.0] {
        assert_eq!(at(h, DayNight::EndlessDay).night_strength(), 0.0);
        assert_eq!(at(h, DayNight::EndlessNight).night_strength(), 1.0);
    }
    assert_eq!(at(12.0, DayNight::EndlessNight).weather(0.0, 0.0).vision_radius(), 15.0);
    assert_eq!(at(2.0, DayNight::EndlessDay).weather(0.0, 0.0).vision_radius(), 20.0);
    assert!(at(2.0, DayNight::Normal).night_strength() > 0.99);
}
