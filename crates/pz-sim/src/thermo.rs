//! Body heat model, ported from `Thermoregulator.update()` (and `ThermalNode`, `Metabolics`).
//!
//! Scope of this port: a character with **no clothing** (insulation and wind resistance are zero; only the body
//! wetness of each part is an input). Everything the original reads from other systems — air temperature, humidity,
//! hunger, thirst, fatigue, endurance, fitness, body weight, carried weight, activity — is passed in through
//! [`Inputs`]. The numbers are transcribed from the decompiled Java and have not been compared against a running
//! game; tests only check physical behaviour.

use crate::body::PARTS;

/// `Metabolics` values used by the update.
pub mod met {
    pub const DEFAULT: f32 = 1.5;
    pub const DEFAULT_EXERCISE: f32 = 3.0;
    pub const MAX: f32 = 10.3;
    pub const WALKING_2KMH: f32 = 1.9;
    pub const WALKING_5KMH: f32 = 3.1;
    pub const RUNNING_10KMH: f32 = 6.9;
    pub const RUNNING_15KMH: f32 = 9.5;
}

/// `BodyPartType.skinSurface` and `distToCore`, in `BodyPartType` order (see `body::PARTS`).
pub const SKIN_SURFACE: [f32; PARTS] = [0.01, 0.01, 0.035, 0.035, 0.045, 0.045, 0.18, 0.12, 0.08, 0.02, 0.06, 0.09, 0.09, 0.07, 0.07, 0.02, 0.02];
pub const DIST_TO_CORE: [f32; PARTS] = [0.8, 0.8, 0.6, 0.6, 0.3, 0.3, 0.0, 0.05, 0.05, 0.02, 0.3, 0.45, 0.45, 0.75, 0.75, 1.0, 1.0];
const CORE: usize = 6; // Torso_Upper

const BODY_HEAT: f32 = 2.5e-4;
const METABOLIC_INC: f32 = 1.0e-3;
const METABOLIC_DEC: f32 = 4.0e-4;
const SKIN_CELCIUS: f32 = 0.0025;

#[derive(Debug, Clone, Copy)]
pub struct Inputs {
    pub air_temperature: f32,
    /// Air temperature with wind chill (never above `air_temperature`).
    pub air_and_wind_temperature: f32,
    pub humidity: f32,
    pub sickness: f32,
    pub hunger: f32,
    pub thirst: f32,
    pub fatigue: f32,
    pub endurance: f32,
    pub fitness: f32,
    pub body_weight_kg: f32,
    /// Carried weight / maximum carry weight.
    pub carried_ratio: f32,
    /// The metabolic rate the current activity demands (e.g. `met::WALKING_5KMH`), or `met::DEFAULT` when idle.
    pub activity: f32,
    /// Moodle levels 0-4 that gate thermal damage.
    pub hypothermia_level: u8,
    pub hyperthermia_level: u8,
    /// Game-world seconds elapsed this tick.
    pub world_seconds: f32,
    /// `BodyPart.wetness` per part, 0..=100.
    pub wetness: [f32; PARTS],
}

