//! Body parts and wounds, ported from `BodyPartType`, `BodyPart.DamageUpdate` and the health helpers of
//! `BodyDamage`. Covers bleeding, scratches, cuts, bites, deep wounds and bandages; fractures, burns,
//! glass, bullets and wound infection are not ported yet.

pub const PARTS: usize = 17;

/// `BodyPartType` order: Hand_L, Hand_R, ForeArm_L, ForeArm_R, UpperArm_L, UpperArm_R, Torso_Upper, Torso_Lower,
/// Head, Neck, Groin, UpperLeg_L, UpperLeg_R, LowerLeg_L, LowerLeg_R, Foot_L, Foot_R.
pub const DAMAGE_MODIFIER: [f32; PARTS] = [0.1, 0.1, 0.2, 0.2, 0.3, 0.3, 0.35, 0.4, 0.6, 0.7, 0.4, 0.3, 0.3, 0.2, 0.2, 0.2, 0.2];
pub const BLEEDING_TIME_MODIFIER: [f32; PARTS] = [0.2, 0.2, 0.3, 0.3, 0.4, 0.4, 0.5, 0.9, 1.0, 1.5, 0.5, 0.4, 0.4, 0.3, 0.3, 0.2, 0.2];

/// `BodyPart.damageScaler`.
const SCALER: f32 = 0.0057142857;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Part {
    pub health: f32,
    pub bleeding_time: f32,
    pub scratch_time: f32,
    pub cut_time: f32,
    pub bite_time: f32,
    pub deep_wound_time: f32,
    pub bandaged: bool,
    pub bandage_life: f32,
    pub stitched: bool,
    /// Progress of a stitched wound (0..=50): grows while it heals, hurts until 35.
    pub stitch_time: f32,
    /// Pain left over from a procedure (stitching), fading by 0.005 per tick.
    pub additional_pain: f32,
    /// Bone fracture: time left to heal (30-80 when broken).
    pub fracture_time: f32,
    pub splint: bool,
    /// How well the splint is set (0 = none); speeds healing by that factor.
    pub splint_factor: f32,
    /// A festering wound (`isInfectedWound`) and how far it has progressed (0..=10).
    pub infected_wound: bool,
    pub wound_infection_level: f32,
    /// Burn: time left to heal.
    pub burn_time: f32,
}

impl Default for Part {
    fn default() -> Self {
        Part { health: 100.0, bleeding_time: 0.0, scratch_time: 0.0, cut_time: 0.0, bite_time: 0.0, deep_wound_time: 0.0, bandaged: false, bandage_life: 0.0, stitched: false, stitch_time: 0.0, additional_pain: 0.0, fracture_time: 0.0, splint: false, splint_factor: 0.0, infected_wound: false, wound_infection_level: 0.0, burn_time: 0.0 }
    }
}

impl Part {
    /// `BodyPart.getPain` for the wounds modelled here (default injury severity): each wound contributes in
    /// proportion to its remaining time, and a bandage eases the total by a third. Stitches, fractures, bullets,
    /// glass, burns, wound infection and the temporary extra pain are not ported.
    pub fn pain(&self) -> f32 {
        let mut r = 0.0;
        if self.scratch_time > 0.0 { r += self.scratch_time * 1.7; }
        if self.cut_time > 0.0 { r += self.cut_time * 2.5; }
        if self.bite_time > 0.0 { r += if self.bandaged { 30.0 } else { 50.0 }; }
        if self.deep_wound_time > 0.0 { r += self.deep_wound_time * 3.7; }
        if self.stitch_time > 0.0 && self.stitch_time < 35.0 {
            r += if self.bandaged { (35.0 - self.stitch_time) / 2.0 } else { 35.0 - self.stitch_time };
        }
        if self.burn_time > 0.0 { r += self.burn_time; }
        if self.fracture_time > 0.0 { r += if self.splint_factor > 0.0 { self.fracture_time / 2.0 } else { self.fracture_time }; }
        if self.bandaged { r /= 1.5; }
        if self.wound_infection_level > 0.0 { r += self.wound_infection_level; }
        r + self.additional_pain
    }

