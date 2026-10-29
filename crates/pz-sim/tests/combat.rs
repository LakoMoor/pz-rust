use pz_sim::*;
use pz_world::World;

fn w() -> World { World::new(std::path::PathBuf::from("nonexistent")) }

fn bat() -> Weapon {
    Weapon { damage: 0.5, range: 1.2, cooldown_ticks: 20, crit_chance: 0.0, crit_mult: 1.0, push_back_mod: 1.0, small_blade: false, sound_radius: 0.0, sound_volume: 0.0 }
}

/// A zombie facing the wielder at the origin (no back-attack bonus).
fn facing_zombie() -> Zombie {
    let mut z = Zombie::new(1.0, 0.0, 0);
    z.facing = [-1.0, 0.0];
    z.health = 1000.0;
    z
}

#[test]
fn damage_scales_with_back_attacks_crits_and_repeats() {
    // Zombie facing away from the wielder: x1.5.
    let mut z = Zombie::new(1.0, 0.0, 0);
    z.health = 1000.0;
    z.facing = [1.0, 0.0];
    assert!((z.hit(&bat(), 1.0, false, [0.0, 0.0]) - 1.5).abs() < 1e-5);
    // Facing the wielder: plain damage; a crit multiplies by max(2, mult).
    assert!((facing_zombie().hit(&bat(), 1.0, false, [0.0; 2]) - 1.0).abs() < 1e-5);
    assert!((facing_zombie().hit(&bat(), 1.0, true, [0.0; 2]) - 2.0).abs() < 1e-5);
    let big = Weapon { crit_mult: 3.0, ..bat() };
    assert!((facing_zombie().hit(&big, 1.0, true, [0.0; 2]) - 3.0).abs() < 1e-5);
    // Hit 4 on the same zombie: x (4 - 2) * 1.5 = 3.
    let mut z = facing_zombie();
    let dmg: Vec<f32> = (0..4).map(|_| z.hit(&bat(), 1.0, false, [0.0; 2])).collect();
    assert_eq!(dmg, vec![1.0, 1.0, 1.0, 3.0]);
}

#[test]
fn knockback_time_follows_stagger_back_state() {
    // hitForce = min(dmg,1)*2; stagger = clamp(35 * hitForce * pushBackMod, 20, 30).
    let mut z = facing_zombie();
    z.hit(&bat(), 0.5, false, [0.0; 2]); // force 1.0 -> 35 -> clamped to 30
    assert_eq!(z.stagger, 30);
    let mut z = facing_zombie();
    z.hit(&bat(), 0.1, false, [0.0; 2]); // force 0.2 -> 7 -> clamped up to 20
    assert_eq!(z.stagger, 20);
    let heavy = Weapon { push_back_mod: 0.3, ..bat() };
    let mut z = facing_zombie();
    z.hit(&heavy, 0.5, false, [0.0; 2]); // 35 * 1.0 * 0.3 = 10.5 -> 20
    assert_eq!(z.stagger, 20);
    // Small blades never push zombies back (mod 0): minimum stagger.
    let knife = Weapon { push_back_mod: 5.0, small_blade: true, ..bat() };
    let mut z = facing_zombie();
    z.hit(&knife, 0.5, false, [0.0; 2]);
    assert_eq!(z.stagger, 20);
}

#[test]
fn only_crits_knock_zombies_down_and_they_get_back_up() {
    let mut world = w();
    let mut z = facing_zombie();
    z.hit(&bat(), 0.5, false, [0.0; 2]);
    assert!(!z.on_floor && !z.knocked_down, "a plain hit only staggers");

    let mut z = facing_zombie();
    z.hit(&bat(), 0.5, true, [0.0; 2]);
    assert!(z.knocked_down && z.on_floor, "a critical hit knocks the zombie down");
    assert!((30.0..90.0).contains(&z.reanimate_timer), "Rand.Next(60) + 30, got {}", z.reanimate_timer);

    // While down it neither moves nor bites.
    let mut victim = Player::new(1.0, 0.0);
    let start = z.pos;
    let ticks = z.reanimate_timer as u32;
    for _ in 0..ticks - 1 { z.tick(&mut victim, &mut world); }
    assert_eq!(victim.health, 100.0);
    assert_eq!(z.pos, start);
    assert!(z.on_floor);
    // ... then it gets up.
    z.tick(&mut victim, &mut world);
    assert!(!z.on_floor && !z.knocked_down);

    // A zombie already on the floor is always "knocked down" again by any hit.
    let mut z = facing_zombie();
    z.on_floor = true;
    z.reanimate_timer = 50.0;
    z.hit(&bat(), 0.1, false, [0.0; 2]);
    assert!(z.knocked_down && z.on_floor);
}

#[test]
fn swings_hit_in_front_only_and_kill() {
    let mut world = w();
    let mut p = Player::new(10.0, 10.0);
    p.tick(Input { dir: [1.0, 0.0], run: false }, &mut world); // face +x
    let mut front = Zombie::new(p.pos[0] + 0.8, 10.0, 0);
    front.facing = [-1.0, 0.0];
    let mut behind = Zombie::new(p.pos[0] - 0.8, 10.0, 0);
    behind.facing = [-1.0, 0.0];
    let mut zs = vec![front, behind];
    assert_eq!(p.attack(&mut zs, &bat()), 0);
    assert!((zs[0].health - 1.45).abs() < 1e-4, "front zombie takes 0.5");
    assert!(zs[0].stagger > 0);
    assert!((zs[1].health - 1.95).abs() < 1e-4, "zombie behind the player is out of the cone");
    assert_eq!(p.attack(&mut zs, &bat()), 0, "swing is on cooldown");

    let big = Weapon { damage: 5.0, ..bat() };
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
    let mut z = Zombie::new(p.pos[0] + 0.8, 10.0, 0);
    z.facing = [-1.0, 0.0];
    let weak = Weapon { damage: 0.1, ..bat() };
    p.attack(&mut vec![z], &weak);
    assert_eq!(p.sounds.len(), 1);
    assert_eq!((p.sounds[0].radius, p.sounds[0].volume), (5.0, 1.0));
}

#[test]
fn every_swing_is_heard_with_the_weapons_sound_radius() {
    let mut world = w();
    let mut p = Player::new(10.0, 10.0);
    p.tick(Input { dir: [0.0, 0.0], run: false }, &mut world);
    p.sounds.clear();
    let loud = Weapon { sound_radius: 20.0, sound_volume: 25.0, ..bat() };
    // Nothing in reach: a miss still makes noise.
    p.attack(&mut vec![], &loud);
    assert_eq!(p.sounds.len(), 1);
    assert_eq!((p.sounds[0].radius, p.sounds[0].volume), (20.0, 25.0));
    // A silent weapon (radius 0) adds nothing on a miss.
    p.sounds.clear();
    p.tick(Input { dir: [0.0, 0.0], run: false }, &mut world);
    for _ in 0..40 { p.tick(Input { dir: [0.0, 0.0], run: false }, &mut world); }
    p.sounds.clear();
    p.attack(&mut vec![], &bat());
    assert!(p.sounds.is_empty());
}
