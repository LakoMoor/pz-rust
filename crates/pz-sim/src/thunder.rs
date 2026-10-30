//! Thunderstorm clouds and their strike schedule, ported from `ThunderStorm.update`.
//!
//! Only the *events* are produced (where and when lightning, thunder and rumbles happen); the flash of light and
//! the sounds belong to the renderer and the audio engine, which are not ported. Timer units in the original are
//! animation-timer frames; here the suspend time is read in seconds of simulation time.

use crate::TICK_HZ;

/// One thunder event: `ThunderStorm.triggerThunderEvent(x, y, doStrike, doLightning, doRumble)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThunderEvent { pub x: i32, pub y: i32, pub strike: bool, pub lightning: bool, pub rumble: bool }

#[derive(Clone, Debug)]
pub struct ThunderCloud {
    pub start: [f32; 2],
    pub end: [f32; 2],
    pub radius: f32,
    /// 0..1: the cloud's strength (the front's strength).
    pub strength: f32,
    /// Share of events that are real strikes (the rest are rumbles only).
    pub thunder_ratio: f32,
    /// Total life in simulation ticks.
    pub life_ticks: u32,
    age: u32,
    suspend: u32,
}

impl ThunderCloud {
    pub fn new(start: [f32; 2], end: [f32; 2], radius: f32, strength: f32, thunder_ratio: f32, life_hours: f64, day_seconds: f32) -> Self {
        // `life_hours` of game time at `day_seconds` real seconds per game day.
        let life_ticks = (life_hours / 24.0 * day_seconds as f64 * TICK_HZ as f64) as u32;
        Self { start, end, radius, strength, thunder_ratio, life_ticks, age: 0, suspend: 0 }
    }

    pub fn is_running(&self) -> bool { self.age < self.life_ticks }

    /// Where the cloud is now (it drifts in a straight line from `start` to `end`).
    pub fn position(&self) -> [i32; 2] {
        let t = self.age as f32 / self.life_ticks.max(1) as f32;
        [(self.start[0] + (self.end[0] - self.start[0]) * t) as i32, (self.start[1] + (self.end[1] - self.start[1]) * t) as i32]
    }

    /// Advance one tick. When the suspend timer runs out, schedule the next one (3.5-3s .. 24-20s long for
    /// strength s, in seconds) and emit an event. `rand(lo, hi)` is a uniform draw.
    pub fn tick(&mut self, rand: &mut dyn FnMut(f32, f32) -> f32) -> Option<ThunderEvent> {
        if !self.is_running() { return None; }
        self.age += 1;
        if self.suspend > 0 { self.suspend -= 1; if self.suspend > 0 { return None; } }
        let s = self.strength;
        self.suspend = (rand(3.5 - 3.0 * s, 24.0 - 20.0 * s) * TICK_HZ).max(1.0) as u32;
        let r = rand(0.0, 1.0);
        let strike_radius = if r < 0.6 { (self.radius / 2.0) as i32 / 3 } else if r < 0.9 { (self.radius / 2.0) as i32 / 4 * 3 } else { (self.radius / 2.0) as i32 };
        let [cx, cy] = self.position();
        let mut around = |c: i32| rand((c - strike_radius) as f32, (c + strike_radius) as f32) as i32;
        let (x, y) = (around(cx), around(cy));
        Some(if rand(0.0, 1.0) < self.thunder_ratio {
            ThunderEvent { x, y, strike: true, lightning: true, rumble: rand(0.0, 1.0) > 0.4 }
        } else {
            ThunderEvent { x, y, strike: false, lightning: false, rumble: true }
        })
    }
}

/// Parameters `WeatherPeriod` gives a storm's cloud (`updateCurrentStage`, stage 3): radius `8000 x strength`,
/// thunder ratio `0.6 x strength`, and for a tropical storm radius 15000, ratio 0.8 and strength 1.
pub fn storm_cloud_params(front_strength: f32, tropical: bool) -> (f32, f32, f32) {
    if tropical { (15000.0, 0.8, 1.0) } else { (8000.0 * front_strength, 0.6 * front_strength, front_strength) }
}
