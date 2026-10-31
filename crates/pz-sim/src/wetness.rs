//! Body wetness of an unclothed character, ported from `BodyDamage.UpdateWetness` and the bare-skin paths of
//! `ClothingWetness.updateWetness`: rain soaks the skin, sweat wets it, body heat and warm air dry it, and the
//! parts then blend toward their average. Under clothing the skin only gets wet (or dries) through the innermost
//! garment (`innermost`: its wetness per covered part). Umbrellas, tents and vehicles are not modelled.

use crate::body::PARTS;
use crate::thermo::{Thermoregulator, DIST_TO_CORE};

/// `ZomboidGlobals` from `defines.lua`.
pub const WETNESS_INCREASE: f32 = 0.015;
pub const WETNESS_DECREASE: f32 = 0.01;

#[derive(Debug, Clone, Copy)]
pub struct Weather {
    /// Standing outdoors with no roof overhead.
    pub outside: bool,
    pub raining: bool,
    /// `ClimateManager.getRainIntensity`.
    pub rain_intensity: f32,
    /// Air temperature at the character (no wind chill).
    pub air_temperature: f32,
}

/// One tick (game-time multiplier 1). `wetness` is per part, 0..=100; `stat` is the `WETNESS` stat, the
/// running blended value the parts are pulled toward. Returns the new stat.
pub fn update(wetness: &mut [f32; PARTS], stat: f32, th: &Thermoregulator, w: Weather, innermost: &[Option<f32>; PARTS]) -> f32 {
    let (mut inc, mut dec) = (0.0f32, 0.0f32);
    if w.outside && w.raining {
        inc = if w.rain_intensity < 0.1 { 0.0 } else { w.rain_intensity };
    } else {
        let mut v = 0.1;
        if w.air_temperature > 5.0 { v += (w.air_temperature - 5.0) / 10.0; }
        dec = v.max(0.0);
    }
    let (base_inc, base_dec) = (WETNESS_INCREASE, WETNESS_DECREASE);
    let mut sweating = [false; PARTS];
    // Sweat and body-heat drying act on the skin directly.
    for k in 0..PARTS {
        let n = &th.nodes[k];
        let mut perspiration = n.secondary_delta.clamp(0.0, 1.0);
        perspiration *= perspiration;
        perspiration *= 0.2 + 0.8 * (1.0 - DIST_TO_CORE[k]);
        let mut d = 0.0;
        if perspiration > 0.1 {
            d += perspiration;
        } else {
            let mut heat = (n.skin_celcius - 20.0) / 22.0;
            heat *= heat;
            heat = (heat - inc).max(0.0);
            d -= heat;
            if inc > 0.0 { d = 0.0; }
        }
        sweating[k] = d > 0.0;
        if d != 0.0 && innermost[k].is_none() {
            d *= if d > 0.0 { base_inc } else { base_dec };
            wetness[k] = (wetness[k] + d).clamp(0.0, 100.0);
        }
    }
    // Rain on, or air drying, the skin.
    let base = if inc > 0.0 { inc * base_inc } else { -dec * base_dec };
    for k in 0..PARTS {
        match innermost[k] {
            None => {
                if (base < 0.0 && !sweating[k]) || wetness[k] <= 100.0 {
                    wetness[k] = (wetness[k] + base).clamp(0.0, 100.0);
                }
            }
            Some(under) => {
                if base > 0.0 && !sweating[k] && under >= 50.0 && wetness[k] <= 100.0 {
                    wetness[k] = (wetness[k] + base / 2.0).clamp(0.0, 100.0);
                }
                if base < 0.0 && !sweating[k] && under <= 50.0 {
                    wetness[k] = (wetness[k] + base / 2.0).clamp(0.0, 100.0);
                }
            }
        }
    }
    // The parts blend toward their average, which in turn follows the stat.
    let avg = wetness.iter().sum::<f32>() / PARTS as f32;
    let target = avg + (stat - avg) * 0.1;
    *wetness = [target; PARTS];
    target
}
