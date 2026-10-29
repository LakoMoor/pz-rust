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

use crate::noise2d::Noise2D;

/// Seasonal temperature model from `ErosionSeason` (`init` + the temperature half of `setSeasonData`).
///
/// The defaults are the `ErosionConfig.Season` field defaults; a save's real values (and its three noise seeds,
/// which are random per save) must be supplied by the caller to reproduce that save's weather.
pub struct Seasons {
    temp_max: i32,
    temp_min: i32,
    temp_diff: i32,
    season_lag: i64,
    noise: Noise2D,
}

impl Default for Seasons {
    fn default() -> Self { Self::new(25, 0, 7, 31, 64, 128, 255) }
}

impl Seasons {
    pub fn new(temp_max: i32, temp_min: i32, temp_diff: i32, season_lag: i32, seed_a: i32, seed_b: i32, seed_c: i32) -> Self {
        let mut noise = Noise2D::new();
        noise.add_layer(seed_a, 8.0, 2.0);
        noise.add_layer(seed_b, 6.0, 4.0);
        noise.add_layer(seed_c, 4.0, 6.0);
        Self { temp_max, temp_min, temp_diff, season_lag: season_lag as i64, noise }
    }

    /// Strength of the summer / winter of `year`: `S + E - 1`, each from the layered noise.
    fn summer_str(&self, y: i32) -> f32 { self.noise.layered_noise((128 + y) as f32, 128.0) + self.noise.layered_noise(128.0, (128 + y) as f32) - 1.0 }
    fn winter_str(&self, y: i32) -> f32 { self.noise.layered_noise((64 + y) as f32, 64.0) + self.noise.layered_noise(64.0, (64 + y) as f32) - 1.0 }

    /// `(mean temperature, actual temperature)` of a day (month 0-based): the seasonal average between the hottest
    /// and coldest days of the year, and that average plus the day-to-day noise swing.
    pub fn day_temperature(&self, year: i32, month: i32, day: i32) -> (f32, f32) {
        let d = days_from_civil(year, month, day);
        let hot = |y: i32| days_from_civil(y, 5, 22) + self.season_lag;
        let cold = |y: i32| days_from_civil(y, 11, 22) + self.season_lag;
        let dd = |a: i64, b: i64| (a - b).abs() as f32;
        let half = (self.temp_diff / 2) as f32;
        let (tmax, tmin) = (self.temp_max as f32, self.temp_min as f32);
        let (hot0, cold0, hot1, cold1, hot2) = (hot(year - 1), cold(year - 1), hot(year), cold(year), hot(year + 1));
        let (a, b, perc) = if d < cold0 && d >= hot0 {
            (tmax + half * self.summer_str(year - 1), tmin + half * self.winter_str(year - 1), dd(d, hot0) / dd(hot0, cold0))
        } else if d < hot1 && d >= cold0 {
            (tmin + half * self.winter_str(year - 1), tmax + half * self.summer_str(year), dd(d, cold0) / dd(hot1, cold0))
        } else if d < cold1 && d >= hot1 {
            (tmax + half * self.summer_str(year), tmin + half * self.winter_str(year), dd(d, hot1) / dd(hot1, cold1))
        } else {
            (tmin + half * self.winter_str(year), tmax + half * self.summer_str(year + 1), dd(d, cold1) / dd(cold1, hot2))
        };
        let _ = hot2;
        let avg = clerp64(perc as f64, a as f64, b as f64) as f32;
        let noise = self.noise.layered_noise(d as f32 / 20.0, 0.0);
        (avg, avg + self.temp_diff as f32 * (noise * 2.0 - 1.0))
    }
}

use crate::simplex::noise as simplex;

/// World-age driven weather noise from `ClimateValues.updateValues`: air masses, temperature offset and wind.
/// The four offsets are random per save (`ClimateManager.simplexOffsetA..D`); callers supply them.
#[derive(Clone, Copy, Debug)]
pub struct Offsets { pub a: f64, pub b: f64, pub c: f64, pub d: f64 }

