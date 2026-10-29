//! Fixed-timestep game simulation (deterministic; independent of rendering).
use pz_world::{flag, World};

pub mod firearm;

pub const TICK_HZ: f32 = 30.0;
pub const TICK_DT: f32 = 1.0 / TICK_HZ;
const WALK: f32 = 3.0; // tiles / second
const RUN: f32 = 5.5;

#[derive(Clone, Copy, Default)]
pub struct Input {
    /// Desired move direction in world tile axes (not normalised).
    pub dir: [f32; 2],
    pub run: bool,
}

#[derive(Clone)]
pub struct Player {
    /// Continuous tile coordinates; square (x, y) is centred on integer coordinates.
    pub pos: [f32; 2],
    pub z: usize,
    /// Continuous height in floors (z plus progress along a staircase).
    pub height: f32,
    /// 0..=100; zombies bite it down.
    pub health: f32,
    /// Set by a zombie bite (the game's infection mechanic; no cure in vanilla).
    pub infected: bool,
    /// Unit vector of the last movement direction (swings go this way).
    pub facing: [f32; 2],
    swing_cd: u32,
    step_clock: u32,
    rng: u32,
    /// Sounds made this tick (footsteps); drained by the caller.
    pub sounds: Vec<WorldSound>,
}

pub fn square_of(p: [f32; 2]) -> (i32, i32) { ((p[0] + 0.5).floor() as i32, (p[1] + 0.5).floor() as i32) }

impl Player {
    pub fn new(x: f32, y: f32) -> Self { Self { pos: [x, y], z: 0, height: 0.0, health: 100.0, infected: false, facing: [1.0, 0.0], swing_cd: 0, step_clock: 0, rng: 0x1234_5679 ^ (x.to_bits() ^ y.to_bits().rotate_left(7)) | 1, sounds: Vec::new() } }

    /// Advance one tick. Axes are resolved separately so the player slides along walls.
    pub fn tick(&mut self, input: Input, world: &mut World) {
        self.swing_cd = self.swing_cd.saturating_sub(1);
        let moved_from = self.pos;
        let len = (input.dir[0] * input.dir[0] + input.dir[1] * input.dir[1]).sqrt();
        if len > 1e-4 { self.facing = [input.dir[0] / len, input.dir[1] / len]; }
        let before = square_of(self.pos);
        self.move_xy(input, world);
        self.update_level(before, world);
        self.footsteps(input, moved_from, world);
    }

    /// Swing `weapon` in the facing direction; hits every zombie in range and in the front cone.
    /// Returns the number of zombies killed (dead zombies are removed).
    pub fn attack(&mut self, zombies: &mut Vec<Zombie>, weapon: &Weapon) -> usize {
        if self.swing_cd > 0 { return 0; }
        self.swing_cd = weapon.cooldown_ticks;
        // Every swing makes noise, hit or miss.
        if weapon.sound_radius > 0.0 {
            self.sounds.push(WorldSound { pos: self.pos, z: self.z, radius: weapon.sound_radius, volume: weapon.sound_volume, zombie_ignore_dist: 0.0, source_is_zombie: false });
        }
        let before = zombies.len();
        let mut struck = false;
        for zb in zombies.iter_mut().filter(|zb| zb.z == self.z) {
            let d = [zb.pos[0] - self.pos[0], zb.pos[1] - self.pos[1]];
            let dist = (d[0] * d[0] + d[1] * d[1]).sqrt();
            let front = dist < 1e-4 || (d[0] * self.facing[0] + d[1] * self.facing[1]) / dist > 0.5;
            if dist <= weapon.range && front {
                self.rng ^= self.rng << 13; self.rng ^= self.rng >> 17; self.rng ^= self.rng << 5;
                let crit = (self.rng % 100) < melee_crit_chance(weapon.crit_chance, false, 2.0, 0) as u32;
                zb.hit(weapon, weapon.damage, crit, self.pos);
                struck = true;
            }
        }
        // Character.Hit: addWorldSoundUnlessInvisible(5, 1, false)
        if struck {
            self.sounds.push(WorldSound { pos: self.pos, z: self.z, radius: 5.0, volume: 1.0, zombie_ignore_dist: 0.0, source_is_zombie: false });
        }
        zombies.retain(|zb| zb.health > 0.0);
        before - zombies.len()
    }

