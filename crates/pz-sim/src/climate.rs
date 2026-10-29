//! Daily climate rolls from `ClimateValues.updateValues`.
use crate::jrandom::JavaRandom;
use crate::FogDay;

/// Roll today's morning fog exactly as the original does: reseed a `Random` from the date and the world's two
/// simplex offsets, then draw the chance, strength and duration.
///
/// `month` is 0-based (as in `DayInfo`), `day` is the day of month, `offset_c`/`offset_d` are the climate's
/// `simplexOffsetC/D` (truncated to int), `dawn` the day's dawn hour and `mean_temp` the day's mean temperature
/// including the air-mass term (`season mean + airmassSample * 8`).
pub fn roll_fog(year: i32, month: i32, day: i32, offset_c: i32, offset_d: i32, dawn: f32, mean_temp: f32) -> Option<FogDay> {
    let mut seed = (year - 1990).wrapping_mul(100000) as i64;
    seed += (month.wrapping_mul(day).wrapping_mul(1234)) as i64;
    seed += ((year - 1990).wrapping_mul(month).wrapping_mul(10000)) as i64;
    seed += (offset_d.wrapping_sub(offset_c).wrapping_mul(day)) as i64;
    // `new Random(1984)` is replaced by `setSeed(seed)` before the first draw, so the initial seed never matters.
    let mut rng = JavaRandom::new(seed);
    let r = rng.next_int_bound(1000) as f32;
    if r >= 200.0 { return None; }
    let mut duration = 4.0f32;
    let strength = if r < 25.0 { duration += 2.0; 1.0 } else { rng.next_float() };
    let randy = rng.next_float();
    if mean_temp < 0.0 { duration += 5.0 * strength; duration += 8.0 * randy; }
    else if mean_temp < 10.0 { duration += 2.5 * strength; duration += 5.0 * randy; }
    else if mean_temp < 20.0 { duration += 1.5 * strength; duration += 2.5 * randy; }
    else { duration += 1.0 * strength; duration += 1.0 * randy; }
    if duration > 24.0 - dawn { duration = 24.0 - dawn - 1.0; }
    Some(FogDay { strength, duration })
}
