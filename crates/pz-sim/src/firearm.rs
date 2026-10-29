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