    /// IsoGameCharacter.DoFootstepSound: volume by gait (walk 0.5, run 1.3), x1.4, radius ceil(vol*10),
    /// halved indoors, emitted with probability 1/2. Step cadence (every 15 / 10 ticks) is approximate.
    fn footsteps(&mut self, input: Input, from: [f32; 2], world: &mut World) {
        if self.pos == from { self.step_clock = 0; return; }
        self.step_clock += 1;
        let period = if input.run { 10 } else { 15 };
        if self.step_clock % period != 0 { return; }
        self.rng ^= self.rng << 13; self.rng ^= self.rng >> 17; self.rng ^= self.rng << 5;
        if self.rng % 2 != 0 { return; }
        let (x, y) = square_of(self.pos);
        let indoors = world.ensure(x, y) && world.column(x, y, self.z).is_some_and(|c| c.room != -1);
        let volume = (if input.run { 1.3 } else { 0.5 }) * 1.4;
        let mut rad = (volume * 10.0f32).ceil();
        if indoors { rad = (rad * 0.5).floor(); }
        self.sounds.push(WorldSound { pos: self.pos, z: self.z, radius: rad, volume: rad, zombie_ignore_dist: 0.0, source_is_zombie: false });
    }

    /// Open/close the nearest door on the player's square or an orthogonal neighbour.
    pub fn interact(&self, world: &mut World) -> bool {
        let (x, y) = square_of(self.pos);
        [(0, 0), (1, 0), (0, 1), (-1, 0), (0, -1)].iter().any(|&(dx, dy)| world.toggle_door(x + dx, y + dy, self.z).is_some())
    }

    fn move_xy(&mut self, input: Input, world: &mut World) {
        let speed = if input.run { RUN } else { WALK };
        slide(&mut self.pos, self.z, input.dir, speed, world, true);
    }

    /// Stairs: leaving a staircase's top step climbs a floor; walking into the hole above one descends.
    fn update_level(&mut self, before: (i32, i32), world: &mut World) {
        let now = square_of(self.pos);
        let z = self.z;
        if now != before {
            let f = world.flags(before.0, before.1, z);
            let up_n = f & flag::STAIR_TN != 0 && now == (before.0, before.1 - 1);
            let up_w = f & flag::STAIR_TW != 0 && now == (before.0 - 1, before.1);
            if up_n || up_w { self.z += 1; }
        }
        let z = self.z;
        if z > 0 && !world.has_floor(now.0, now.1, z)
            && world.flags(now.0, now.1, z - 1) & (flag::STAIR_TN | flag::STAIR_TW) != 0 {
            self.z -= 1;
        }
        self.height = self.z as f32 + self.stair_progress(world);
    }

    fn stair_progress(&self, world: &mut World) -> f32 {
        let (sx, sy) = square_of(self.pos);
        let f = world.flags(sx, sy, self.z);
        // Pieces climb toward smaller coordinates; each covers a third of a floor.
        let piece = |b: u16, m: u16| if f & b != 0 { Some(0.0) } else if f & m != 0 { Some(1.0) } else { None };
        let n = piece(flag::STAIR_BN, flag::STAIR_MN).or(if f & flag::STAIR_TN != 0 { Some(2.0) } else { None });
        if let Some(i) = n { return (i + (sy as f32 + 0.5 - self.pos[1]).clamp(0.0, 1.0)) / 3.0; }
        let w = piece(flag::STAIR_BW, flag::STAIR_MW).or(if f & flag::STAIR_TW != 0 { Some(2.0) } else { None });
        if let Some(i) = w { return (i + (sx as f32 + 0.5 - self.pos[0]).clamp(0.0, 1.0)) / 3.0; }
        0.0
    }
}

