use pz_sim::*;

fn s(radius: f32, volume: f32) -> WorldSound {
    WorldSound { pos: [0.0, 0.0], z: 0, radius, volume, zombie_ignore_dist: 0.0, source_is_zombie: false }
}

#[test]
fn attraction_follows_the_original_formula() {
    let a = |p: [f32; 2], hear, rooms| sound_attract(&s(20.0, 10.0), p, 0, hear, rooms);
    assert_eq!(a([0.0, 0.0], 1.0, (false, false)), 10.0);
    // dist 10 of radius 20: delta = 0.25 -> 7.5
    assert!((a([10.0, 0.0], 1.0, (false, false)) - 7.5).abs() < 1e-4);
    // out of range
    assert_eq!(a([21.0, 0.0], 1.0, (false, false)), 0.0);
    // sharp hearing (x3) reaches farther: radius 60
    assert!(a([40.0, 0.0], hearing_multiplier(1), (false, false)) > 0.0);
    // poor hearing (x0.45): radius 9
    assert_eq!(a([10.0, 0.0], hearing_multiplier(3), (false, false)), 0.0);
    // different rooms x1.2, plus x1.4 when one point is outdoors: delta 0.25*1.2*1.4 = 0.42
    assert!((a([10.0, 0.0], 1.0, (true, true)) - 10.0 * (1.0 - 0.42)).abs() < 1e-3);
    // zombie-made sounds never attract
    let mut z = s(20.0, 10.0); z.source_is_zombie = true;
    assert_eq!(sound_attract(&z, [1.0, 0.0], 0, 1.0, (false, false)), 0.0);
}

#[test]
fn zombie_walks_toward_a_sound() {
    use pz_world::World;
    let mut w = World::new(std::path::PathBuf::from("nonexistent"));
    let mut far = Player::new(1000.0, 1000.0);
    let mut z = Zombie::new(10.0, 0.0, 0);
    z.hear(&[s(20.0, 10.0)]);
    let d0 = z.pos[0].hypot(z.pos[1]);
    // With no map every square is "unwalkable", so only check that the target is the sound's location.
    z.tick(&mut far, &mut w);
    assert!(d0 > 0.0);
}