    fn reduce(&mut self, v: f32) { self.health = (self.health - v).clamp(0.0, 100.0); }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Body { pub parts: [Part; PARTS], rng: u32 }

impl Default for Body { fn default() -> Self { Body { parts: [Part::default(); PARTS], rng: 0x2545_F491 } } }

impl Body {
    /// `BodyDamage.calculateOverallHealth` (without pill damage).
    pub fn overall_health(&self) -> f32 {
        let damage: f32 = self.parts.iter().zip(DAMAGE_MODIFIER).map(|(p, m)| (100.0 - p.health) * m).sum();
        100.0 - damage.min(100.0)
    }

    /// `BodyDamage.ReduceGeneralHealth`: spread evenly over parts, scaled by each part's damage modifier.
    pub fn reduce_general_health(&mut self, v: f32) {
        if v <= 0.0 { return; }
        for (p, m) in self.parts.iter_mut().zip(DAMAGE_MODIFIER) { p.reduce(v / PARTS as f32 / m); }
    }

    pub fn num_bleeding(&self) -> usize { self.parts.iter().filter(|p| p.bleeding_time > 0.0).count() }

    /// Wound a part so it starts bleeding for `base_time` scaled by the part's bleeding-time modifier.
    pub fn start_bleeding(&mut self, part: usize, base_time: f32) {
        self.parts[part].bleeding_time = base_time * BLEEDING_TIME_MODIFIER[part];
    }