/// Move `pos` along `dir` for one tick, resolving axes separately so walkers slide along walls.
/// `stairs` allows leaving a staircase's top step (climbing is handled by the caller).
fn slide(pos: &mut [f32; 2], z: usize, dir: [f32; 2], speed: f32, world: &mut World, stairs: bool) {
    let len = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
    if len < 1e-4 { return; }
    let step = speed * TICK_DT;
    for axis in 0..2 {
        let d = dir[axis] / len * step;
        if d == 0.0 { continue; }
        let mut np = *pos;
        np[axis] += d;
        let (from, to) = (square_of(*pos), square_of(np));
        let exits_stairs = stairs && from != to && {
            // Leaving a staircase's top step through its far edge climbs a floor, walls below notwithstanding.
            let f = world.flags(from.0, from.1, z);
            (f & flag::STAIR_TN != 0 && to == (from.0, from.1 - 1)) || (f & flag::STAIR_TW != 0 && to == (from.0 - 1, from.1))
        };
        if from == to || exits_stairs || world.can_step(from, to, z) { *pos = np; }
    }
}

/// A hit-point scale comparable to weapon damages in the item scripts (e.g. 0.4..0.8 per swing for blades).
const ZOMBIE_HEALTH: f32 = 1.95; // IsoZombie: 1.8 + Rand(0, 0.3); midpoint keeps the sim deterministic
const EAT_RANGE: f32 = 1.0;
/// ZombieEatBodyState.enter: stateEventDelayTimer = Rand.Next(1800, 3600); read here as ticks (the timer's unit is not verified).
const EAT_MIN_TICKS: u32 = 1800;
const CLIMB_TICKS: u32 = 45; // 1.5 s over a window or fence (animation-driven in the original)
const ZOMBIE_SPEED: f32 = 1.1; // tiles / second (shamble)
const ZOMBIE_SIGHT: f32 = 20.0; // IsoZombie.updateVisionRadius: 20 in clear daylight (clamped 10..20)

/// Conditions that shrink how far a zombie sees (IsoZombie.updateVisionRadius).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weather {
    /// 0..1
    pub rain: f32,
    /// 0..1
    pub fog: f32,
    /// Light level at the target's square, 0 (pitch dark) .. 1 (full light).
    pub light: f32,
}

impl Default for Weather {
    fn default() -> Self { Weather { rain: 0.0, fog: 0.0, light: 1.0 } }
}

impl Weather {
    /// Vision radius in squares: 20 minus the worst of the darkness and rain+fog penalties, clamped to 10..20.
    pub fn vision_radius(&self) -> f32 { self.vision_radius_for(Sight::Normal, 1.0, false) }

    /// `IsoZombie.getVisionRadiusAdjusted` + clamp: the sandbox sight setting scales the radius, worn items divide it
    /// (`worn_modifier` >= 1 for masks and the like), eating a body halves it; the clamp to 10..20 comes last.
    pub fn vision_radius_for(&self, sight: Sight, worn_modifier: f32, eating: bool) -> f32 {
        let dark = (1.0 - self.light) * 5.0;
        let wet = self.rain * 2.5 + self.fog * 7.0;
        let mut r = ZOMBIE_SIGHT - dark.max(wet);
        r *= match sight { Sight::Eagle => 1.75, Sight::Normal => 1.0, Sight::Poor => 0.35 };
        r /= worn_modifier;
        if eating { r *= 0.5; }
        r.clamp(10.0, 20.0)
    }
}

/// Sandbox "zombie sight" setting (`lore.sight`): 1 = eagle-eyed, 2 = normal, 3 = poor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sight { Eagle, Normal, Poor }
const ATTACK_RANGE: f32 = 0.9;
const ATTACK_COOLDOWN: u32 = 45; // ticks (1.5 s)

