use pz_sim::firearm::Firearm;

/// Base.Pistol: HitChance 50, MinSightRange 2, MaxSightRange 6.
const PISTOL: Firearm = Firearm { hit_chance: 50.0, min_sight_range: 2.0, max_sight_range: 6.0 };

#[test]
fn hit_chance_matches_hand_computed_values_from_the_decompiled_formula() {
    // Centre of the sight band (dist 4): sights bonus 15 beats the sightless -7.7 -> 65.
    assert_eq!(PISTOL.hit_chance(4.0, false, 0.0, 0.0), 65);
    // Far beyond the band (dist 12): -(6 * (4 + 6 * 0.3)) = -34.8 -> 15.2 -> 15.
    assert_eq!(PISTOL.hit_chance(12.0, false, 0.0, 0.0), 15);
    // Hopeless range clamps to the 5% floor.
    assert_eq!(PISTOL.hit_chance(40.0, false, 0.0, 0.0), 5);
    // The starting chance is capped at 95 before any bonus.
    let sniper = Firearm { hit_chance: 120.0, ..PISTOL };
    assert_eq!(sniper.hit_chance(4.0, false, 0.0, 0.0), 100);
}

#[test]
fn moving_and_unfinished_aiming_cost_accuracy() {
    let still = PISTOL.hit_chance(4.0, false, 0.0, 0.0);
    assert!(PISTOL.hit_chance(4.0, false, 0.0, 20.0) < still);
    assert!(PISTOL.hit_chance(4.0, false, 10.0, 0.0) < still);
    // Move penalty at dist 4 is 20 * 0.4 = 8 -> 57.
    assert_eq!(PISTOL.hit_chance(4.0, false, 0.0, 20.0), 57);
}

#[test]
fn point_blank_is_easy_even_for_a_bad_gun() {
    assert!(PISTOL.hit_chance(1.0, false, 0.0, 0.0) >= 90);
    let prone_bonus = PISTOL.hit_chance(2.5, true, 0.0, 0.0) >= PISTOL.hit_chance(2.5, false, 0.0, 0.0);
    assert!(prone_bonus);
}

use pz_sim::firearm::{Gun, Shot};
use pz_sim::{Input, Player, Zombie};

fn pistol(ammo: u32) -> Gun {
    Gun { firearm: PISTOL, min_damage: 0.6, max_damage: 1.0, max_range: 15.0, recoil_delay: 12, ammo, sound_radius: 100.0, sound_volume: 100.0 }
}

#[test]
fn shooting_spends_ammo_makes_noise_and_respects_recoil() {
    let mut p = Player::new(0.0, 0.0);
    let mut zs: Vec<Zombie> = vec![];
    let mut gun = pistol(2);
    assert_eq!(p.shoot(&mut zs, &mut gun), Shot::Miss);
    assert_eq!(gun.ammo, 1);
    assert_eq!(p.sounds.len(), 1);
    assert_eq!((p.sounds[0].radius, p.sounds[0].volume), (100.0, 100.0));
    assert_eq!(p.shoot(&mut zs, &mut gun), Shot::Cooldown);
    assert_eq!(gun.ammo, 1, "no round spent during recoil");
    let mut world = pz_world::World::new(std::path::PathBuf::from("."));
    for _ in 0..12 { p.tick(Input::default(), &mut world); }
    assert_eq!(p.shoot(&mut zs, &mut gun), Shot::Miss);
    for _ in 0..12 { p.tick(Input::default(), &mut world); }
    assert_eq!(p.shoot(&mut zs, &mut gun), Shot::Dry);
}

#[test]
fn bullets_kill_zombies_in_front_and_ignore_those_behind() {
    let mut world = pz_world::World::new(std::path::PathBuf::from("."));
    let mut p = Player::new(0.0, 0.0);
    let mut gun = pistol(1000);
    // Zombies directly ahead at the centre of the sight band (65% per shot), and one behind.
    let mut zs = vec![Zombie::new(4.0, 0.0, 0), Zombie::new(-4.0, 0.0, 0)];
    let mut hits = 0;
    for _ in 0..200 {
        for _ in 0..12 { p.tick(Input::default(), &mut world); }
        if let Shot::Hit { .. } = p.shoot(&mut zs, &mut gun) { hits += 1; }
        if zs.len() < 2 { break; }
    }
    assert!(hits >= 1);
    assert_eq!(zs.len(), 1, "the zombie in front dies");
    assert_eq!(zs[0].pos, [-4.0, 0.0], "the one behind is untouched");
}
