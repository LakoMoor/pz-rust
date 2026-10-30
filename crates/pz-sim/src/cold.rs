//! Catching and recovering from a common cold, ported from `BodyDamage.Update` (the catch-a-cold part) and
//! `UpdateCold`. Traits (prone to illness, resilient, outdoorsman), cold-reducing medicine and sneezing are not
//! modelled; rates are from `defines.lua` (`CatchAColdIncreaseRate`, `CatchAColdDecreaseRate`).

pub const CATCH_INCREASE: f32 = 0.003;
pub const CATCH_DECREASE: f32 = 0.175;
pub const PROGRESSION_RATE: f32 = 0.0112;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Cold {
    /// 0..100: how close the character is to coming down with a cold.
    pub catch: f32,
    pub has_cold: bool,
    /// 0..100 while ill.
    pub strength: f32,
}

/// What `UpdateCold` checks to decide whether the character can rest and recover.
#[derive(Debug, Clone, Copy)]
pub struct Rest {
    pub in_room: bool,
    pub wet_moodle: bool,
    pub hypothermia_level: u8,
    pub fatigue: f32,
    pub hunger: f32,
    pub thirst: f32,
}

impl Cold {
    /// One tick. `delta` is `Thermoregulator::catch_a_cold_delta`.
    pub fn tick(&mut self, delta: f32, rest: Rest) {
        if !self.has_cold && delta > 0.1 {
            self.catch += CATCH_INCREASE * delta;
            if self.catch >= 100.0 {
                self.catch = 0.0;
                self.has_cold = true;
                self.strength = 20.0;
            }
        }
        if delta <= 0.1 {
            self.catch = (self.catch - CATCH_DECREASE).max(0.0);
        }
        if self.has_cold {
            let recovering = rest.in_room && !rest.wet_moodle && rest.hypothermia_level < 1
                && rest.fatigue <= 0.5 && rest.hunger <= 0.25 && rest.thirst <= 0.25;
            if recovering {
                self.strength -= PROGRESSION_RATE;
                if self.strength < 0.0 {
                    self.strength = 0.0;
                    self.has_cold = false;
                    self.catch = 0.0;
                }
            } else {
                self.strength = (self.strength + PROGRESSION_RATE).min(100.0);
            }
        }
    }

    /// `getColdStrength`: zero unless ill.
    pub fn strength(&self) -> f32 { if self.has_cold { self.strength } else { 0.0 } }
}

/// `MoodleType.HAS_A_COLD`: cold strength above 20 / 40 / 60 / 75.
pub fn has_a_cold(strength: f32, overall_health: f32) -> u8 {
    if overall_health == 0.0 { return 0; }
    match strength { s if s > 75.0 => 4, s if s > 60.0 => 3, s if s > 40.0 => 2, s if s > 20.0 => 1, _ => 0 }
}