/// A zombie that shambles straight toward a visible player on the same floor.
/// Melee weapon stats (damage and range follow the item scripts' units).
#[derive(Clone, Copy, Debug)]
pub struct Weapon { pub damage: f32, pub range: f32, pub cooldown_ticks: u32, pub crit_chance: f32, pub crit_mult: f32, pub push_back_mod: f32, pub small_blade: bool,
    /// Script `SoundRadius` / `SoundVolume`: the noise of every melee swing (IsoLivingCharacter.AttackWorldSound); 0 = silent.
    pub sound_radius: f32, pub sound_volume: f32 }

impl Weapon {
    pub const FISTS: Weapon = Weapon { damage: 0.5, range: 1.0, cooldown_ticks: 30, crit_chance: 0.0, crit_mult: 1.0, push_back_mod: 1.0, small_blade: false, sound_radius: 0.0, sound_volume: 0.0 };
}

#[derive(Clone)]
pub struct Zombie { pub pos: [f32; 2], pub z: usize, pub health: f32, cooldown: u32, pub hit_time: u32, pub stagger: u32, pub on_floor: bool, pub knocked_down: bool, path: Vec<(i32, i32)>, stuck: u32, pub reanimate_timer: f32, pub facing: [f32; 2], idle_timer: f32, wander: Option<[f32; 2]>, rng: u32,
    /// Per-zombie sight setting (sandbox `lore.sight`).
    pub sight: Sight,
    /// Crawler: `IsoZombie.doCrawlerSpeed` speedMod (0.3 + 0..0.15) instead of a shambler's (about 0.625).
    crawl_mod: Option<f32>,
    /// Climbing a window or fence: ticks left and the square being climbed to.
    climb: Option<(u32, (i32, i32))>,
    /// ZombieEatBodyState: ticks of feeding left, and whether this zombie has had its fill of the current body.
    eating: Option<u32>,
    fed: bool }

impl Zombie {
    pub fn new(x: f32, y: f32, z: usize) -> Self { let rng = (x.to_bits() ^ y.to_bits().rotate_left(13) ^ 0x9E37_79B9) | 1;
        let mut zb = Self { pos: [x, y], z, health: ZOMBIE_HEALTH, cooldown: 0, hit_time: 0, stagger: 0, on_floor: false, knocked_down: false, path: Vec::new(), stuck: 0, reanimate_timer: 0.0, facing: [1.0, 0.0], idle_timer: 0.0, wander: None, rng, sight: Sight::Normal, crawl_mod: None, climb: None, eating: None, fed: false };
        zb.idle_timer = zb.pick_wander_interval();
        zb
    }

    pub fn is_crawler(&self) -> bool { self.crawl_mod.is_some() }

    /// Turn into a crawler (`setCrawler(true)`): much slower, speedMod rolled once.
    pub fn set_crawler(&mut self) {
        if self.crawl_mod.is_none() { self.crawl_mod = Some(0.3 + self.rand(1500) as f32 / 10000.0); }
    }

    /// Walking speed in tiles/second; `ZOMBIE_SPEED` is calibrated to an average shambler (speedMod 0.625).
    fn speed(&self) -> f32 { ZOMBIE_SPEED * self.crawl_mod.map_or(1.0, |m| m / 0.625) }

