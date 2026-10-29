//! Firearm to-hit chance, ported from `CombatManager.calculateHitChanceData` and the helpers it calls.
//!
//! Config values are the defaults of `CombatConfigKey`. The shooter is a neutral character (Aiming 0, no traits,
//! no moodles, no pain, no head gear, clear weather), standing or walking; the sandbox weather and character
//! modifiers are not ported.

const POINT_BLANK_DISTANCE: f32 = 3.5;
const POINT_BLANK_TO_HIT_MAXIMUM_BONUS: f32 = 40.0;
const POINT_BLANK_DROP_OFF_TO_HIT_PENALTY: f32 = 0.7;
const SIGHTLESS_TO_HIT_BASE_DISTANCE: f32 = 15.0;
const SIGHTLESS_TO_HIT_PRONE_MODIFIER: f32 = 2.0;
const SIGHTLESS_AIM_DELAY_TO_HIT_DISTANCE_MODIFIER: f32 = 0.1;
const OPTIMAL_RANGE_TO_HIT_MAXIMUM_BONUS: f32 = 15.0;
const OPTIMAL_RANGE_DROP_OFF_TO_HIT_PENALTY: f32 = 4.0;
const OPTIMAL_RANGE_DROP_OFF_TO_HIT_PENALTY_INCREMENT: f32 = 0.3;
const MINIMUM_TO_HIT_CHANCE: f32 = 5.0;
const MAXIMUM_START_TO_HIT_CHANCE: f32 = 95.0;
const MAXIMUM_TO_HIT_CHANCE: f32 = 100.0;

/// The script fields of a firearm that feed the hit chance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Firearm {
    /// `HitChance`
    pub hit_chance: f32,
    /// `MinSightRange` / `MaxSightRange`: the distance band where the sights help most.
    pub min_sight_range: f32,
    pub max_sight_range: f32,
}

fn distance_modifier_sightless(dist: f32, prone: bool) -> f32 {
    if dist > POINT_BLANK_DISTANCE {
        let d = POINT_BLANK_DISTANCE - dist;
        d * (SIGHTLESS_TO_HIT_BASE_DISTANCE + d * -POINT_BLANK_DROP_OFF_TO_HIT_PENALTY)
    } else if dist < POINT_BLANK_DISTANCE {
        (POINT_BLANK_DISTANCE - dist) / POINT_BLANK_DISTANCE * POINT_BLANK_TO_HIT_MAXIMUM_BONUS
            * if prone { SIGHTLESS_TO_HIT_PRONE_MODIFIER } else { 1.0 }
    } else {
        0.0
    }
}

fn aim_delay_penalty_sightless(mut delay: f32, dist: f32) -> f32 {
    if dist < POINT_BLANK_DISTANCE {
        delay *= dist / POINT_BLANK_DISTANCE;
    } else if dist > POINT_BLANK_DISTANCE {
        delay *= 1.0 + (dist - POINT_BLANK_DISTANCE * SIGHTLESS_AIM_DELAY_TO_HIT_DISTANCE_MODIFIER);
    }
    delay
}

fn distance_modifier(dist: f32, min: f32, max: f32) -> f32 {
    if dist < min {
        return if dist > POINT_BLANK_DISTANCE {
            (dist - min) * (OPTIMAL_RANGE_DROP_OFF_TO_HIT_PENALTY + (dist - min) * -OPTIMAL_RANGE_DROP_OFF_TO_HIT_PENALTY_INCREMENT)
        } else { 0.0 };
    }
    if dist > max {
        return -((dist - max) * (OPTIMAL_RANGE_DROP_OFF_TO_HIT_PENALTY + (dist - max) * OPTIMAL_RANGE_DROP_OFF_TO_HIT_PENALTY_INCREMENT));
    }
    let scale = (max - min) * 0.5;
    OPTIMAL_RANGE_TO_HIT_MAXIMUM_BONUS * (-(dist - (min + scale)).powi(2) / (2.0 * ((max - min) / 7.0)).powi(2)).exp()
}

fn aim_delay_penalty(mut delay: f32, dist: f32, min: f32, max: f32) -> f32 {
    if min > -1.0 && dist >= min && dist <= max {
        let scale = (max - min) * 0.5;
        delay *= 1.0 - (1.0 - (dist - (min + scale)).abs() / scale) * 0.25;
    } else if dist > max {
        delay *= 1.0 + (dist - max) * 0.1;
    }
    delay * if dist < POINT_BLANK_DISTANCE { dist / POINT_BLANK_DISTANCE } else { 1.0 }
}

/// `CombatManager.getMovePenalty` for a character with Aiming 0 and Nimble 0.
fn move_penalty(been_moving_for: f32, dist: f32) -> f32 {
    let p = been_moving_for;
    if dist < 10.0 { p * dist / 10.0 } else { p * (1.0 + (dist - 10.0) * 0.07) }
}

