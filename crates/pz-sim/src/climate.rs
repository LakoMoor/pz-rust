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

    /// Boundary days of a year's seasons (`setYearData`): the winter that starts before 22 Dec, the one that ends
    /// after it, and the summer around 22 Jun, each stretched by the noise-driven strength of that year.
    fn season_days(&self, y: i32) -> (i64, i64, i64, i64) {
        let winter_mod = if self.temp_min < 0 { 0.05 * -self.temp_min as f32 } else { 0.02 * -self.temp_min as f32 };
        let summer_mod = if self.temp_max < 0 { 0.05 * self.temp_max as f32 } else { 0.02 * self.temp_max as f32 };
        let s = |m: f32, n: f32| (40.0 + 40.0 * m + 20.0 * n).floor() as i64;
        let (dec, jun) = (days_from_civil(y, 11, 22), days_from_civil(y, 5, 22));
        let wsn = self.noise.layered_noise((64 + y) as f32, 64.0);
        let wen = self.noise.layered_noise(64.0, (64 + y) as f32);
        let ssn = self.noise.layered_noise((128 + y) as f32, 128.0);
        let sen = self.noise.layered_noise(128.0, (128 + y) as f32);
        (dec - s(winter_mod, wsn), dec + s(winter_mod, wen), jun - s(summer_mod, ssn), jun + s(summer_mod, sen))
    }

    /// `ErosionSeason.curSeason`: 1 spring, 2 summer, 4 autumn, 5 winter.
    pub fn season(&self, year: i32, month: i32, day: i32) -> u8 {
        let d = days_from_civil(year, month, day);
        let (_, prev_winter_end, _, _) = self.season_days(year - 1);
        let (winter_start, _, summer_start, summer_end) = self.season_days(year);
        if d < prev_winter_end { 5 } else if d < summer_start { 1 } else if d < summer_end { 2 } else if d < winter_start { 4 } else { 5 }
    }

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
    /// The air front currently tracked (`ClimateManager.currentFront`).
    pub front: AirFront,
    /// Fronts arrive by themselves when the air mass flips sign (`ClimateManager.updateValues`).
    pub natural_fronts: bool,
    last_hour: f32,
}

impl Climate {
    pub fn new(seasons: Seasons, offsets: Offsets, year: i32, month: i32, day: i32, clock: &mut GameClock) -> Self {
        let mut c = Climate { seasons, offsets, sandbox_rain: 3, year, month, day, world_age_hours: 0.0, period: None, front: AirFront::default(), natural_fronts: true, last_hour: clock.hour };
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
        if self.natural_fronts && (clock.hour.floor() != self.last_hour.floor() || clock.hour < self.last_hour) {
            self.hour_changed(clock);
        }
        self.last_hour = clock.hour;
    }

    /// `ClimateManager.updateValues` on an hour change: when the air mass changes sign, the front that built up
    /// during the previous air mass breaks as a weather period, and tracking starts over for the new sign.
    fn hour_changed(&mut self, clock: &GameClock) {
        let f = air_mass_frequency(self.sandbox_rain);
        let air_type = if simplex(self.offsets.a, self.world_age_hours / f) < 0.0 { -1 } else { 1 };
        if self.front.kind == air_type { return; }
        if self.world_age_hours > 0.0 && !self.period.is_some_and(|p| !p.is_over(self.world_age_hours)) {
            let midnight = self.world_age_hours - clock.hour as f64;
            for i in 0..4 {
                let age = midnight + 24.0 * (i as f64 - 3.0);
                self.front.add_day_sample(simplex(self.offsets.a, (age.floor() + 12.0) / f) as f32);
            }
            self.begin_period(clock);
        }
        self.front = AirFront::of_type(air_type);
    }

