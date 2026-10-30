//! Moodle levels that follow from the body model, ported from `Moodle.java` / `MoodleStat.java`.
//! Levels are 0 (none) to 4 (max). Moodles driven by stats that are not simulated yet (hunger, tiredness, pain
//! stat, ...) are not here.

use crate::body::Body;

/// Threshold ladder (`MoodleStat`): a value strictly above the n-th entry reaches level n+1.
fn ladder(value: f32, t: [f32; 4]) -> u8 { t.iter().filter(|&&x| value > x).count() as u8 }

/// `MoodleType.INJURED`: from missing health, thresholds 20 / 40 / 60 / 75. Dead characters show none.
pub fn injured(overall_health: f32) -> u8 {
    if overall_health == 0.0 { 0 } else { ladder(100.0 - overall_health, [20.0, 40.0, 60.0, 75.0]) }
}

/// `MoodleType.BLEEDING`: one level per bleeding part (max 4); a bleeding neck is always max.
pub fn bleeding(body: &Body, overall_health: f32) -> u8 {
    if overall_health == 0.0 { return 0; }
    const NECK: usize = 9;
    if body.parts[NECK].bleeding_time > 0.0 { return 4; }
    body.num_bleeding().min(4) as u8
}

/// `MoodleType.SICK`: `apparent infection / 100 + SICKNESS stat`, thresholds 0.25 / 0.5 / 0.75 / 0.9. The apparent
/// infection is the largest of zombie fever, zombie infection and food sickness (all 0..=100); `sickness` is the
/// separate 0..1 stat. Dead characters show none.
pub fn sick(apparent_infection_pct: f32, sickness: f32, overall_health: f32) -> u8 {
    if overall_health == 0.0 { 0 } else { ladder(apparent_infection_pct / 100.0 + sickness, [0.25, 0.5, 0.75, 0.9]) }
}

/// `MoodleType.PAIN`: from the pain stat, thresholds 10 / 20 / 50 / 75. (The game only re-evaluates it every
/// 120 ticks; callers wanting that lag should sample accordingly.)
pub fn pain(pain_stat: f32, overall_health: f32) -> u8 {
    if overall_health == 0.0 { 0 } else { ladder(pain_stat, [10.0, 20.0, 50.0, 75.0]) }
}

/// `MoodleType.HUNGRY`: from the hunger stat, thresholds 0.15 / 0.25 / 0.45 / 0.7.
pub fn hungry(hunger: f32, overall_health: f32) -> u8 {
    if overall_health == 0.0 { 0 } else { ladder(hunger, [0.15, 0.25, 0.45, 0.7]) }
}

/// `MoodleType.THIRST`: from the thirst stat, thresholds 0.12 / 0.25 / 0.7 / 0.84.
pub fn thirst(thirst: f32) -> u8 { ladder(thirst, [0.12, 0.25, 0.7, 0.84]) }

/// `MoodleType.TIRED`: from the fatigue stat, thresholds 0.6 / 0.7 / 0.8 / 0.9.
pub fn tired(fatigue: f32, overall_health: f32) -> u8 {
    if overall_health == 0.0 { 0 } else { ladder(fatigue, [0.6, 0.7, 0.8, 0.9]) }
}

/// `MoodleType.HYPOTHERMIA`: from the core temperature, below 36.5 / 35 / 30 / 25 degrees. (The two lowest levels
/// are also withheld by high intoxication in the game; that stat is not modelled.) A zero reading means "unset".
pub fn hypothermia(core_celcius: f32) -> u8 {
    if core_celcius == 0.0 { return 0; }
    match core_celcius { t if t < 25.0 => 4, t if t < 30.0 => 3, t if t < 35.0 => 2, t if t < 36.5 => 1, _ => 0 }
}

/// `MoodleType.HYPERTHERMIA`: above 37.5 / 39 / 40 / 41 degrees.
pub fn hyperthermia(core_celcius: f32) -> u8 {
    match core_celcius { t if t > 41.0 => 4, t if t > 40.0 => 3, t if t > 39.0 => 2, t if t > 37.5 => 1, _ => 0 }
}