impl Default for Inputs {
    fn default() -> Self {
        Inputs {
            air_temperature: 27.0,
            air_and_wind_temperature: 27.0,
            humidity: 0.5,
            sickness: 0.0,
            hunger: 0.0,
            thirst: 0.0,
            fatigue: 0.0,
            endurance: 1.0,
            fitness: 0.0,
            body_weight_kg: 75.0,
            carried_ratio: 0.0,
            activity: met::DEFAULT,
            hypothermia_level: 0,
            hyperthermia_level: 0,
            world_seconds: 1.0,
            wetness: [0.0; PARTS],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Node {
    pub celcius: f32,
    pub skin_celcius: f32,
    pub heat_delta: f32,
    pub primary_delta: f32,
    pub secondary_delta: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Thermoregulator {
    pub nodes: [Node; PARTS],
    pub set_point: f32,
    pub metabolic_rate: f32,
    metabolic_target: f32,
    pub metabolic_rate_real: f32,
    pub body_heat_delta: f32,
    pub core_heat_delta: f32,
    /// The `TEMPERATURE` stat: the core temperature as last written back.
    pub temperature: f32,
    pub energy_multiplier: f32,
    pub fluids_multiplier: f32,
    pub fatigue_multiplier: f32,
    pub prim_total: f32,
    pub sec_total: f32,
    pub thermal_damage: f32,
    damage_counter: f32,
    thermal_duration: f32,
    rate_counter: f32,
    core_cache: f32,
    pub core_rate_of_change: f32,
    pub chevron_up: bool,
}

fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

impl Default for Thermoregulator {
    fn default() -> Self {
        // `reset()`: core 37, every other node 35, skin 33.
        let mut nodes = [Node { celcius: 35.0, skin_celcius: 33.0, heat_delta: 0.0, primary_delta: 0.0, secondary_delta: 0.0 }; PARTS];
        nodes[CORE].celcius = 37.0;
        Thermoregulator {
            nodes,
            set_point: 37.0,
            metabolic_rate: met::DEFAULT,
            metabolic_target: met::DEFAULT,
            metabolic_rate_real: met::DEFAULT,
            body_heat_delta: 0.0,
            core_heat_delta: 0.0,
            temperature: 37.0,
            energy_multiplier: 1.0,
            fluids_multiplier: 1.0,
            fatigue_multiplier: 1.0,
            prim_total: 0.0,
            sec_total: 0.0,
            thermal_damage: 0.0,
            damage_counter: 0.0,
            thermal_duration: 0.0,
            rate_counter: 0.0,
            core_cache: 37.0,
            core_rate_of_change: 0.0,
            chevron_up: true,
        }
    }
}

impl Thermoregulator {
    pub fn core_celcius(&self) -> f32 {
        self.nodes[CORE].celcius
    }

    /// `getEnergy`: how fed and rested the body is, 0..1.
    pub fn energy(hunger: f32, fatigue: f32) -> f32 {
        let h = 1.0 - (0.4 * hunger + 0.6 * hunger * hunger);
        let f = 1.0 - (0.4 * fatigue + 0.6 * fatigue * fatigue);
        0.6 * h + 0.4 * f
    }

    /// `getBodyFluids`.
    pub fn body_fluids(thirst: f32) -> f32 {
        1.0 - thirst
    }

    fn raise_target(&mut self, t: f32) {
        if t >= 0.0 && t >= self.metabolic_target {
            self.metabolic_target = t.min(met::MAX);
        }
    }

    /// `getCatchAColdDelta`: how strongly chilled skin (below 33 degrees) and wet skin push toward a cold, weighting
    /// the neck, upper torso and head most. `wetness` is the per-part body wetness (0..=100); clothing is not modelled.
    pub fn catch_a_cold_delta(&self, wetness: &[f32; PARTS], hypothermia_level: u8) -> f32 {
        let mut delta = 0.0;
        for k in 0..PARTS {
            let skin_c = self.nodes[k].skin_celcius;
            let mut skin = 0.0;
            if skin_c < 33.0 {
                skin = 1.0 - (skin_c - 20.0) / 13.0;
                skin *= skin;
            }
            let mut add = 0.25 * skin * SKIN_SURFACE[k];
            let wet = clamp01(wetness[k] * 0.01);
            if wet > 0.0 { add *= 1.0 + wet; }
            add *= match k { 9 => 8.0, 6 => 16.0, 8 => 4.0, _ => 1.0 };
            delta += add;
        }
        if hypothermia_level > 1 { delta *= hypothermia_level as f32; }
        delta
    }

    /// One tick (game-time multiplier 1).
    pub fn update(&mut self, i: &Inputs) {
        self.set_point = 37.0;
        if i.sickness > 0.0 {
            self.set_point += i.sickness * 2.0;
        }
        let energy = Self::energy(i.hunger, i.fatigue);
        let fluids = Self::body_fluids(i.thirst);

        // updateCoreRateOfChange
        self.rate_counter += 1.0;
        if self.rate_counter > 100.0 {
            self.rate_counter = 0.0;
            let d = self.nodes[CORE].celcius - self.core_cache;
            self.chevron_up = d >= 0.0;
            self.core_rate_of_change = d.abs();
            self.core_cache = self.nodes[CORE].celcius;
        }

        // updateMetabolicRate
        self.raise_target(met::DEFAULT);
        self.raise_target(i.activity);
        let exercise = clamp01(1.0 - i.endurance) * met::DEFAULT_EXERCISE;
        self.raise_target(exercise * energy);
        let w = clamp01(i.carried_ratio);
        self.metabolic_target *= 1.0 + w * w * 0.35;
        if (self.metabolic_rate - self.metabolic_target).abs() > 1.0e-4 {
            let rate = self.metabolic_target - self.metabolic_rate;
            self.metabolic_rate += rate * if self.metabolic_target > self.metabolic_rate { METABOLIC_INC } else { METABOLIC_DEC };
        }
        self.metabolic_rate_real = self.metabolic_rate * (0.2 + 0.8 * energy);
        self.metabolic_target = -1.0;

        // updateNodesHeatDelta (no clothing: insulation and wind resistance are zero)
        let weight_delta = (clamp01((i.body_weight_kg / 75.0 - 0.5) * 0.666) - 0.5) * 2.0;
        let fitness = i.fitness;
        let mut prim_heat_mod = 1.0;
        if i.air_and_wind_temperature > self.set_point - 2.0 {
            if i.air_temperature < self.set_point + 2.0 {
                prim_heat_mod = 1.0 - (i.air_temperature - (self.set_point - 2.0)) / 4.0;
            } else {
                prim_heat_mod = 0.0;
            }
        }
        let mut humidity_mod = 1.0;
        if i.humidity > 0.5 {
            humidity_mod -= (i.humidity - 0.5) * 2.0;
        }
        let mut total = 0.0;
        for k in 0..PARTS {
            let body_wetness = clamp01(i.wetness[k] * 0.01);
            let n = &mut self.nodes[k];
            let mut external = i.air_temperature;
            if i.air_and_wind_temperature < i.air_temperature {
                // Wind resistance is 0 without clothing, so the full chill applies.
                external -= i.air_temperature - i.air_and_wind_temperature;
            }
            let mut d = external - n.skin_celcius;
            if d <= 0.0 {
                d *= 1.0 + 0.75 * body_wetness;
            } else {
                d /= 1.0 + 3.0 * body_wetness;
            }
            d *= 0.3;
            n.heat_delta = d * SKIN_SURFACE[k];
            if n.primary_delta > 0.0 {
                let fl = 0.2 + 0.8 * fluids;
                let mut c = met::DEFAULT * n.primary_delta * SKIN_SURFACE[k];
                c *= fl * (0.1 + 0.9 * prim_heat_mod);
                c *= humidity_mod;
                c *= 1.0 - 0.2 * weight_delta;
                c *= 1.0 + 0.2 * fitness;
                n.heat_delta -= c;
            } else {
                let en = 0.2 + 0.8 * energy;
                let mut c = met::DEFAULT * n.primary_delta.abs() * SKIN_SURFACE[k];
                c *= en;
                c *= 1.0 + 0.2 * weight_delta;
                c *= 1.0 + 0.2 * fitness;
                n.heat_delta += c;
            }
            if n.secondary_delta > 0.0 {
                let fl = 0.1 + 0.9 * fluids;
                let mut c = met::MAX * 0.75 * n.secondary_delta * SKIN_SURFACE[k];
                c *= fl;
                c *= 0.85 + 0.15 * humidity_mod;
                c *= 1.0 - 0.2 * weight_delta;
                c *= 1.0 + 0.2 * fitness;
                n.heat_delta -= c;
            } else {
                let en = 0.1 + 0.9 * energy;
                let mut c = met::DEFAULT * n.secondary_delta.abs() * SKIN_SURFACE[k];
                c *= en;
                c *= 1.0 + 0.2 * weight_delta;
                c *= 1.0 + 0.2 * fitness;
                n.heat_delta += c;
            }
            total += n.heat_delta;
        }
        let total_heat = total + self.metabolic_rate_real;

        // updateHeatDeltas
        self.core_heat_delta = total_heat * BODY_HEAT;
        let core = self.nodes[CORE].celcius;
        if self.core_heat_delta < 0.0 {
            if core > self.set_point {
                self.core_heat_delta *= 1.0 + (core - self.set_point) / 2.0;
            }
        } else if core < self.set_point {
            self.core_heat_delta *= 1.0 + (self.set_point - core) / 4.0;
        }
        self.nodes[CORE].celcius = (self.nodes[CORE].celcius + self.core_heat_delta).clamp(20.0, 42.0);
        if (self.temperature - self.nodes[CORE].celcius).abs() > 0.001 {
            let c = self.nodes[CORE].celcius;
            self.nodes[CORE].celcius = c + (self.temperature - c) * 0.5;
        }
        self.temperature = self.nodes[CORE].celcius;
        self.body_heat_delta = self.nodes[CORE].celcius - self.set_point;
        if self.body_heat_delta < 0.0 {
            let bhd = self.body_heat_delta.abs();
            if bhd <= 1.0 {
                self.body_heat_delta *= 0.8;
            } else {
                self.body_heat_delta = -0.8 + -0.2 * ((bhd.clamp(1.0, 11.0) - 1.0) / 10.0);
            }
        }
        self.body_heat_delta = self.body_heat_delta.clamp(-1.0, 1.0);

        // updateNodes
        let (mut prim, mut sec) = (0.0, 0.0);
        let core_c = self.nodes[CORE].celcius;
        for k in 0..PARTS {
            let dist = DIST_TO_CORE[k];
            let insulation = 1.0; // 1 + node insulation (0 without clothing)
            let n = &mut self.nodes[k];
            n.primary_delta = if self.body_heat_delta < 0.0 {
                self.body_heat_delta * (1.0 + dist)
            } else {
                self.body_heat_delta * (1.0 + (1.0 - dist))
            };
            n.primary_delta = n.primary_delta.clamp(-1.0, 1.0);
            n.secondary_delta = n.primary_delta * n.primary_delta.abs() * n.primary_delta.abs();
            prim += n.primary_delta * SKIN_SURFACE[k];
            sec += n.secondary_delta * SKIN_SURFACE[k];
            let skin_max = core_c;
            let mut skin_min = core_c - 20.0;
            if skin_min < i.air_temperature {
                if i.air_temperature < 33.0 {
                    skin_min = i.air_temperature;
                } else {
                    let dist_core = 0.4 + 0.6 * (1.0 - dist);
                    let skinmod = (i.air_temperature - 33.0) / 6.0;
                    skin_min = (33.0 + 4.0 * skinmod * dist_core).clamp(33.0, i.air_temperature);
                    if skin_min > skin_max {
                        skin_min = skin_max - 0.25;
                    }
                }
            }
            let mut skin_target = core_c - 4.0;
            if n.primary_delta < 0.0 {
                let dist_core = 0.4 + 0.6 * dist;
                let target = skin_target - 12.0 * dist_core / insulation;
                let t = n.primary_delta.abs().clamp(0.0, 1.0);
                skin_target += (target - skin_target) * t;
            } else {
                let dist_core = 0.4 + 0.6 * (1.0 - dist);
                let mut add = 4.0 * dist_core;
                add *= (insulation * 0.5 * dist_core).max(1.0);
                let target = (skin_target + add).min(skin_max);
                skin_target += (target - skin_target) * n.primary_delta.clamp(0.0, 1.0);
            }
            skin_target = skin_target.clamp(skin_min.min(skin_max), skin_max.max(skin_min));
            let skin_delta = skin_target - n.skin_celcius;
            let mut mult = SKIN_CELCIUS;
            if (skin_delta < 0.0 && n.skin_celcius > 33.0) || (skin_delta > 0.0 && n.skin_celcius < 33.0) {
                mult *= 3.0;
            }
            if mult > 1.0 {
                mult = 1.0;
            }
            n.skin_celcius += skin_delta * mult;
            if k != CORE {
                n.celcius = if n.skin_celcius >= core_c { core_c } else { n.skin_celcius + (core_c - n.skin_celcius) * 0.5 };
            }
        }
        self.prim_total = prim;
        self.sec_total = sec;

        // updateBodyMultipliers
        self.energy_multiplier = 1.0;
        self.fluids_multiplier = 1.0;
        self.fatigue_multiplier = 1.0;
        let m = prim.abs() * prim.abs();
        if prim < 0.0 {
            self.energy_multiplier += 0.05 * m;
            self.fatigue_multiplier += 0.25 * m;
        } else if prim > 0.0 {
            self.fluids_multiplier += 0.25 * m;
            self.fatigue_multiplier += 0.25 * m;
        }
        let m = sec.abs() * sec.abs();
        if sec < 0.0 {
            self.energy_multiplier += 0.1 * m;
            self.fatigue_multiplier += 0.75 * m;
        } else if sec > 0.0 {
            self.fluids_multiplier += 3.75 * m;
            self.fatigue_multiplier += 1.75 * m;
        }

        // updateThermalDamage
        self.damage_counter += i.world_seconds;
        if self.damage_counter > 1.0 {
            let air = i.air_and_wind_temperature;
            let core = self.nodes[CORE].celcius;
            if i.hypothermia_level == 4 && air < 0.0 && core - self.core_cache <= 0.0 {
                let frost = 1.0 - (core - 20.0) / 5.0;
                let seconds = 120.0 + 480.0 * frost;
                self.thermal_damage += 1.0 / seconds * clamp01(air.abs() / 10.0);
                self.thermal_damage *= 1.2;
                self.thermal_duration += self.damage_counter;
                self.thermal_damage = self.thermal_damage.min(self.thermal_duration / 7200.0);
            } else if i.hyperthermia_level == 4 && air > 37.0 && core - self.core_cache >= 0.0 {
                let burn = core - 41.0;
                let seconds = 120.0 + 480.0 * burn;
                self.thermal_damage += 1.0 / seconds * clamp01((air - 37.0) / 8.0);
                self.thermal_damage = self.thermal_damage.min(0.3);
                self.thermal_duration = 0.0;
            } else {
                self.thermal_damage -= 0.011111111;
                self.thermal_duration = 0.0;
            }
            self.damage_counter = 0.0;
            self.thermal_damage = clamp01(self.thermal_damage);
        }
    }
}