/// `getAirMassNoiseFrequencyMod`: hours per noise unit for the sandbox rain setting (1..5; 3 = normal).
pub fn air_mass_frequency(sandbox_rain: i32) -> f64 {
    match sandbox_rain { 1 => 300.0, 2 => 240.0, 4 => 145.0, 5 => 120.0, _ => 166.0 }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AirMass {
    /// -1..1; positive is a warm, clear front.
    pub temperature: f32,
    /// -1..1
    pub humidity_noise: f32,
    /// 0..0.65
    pub wind: f32,
    /// `cloudyT`: 0..1, how overcast the sky is.
    pub cloudy: f32,
}

impl AirMass {
    /// `world_age_hours` is the time since the world began; `day_perc` the day's position in the daylight cycle
    /// (`Daylight`'s `perc2`, 0 in midwinter .. 1 in midsummer); `hour` the hour of day.
    pub fn at(o: Offsets, world_age_hours: f64, hour: f32, day_perc: f32, sandbox_rain: i32) -> AirMass {
        let f = air_mass_frequency(sandbox_rain);
        let temperature = simplex(o.a, (world_age_hours - 48.0) / f) as f32;
        let humidity_noise = simplex(o.c, world_age_hours / f) as f32;
        let wind_mod = 1.0 - (temperature + 1.0) * 0.5;
        let wind_mod2 = 1.0 - day_perc * 0.4;
        let base = (simplex(world_age_hours / 40.0, o.a) as f32 + 1.0) * 0.5;
        let wind = base * wind_mod * wind_mod2 * 0.65;
        let _ = hour;
        let cloudy = (1.0 - ((temperature + 0.8) * 0.625).clamp(0.0, 1.0)) * 0.8;
        AirMass { temperature, humidity_noise, wind, cloudy: (cloudy + wind).clamp(0.0, 1.0) }
    }

    /// The air-mass term added to the seasonal mean temperature (`airMassTemperature * 8`).
    pub fn temperature_offset(&self) -> f32 { self.temperature * 8.0 }
}

use crate::GameClock;

fn days_in_month(year: i32, m0: i32) -> i32 {
    match m0 { 1 => if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 { 29 } else { 28 }, 3 | 5 | 8 | 10 => 30, _ => 31 }
}

/// Calendar plus climate state driving a [`GameClock`]: each new day sets dawn and dusk from the daylight curve
/// and rolls that day's fog from the date, the seasonal temperature and the air mass.
pub struct Climate {
    pub seasons: Seasons,
    pub offsets: Offsets,
    pub sandbox_rain: i32,
    pub year: i32,
    /// 0-based month.
    pub month: i32,
    pub day: i32,
    /// Hours since the world began (continuous, includes the current day's progress).
    pub world_age_hours: f64,
    /// The rain front in progress, if any.
    pub period: Option<RainPeriod>,
    last_hour: f32,
}

impl Climate {
    pub fn new(seasons: Seasons, offsets: Offsets, year: i32, month: i32, day: i32, clock: &mut GameClock) -> Self {
        let mut c = Climate { seasons, offsets, sandbox_rain: 3, year, month, day, world_age_hours: 0.0, period: None, last_hour: clock.hour };
        c.start_day(clock);
        c
    }

    /// Dawn, dusk and today's fog for the current date.
    fn start_day(&mut self, clock: &mut GameClock) {
        let d = Daylight::default();
        clock.set_date(self.year, self.month, self.day);
        let (dawn, dusk) = d.dawn_dusk(self.year, self.month, self.day);
        let (mean, _) = self.seasons.day_temperature(self.year, self.month, self.day);
        let air = AirMass::at(self.offsets, self.world_age_hours.floor() + 12.0, 12.0, 0.5, self.sandbox_rain);
        clock.fog = roll_fog(self.year, self.month, self.day, self.offsets.c as i32, self.offsets.d as i32, dawn, mean + air.temperature_offset());
        let _ = dusk;
    }

    /// Call once per tick after `clock.tick()`: advances world age and rolls the calendar over at midnight.
    pub fn advance(&mut self, clock: &mut GameClock) {
        self.world_age_hours += 24.0 / (clock.day_seconds as f64 * crate::TICK_HZ as f64);
        if clock.hour < self.last_hour {
            self.day += 1;
            if self.day > days_in_month(self.year, self.month) { self.day = 1; self.month += 1; }
            if self.month > 11 { self.month = 0; self.year += 1; }
            self.start_day(clock);
        }
        self.last_hour = clock.hour;
    }

    /// Current air mass (wind and temperature offset) at the clock's time.
    pub fn air(&self, clock: &GameClock) -> AirMass {
        AirMass::at(self.offsets, self.world_age_hours, clock.hour, 0.5, self.sandbox_rain)
    }
}

/// Precipitation of a weather period at `hours` (`WeatherPeriod.update`): a noise value in 0..1 that must beat the
/// period's rain threshold, rescaled above it and multiplied by the period's current strength.
pub fn precipitation(hours: f64, front_strength: f32, current_strength: f32, rain_threshold: f32) -> f32 {
    let noise = 0.3 * front_strength + simplex(hours, 24000.0) as f32 * (1.0 - 0.3 * front_strength);
    let noise = (noise + 1.0) * 0.5;
    if noise > rain_threshold { (noise - rain_threshold) / (1.0 - rain_threshold) * current_strength } else { 0.0 }
}

/// A rain front. The original builds it from a chain of stages (`WeatherPeriod.createAndAddStage`) whose strength
/// rises and falls; this keeps the same precipitation formula but replaces the stage chain with one smooth
/// rise-and-fall envelope, so rain *amount* over a front is an approximation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RainPeriod {
    pub start_hours: f64,
    pub duration: f64,
    /// Peak strength 0..1 of the front.
    pub strength: f32,
    pub rain_threshold: f32,
}

impl RainPeriod {
    pub fn is_over(&self, hours: f64) -> bool { hours >= self.start_hours + self.duration }

    /// Rain intensity 0..1 at `hours` (0 outside the period).
    pub fn intensity(&self, hours: f64) -> f32 {
        if hours < self.start_hours || self.is_over(hours) { return 0.0; }
        let t = ((hours - self.start_hours) / self.duration) as f32;
        let current = self.strength * (std::f32::consts::PI * t).sin();
        precipitation(hours, self.strength, current, self.rain_threshold)
    }
}

impl Climate {
    /// Start a rain front now, lasting `duration` hours.
    pub fn start_rain(&mut self, duration: f64, strength: f32, rain_threshold: f32) {
        self.period = Some(RainPeriod { start_hours: self.world_age_hours, duration, strength, rain_threshold });
    }

    /// Current rain intensity (feeds the zombies' vision penalty).
    pub fn rain(&self) -> f32 {
        self.period.map_or(0.0, |p| p.intensity(self.world_age_hours))
    }
}
