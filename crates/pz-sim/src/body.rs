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
}

impl Default for Part {
    fn default() -> Self {
        Part { health: 100.0, bleeding_time: 0.0, scratch_time: 0.0, cut_time: 0.0, bite_time: 0.0, deep_wound_time: 0.0, bandaged: false, bandage_life: 0.0, stitched: false }
    }
}

impl Part {
    fn reduce(&mut self, v: f32) { self.health = (self.health - v).clamp(0.0, 100.0); }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Body { pub parts: [Part; PARTS] }

impl Default for Body { fn default() -> Self { Body { parts: [Part::default(); PARTS] } } }

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