    /// `BodyPart.DamageUpdate` for every part. `mult` is the game-time multiplier (1.0 per tick at normal speed).
    pub fn update(&mut self, mult: f32) {
        let mut general = 0.0;
        for p in self.parts.iter_mut() {
            let s = SCALER * mult;
            let bandaged = p.bandaged;
            if p.deep_wound_time > 0.0 && !p.stitched { p.reduce(if bandaged { 1.5625 } else { 3.125 } * s); }
            if p.scratch_time > 0.0 && !bandaged { p.reduce(0.9375 * s); }
            if p.cut_time > 0.0 && !bandaged { p.reduce(1.875 * s); }
            if p.bite_time > 0.0 && !bandaged { p.reduce(2.1875 * s); }
            if p.fracture_time > 0.0 && !p.splint { p.reduce(3.125 * s); }
            if p.burn_time > 0.0 && !bandaged { p.reduce(3.75 * s); }
            if p.bleeding_time > 0.0 && !bandaged { general += 0.2857143 * s * (p.bleeding_time / 10.0); }
            if p.bite_time > 0.0 {
                if bandaged {
                    p.bite_time -= 1.0e-4 * mult;
                    p.bandage_life -= 1.0e-4 * mult;
                } else { p.bite_time -= 5.0e-6 * mult; }
            }
            if p.bleeding_time > 0.0 {
                if bandaged {
                    p.bleeding_time -= 2.0e-4 * mult;
                    p.bandage_life -= if p.deep_wound_time > 0.0 { 0.005 } else { 3.0e-4 } * mult;
                } else { p.bleeding_time -= 2.0e-5 * mult; }
                if p.bleeding_time < 0.0 { p.bleeding_time = 0.0; }
            }
            // A fresh wound may start to fester; a festering one worsens slowly and wears the bandage.
            if !p.infected_wound && !(p.deep_wound_time <= 0.0 && p.scratch_time <= 0.0 && p.cut_time <= 0.0 && p.stitch_time <= 0.0) {
                let mut chance: i32 = 40_000;
                if !bandaged { chance -= 10_000; } else if p.bandage_life == 0.0 { chance -= 35_000; }
                if p.scratch_time > 0.0 { chance -= 20_000; }
                if p.cut_time > 0.0 { chance -= 25_000; }
                if p.deep_wound_time > 0.0 { chance -= 30_000; }
                if p.burn_time > 0.0 { chance -= 23_000; }
                let chance = chance.max(5_000) as u32;
                // xorshift32 stands in for Rand.Next(chance) == 0 once per tick.
                self.rng ^= self.rng << 13; self.rng ^= self.rng >> 17; self.rng ^= self.rng << 5;
                if self.rng % chance == 0 { p.infected_wound = true; }
            } else if p.infected_wound {
                p.wound_infection_level += 1.0e-5 * mult;
            }
            if p.infected_wound && p.bandage_life > 0.0 { p.bandage_life -= 2.0e-4 * mult; }
            if p.wound_infection_level > 10.0 { p.wound_infection_level = 10.0; }
            // Healing: wound timers run down faster under a bandage, which wears as it works.
            if p.scratch_time > 0.0 {
                if bandaged { p.scratch_time -= 1.5e-4 * mult; p.bandage_life -= 8.0e-5 * mult; } else { p.scratch_time -= 1.0e-5 * mult; }
                if p.scratch_time < 0.0 { p.scratch_time = 0.0; p.bleeding_time = 0.0; }
            }
            if p.cut_time > 0.0 {
                if bandaged { p.cut_time -= 5.0e-5 * mult; p.bandage_life -= 1.0e-5 * mult; } else { p.cut_time -= 1.0e-6 * mult; }
                if p.cut_time < 0.0 { p.cut_time = 0.0; p.bleeding_time = 0.0; }
            }
            if p.deep_wound_time > 0.0 {
                if bandaged { p.deep_wound_time -= 2.0e-5 * mult; p.bandage_life -= 1.0e-4 * mult; } else { p.deep_wound_time -= 2.0e-6 * mult; }
                // An unbandaged deep wound never closes on its own.
                if !bandaged && p.deep_wound_time < 3.0 { p.deep_wound_time = 3.0; }
                if p.deep_wound_time < 0.0 { p.deep_wound_time = 0.0; }
            }
            if p.burn_time > 0.0 {
                if bandaged { p.burn_time -= 1.0e-4 * mult; p.bandage_life -= 1.0e-4 * mult; } else { p.burn_time -= 5.0e-6 * mult; }
                if p.burn_time < 0.0 { p.burn_time = 0.0; }
            }
            if p.fracture_time > 0.0 {
                p.fracture_time -= if p.splint_factor > 0.0 { 5.0e-5 * mult * p.splint_factor } else { 5.0e-6 * mult };
                if p.fracture_time < 0.0 { p.fracture_time = 0.0; }
            }
            if p.stitch_time > 0.0 && p.stitch_time < 50.0 {
                if bandaged { p.stitch_time += (4.0e-4 + 1.0e-4) * mult; p.bandage_life -= 1.0e-4 * mult; } else { p.stitch_time += 2.0e-4 * mult; }
                if p.stitch_time > 50.0 { p.stitch_time = 50.0; }
            }
            if p.additional_pain > 0.0 { p.additional_pain = (p.additional_pain - 0.005 * mult).max(0.0); }
            if p.bandage_life < 0.0 { p.bandage_life = 0.0; }
        }
        self.reduce_general_health(general);
    }
}

impl Body {
    /// `BodyPart.setScratched`: 7-15 time units, bleeding for 30-60% of that (default injury severity).
    /// `r1`, `r2` are uniform [0,1) samples standing in for the two `Rand.Next` calls.
    pub fn scratch(&mut self, part: usize, r1: f32, r2: f32) {
        let t = 7.0 + r1 * 8.0;
        let p = &mut self.parts[part];
        p.scratch_time = t;
        p.bandaged = false;
        p.stitched = false;
        p.bleeding_time = (t * (0.3 + r2 * 0.3)) * BLEEDING_TIME_MODIFIER[part];
    }

    /// `BodyPart.setCut`: 10-20 time units, bleeding for 70-100% of that.
    pub fn cut(&mut self, part: usize, r1: f32, r2: f32) {
        let t = 10.0 + r1 * 10.0;
        let p = &mut self.parts[part];
        p.cut_time = t;
        p.bandaged = false;
        p.stitched = false;
        p.bleeding_time = (t * (0.7 + r2 * 0.3)) * BLEEDING_TIME_MODIFIER[part];
    }

    /// `BodyPart.SetBitten`: bite lasts 50-80 time units and bleeds for 7.5-15.
    pub fn bite(&mut self, part: usize, r1: f32, r2: f32) {
        let p = &mut self.parts[part];
        p.bite_time = 50.0 + r1 * 30.0;
        p.bandaged = false;
        p.bleeding_time = (7.5 + r2 * 7.5) * BLEEDING_TIME_MODIFIER[part];
    }
}

impl Body {
    /// `BodyPart.setBandaged(true, life)`: wrap a part. Scratches, cuts and bites stop hurting it directly and
    /// the bleeding slows (see `update`); the bandage then wears out and ends up dirty at `life == 0`.
    pub fn apply_bandage(&mut self, part: usize, life: f32) {
        let p = &mut self.parts[part];
        p.bandaged = true;
        p.stitched = false;
        p.bandage_life = life;
    }

