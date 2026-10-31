//! The `STRESS` stat and its moodle, ported from `IsoGameCharacter.updateStats_Awake` / `updateStress` and
//! `Moodle` (`STRESS`). Rates are from `defines.lua`. Sound-driven stress, hemophobia, discomfort, nicotine
//! withdrawal and the pill/food/book effects are not modelled yet.

pub const DECREASE: f32 = 0.00003;
pub const FROM_BITE_OR_SCRATCH: f32 = 0.00005;

/// One awake tick. `delta_minutes_per_day` is `GameTime.getDeltaMinutesPerDay()`; `bitten`, `scratched`,
/// `infected` say whether the character has any bitten / scratched part or the zombie infection. Each adds the
/// same amount, so a bitten, scratched and infected character stresses three times as fast.
pub fn tick(stress: f32, delta_minutes_per_day: f32, bitten: bool, scratched: bool, infected: bool) -> f32 {
    let mut s = stress - DECREASE * delta_minutes_per_day;
    let per = FROM_BITE_OR_SCRATCH * delta_minutes_per_day;
    for on in [bitten, scratched, infected] {
        if on { s += per; }
    }
    s.clamp(0.0, 1.0)
}

/// `MoodleType.STRESS`: stress (plus nicotine withdrawal, not modelled) above 0.25 / 0.5 / 0.75 / 0.9.
pub fn moodle(stress: f32) -> u8 {
    match stress { s if s > 0.9 => 4, s if s > 0.75 => 3, s if s > 0.5 => 2, s if s > 0.25 => 1, _ => 0 }
}
