//! Fixed-timestep game simulation (deterministic; independent of rendering).
use pz_world::{flag, World};

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

#[derive(Clone, Copy)]
pub struct Player {
    /// Continuous tile coordinates; square (x, y) is centred on integer coordinates.
    pub pos: [f32; 2],
    pub z: usize,
    /// Continuous height in floors (z plus progress along a staircase).
    pub height: f32,
    /// 0..=100; zombies bite it down.
    pub health: f32,
    /// Unit vector of the last movement direction (swings go this way).
    pub facing: [f32; 2],
    swing_cd: u32,
}

pub fn square_of(p: [f32; 2]) -> (i32, i32) { ((p[0] + 0.5).floor() as i32, (p[1] + 0.5).floor() as i32) }

impl Player {
    pub fn new(x: f32, y: f32) -> Self { Self { pos: [x, y], z: 0, height: 0.0, health: 100.0, facing: [1.0, 0.0], swing_cd: 0 } }

    /// Advance one tick. Axes are resolved separately so the player slides along walls.
    pub fn tick(&mut self, input: Input, world: &mut World) {
        self.swing_cd = self.swing_cd.saturating_sub(1);
        let len = (input.dir[0] * input.dir[0] + input.dir[1] * input.dir[1]).sqrt();
        if len > 1e-4 { self.facing = [input.dir[0] / len, input.dir[1] / len]; }
        let before = square_of(self.pos);
        self.move_xy(input, world);
        self.update_level(before, world);
    }

    /// Swing `weapon` in the facing direction; hits every zombie in range and in the front cone.
    /// Returns the number of zombies killed (dead zombies are removed).
    pub fn attack(&mut self, zombies: &mut Vec<Zombie>, weapon: &Weapon) -> usize {
        if self.swing_cd > 0 { return 0; }
        self.swing_cd = weapon.cooldown_ticks;
        let before = zombies.len();
        for zb in zombies.iter_mut().filter(|zb| zb.z == self.z) {
            let d = [zb.pos[0] - self.pos[0], zb.pos[1] - self.pos[1]];
            let dist = (d[0] * d[0] + d[1] * d[1]).sqrt();
            let front = dist < 1e-4 || (d[0] * self.facing[0] + d[1] * self.facing[1]) / dist > 0.5;
            if dist <= weapon.range && front { zb.health -= weapon.damage; }
        }
        zombies.retain(|zb| zb.health > 0.0);
        before - zombies.len()
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
const ZOMBIE_SPEED: f32 = 1.1; // tiles / second (shamble)
const ZOMBIE_SIGHT: f32 = 20.0; // IsoZombie.updateVisionRadius: 20 in clear daylight (clamped 10..20)
const ATTACK_RANGE: f32 = 0.9;
const ATTACK_DAMAGE: f32 = 12.0;
const ATTACK_COOLDOWN: u32 = 45; // ticks (1.5 s)

/// A zombie that shambles straight toward a visible player on the same floor.
/// Melee weapon stats (damage and range follow the item scripts' units).
#[derive(Clone, Copy, Debug)]
pub struct Weapon { pub damage: f32, pub range: f32, pub cooldown_ticks: u32 }

impl Weapon {
    pub const FISTS: Weapon = Weapon { damage: 0.5, range: 1.0, cooldown_ticks: 30 };
}

#[derive(Clone, Copy)]
pub struct Zombie { pub pos: [f32; 2], pub z: usize, pub health: f32, cooldown: u32, idle_timer: f32, wander: Option<[f32; 2]>, rng: u32 }

impl Zombie {
    pub fn new(x: f32, y: f32, z: usize) -> Self { let rng = (x.to_bits() ^ y.to_bits().rotate_left(13) ^ 0x9E37_79B9) | 1;
        let mut zb = Self { pos: [x, y], z, health: ZOMBIE_HEALTH, cooldown: 0, idle_timer: 0.0, wander: None, rng };
        zb.idle_timer = zb.pick_wander_interval();
        zb
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
            if d[0].hypot(d[1]) < 0.2 { self.wander = None; } else { slide(&mut self.pos, self.z, d, ZOMBIE_SPEED * 0.5, world, false); }
        }
    }

    pub fn tick(&mut self, target: &mut Player, world: &mut World) {
        self.cooldown = self.cooldown.saturating_sub(1);
        if target.z != self.z { self.idle(world); return; }
        let d = [target.pos[0] - self.pos[0], target.pos[1] - self.pos[1]];
        let dist = (d[0] * d[0] + d[1] * d[1]).sqrt();
        if dist > ZOMBIE_SIGHT { self.idle(world); return; }
        if dist < ATTACK_RANGE {
            if self.cooldown == 0 { target.health = (target.health - ATTACK_DAMAGE).max(0.0); self.cooldown = ATTACK_COOLDOWN; }
            return;
        }
        slide(&mut self.pos, self.z, d, ZOMBIE_SPEED, world, false);
    }
}