    /// `setBandaged(false, ..)`: take the bandage off.
    pub fn remove_bandage(&mut self, part: usize) {
        let p = &mut self.parts[part];
        p.bandaged = false;
        p.bandage_life = 0.0;
    }

    /// `BodyPart.isBandageDirty`: a worn-out bandage is still on but no longer helps with infection.
    pub fn bandage_is_dirty(&self, part: usize) -> bool { self.parts[part].bandaged && self.parts[part].bandage_life <= 0.0 }
}

impl Body {
    /// `ISStitch:complete` -> `BodyPart.setStitched(true)`: closes a deep wound and stops its bleeding. `stitch_time`
    /// is `((1 + doctor) / 2) * ZombRandFloat(2, 5)` and `end_pain` the procedure's pain (`basePain - doctor`).
    pub fn stitch(&mut self, part: usize, stitch_time: f32, end_pain: f32) {
        let p = &mut self.parts[part];
        p.bleeding_time = 0.0;
        p.deep_wound_time = 0.0;
        p.stitched = true;
        p.stitch_time = stitch_time;
        p.additional_pain += end_pain.max(0.0);
    }

    /// `setStitched(false)` on a stitched part (removing the stitches): early removal reopens the wound, later
    /// removal only leaves a scratch. `r1`, `r2` are uniform [0,1) samples for the two `Rand.Next` calls.
    pub fn remove_stitches(&mut self, part: usize, r1: f32, r2: f32, end_pain: f32) {
        let p = &mut self.parts[part];
        if p.stitched {
            let t = p.stitch_time;
            if t < 40.0 {
                p.deep_wound_time = 10.0 + r1 * (t - 10.0);
                p.bleeding_time = 10.0 + r2 * (t - 10.0);
            } else {
                p.scratch_time = 2.0 + r1 * (t - 42.0);
            }
            p.stitch_time = 0.0;
        }
        p.stitched = false;
        p.additional_pain += end_pain.max(0.0);
    }
}

impl Body {
    /// `BodyPart.generateFracture` (bone fractures enabled): breaks a part for `time` units.
    /// Falls use 50-80; random trauma 30-50.
    pub fn fracture(&mut self, part: usize, time: f32) { self.parts[part].fracture_time = time; }

    /// `BodyPart.setSplint(true, factor)`.
    pub fn apply_splint(&mut self, part: usize, factor: f32) {
        let p = &mut self.parts[part];
        p.splint = true;
        p.splint_factor = factor;
    }

    pub fn remove_splint(&mut self, part: usize) {
        let p = &mut self.parts[part];
        p.splint = false;
        p.splint_factor = 0.0;
    }
}

impl Body {
    /// `BodyDamage.getGeneralWoundInfectionLevel`: festering wounds summed, x10 x the sandbox factor, capped at 100.
    pub fn general_wound_infection_level(&self, factor: f32) -> f32 {
        if factor <= 0.0 { return 0.0; }
        let total: f32 = self.parts.iter().filter(|p| p.infected_wound).map(|p| p.wound_infection_level).sum();
        (total * 10.0 * factor).min(100.0)
    }
}

impl Body {
    /// A body whose random wound events follow a given stream (so independent characters differ).
    pub fn with_seed(seed: u32) -> Self { Body { rng: seed | 1, ..Body::default() } }
}

impl Body {
    /// Burn a part (`setBurnTime`, e.g. 30-50 from random trauma). The burn also bleeds for 30-60% of its
    /// time, as `generateBleeding` adds; `r` is a uniform [0,1) sample for that range.
    pub fn burn(&mut self, part: usize, time: f32, r: f32) {
        let p = &mut self.parts[part];
        p.burn_time = time;
        p.bleeding_time += time * (0.3 + r * 0.3) * BLEEDING_TIME_MODIFIER[part];
    }
}