    /// IsoZombie.Hit -> IsoGameCharacter.Hit/processHitDamage/hitConsequences for a player's melee hit
    /// (default traits: Strength 5 so shovingMod 1.0, full endurance). Returns the damage dealt.
    pub fn hit(&mut self, w: &Weapon, damage_roll: f32, crit: bool, from: [f32; 2]) -> f32 {
        // Repeated hits scale up after the third: hitTime >= 4 -> x (hitTime - 2) * 1.5.
        self.hit_time += 1;
        let mut split = damage_roll;
        if self.hit_time >= 4 { split *= (self.hit_time as f32 - 2.0) * 1.5; }
        // hitForce = min(damage * shovingMod, 1), doubled for a player's non-shove hit.
        let hit_force = split.min(1.0) * 2.0;
        let mut dmg = split;
        // From behind (dot(wielder->zombie, zombie facing) > -0.3): x1.5.
        let d = [self.pos[0] - from[0], self.pos[1] - from[1]];
        let l = d[0].hypot(d[1]).max(1e-6);
        if (d[0] * self.facing[0] + d[1] * self.facing[1]) / l > -0.3 { dmg *= 1.5; }
        if crit { dmg *= w.crit_mult.max(2.0); }
        self.health -= dmg;
        if self.health > 0.0 {
            // Hit reaction: IsoZombie.hitConsequences -> knockedDown = crit || onFloor || alwaysKnockedDown.
            self.knocked_down = crit || self.on_floor;
            // StaggerBackState.getMaxStaggerTime: clamp(35 * hitForce * staggerTimeMod, 20, 30).
            let stagger_mod = if w.small_blade { 0.0 } else { w.push_back_mod };
            self.stagger = (35.0 * hit_force * stagger_mod).clamp(20.0, 30.0) as u32;
            if self.knocked_down && !self.on_floor {
                // Falls down (ZombieOnGroundState.startReanimateTimer: Rand.Next(60) + 30).
                self.on_floor = true;
                self.reanimate_timer = (30 + self.rand(60)) as f32;
            }
        }
        dmg
    }

    /// xorshift32 in `0..n` (deterministic per zombie).
    fn rand(&mut self, n: u32) -> u32 {
        self.rng ^= self.rng << 13; self.rng ^= self.rng >> 17; self.rng ^= self.rng << 5;
        self.rng % n
    }

    /// ZombieIdleState.pickRandomWanderInterval: Rand.Next(400, 1000), x1.5 when it is not raining.
    fn pick_wander_interval(&mut self) -> f32 { (400 + self.rand(600)) as f32 * 1.5 }

    /// ZombieIdleState.execute: with no target, every so often pick a free square within +-4 and walk there.
    fn idle(&mut self, world: &mut World) {
        self.idle_timer -= 1.0;
        if self.idle_timer <= 0.0 {
            self.idle_timer = self.pick_wander_interval();
            let (sx, sy) = square_of(self.pos);
            let (x, y) = (sx + self.rand(8) as i32 - 4, sy + self.rand(8) as i32 - 4);
            if world.has_floor(x, y, self.z) && world.flags(x, y, self.z) & flag::SOLID == 0 {
                self.wander = Some([x as f32, y as f32]);
            }
        }
        if let Some(t) = self.wander {
            let d = [t[0] - self.pos[0], t[1] - self.pos[1]];
            if d[0].hypot(d[1]) < 0.2 { self.wander = None; } else { { let v = self.speed() * 0.5; slide(&mut self.pos, self.z, d, v, world, false); } }
        }
    }

    pub fn tick(&mut self, target: &mut Player, world: &mut World) {
        self.tick_in(target, world, &Weather::default());
    }

