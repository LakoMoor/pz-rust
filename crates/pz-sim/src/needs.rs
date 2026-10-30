//! Hunger and thirst, ported from `IsoGameCharacter.updateThirst` / the hunger part of its stats update and the
//! `HUNGRY` / `THIRST` moodles. Rates are `ZomboidGlobals` from the game's `defines.lua` (a pz-lua test checks
//! they match the file). Traits, eating, and the well-fed moodle are not modelled yet.

pub const HUNGER_INCREASE: f32 = 0.0000032 * 3.0;
pub const HUNGER_INCREASE_EXERCISE: f32 = 0.0000032 * 6.0;
pub const THIRST_INCREASE: f32 = 0.0000040 * 2.0;
pub const THIRST_SLEEPING_INCREASE: f32 = 0.0000010;

/// Severe-moodle health cost divisors from `BodyDamage.Update` (relative to `healthReductionFromSevereBadMoodles`).
pub const HUNGRY_DAMAGE_DIVISOR: f32 = 50.0;
pub const THIRST_DAMAGE_DIVISOR: f32 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Needs {
    /// 0 (full) .. 1 (starving).
    pub hunger: f32,
    /// 0 (quenched) .. 1 (parched).
    pub thirst: f32,
}

impl Needs {
    /// One tick. `delta_minutes_per_day` is `GameTime.getDeltaMinutesPerDay()` (30 / minutes per day);
    /// `running` means running and actually moving; `asleep` swaps in the sleeping thirst rate.
    pub fn tick(&mut self, delta_minutes_per_day: f32, running: bool, asleep: bool) {
        let appetite = 1.0 - self.hunger;
        self.hunger += if running { HUNGER_INCREASE_EXERCISE / 3.0 } else { HUNGER_INCREASE } * appetite * delta_minutes_per_day;
        let thirst_rate = if asleep { THIRST_SLEEPING_INCREASE } else { THIRST_INCREASE * if running { 1.2 } else { 1.0 } };
        self.thirst += thirst_rate * delta_minutes_per_day;
        self.hunger = self.hunger.clamp(0.0, 1.0);
        self.thirst = self.thirst.clamp(0.0, 1.0);
    }
}