    /// `WeatherPeriod.createWeatherPattern`, reduced to what the rain model needs: the reseed from the date and
    /// offsets, the front's maximum duration (24-48 h x strength warm-to-cold, 12-24 h cold-to-warm) plus the
    /// opening and closing stages at their mean length, and the rain threshold `0.35 - 0.2 x strength`.
    /// The random draws are not in the original's exact order, so the length of a given front can differ.
    fn begin_period(&mut self, _clock: &GameClock) {
        let s = self.front.strength;
        if s <= 0.0 { return; }
        let seed = ((self.year - 1990).wrapping_mul(100000)) as i64
            + (self.month.wrapping_mul(self.day).wrapping_mul(1234)) as i64
            + ((self.year - 1990).wrapping_mul(self.month).wrapping_mul(10000)) as i64
            + (((self.offsets.b as i32).wrapping_sub(self.offsets.a as i32)).wrapping_mul(self.day)) as i64;
        let mut rng = JavaRandom::new(seed);
        let mut rand = |lo: f32, hi: f32| lo + rng.next_float() * (hi - lo);
        // Per-season chances and rain-time multiplier (`createWeatherPattern`).
        let mean_temp = self.seasons.day_temperature(self.year, self.month, self.day).0;
        let (storm_chance, tropical_chance, blizzard_chance, season_mult) = match self.seasons.season(self.year, self.month, self.day) {
            1 => (75.0, 10.0, 0.0, 1.25),
            2 => (60.0, 55.0, 0.0, 1.0),
            4 => (100.0, 25.0, 0.0, 1.35),
            _ => (10.0, 0.0, blizzard_chance(mean_temp), 1.0),
        };
        let time_mult = season_mult * rain_time_multiplier(self.sandbox_rain);
        let _cloud_colour_roll = rand(0.0, 100.0);
        let warm = self.front.kind == 1;
        let (mut kind, mut staged) = (PeriodKind::Rain, 0.0f32);
        if warm {
            let tropical = s > 0.75 && tropical_chance > 0.0 && rand(0.0, 100.0) < tropical_chance;
            if tropical {
                kind = PeriodKind::Tropical;
                staged += 8.0 + rand(0.0, 16.0 * s);
                for p in [60.0, 30.0] { if rand(0.0, 100.0) < p * s { staged += 5.0 + rand(0.0, 5.0 * s); } }
            } else if s > 0.75 && blizzard_chance > 0.0 && rand(0.0, 100.0) < blizzard_chance {
                kind = PeriodKind::Blizzard;
                staged += 24.0 + rand(0.0, 24.0 * s);
            } else if s > 0.5 && storm_chance > 0.0 && rand(0.0, 100.0) < storm_chance {
                kind = PeriodKind::Thunderstorm;
                staged += 5.0 + rand(0.0, 5.0 * s);
                for (p, span) in [(70.0, 4.0), (50.0, 4.0), (25.0, 3.0), (12.5, 2.0)] {
                    if rand(0.0, 100.0) < p * s { staged += 4.0 + rand(0.0, span * s); }
                }
            }
        }
        let (lo, hi) = if warm { (24.0, 48.0) } else { (12.0, 24.0) };
        let max_duration = rand(lo, hi) * s;
        let duration = (max_duration.max(staged) as f64 + 3.0 + 2.5 * s as f64) * time_mult as f64;
        self.period = Some(RainPeriod { start_hours: self.world_age_hours, duration, strength: s, rain_threshold: 0.35 - s * 0.2, kind });
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
    pub kind: PeriodKind,
}

/// What a front is made of: ordinary rain, a thunderstorm (stage 3), a tropical storm (stage 8) or a blizzard (stage 7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeriodKind { Rain, Thunderstorm, Tropical, Blizzard }

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
        self.period = Some(RainPeriod { start_hours: self.world_age_hours, duration, strength, rain_threshold, kind: PeriodKind::Rain });
    }

    /// Air temperature in degrees C (`ClimateValues`): the seasonal mean plus the air-mass term, warmed by up to
    /// 5 degrees through the lagged daylight and cooled by up to 5 through the lagged night.
    pub fn temperature(&self, clock: &GameClock) -> f32 {
        let (mean, _) = self.seasons.day_temperature(self.year, self.month, self.day);
        let base = mean + self.air(clock).temperature_offset();
        let mut dd = clock.dusk + 4.0;
        if dd >= 24.0 { dd -= 24.0; }
        let day_lagged = crate::time_lerp_hours(clock.hour, clock.dawn + 4.0, dd);
        let night_lagged = crate::time_lerp_hours(clock.hour, dd, clock.dawn + 4.0);
        base + 1.0 - (5.0 * (1.0 - day_lagged) + 5.0 * night_lagged)
    }

    /// Total precipitation of the front in progress, before it is split into rain and snow.
    pub fn precipitation_now(&self) -> f32 {
        self.period.map_or(0.0, |p| p.intensity(self.world_age_hours))
    }

    /// Whether what falls is snow: below freezing in winter (`WeatherPeriod.updateCurrentStage`; the
    /// Winter-is-Coming mode, which forces winter, is not ported).
    pub fn is_snow(&self, clock: &GameClock) -> bool {
        self.temperature(clock) < 0.0 && self.seasons.season(self.year, self.month, self.day) == 5
    }

    /// `ClimateManager.getRainIntensity`: precipitation that is not snow (feeds the zombies' vision penalty).
    pub fn rain_at(&self, clock: &GameClock) -> f32 {
        if self.is_snow(clock) { 0.0 } else { self.precipitation_now() }
    }

    /// `ClimateManager.getSnowIntensity`.
    pub fn snow_at(&self, clock: &GameClock) -> f32 {
        if self.is_snow(clock) { self.precipitation_now() } else { 0.0 }
    }

    /// Rain ignoring temperature; kept for callers without a clock.
    pub fn rain(&self) -> f32 { self.precipitation_now() }
}

