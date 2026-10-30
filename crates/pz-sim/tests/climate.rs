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

#[test]
fn rain_comes_and_goes_with_a_front_and_blinds_zombies() {
    use pz_sim::climate::{precipitation, Climate, Offsets, RainPeriod, Seasons};
    use pz_sim::GameClock;
    // Below the threshold nothing falls; above it the amount scales with the strength.
    assert_eq!(precipitation(10.0, 0.5, 0.0, 0.3), 0.0);
    let p = RainPeriod { start_hours: 100.0, duration: 20.0, strength: 1.0, rain_threshold: 0.2, kind: pz_sim::climate::PeriodKind::Rain };
    assert_eq!(p.intensity(99.0), 0.0);
    assert_eq!(p.intensity(120.5), 0.0);
    let mid: Vec<f32> = (0..200).map(|i| p.intensity(105.0 + i as f64 * 0.05)).collect();
    assert!(mid.iter().all(|r| (0.0..=1.0).contains(r)));
    assert!(mid.iter().any(|&r| r > 0.2), "a strong front must rain at some point");

    let mut clock = GameClock::default();
    let o = Offsets { a: 1.0, b: 2.0, c: -1.0, d: -2.0 };
    let mut cl = Climate::new(Seasons::default(), o, 1993, 6, 9, &mut clock);
    assert_eq!(cl.rain(), 0.0);
    cl.start_rain(10.0, 1.0, 0.0);
    cl.world_age_hours += 5.0;
    let rain = cl.rain();
    assert!(rain > 0.0, "{rain}");
    // Rain narrows what zombies see: -2.5 * rain from the 20-square base.
    let w = clock.weather_now(rain);
    assert!((w.vision_radius() - (20.0 - 2.5 * rain).max(10.0)).abs() < 1e-3);
    cl.world_age_hours += 6.0;
    assert_eq!(cl.rain(), 0.0, "the front has passed");
}

#[test]
fn air_front_strength_follows_add_day_sample() {
    use pz_sim::climate::AirFront;
    let mut f = AirFront::of_type(1);
    f.add_day_sample(-0.4); // wrong side: no strength
    assert_eq!(f.strength, 0.0);
    f.add_day_sample(0.4);
    assert!((f.strength - 0.4).abs() < 1e-6);
    f.add_day_sample(0.8);
    assert!((f.strength - (0.8 * 0.75 + 0.8 * 0.25)).abs() < 1e-6);
    f.add_day_sample(0.1);
    // recent window holds {0.1, 0.8} -> 0.8; max 0.8
    assert!((f.strength - 0.8).abs() < 1e-6);
    let mut c = AirFront::of_type(-1);
    c.add_day_sample(-0.5);
    assert!((c.strength - 0.5).abs() < 1e-6);
    // Type 0 (before any front is tracked) accepts samples of either sign, as in the original condition.
    let mut z = AirFront::default();
    z.add_day_sample(-0.9);
    assert!((z.strength - 0.9).abs() < 1e-6);
}

#[test]
fn rain_fronts_arrive_on_their_own_over_weeks() {
    use pz_sim::climate::{Climate, Offsets, Seasons};
    use pz_sim::{GameClock, TICK_HZ};
    let mut clock = GameClock { hour: 6.0, day_seconds: 120.0, ..GameClock::default() };
    let o = Offsets { a: 3100.0, b: 9000.0, c: -2000.0, d: -9000.0 };
    let mut cl = Climate::new(Seasons::default(), o, 1993, 6, 9, &mut clock);
    let (mut fronts, mut wet_ticks, mut max_rain) = (0, 0u32, 0.0f32);
    let mut last = None;
    // 120 game days at 120 s/day.
    for _ in 0..(120.0 * 120.0 * TICK_HZ) as u32 {
        clock.tick();
        cl.advance(&mut clock);
        if cl.period != last { if cl.period.is_some() { fronts += 1; } last = cl.period; }
        let r = cl.rain();
        if r > 0.0 { wet_ticks += 1; max_rain = max_rain.max(r); }
        assert!((0.0..=1.0).contains(&r));
    }
    assert!(fronts >= 3, "expected several fronts in 120 days, got {fronts}");
    assert!(wet_ticks > 0 && max_rain > 0.1, "wet {wet_ticks} max {max_rain}");
    let dry_share = 1.0 - wet_ticks as f32 / (120.0 * 120.0 * TICK_HZ);
    assert!(dry_share > 0.5, "most of the time it is dry: {dry_share}");
}

