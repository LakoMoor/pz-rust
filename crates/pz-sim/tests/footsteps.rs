use pz_sim::*;
use pz_world::World;

#[test]
fn running_is_louder_than_walking() {
    let measure = |run| {
        let mut w = World::new(std::path::PathBuf::from("nonexistent"));
        let mut p = Player::new(0.0, 0.0);
        // No map: nothing blocks, but nothing is "indoors" either.
        let mut radii = vec![];
        for _ in 0..30 * 20 {
            p.tick(Input { dir: [1.0, 0.0], run }, &mut w);
            radii.extend(p.sounds.drain(..).map(|s| s.radius));
        }
        radii
    };
    let (walk, run) = (measure(false), measure(true));
    assert!(!walk.is_empty() && !run.is_empty());
    assert!(walk.iter().all(|&r| r == 7.0), "walk: ceil(0.5*1.4*10) = 7, got {walk:?}");
    assert!(run.iter().all(|&r| r == 19.0), "run: ceil(1.3*1.4*10) = 19, got {run:?}");
}

#[test]
fn standing_still_is_silent() {
    let mut w = World::new(std::path::PathBuf::from("nonexistent"));
    let mut p = Player::new(0.0, 0.0);
    for _ in 0..300 { p.tick(Input::default(), &mut w); }
    assert!(p.sounds.is_empty());
}
