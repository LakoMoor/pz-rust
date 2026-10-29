use pz_sim::climate::roll_fog;

#[test]
fn fog_roll_is_deterministic_and_about_one_day_in_five() {
    assert_eq!(roll_fog(1993, 6, 9, 100, 250, 7.0, 15.0), roll_fog(1993, 6, 9, 100, 250, 7.0, 15.0));
    let mut foggy = 0;
    for d in 0..2000 {
        let (month, day) = ((d / 28) % 12, d % 28 + 1);
        if let Some(f) = roll_fog(1993 + d / 336, month, day, 17, 91, 7.0, 12.0) {
            foggy += 1;
            assert!((0.0..=1.0).contains(&f.strength));
            assert!(f.duration >= 4.0 && f.duration <= 16.0, "{f:?}");
        }
    }
    let share = foggy as f32 / 2000.0;
    assert!((0.15..0.25).contains(&share), "fog share {share}");
}

#[test]
fn colder_days_have_longer_fog_and_it_is_capped_by_the_day_length() {
    // Same seed (same date, offsets), only the temperature differs. Find a foggy day.
    let (y, m, d) = (0..400).map(|i| (1993, i % 12, i % 28 + 1)).find(|&(y, m, d)| roll_fog(y, m, d, 5, 9, 7.0, 15.0).is_some()).unwrap();
    let cold = roll_fog(y, m, d, 5, 9, 7.0, -5.0).unwrap();
    let warm = roll_fog(y, m, d, 5, 9, 7.0, 25.0).unwrap();
    assert_eq!(cold.strength, warm.strength);
    assert!(cold.duration > warm.duration);
    let late_dawn = roll_fog(y, m, d, 5, 9, 22.0, -5.0).unwrap();
    assert_eq!(late_dawn.duration, 24.0 - 22.0 - 1.0);
}

#[test]
fn dawn_and_dusk_follow_the_seasons_at_latitude_38() {
    use pz_sim::climate::Daylight;
    let d = Daylight::default();
    // Summer solstice: 14.64 h of light around a noon pushed to 14.5.
    let (dawn, dusk) = d.dawn_dusk(1993, 5, 22);
    assert!((dawn - 7.18).abs() < 0.02 && (dusk - 21.82).abs() < 0.02, "{dawn} {dusk}");
    // Winter solstice: 9.36 h around 12.5.
    let (dawn, dusk) = d.dawn_dusk(1993, 11, 22);
    assert!((dawn - 7.82).abs() < 0.02 && (dusk - 17.18).abs() < 0.02, "{dawn} {dusk}");
    // The day is longest in June and shortest in December, and the year wraps smoothly.
    let len = |m, dd| { let (a, b) = d.dawn_dusk(1994, m, dd); b - a };
    assert!(len(5, 22) > len(2, 20) && len(2, 20) > len(11, 22));
    assert!((len(0, 1) - len(11, 31)).abs() < 0.05);
    // The default clock's fixed 7:00 / 21:00 sits between the two extremes.
    assert!(len(5, 22) > 14.0 && len(11, 22) < 10.0);
}

#[test]
fn the_clock_takes_dawn_and_dusk_from_the_date() {
    use pz_sim::GameClock;
    let mut c = GameClock::default();
    c.set_date(1993, 11, 22);
    assert!((c.dawn - 7.82).abs() < 0.02 && (c.dusk - 17.18).abs() < 0.02);
    c.hour = 18.0; // already dark in December, still light in June
    let dark = c.night_strength();
    c.set_date(1993, 5, 22);
    assert!(dark > 0.02 && c.night_strength() == 0.0, "{dark}");
}