    pub fn tick_in(&mut self, target: &mut Player, world: &mut World, weather: &Weather) {
        self.cooldown = self.cooldown.saturating_sub(1);
        if let Some((left, dest)) = self.climb {
            if self.on_floor || self.stagger > 0 { self.climb = None; } else {
                if left <= 1 { self.pos = [dest.0 as f32, dest.1 as f32]; self.climb = None; } else { self.climb = Some((left - 1, dest)); }
                return;
            }
        }
        if self.on_floor {
            // ZombieOnGroundState: lie there until the reanimate timer runs out, then get up.
            self.reanimate_timer -= 1.0;
            if self.reanimate_timer <= 0.0 { self.on_floor = false; self.knocked_down = false; }
            return;
        }
        if self.stagger > 0 { self.stagger -= 1; return; }
        if target.z != self.z { self.idle(world); return; }
        let d = [target.pos[0] - self.pos[0], target.pos[1] - self.pos[1]];
        let dist = (d[0] * d[0] + d[1] * d[1]).sqrt();
        if dist > weather.vision_radius_for(self.sight, 1.0, self.eating.is_some()) { self.idle(world); return; }
        if target.health <= 0.0 {
            // Nothing left to fight: walk up to the body and feed on it.
            if self.fed { self.eating = None; self.idle(world); return; }
            if dist < EAT_RANGE {
                self.facing = [d[0] / dist.max(1e-6), d[1] / dist.max(1e-6)];
                let left = match self.eating { Some(t) => t, None => EAT_MIN_TICKS + self.rand(EAT_MIN_TICKS) };
                if left <= 1 { self.eating = None; self.fed = true; } else { self.eating = Some(left - 1); }
            } else {
                self.eating = None;
                self.chase(target, d, world);
            }
            return;
        }
        self.fed = false;
        self.eating = None;
        if dist < ATTACK_RANGE {
            if self.cooldown == 0 {
                let side = side_of(target.facing, [-d[0], -d[1]]);
                let (r1, r2, r3, r4, r5) = (self.rand(100), self.rand(100), self.rand(100), self.rand(1000), self.rand(10));
                let hit = resolve_attack(AttackRolls { hit: r1, laceration: r2, bite: r3, dmg: r4, dmg_scale: r5 }, side, 1);
                if hit.outcome != Outcome::Miss {
                    target.health = (target.health - hit.damage).max(0.0);
                    if hit.outcome == Outcome::Bite { target.infected = true; }
                }
                self.cooldown = ATTACK_COOLDOWN;
            }
            return;
        }
        self.chase(target, d, world);
    }

    /// Walk toward the target; when walls block the straight line, route around them with A*.
    fn chase(&mut self, target: &Player, d: [f32; 2], world: &mut World) {
        if let Some(&(wx, wy)) = self.path.first() {
            let to = [wx as f32 - self.pos[0], wy as f32 - self.pos[1]];
            if to[0].hypot(to[1]) < 0.15 { self.path.remove(0); } else { self.step_toward(to, world); }
            // The target moved on: drop a stale route.
            let end = *self.path.last().unwrap_or(&(wx, wy));
            let t = square_of(target.pos);
            if (end.0 - t.0).abs() + (end.1 - t.1).abs() > 3 { self.path.clear(); }
            return;
        }
        if self.try_climb(d, world) { return; }
        let before = self.pos;
        self.step_toward(d, world);
        if self.pos == before {
            self.stuck += 1;
            if self.stuck >= 5 {
                self.stuck = 0;
                if let Some(p) = world.find_path(square_of(self.pos), square_of(target.pos), self.z, 4000) { self.path = p; }
            }
        } else {
            self.stuck = 0;
        }
    }

    pub fn is_eating(&self) -> bool { self.eating.is_some() }

    pub fn is_climbing(&self) -> bool { self.climb.is_some() }

    /// ClimbThroughWindowState / ClimbOverFenceState: standing at a window or hoppable fence that lies in the way,
    /// start climbing over it. The original times this by animation events; `CLIMB_TICKS` approximates that.
    fn try_climb(&mut self, d: [f32; 2], world: &mut World) -> bool {
        if self.crawl_mod.is_some() { return false; } // crawlers cannot climb
        let from = square_of(self.pos);
        let axes = if d[0].abs() >= d[1].abs() { [0, 1] } else { [1, 0] };
        for a in axes {
            if d[a].abs() < 0.05 { continue; }
            let sgn = d[a].signum() as i32;
            let to = if a == 0 { (from.0 + sgn, from.1) } else { (from.0, from.1 + sgn) };
            // Only when pressed up against the edge.
            let edge = [from.0, from.1][a] as f32 + 0.5 * sgn as f32;
            if (self.pos[a] - edge).abs() > 0.2 { continue; }
            if !world.can_step(from, to, self.z) && world.hurdle(from, to, self.z) {
                self.climb = Some((CLIMB_TICKS, to));
                self.facing = if a == 0 { [sgn as f32, 0.0] } else { [0.0, sgn as f32] };
                return true;
            }
        }
        false
    }