/// `ClimateManager.AirFront`: tracks the strength of the front that builds up while one air mass (warm = 1,
/// cold = -1) dominates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AirFront {
    pub kind: i32,
    pub strength: f32,
    max_noise: f32,
    cache: [f32; 2],
}

impl Default for AirFront {
    fn default() -> Self { AirFront { kind: 0, strength: 0.0, max_noise: 0.0, cache: [-1.0; 2] } }
}

impl AirFront {
    pub fn of_type(kind: i32) -> Self { AirFront { kind, ..Default::default() } }

    /// `AirFront.addDaySample`: only samples on the front's own side count; the strength blends the strongest
    /// recent sample (75%) with the strongest ever (25%).
    pub fn add_day_sample(&mut self, noise: f32) {
        if (self.kind != 1 || noise > 0.0) && (self.kind != -1 || noise < 0.0) {
            let abs = noise.abs();
            if abs > self.max_noise { self.max_noise = abs; }
            let mut recent = 0.0f32;
            for i in (0..self.cache.len()).rev() {
                if self.cache[i] > recent { recent = self.cache[i]; }
                if i < self.cache.len() - 1 { self.cache[i + 1] = self.cache[i]; }
            }
            self.cache[0] = abs;
            if abs > recent { recent = abs; }
            self.strength = recent * 0.75 + self.max_noise * 0.25;
        } else {
            self.strength = 0.0;
        }
    }
}

/// Chance (percent) that a strong winter warm-to-cold front is a blizzard, from the day's mean temperature
/// (`createWeatherPattern`, season 5). Below 5.5 degrees it grows with the cold; the original's `< 0` branch is
/// unreachable because `< 2.5` matches first, and that is kept.
pub fn blizzard_chance(mean_temp: f32) -> f32 {
    if mean_temp >= 5.5 { return 0.0; }
    let mut c = ((5.5 - mean_temp) * 3.0).clamp(0.0, 85.0) + 25.0;
    if mean_temp < 2.5 { c += 55.0; }
    c.min(95.0)
}

/// `ClimateManager.getRainTimeMultiplierMod` for the sandbox rain setting.
pub fn rain_time_multiplier(sandbox_rain: i32) -> f32 {
    match sandbox_rain { 1 => 0.5, 2 => 0.75, 4 => 1.25, 5 => 1.5, _ => 1.0 }
}
