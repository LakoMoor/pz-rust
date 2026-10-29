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

/// Days since 1970-01-01 for a proleptic Gregorian date (month 0-based).
fn days_from_civil(y: i32, m0: i32, d: i32) -> i64 {
    let (y, m) = (if m0 < 2 { y - 1 } else { y } as i64, (m0 + 1) as i64);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn clerp64(t: f64, a: f64, b: f64) -> f64 {
    let t2 = (1.0 - (t * std::f64::consts::PI).cos()) / 2.0;
    a * (1.0 - t2) + b * t2
}

/// Dawn and dusk (hours) of a day, from `ErosionSeason.init` and `setDaylightData`: the day length follows a cosine
/// between the winter and summer solstice values for the latitude, and solar noon drifts by `summer_tilt` hours.
#[derive(Clone, Copy, Debug)]
pub struct Daylight { pub lat: f64, pub high_noon: f32, pub summer_tilt: f32 }

impl Default for Daylight {
    /// The `ErosionSeason` field defaults (latitude 38, noon 12.5, tilt 2).
    fn default() -> Self { Daylight { lat: 38.0, high_noon: 12.5, summer_tilt: 2.0 } }
}

impl Daylight {
    fn solstice_hours(&self) -> (f64, f64) {
        let k = self.lat.to_radians().tan() * 23.44f64.to_radians().tan();
        (2.0 * (-k).acos().to_degrees() / 15.0, 2.0 * k.acos().to_degrees() / 15.0)
    }

    /// `(dawn, dusk)` for the given date (month 0-based).
    pub fn dawn_dusk(&self, year: i32, month: i32, day: i32) -> (f32, f32) {
        let (su, wi) = self.solstice_hours();
        let today = days_from_civil(year, month, day);
        let (sum, win) = (days_from_civil(year, 5, 22), days_from_civil(year, 11, 22));
        let diff = |a: i64, b: i64| (a - b).abs() as f32;
        let (h1, start, end) = if today < win && today >= sum { (false, sum, win) }
            else if today >= win { (true, win, days_from_civil(year + 1, 5, 22)) }
            else { (true, days_from_civil(year - 1, 11, 22), sum) };
        let perc = (diff(today, start) / diff(start, end)) as f64;
        let (daylight, perc2) = if h1 { (clerp64(perc, wi, su), perc as f32) } else { (clerp64(perc, su, wi), 1.0 - perc as f32) };
        let noon = self.high_noon + self.summer_tilt * perc2;
        let half = daylight as f32 / 2.0;
        (noon - half, noon + half)
    }
}