#[test]
fn cold_precipitation_falls_as_snow_and_does_not_blind_zombies() {
    use pz_sim::climate::{Climate, Offsets, Seasons};
    use pz_sim::GameClock;
    let o = Offsets { a: 1200.0, b: 9000.0, c: -2000.0, d: -9000.0 };
    // Midsummer afternoon vs. January pre-dawn.
    let (mut summer, mut winter) = (GameClock { hour: 15.0, ..GameClock::default() }, GameClock { hour: 4.0, ..GameClock::default() });
    let mut cs = Climate::new(Seasons::default(), o, 1993, 6, 23, &mut summer);
    let mut cw = Climate::new(Seasons::default(), o, 1994, 0, 22, &mut winter);
    let (ts, tw) = (cs.temperature(&summer), cw.temperature(&winter));
    assert!(ts > 15.0 && tw < ts - 15.0, "summer {ts} winter {tw}");
    // The day warms from the small hours towards the afternoon.
    let mut noon = winter; noon.hour = 15.0;
    assert!(cw.temperature(&noon) > tw + 3.0);
    for c in [&mut cs, &mut cw] { c.natural_fronts = false; c.start_rain(10.0, 1.0, 0.0); c.world_age_hours += 5.0; }
    assert!(cs.precipitation_now() > 0.0 && cw.precipitation_now() > 0.0);
    assert!(cs.rain_at(&summer) > 0.0 && cs.snow_at(&summer) == 0.0);
    // In deep winter the same front is snow (when the air is below freezing), so it adds no rain penalty.
    if cw.is_snow(&winter) { assert_eq!(cw.rain_at(&winter), 0.0); assert!(cw.snow_at(&winter) > 0.0); }
    else { assert!(cw.rain_at(&winter) > 0.0 && cw.snow_at(&winter) == 0.0); }
}

#[test]
fn seasons_follow_the_calendar_with_noisy_boundaries() {
    use pz_sim::climate::Seasons;
    let s = Seasons::default();
    // Midwinter, midsummer; spring and autumn in between.
    assert_eq!(s.season(1993, 0, 15), 5);
    assert_eq!(s.season(1993, 5, 22), 2);
    assert_eq!(s.season(1993, 11, 22), 5);
    assert_eq!(s.season(1993, 3, 10), 1);
    assert_eq!(s.season(1993, 9, 25), 4);
    // Walking through a year visits the seasons in order 5 -> 1 -> 2 -> 4 -> 5 exactly once each.
    let mut order = vec![];
    for m in 0..12 { for d in [1, 10, 20] {
        let x = s.season(1994, m, d);
        if order.last() != Some(&x) { order.push(x); }
    } }
    assert_eq!(order, vec![5, 1, 2, 4, 5]);
    // Other years shift the boundaries by a few days only.
    let diff = (0..365).filter(|&k| s.season(1993, (k / 31).min(11), k % 28 + 1) != s.season(1995, (k / 31).min(11), k % 28 + 1)).count();
    assert!(diff < 90, "{diff}");
}

#[test]
fn strong_warm_fronts_sometimes_become_storms_and_last_longer() {
    use pz_sim::climate::{Climate, Offsets, PeriodKind, Seasons};
    use pz_sim::{GameClock, TICK_HZ};
    let (mut storms, mut tropical, mut plain, mut storm_len, mut plain_len) = (0, 0, 0, 0.0f64, 0.0f64);
    for seed in 0..12 {
        let mut clock = GameClock { hour: 6.0, day_seconds: 60.0, ..GameClock::default() };
        let o = Offsets { a: 500.0 + seed as f64 * 613.0, b: 9000.0 + seed as f64 * 77.0, c: -3000.0, d: -9000.0 };
        let mut cl = Climate::new(Seasons::default(), o, 1993, 6, 9, &mut clock);
        let mut last = None;
        for _ in 0..(600.0 * 60.0 * TICK_HZ) as u32 {
            clock.tick();
            cl.advance(&mut clock);
            if cl.period != last {
                last = cl.period;
                if let Some(p) = last {
                    assert!(p.duration > 0.0 && p.duration < 400.0, "{p:?}");
                    match p.kind {
                        PeriodKind::Thunderstorm => { storms += 1; storm_len += p.duration; assert!(p.strength > 0.5); }
                        PeriodKind::Tropical => { tropical += 1; assert!(p.strength > 0.75); }
                        PeriodKind::Rain => { plain += 1; plain_len += p.duration; }
                        PeriodKind::Blizzard => { assert!(p.strength > 0.75); }
                    }
                }
            }
        }
    }
    assert!(plain > 10 && storms > 0, "plain {plain} storms {storms} tropical {tropical}");
    assert!(storm_len / storms as f64 > plain_len / plain as f64, "storms should outlast ordinary rain");
}