    fn step_toward(&mut self, d: [f32; 2], world: &mut World) {
        let v = self.speed();
        slide(&mut self.pos, self.z, d, v, world, false);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side { Front, Behind, Side }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome { Miss, Scratch, Laceration, Bite }

#[derive(Clone, Copy, Debug)]
pub struct Hit { pub outcome: Outcome, pub damage: f32 }

/// Pre-rolled dice (`Rand.Next(100)` x3, `Rand.Next(1000)`, `Rand.Next(10)`), so the rule is a pure function.
#[derive(Clone, Copy)]
pub struct AttackRolls { pub hit: u32, pub laceration: u32, pub bite: u32, pub dmg: u32, pub dmg_scale: u32 }

/// Which side of the victim the attacker is on. `to_attacker` is the vector victim -> attacker.
/// (The game uses `testDotSide`; the thresholds here are an approximation.)
pub fn side_of(facing: [f32; 2], to_attacker: [f32; 2]) -> Side {
    let l = to_attacker[0].hypot(to_attacker[1]).max(1e-6);
    let dot = (facing[0] * to_attacker[0] + facing[1] * to_attacker[1]) / l;
    if dot > 0.5 { Side::Front } else if dot < -0.5 { Side::Behind } else { Side::Side }
}

/// BodyDamage.AddRandomDamageFromZombie (default sandbox options, no traits/clothing/body parts):
/// the zombie lands a hit when `Rand(100) > baseChance`; a hit is a scratch unless the laceration
/// roll beats `baseLacerationChance` or the bite roll beats `baseBiteChance`.
pub fn resolve_attack(r: AttackRolls, side: Side, zombies_attacking: u32) -> Hit {
    let n = zombies_attacking.max(1) as i32;
    let (mut base, mut bite, mut lac) = (15 - (n - 1) * 10, 85 - (n - 1) * 30, 65 - (n - 1) * 15);
    match side {
        Side::Behind => {
            base -= 15; bite -= 25; lac -= 35;
            if n > 2 { bite -= 15; lac -= 15; }
        }
        Side::Side => { base -= 30; bite -= 7; lac -= 27; }
        Side::Front => {}
    }
    if (r.hit as i32) <= base { return Hit { outcome: Outcome::Miss, damage: 0.0 }; }
    let outcome = if (r.bite as i32) > bite { Outcome::Bite } else if (r.laceration as i32) > lac { Outcome::Laceration } else { Outcome::Scratch };
    Hit { outcome, damage: (r.dmg as f32 / 1000.0) * (r.dmg_scale as f32 + 10.0) }
}

/// A world sound (WorldSoundManager.WorldSound): zombies within `radius * hearing` are attracted.
#[derive(Clone, Copy, Debug)]
pub struct WorldSound {
    pub pos: [f32; 2],
    pub z: usize,
    pub radius: f32,
    pub volume: f32,
    /// Zombies closer than this on the same floor ignore the sound.
    pub zombie_ignore_dist: f32,
    pub source_is_zombie: bool,
}

/// `WorldSoundManager.getHearingMultiplier`: sandbox hearing 1 = pinpoint (x3), 3 = poor (x0.45), else x1.
pub fn hearing_multiplier(hearing: u8) -> f32 {
    match hearing { 1 => 3.0, 3 => 0.45, _ => 1.0 }
}

/// WorldSoundManager.getSoundAttract. `rooms` is (different rooms, either point is outdoors).
pub fn sound_attract(s: &WorldSound, zpos: [f32; 2], zz: usize, hearing: f32, rooms: (bool, bool)) -> f32 {
    if s.radius == 0.0 || s.source_is_zombie { return 0.0; }
    // Floors count as 3 tiles of distance, like the original's z * 3.
    let dz = (zz as f32 - s.z as f32) * 3.0;
    let dist_sq = (zpos[0] - s.pos[0]).powi(2) + (zpos[1] - s.pos[1]).powi(2) + dz * dz;
    let radius = s.radius * hearing;
    if dist_sq > radius * radius { return 0.0; }
    if dist_sq < s.zombie_ignore_dist * s.zombie_ignore_dist && zz == s.z { return 0.0; }
    let mut delta = dist_sq / (radius * radius);
    if rooms.0 {
        delta *= 1.2;
        if rooms.1 { delta *= 1.4; }
    }
    s.volume * (1.0 - delta).clamp(0.0, 1.0)
}

impl Zombie {
    /// React to the loudest audible sound: head for it unless already chasing something.
    pub fn hear(&mut self, sounds: &[WorldSound]) {
        let best = sounds.iter()
            .map(|s| (sound_attract(s, self.pos, self.z, 1.0, (false, false)), s))
            .filter(|(a, _)| *a > 0.0)
            .max_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, s)) = best { self.wander = Some(s.pos); self.z = self.z.min(s.z.max(self.z)); }
    }
}

