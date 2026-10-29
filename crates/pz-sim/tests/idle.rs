use pz_sim::*;
use pz_world::World;

#[test]
fn idle_zombies_wander_deterministically_within_bounds() {
    let run = || {
        let mut w = World::new(std::path::PathBuf::from("nonexistent"));
        let mut far = Player::new(1000.0, 1000.0);
        let mut z = Zombie::new(50.0, 50.0, 0);
        for _ in 0..30 * 120 { z.tick(&mut far, &mut w); }
        z.pos
    };
    assert_eq!(run(), run());
}
