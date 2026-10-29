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

#[test]
fn seasonal_mean_temperature_peaks_in_summer_and_bottoms_out_in_winter() {
    use pz_sim::climate::Seasons;
    let s = Seasons::default();
    let mean = |m, d| s.day_temperature(1993, m, d).0;
    // Hottest around Jun 22 + 31 days (late July), coldest around late January.
    let (jul, jan, apr, oct) = (mean(6, 23), mean(0, 22), mean(3, 15), mean(9, 15));
    assert!(jul > 20.0 && jul < 30.0, "july {jul}");
    assert!(jan > -6.0 && jan < 5.0, "january {jan}");
    assert!(jul > apr && apr > jan && oct > jan);
    // The mean is smooth day to day and across the new year; the actual temperature wiggles by +-tempDiff.
    assert!((mean(11, 31) - s.day_temperature(1994, 0, 1).0).abs() < 1.0);
    for dd in 1..28 { assert!((mean(5, dd) - mean(5, dd + 1)).abs() < 1.0); }
    for dd in 1..28 {
        let (m, t) = s.day_temperature(1993, 5, dd);
        assert!((t - m).abs() <= 7.0 + 1e-3);
    }
    // A different save (other seeds / tempDiff) gives a different year.
    let other = Seasons::new(25, 0, 7, 31, 3, 5, 9);
    assert_ne!(other.day_temperature(1993, 6, 23).0, jul);
}

#[test]
fn air_masses_and_wind_stay_in_range_and_drift_slowly() {
    use pz_sim::climate::{AirMass, Offsets};
    let o = Offsets { a: 1234.0, b: 77.0, c: -4321.0, d: -99.0 };
    let (mut warm, mut cold) = (false, false);
    let mut prev = AirMass::at(o, 1000.0, 12.0, 0.5, 3);
    for h in 0..2000 {
        let a = AirMass::at(o, 1000.0 + h as f64, 12.0, 0.5, 3);
        assert!((-1.0..=1.0).contains(&a.temperature) && (0.0..=0.65).contains(&a.wind) && (0.0..=1.0).contains(&a.cloudy), "{a:?}");
        assert!((a.temperature - prev.temperature).abs() < 0.1, "air masses move slowly");
        warm |= a.temperature > 0.3; cold |= a.temperature < -0.3;
        prev = a;
    }
    assert!(warm && cold, "both warm and cold fronts occur over 2000 hours");
    // Slower noise (sandbox 'rare rain') changes more slowly than the fast setting.
    let step = |rain| (AirMass::at(o, 5001.0, 0.0, 0.0, rain).temperature - AirMass::at(o, 5000.0, 0.0, 0.0, rain).temperature).abs();
    assert!(step(1) <= step(5) + 1e-3 || step(1) < 0.02);
    assert_eq!(AirMass::at(o, 10.0, 0.0, 0.0, 3).temperature_offset(), AirMass::at(o, 10.0, 0.0, 0.0, 3).temperature * 8.0);
}

#[test]
fn the_calendar_rolls_days_months_and_years_and_refreshes_dawn_and_fog() {
    use pz_sim::climate::{Climate, Offsets, Seasons};
    use pz_sim::{GameClock, TICK_HZ};
    let mut clock = GameClock { hour: 23.0, day_seconds: 240.0, ..GameClock::default() };
    let o = Offsets { a: 10.0, b: 9000.0, c: -10.0, d: -9000.0 };
    let mut cl = Climate::new(Seasons::default(), o, 1993, 11, 31, &mut clock);
    let dec_dawn = clock.dawn;
    assert!((7.7..8.0).contains(&dec_dawn), "{dec_dawn}");
    // Two game hours at 240 s/day pass in 20 s: midnight falls inside.
    for _ in 0..(20.0 * TICK_HZ) as u32 { clock.tick(); cl.advance(&mut clock); }
    assert_eq!((cl.year, cl.month, cl.day), (1994, 0, 1));
    assert!((cl.world_age_hours - 2.0).abs() < 0.05, "{}", cl.world_age_hours);
    // Over a few weeks every day gets a fresh dawn and some days fog up, deterministically.
    let (mut foggy, mut dawns) = (0, std::collections::BTreeSet::new());
    for _ in 0..(40.0 * 240.0 * TICK_HZ) as u32 / 30 { clock.tick(); cl.advance(&mut clock); }
    for _ in 0..100 {
        for _ in 0..(240.0 * TICK_HZ) as u32 { clock.tick(); cl.advance(&mut clock); }
        foggy += clock.fog.is_some() as u32;
        dawns.insert((clock.dawn * 100.0) as i32);
    }
    assert!(dawns.len() > 20, "dawn should move day to day");
    assert!((3..45).contains(&foggy), "foggy days {foggy}/100");
    // Month lengths: February 1996 has 29 days.
    let mut clock = GameClock { hour: 23.9, day_seconds: 60.0, ..GameClock::default() };
    let mut cl = Climate::new(Seasons::default(), o, 1996, 1, 28, &mut clock);
    for _ in 0..(60.0 * TICK_HZ) as u32 / 4 { clock.tick(); cl.advance(&mut clock); }
    assert_eq!((cl.month, cl.day), (1, 29));
}