/// IsoPlayer.calculateCritChance, melee branch (no moodles, default sandbox).
/// `crit_chance` is the weapon's `CriticalChance`; `skill` is the matching weapon-skill level (0..10).
pub fn melee_crit_chance(crit_chance: f32, two_handed_not_held_in_both: bool, charge_time: f32, skill: u32) -> i32 {
    let mut c = crit_chance;
    if two_handed_not_held_in_both { c -= c / 3.0; }
    if charge_time < 2.0 { c -= c / 5.0; }
    c += skill as f32 * 3.0;
    c.clamp(10.0, 90.0) as i32
}

/// Cosine interpolation, `ClimateManager.clerp`.
fn clerp(t: f32, a: f32, b: f32) -> f32 {
    let t2 = (1.0 - (t * std::f32::consts::PI).cos()) / 2.0;
    a * (1.0 - t2) + b * t2
}

/// `ClimateManager.getTimeLerp(.., doClerp = true)` over hours: 0 outside [min, max], rising to 1 at the middle and back.
/// A window with min > max wraps past midnight.
fn time_lerp_hours(cur: f32, min: f32, max: f32) -> f32 {
    let n = |h: f32| (h / 24.0).clamp(0.0, 1.0);
    let (cur, min, max) = (n(cur), n(min), n(max));
    let (c, len) = if min <= max {
        if cur < min || cur > max { return 0.0; }
        (cur - min, max - min)
    } else {
        if cur < min && cur > max { return 0.0; }
        let off = 1.0 - min;
        (if cur >= min { cur - min } else { cur + off }, max + off)
    };
    let mid = len * 0.5;
    if c < mid { clerp(c / mid, 0.0, 1.0) } else { clerp((c - mid) / mid, 1.0, 0.0) }
}

/// The in-game clock and the day/night cycle derived from it.
///
/// Dawn and dusk are fixed parameters here; the original derives them from latitude and day of year
/// (`ErosionSeason`), which is not ported. The night curve itself is `ClimateValues.lerpNight`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameClock {
    /// Hour of day, 0..24.
    pub hour: f32,
    pub dawn: f32,
    pub dusk: f32,
    /// Real seconds per in-game day (3600 = the default "1 hour" day length).
    pub day_seconds: f32,
}

impl Default for GameClock {
    fn default() -> Self { GameClock { hour: 9.0, dawn: 7.0, dusk: 21.0, day_seconds: 3600.0 } }
}

impl GameClock {
    pub fn tick(&mut self) {
        self.hour += 24.0 / (self.day_seconds * TICK_HZ);
        if self.hour >= 24.0 { self.hour -= 24.0; }
    }
    /// 0 in daylight, 1 deep in the night (`ClimateValues.nightStrength`).
    pub fn night_strength(&self) -> f32 {
        (time_lerp_hours(self.hour, self.dusk, self.dawn) * 2.0).clamp(0.0, 1.0)
    }
    /// Daylight strength, ignoring the small wind/time-of-day reductions of the original.
    pub fn daylight(&self) -> f32 { 1.0 - self.night_strength() }
    /// Vision conditions at this time of day with the given rain and fog.
    pub fn weather(&self, rain: f32, fog: f32) -> Weather { Weather { rain, fog, light: self.daylight() } }
}
