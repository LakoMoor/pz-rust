use pz_sim::thunder::*;
use pz_sim::TICK_HZ;

fn rng(seed: u32) -> impl FnMut(f32, f32) -> f32 {
    let mut s = seed | 1;
    move |lo, hi| { s ^= s << 13; s ^= s >> 17; s ^= s << 5; lo + (s as f32 / u32::MAX as f32) * (hi - lo) }
}

fn run(strength: f32, seed: u32) -> (Vec<ThunderEvent>, ThunderCloud) {
    let (radius, ratio, s) = storm_cloud_params(strength, false);
    // Two game hours at 1 hour of real time per day: 300 s of simulation.
    let mut c = ThunderCloud::new([1000.0, 2000.0], [3000.0, 2000.0], radius, s, ratio, 2.0, 3600.0);
    let mut r = rng(seed);
    let mut ev = vec![];
    while c.is_running() { if let Some(e) = c.tick(&mut r) { ev.push(e); } }
    (ev, c)
}

#[test]
fn stronger_storms_strike_more_often() {
    let (weak, _) = run(0.55, 7);
    let (strong, _) = run(1.0, 7);
    assert!(strong.len() > weak.len() * 2, "weak {} strong {}", weak.len(), strong.len());
    assert!(weak.len() >= 3);
}

#[test]
fn events_stay_near_the_cloud_and_split_into_strikes_and_rumbles() {
    let (ev, _) = run(1.0, 99);
    let (radius, ratio, _) = storm_cloud_params(1.0, false);
    assert_eq!((radius, ratio), (8000.0, 0.6));
    let strikes = ev.iter().filter(|e| e.strike).count();
    let share = strikes as f32 / ev.len() as f32;
    assert!((0.35..0.85).contains(&share), "strike share {share} of {}", ev.len());
    for e in &ev {
        assert_eq!(e.strike, e.lightning, "strikes flash, rumbles do not");
        assert!(e.strike || e.rumble);
        // Never farther than half the radius from the cloud's path (x 1000..3000, y 2000).
        assert!((e.y - 2000).abs() <= 4000 && e.x >= 1000 - 4000 && e.x <= 3000 + 4000);
    }
}

#[test]
fn a_cloud_drifts_and_dies() {
    let mut c = ThunderCloud::new([0.0, 0.0], [100.0, 200.0], 100.0, 0.5, 0.3, 1.0, 120.0);
    assert!(c.is_running());
    assert_eq!(c.position(), [0, 0]);
    let mut r = rng(3);
    for _ in 0..(5.0 * TICK_HZ) as u32 { c.tick(&mut r); }
    let p = c.position();
    assert!(p[0] > 0 && p[1] > p[0], "{p:?}");
    for _ in 0..(10.0 * TICK_HZ) as u32 { c.tick(&mut r); }
    assert!(!c.is_running());
    assert_eq!(c.tick(&mut r), None);
    assert_eq!(storm_cloud_params(0.9, true), (15000.0, 0.8, 1.0));
}
