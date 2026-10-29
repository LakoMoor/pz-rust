use pz_sim::*;
use pz_world::World;

fn w() -> World { World::new(std::path::PathBuf::from("nonexistent")) }

#[test]
fn swings_hit_in_front_only_and_kill() {
    let mut world = w();
    let mut p = Player::new(10.0, 10.0);
    p.tick(Input { dir: [1.0, 0.0], run: false }, &mut world); // face +x (no map: free movement)
    let mut zs = vec![Zombie::new(p.pos[0] + 0.8, 10.0, 0), Zombie::new(p.pos[0] - 0.8, 10.0, 0)];
    let bat = Weapon { damage: 1.5, range: 1.2, cooldown_ticks: 20 };
    assert_eq!(p.attack(&mut zs, &bat), 0);
    assert!((zs[0].health - 0.45).abs() < 1e-4);
    assert!((zs[1].health - 1.95).abs() < 1e-4, "zombie behind is untouched");
    assert_eq!(p.attack(&mut zs, &bat), 0, "swing is on cooldown");
    for _ in 0..20 { p.tick(Input::default(), &mut world); }
    assert_eq!(p.attack(&mut zs, &bat), 1);
    assert_eq!(zs.len(), 1);
}
