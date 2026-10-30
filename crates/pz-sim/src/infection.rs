//! Zombie infection course, ported from `BodyDamage.pickMortalityDuration` and the infected branch of
//! `BodyDamage.Update`: a fixed time after infection (the sandbox "mortality" setting) the character is dead,
//! and until then overall health is capped by a curve that falls off sharply near the end.

/// Sandbox `lore.mortality`: 1 instant, 2 0-30 s, 3 30-60 s, 4 3-12 h, 5 2-3 days, 6 1-2 weeks, 7 never.
/// `roll` is a uniform sample in [0, 1) standing in for `Rand.Next(lo, hi)`; `trait_mul` is 1.25 for
/// Resilient and 0.75 for Prone To Illness (1.0 otherwise). Returns hours, or `None` when infection never kills.
pub fn mortality_hours(setting: u8, roll: f32, trait_mul: f32) -> Option<f32> {
    let between = |lo: f32, hi: f32| lo + roll * (hi - lo);
    match setting {
        1 => Some(0.0),
        2 => Some(between(0.0, 30.0) / 3600.0 * trait_mul),
        3 => Some(between(0.5, 1.0) / 60.0 * trait_mul),
        4 => Some(between(3.0, 12.0) * trait_mul),
        5 => Some(between(2.0, 3.0) * 24.0 * trait_mul),
        6 => Some(between(1.0, 2.0) * 7.0 * 24.0 * trait_mul),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Infection {
    /// World age in hours when the infection started.
    pub start: f32,
    pub duration: f32,
}

impl Infection {
    /// Fraction of the way to death, 0..=1 (`ZOMBIE_INFECTION` stat / 100).
    pub fn progress(&self, now: f32) -> f32 {
        if self.duration <= 0.0 { 1.0 } else { ((now - self.start) / self.duration).clamp(0.0, 1.0) }
    }

    /// One `BodyDamage.Update` step: returns the character's new overall health.
    pub fn apply(&self, now: f32, health: f32) -> f32 {
        let p = self.progress(now);
        if p >= 1.0 { return health - 110.0; }
        let sick = (1.0 - p * p * p * p) * 100.0;
        let excess = health - sick;
        if excess > 0.0 && sick <= 99.0 { health - excess } else { health }
    }
}