impl Firearm {
    /// Percent chance (5..=100) that a shot at a target `dist` squares away hits.
    /// `aim_delay` is the remaining aiming delay (0 = fully aimed), `been_moving_for` the shooter's movement penalty.
    pub fn hit_chance(&self, dist: f32, prone: bool, aim_delay: f32, been_moving_for: f32) -> i32 {
        let mut chance = self.hit_chance.min(MAXIMUM_START_TO_HIT_CHANCE);
        let aim_delay = aim_delay.max(0.0);
        let sightless = distance_modifier_sightless(dist, prone) - aim_delay_penalty_sightless(aim_delay, dist);
        let sights = distance_modifier(dist, self.min_sight_range, self.max_sight_range)
            - aim_delay_penalty(aim_delay, dist, self.min_sight_range, self.max_sight_range);
        chance += sightless.max(sights);
        let mut penalty = move_penalty(been_moving_for, dist);
        if dist < POINT_BLANK_DISTANCE {
            let base = dist / POINT_BLANK_DISTANCE;
            penalty *= base / 5.0;
            chance /= base * 1.1;
        }
        chance -= penalty;
        // `(int)` truncates toward zero; the result is then clamped.
        (chance as i32 as f32).clamp(MINIMUM_TO_HIT_CHANCE, MAXIMUM_TO_HIT_CHANCE) as i32
    }
}

use crate::{Player, WorldSound, Zombie};

/// A loaded firearm: the hit-chance parameters plus what a shot does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gun {
    pub firearm: Firearm,
    /// `MinDamage` / `MaxDamage`: a hit deals a uniform roll in this range (`Rand.Next(min, max)`, rangeDel 1.0).
    pub min_damage: f32,
    pub max_damage: f32,
    /// `MaxRange`
    pub max_range: f32,
    /// `RecoilDelay`, used here as the ticks between shots.
    pub recoil_delay: u32,
    /// Rounds in the magazine (`ClipSize` when full).
    pub ammo: u32,
    /// `SoundRadius` / `SoundVolume`: the noise of a shot.
    pub sound_radius: f32,
    pub sound_volume: f32,
}

impl Gun {
    /// Top the magazine up to `clip_size` from `available` rounds; returns how many rounds were taken.
    pub fn load(&mut self, clip_size: u32, available: u32) -> u32 {
        let take = clip_size.saturating_sub(self.ammo).min(available);
        self.ammo += take;
        take
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shot { Dry, Cooldown, Miss, Hit { killed: bool } }

impl Player {
    /// Fire `gun` along the facing direction at the closest zombie within range and a narrow cone.
    /// Always spends a round and makes a noise; the hit is rolled against [`Firearm::hit_chance`].
    ///
    /// Simplified against the original: one bullet, no piercing, no cover or line-of-sight test, and a hit only
    /// subtracts health (no stagger or knock-down from bullets yet).
    pub fn shoot(&mut self, zombies: &mut Vec<Zombie>, gun: &mut Gun) -> Shot {
        if self.swing_cd > 0 { return Shot::Cooldown; }
        if gun.ammo == 0 { return Shot::Dry; }
        gun.ammo -= 1;
        self.swing_cd = gun.recoil_delay;
        self.sounds.push(WorldSound { pos: self.pos, z: self.z, radius: gun.sound_radius, volume: gun.sound_volume, zombie_ignore_dist: 0.0, source_is_zombie: false });
        let target = zombies.iter().enumerate().filter(|(_, zb)| zb.z == self.z).filter_map(|(i, zb)| {
            let d = [zb.pos[0] - self.pos[0], zb.pos[1] - self.pos[1]];
            let dist = d[0].hypot(d[1]);
            let in_cone = dist < 1e-4 || (d[0] * self.facing[0] + d[1] * self.facing[1]) / dist > 0.95;
            (in_cone && dist <= gun.max_range).then_some((i, dist))
        }).min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((i, dist)) = target else { return Shot::Miss };
        let chance = gun.firearm.hit_chance(dist, zombies[i].on_floor, 0.0, 0.0);
        self.rng ^= self.rng << 13; self.rng ^= self.rng >> 17; self.rng ^= self.rng << 5;
        if (self.rng % 100) as i32 >= chance { return Shot::Miss; }
        self.rng ^= self.rng << 13; self.rng ^= self.rng >> 17; self.rng ^= self.rng << 5;
        let roll = (self.rng % 1000) as f32 / 1000.0;
        zombies[i].health -= gun.min_damage + (gun.max_damage - gun.min_damage) * roll;
        let killed = zombies[i].health <= 0.0;
        if killed { zombies.remove(i); }
        Shot::Hit { killed }
    }
}