#[test]
fn blizzard_chance_and_rain_time_multiplier_follow_the_tables() {
    use pz_sim::climate::{blizzard_chance, rain_time_multiplier};
    assert_eq!(blizzard_chance(6.0), 0.0);
    assert_eq!(blizzard_chance(5.5), 0.0);
    assert_eq!(blizzard_chance(4.0), (1.5f32 * 3.0) + 25.0);
    assert_eq!(blizzard_chance(2.5), (3.0f32 * 3.0) + 25.0);
    assert_eq!(blizzard_chance(1.0), 4.5 * 3.0 + 25.0 + 55.0);
    // Colder than 0 is no different from 1 degree except the 85 clamp and the 95 cap.
    assert_eq!(blizzard_chance(-20.0), 95.0);
    assert_eq!((rain_time_multiplier(1), rain_time_multiplier(3), rain_time_multiplier(5)), (0.5, 1.0, 1.5));
}

#[test]
fn thunderstorm_fronts_spawn_clouds_that_produce_strikes() {
    use pz_sim::climate::{Climate, Offsets, PeriodKind, Seasons};
    use pz_sim::{GameClock, TICK_HZ};
    let (mut seen_storm, mut events, mut cloudy) = (false, 0usize, false);
    for seed in 0..12 {
        let mut clock = GameClock { hour: 6.0, day_seconds: 60.0, ..GameClock::default() };
        let o = Offsets { a: 500.0 + seed as f64 * 613.0, b: 9000.0 + seed as f64 * 77.0, c: -3000.0, d: -9000.0 };
        let mut cl = Climate::new(Seasons::default(), o, 1993, 6, 9, &mut clock);
        for _ in 0..(400.0 * 60.0 * TICK_HZ) as u32 {
            clock.tick();
            cl.advance(&mut clock);
            seen_storm |= matches!(cl.period.map(|p| p.kind), Some(PeriodKind::Thunderstorm | PeriodKind::Tropical));
            cloudy |= cl.has_thunder_clouds();
            events += cl.thunder.drain(..).filter(|e| e.strike || e.rumble).count();
        }
        assert!(!cl.has_thunder_clouds() || cl.period.is_some(), "clouds only exist during a storm period");
    }
    assert!(seen_storm && cloudy, "no storm in 12 worlds");
    assert!(events > 20, "storm clouds should strike: {events}");
}

#[test]
fn wind_chill_never_exceeds_air_temperature_and_humidity_is_a_fraction() {
    use pz_sim::climate::*;
    let mut clock = pz_sim::GameClock::default();
    let o = Offsets { a: 1.0, b: 2.0, c: -1.0, d: -2.0 };
    let mut c = Climate::new(Seasons::default(), o, 1993, 6, 9, &mut clock);
    for h in 0..200 {
        c.world_age_hours = 100.0 + h as f64 * 3.7;
        clock.hour = (h % 24) as f32;
        let (t, w, hu) = (c.temperature(&clock), c.wind_chill_temperature(&clock), c.humidity(&clock));
        assert!(w <= t + 1e-4 && (0.0..=1.0).contains(&hu), "t {t} w {w} hu {hu}");
    }
}

#[test]
fn indoor_air_is_pulled_toward_22_degrees() {
    use pz_sim::climate::*;
    let mut clock = pz_sim::GameClock::default();
    let o = Offsets { a: 1.0, b: 2.0, c: -1.0, d: -2.0 };
    let c = Climate::new(Seasons::default(), o, 1993, 6, 9, &mut clock);
    let out = c.temperature(&clock);
    let inside = c.indoor_temperature(&clock, 0, false);
    assert!((inside - 22.0).abs() < (out - 22.0).abs() || out == 22.0, "out {out} in {inside}");
    assert_eq!(c.indoor_temperature(&clock, 0, true), 22.0);
}
