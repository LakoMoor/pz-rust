use pz_sim::*;
use pz_world::World;

fn w() -> World { World::new(std::path::PathBuf::from("nonexistent")) }

fn zombie_facing(x: f32, y: f32, facing: [f32; 2]) -> Zombie {
    let mut z = Zombie::new(x, y, 0);
    z.facing = facing;
    z
}

#[test]
fn hit_scales_with_back_attacks_crits_and_repeats() {
    // Zombie facing away from the wielder (wielder at x=0, zombie faces +x): x1.5.
    let mut z = zombie_facing(1.0, 0.0, [1.0, 0.0]);
    assert!((z.hit(1.0, false, 1.0, [0.0, 0.0]) - 1.5).abs() < 1e-5);
    // Facing the wielder (dot = -1 <= -0.3): no bonus.
    let mut z = zombie_facing(1.0, 0.0, [-1.0, 0.0]);
    assert!((z.hit(1.0, false, 1.0, [0.0, 0.0]) - 1.0).abs() < 1e-5);
    // Critical hit: x max(2, multiplier).
    let mut z = zombie_facing(1.0, 0.0, [-1.0, 0.0]);
    assert!((z.hit(1.0, true, 1.0, [0.0, 0.0]) - 2.0).abs() < 1e-5);
    let mut z = zombie_facing(1.0, 0.0, [-1.0, 0.0]);
    assert!((z.hit(1.0, true, 3.0, [0.0, 0.0]) - 3.0).abs() < 1e-5);
    // Hit 4 on the same zombie: x (4-2)*1.5 = 3.
    let mut z = zombie_facing(1.0, 0.0, [-1.0, 0.0]);
    z.health = 1000.0;
    let dmg: Vec<f32> = (0..4).map(|_| z.hit(1.0, false, 1.0, [0.0, 0.0])).collect();
    assert_eq!(dmg, vec![1.0, 1.0, 1.0, 3.0]);
}

#[test]
fn surviving_hits_stagger_and_kills_remove_the_zombie() {
    let mut world = w();
    let mut p = Player::new(10.0, 10.0);
    p.tick(Input { dir: [1.0, 0.0], run: false }, &mut world); // face +x
    let bat = Weapon { damage: 0.5, range: 1.2, cooldown_ticks: 20, crit_chance: 0.0, crit_mult: 1.0 };
    let mut zs = vec![zombie_facing(p.pos[0] + 0.8, 10.0, [-1.0, 0.0]), zombie_facing(p.pos[0] - 0.8, 10.0, [-1.0, 0.0])];
    assert_eq!(p.attack(&mut zs, &bat), 0);
    assert!((zs[0].health - 1.45).abs() < 1e-4, "front zombie hit for 0.5");
    assert!(zs[0].stagger > 0, "survivor staggers");
    assert!((zs[1].health - 1.95).abs() < 1e-4, "zombie behind the player is out of the cone");
    assert_eq!(p.attack(&mut zs, &bat), 0, "swing is on cooldown");

    // A staggered zombie does not act: it cannot bite.
    let mut victim = Player::new(5.0, 5.0);
    let mut z = zombie_facing(4.5, 5.0, [1.0, 0.0]);
    z.stagger = 10;
    for _ in 0..10 { z.tick(&mut victim, &mut world); }
    assert_eq!(victim.health, 100.0);

    // A big hit kills and the corpse is removed.
    let big = Weapon { damage: 5.0, ..bat };
    for _ in 0..20 { p.tick(Input::default(), &mut world); }
    assert_eq!(p.attack(&mut zs, &big), 1);
    assert_eq!(zs.len(), 1);
}

#[test]
fn a_hit_makes_noise_that_zombies_can_hear() {
    let mut world = w();
    let mut p = Player::new(10.0, 10.0);
    p.tick(Input { dir: [1.0, 0.0], run: false }, &mut world);
    p.sounds.clear();
    let mut zs = vec![zombie_facing(p.pos[0] + 0.8, 10.0, [-1.0, 0.0])];
    let bat = Weapon { damage: 0.1, range: 1.2, cooldown_ticks: 20, crit_chance: 0.0, crit_mult: 1.0 };
    p.attack(&mut zs, &bat);
    assert_eq!(p.sounds.len(), 1);
    assert_eq!((p.sounds[0].radius, p.sounds[0].volume), (5.0, 1.0));
}
